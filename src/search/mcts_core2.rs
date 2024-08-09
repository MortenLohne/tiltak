use std::ops;
use std::process;
use std::sync;

use board_game_traits::{Color, GameResult, Position as PositionTrait};
use half::f16;
use half::slice::HalfFloatSliceExt;
use rand::Rng;
use rand_distr::Distribution;

use crate::evaluation::parameters::IncrementalPolicy;
use crate::position::Move;
/// This module contains the core of the MCTS search algorithm
use crate::position::Position;
use crate::search::{cp_to_win_percentage, MctsSetting};

use super::arena::ArenaError;
use super::ARENA_ELEMENT_SIZE;
use super::{arena, Arena};

#[derive(Debug)]
pub enum MctsError {
    OOM,
    MaxVisits,
}

pub struct TreeRoot<const S: usize> {
    tree: TreeEdge<S>, // Fake edge to the root node
    visits: u32,
    position: Position<S>,
    temp_position: Position<S>,
    settings: MctsSetting<S>,
    temp_vectors: TempVectors<S>,
    arena: Arena,
}

impl<const S: usize> TreeRoot<S> {
    pub fn new(position: Position<S>, settings: MctsSetting<S>) -> TreeRoot<S> {
        let arena = match Arena::new(settings.arena_size) {
            Ok(arena) => arena,
            Err(ArenaError::AllocationFailed(num_bytes)) if !sysinfo::IS_SUPPORTED_SYSTEM => {
                panic!(
                    "Fatal error: failed to allocate {}MB memory for search tree. Could not detect total system memory.",
                    num_bytes
                )
            }
            Err(ArenaError::AllocationFailed(num_bytes)) => {
                // The allocation may have failed because the system doesn't have enough memory
                // Check the system's max memory, and try again

                let mut sys = sysinfo::System::new_all();
                sys.refresh_all();

                if sys.total_memory() < num_bytes as u64 {
                    // Note: The actual memory allocation is two slots larger, to ensure correct alignment
                    let Some(max_num_slots) =
                        ((sys.total_memory() / 16).min(u32::MAX as u64) as u32).checked_sub(2)
                    else {
                        panic!(
                            "Failed to allocated arena, system reports {} bytes total memory",
                            sys.total_memory()
                        );
                    };
                    eprintln!("Warning: failed to allocate {}MB memory for the search tree. Trying again with {}MB.", num_bytes / (1024 * 1024), sys.total_memory() / (1024 * 1024));

                    match <Arena<16>>::new(max_num_slots) {
                        Ok(arena) => arena,
                        Err(ArenaError::AllocationFailed(num_bytes)) => {
                            eprintln!("Fatal error: failed to allocate {}MB memory for search tree. Try reducing the search time.", num_bytes / (1024 * 1024));
                            process::exit(1)
                        }
                        Err(err) => panic!("{}", err),
                    }
                } else {
                    eprintln!("Fatal error: failed to allocate {}MB memory for search tree. Try reducing the search time.", num_bytes / (1024 * 1024));
                    process::exit(1)
                }
            }
            Err(err) => panic!("{}", err),
        };

        let mut tree = TreeEdge { child: None };
        let mut temp_vectors = TempVectors::default();

        // Applying dirichlet noise or excluding moves can only be done once the child edges of the root are initialized,
        // which is done on the 2nd select
        tree.select(
            &mut position.clone(),
            &settings,
            &mut temp_vectors,
            &arena,
            0,
        )
        .unwrap();
        tree.select(
            &mut position.clone(),
            &settings,
            &mut temp_vectors,
            &arena,
            1,
        )
        .unwrap();

        if let Some(alpha) = settings.dirichlet {
            arena
                .get_mut(
                    (arena.get_mut(tree.child.as_mut().unwrap()))
                        .children
                        .as_mut()
                        .unwrap(),
                )
                .apply_dirichlet(&arena, 0.25, alpha);
        }

        if !settings.excluded_moves.is_empty() {
            let bridge = arena.get_mut(
                (arena.get_mut(tree.child.as_mut().unwrap()))
                    .children
                    .as_mut()
                    .unwrap(),
            );
            for excluded_move in settings.excluded_moves.iter() {
                let index = arena
                    .get_slice(&bridge.moves)
                    .iter()
                    .enumerate()
                    .find(|(_, mv)| **mv == Some(*excluded_move))
                    .unwrap()
                    .0;
                let moves = arena.get_slice_mut(&mut bridge.moves);
                let children = arena.get_slice_mut(&mut bridge.children);
                let visitss = arena.get_slice_mut(&mut bridge.visitss);
                let heuristic_scores = arena.get_slice_mut(&mut bridge.heuristic_scores);
                let mean_action_values = arena.get_slice_mut(&mut bridge.mean_action_values);

                moves[index] = None;
                heuristic_scores[index] = f16::NEG_INFINITY; // TODO: Also set infinite visitss?

                for i in index..(moves.len() - 1) {
                    moves.swap(i, i + 1);
                    children.swap(i, i + 1);
                    visitss.swap(i, i + 1);
                    heuristic_scores.swap(i, i + 1);
                    mean_action_values.swap(i, i + 1);
                }
            }
        }

        TreeRoot {
            tree,
            visits: 0,
            position: position.clone(),
            temp_position: position,
            settings,
            temp_vectors,
            arena,
        }
    }

    // TODO: Count up to u64 on root?
    pub fn visits(&self) -> u32 {
        self.visits
    }

    pub fn mem_usage(&self) -> usize {
        self.arena.slots_used() as usize * ARENA_ELEMENT_SIZE
    }

    pub fn mean_action_value(&self) -> f32 {
        self.tree
            .child
            .as_ref()
            .map(|index| self.arena.get(index).total_action_value as f32 / self.visits as f32)
            .unwrap_or(self.settings.initial_mean_action_value())
    }

    pub fn best_move(&self) -> Option<(Move<S>, f32)> {
        self.tree
            .child
            .as_ref()
            .and_then(|index| self.arena.get(index).children.as_ref())
            .and_then(|index| {
                let child = self.arena.get(index);
                let (mv, score) = self
                    .arena
                    .get_slice(&child.moves)
                    .iter()
                    .zip(self.arena.get_slice(&child.mean_action_values))
                    .zip(self.arena.get_slice(&child.visitss))
                    .filter(|((mv, _), _)| mv.is_some())
                    .max_by_key(|(_, visits)| *visits)?
                    .0;
                Some((mv.unwrap(), 1.0 - *score))
            })
    }

    pub fn pv(&self) -> impl Iterator<Item = Move<S>> + '_ {
        Pv::new(&self.tree, &self.arena)
    }

    /// Print human-readable information of the search's progress.
    pub fn print_info(&self) {
        struct GoodEdge<'a, const S: usize> {
            visits: u32,
            mv: Move<S>,
            mean_action_value: f32,
            child: &'a TreeEdge<S>,
        }
        let child = self.arena.get(
            self.arena
                .get(self.tree.child.as_ref().unwrap())
                .children
                .as_ref()
                .unwrap(),
        );
        let mut best_children: Vec<GoodEdge<S>> = self
            .arena
            .get_slice(&child.visitss)
            .iter()
            .zip(
                self.arena.get_slice(&child.moves).iter().zip(
                    self.arena
                        .get_slice(&child.mean_action_values)
                        .iter()
                        .zip(self.arena.get_slice(&child.children).iter()),
                ),
            )
            .filter_map(|(visits, (mv, (score, child)))| {
                Some(GoodEdge {
                    visits: *visits,
                    mv: (*mv)?,
                    mean_action_value: *score,
                    child,
                })
            })
            .collect();

        best_children.sort_by_key(|edge| edge.visits);
        best_children.reverse();

        use sync::atomic::Ordering::*;
        println!(
            "Arena stats: {}MiB allocated, {}MiB structs, {}MiB slices, {}MiB wasted",
            self.arena.stats.bytes_allocated.load(SeqCst) / (1024 * 1024),
            self.arena.stats.bytes_structs.load(SeqCst) / (1024 * 1024),
            self.arena.stats.bytes_slices.load(SeqCst) / (1024 * 1024),
            self.arena.stats.padding_bytes.load(SeqCst) / (1024 * 1024),
        );

        best_children.iter().take(8).for_each(|edge| {
            println!(
                "Move {}: {} visits, {:.2}% mean action value, pv {}",
                edge.mv,
                edge.visits,
                edge.mean_action_value * 100.0,
                Pv::new(edge.child, &self.arena)
                    .map(|mv| mv.to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        });
    }

    pub fn select(&mut self) -> Result<f32, MctsError> {
        if self.visits == u32::MAX {
            return Err(MctsError::MaxVisits);
        }
        self.temp_position.clone_from(&self.position);
        let result = self.tree.select(
            &mut self.temp_position,
            &self.settings,
            &mut self.temp_vectors,
            &self.arena,
            self.visits,
        )?;
        self.visits += 1;
        Ok(result)
    }
}

/// A Monte Carlo Search Tree, containing every node that has been seen in search.
#[derive(PartialEq, Debug)]
pub struct Tree<const S: usize> {
    pub total_action_value: f64,
    pub game_result: Option<GameResultForUs>,
    pub children: Option<arena::Index<TreeBridge<S>>>,
}

#[derive(PartialEq, Debug)]
pub struct TreeBridge<const S: usize> {
    children: arena::SliceIndex<TreeEdge<S>>,
    moves: arena::SliceIndex<Option<Move<S>>>,
    mean_action_values: arena::SliceIndex<f32>,
    visitss: arena::SliceIndex<u32>,
    heuristic_scores: arena::SliceIndex<f16>,
}

#[derive(PartialEq, Debug)]
pub struct TreeEdge<const S: usize> {
    pub child: Option<arena::Index<Tree<S>>>,
}

/// Temporary vectors that are continually re-used during search to avoid unnecessary allocations
#[derive(Debug)]
pub struct TempVectors<const S: usize> {
    simple_moves: Vec<Move<S>>,
    moves: Vec<(Move<S>, f16)>,
    fcd_per_move: Vec<i8>,
    policy_feature_sets: Vec<IncrementalPolicy<S>>,
    unpacked_heuristic_scores: Vec<f32>,
}

impl<const S: usize> Default for TempVectors<S> {
    fn default() -> Self {
        TempVectors {
            simple_moves: vec![],
            moves: vec![],
            fcd_per_move: vec![],
            policy_feature_sets: vec![],
            unpacked_heuristic_scores: vec![0.; 65536],
        }
    }
}

#[inline(always)]
pub fn exploration_value(
    mean_action_value: f32,
    heuristic_score: f32,
    child_visits: u32,
    parent_visits_sqrt: f32,
    cpuct: f32,
) -> f32 {
    (1.0 - mean_action_value)
        + cpuct * heuristic_score * parent_visits_sqrt / (1 + child_visits) as f32
}

impl<const S: usize> TreeBridge<S> {
    #[inline(always)]
    pub fn best_child(
        &mut self,
        settings: &MctsSetting<S>,
        temp_vectors: &mut TempVectors<S>,
        arena: &Arena,
        our_visits: u32,
    ) -> usize {
        let visits_sqrt = (our_visits as f32).sqrt();
        let dynamic_cpuct = settings.c_puct_init()
            + f32::ln((1.0 + our_visits as f32 + settings.c_puct_base()) / settings.c_puct_base());

        let heuristic_scores = arena.get_slice(&self.heuristic_scores);
        let mean_action_values = arena.get_slice(&self.mean_action_values);
        let visitss = arena.get_slice(&self.visitss);

        let unpacked_heuristic_scores =
            &mut temp_vectors.unpacked_heuristic_scores[0..heuristic_scores.len()];
        heuristic_scores.convert_to_f32_slice(unpacked_heuristic_scores);
        let heuristic_scores = unpacked_heuristic_scores;

        assert_eq!(heuristic_scores.len() % SIMD_WIDTH, 0);
        assert_eq!(heuristic_scores.len(), mean_action_values.len());
        assert_eq!(heuristic_scores.len(), visitss.len());

        for i in 0..heuristic_scores.len() {
            let heuristic_score = &mut heuristic_scores[i];
            let mean_action_value = &mean_action_values[i];
            let child_visits = &visitss[i];

            *heuristic_score = exploration_value(
                *mean_action_value,
                *heuristic_score,
                *child_visits,
                visits_sqrt,
                dynamic_cpuct,
            )
        }

        let mut indices = [0u32; SIMD_WIDTH];
        let mut maxes = [f32::NEG_INFINITY; SIMD_WIDTH];

        for (heuristic_scores, new_i) in heuristic_scores
            .chunks_exact(SIMD_WIDTH)
            .zip((0..).step_by(SIMD_WIDTH))
        {
            for i in 0..SIMD_WIDTH {
                let score = heuristic_scores[i];
                let max = &mut maxes[i];
                let index = &mut indices[i];

                let i = *index;
                let m = *max;

                let increased = score > m;
                *index = if increased { new_i } else { i };
                *max = if increased { score } else { m };
            }
        }

        let (best_child_node_index, _) = indices
            .into_iter()
            .enumerate()
            .map(|(c, i)| i + c as u32)
            .zip(maxes)
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap();

        best_child_node_index as usize
    }

    pub fn select(
        &mut self,
        position: &mut Position<S>,
        settings: &MctsSetting<S>,
        temp_vectors: &mut TempVectors<S>,
        arena: &Arena,
        our_visits: u32,
    ) -> Result<f32, MctsError> {
        assert_ne!(
            arena.get_slice(&self.children).len(),
            0,
            "No legal moves in position\n{:?}",
            position
        );

        let best_child_node_index = self.best_child(settings, temp_vectors, arena, our_visits);

        let child_edge = arena
            .get_slice_mut(&mut self.children)
            .get_mut(best_child_node_index)
            .unwrap();
        let child_visits = *arena
            .get_slice_mut(&mut self.visitss)
            .get_mut(best_child_node_index)
            .unwrap();
        let child_move = arena
            .get_slice_mut(&mut self.moves)
            .get_mut(best_child_node_index)
            .unwrap()
            .unwrap_or_else(|| {
                panic!(
                    "Move has {} visits from {} parent vists",
                    child_visits, our_visits
                )
            });

        position.do_move(child_move);

        let result =
            1.0 - child_edge.select(position, settings, temp_vectors, arena, child_visits)?;

        *arena
            .get_slice_mut(&mut self.visitss)
            .get_mut(best_child_node_index)
            .unwrap() += 1;

        *arena
            .get_slice_mut(&mut self.mean_action_values)
            .get_mut(best_child_node_index)
            .unwrap() = arena
            .get(child_edge.child.as_ref().unwrap())
            .total_action_value as f32
            / *arena
                .get_slice_mut(&mut self.visitss)
                .get_mut(best_child_node_index)
                .unwrap() as f32;
        Ok(result)
    }

    /// Apply Dirichlet noise to the heuristic scores of the child node
    /// The noise is given `epsilon` weight.
    /// `alpha` is used to generate the noise, lower values generate more varied noise.
    /// Values above 1 are less noisy, and tend towards uniform outputs
    pub fn apply_dirichlet(&mut self, arena: &Arena, epsilon: f32, alpha: f32) {
        let mut rng = rand::thread_rng();
        let dirichlet =
            rand_distr::Dirichlet::new_with_size(alpha, arena.get_slice(&self.children).len())
                .unwrap();
        let noise_vec = dirichlet.sample(&mut rng);
        for (child_prior, eta) in arena
            .get_slice_mut(&mut self.heuristic_scores)
            .iter_mut()
            .zip(noise_vec)
        {
            *child_prior = f16::from_f32(child_prior.to_f32() * (1.0 - epsilon) + epsilon * eta);
        }
    }
}

impl<const S: usize> TreeEdge<S> {
    pub fn select(
        &mut self,
        position: &mut Position<S>,
        settings: &MctsSetting<S>,
        temp_vectors: &mut TempVectors<S>,
        arena: &Arena,
        parent_visits: u32,
    ) -> Result<f32, MctsError> {
        if let Some(child) = self.child.as_mut() {
            return arena.get_mut(child).select(
                position,
                settings,
                temp_vectors,
                arena,
                parent_visits,
            );
        }

        let (result, game_result) =
            rollout(position, settings, settings.rollout_depth, temp_vectors);
        self.child = Some(
            arena
                .add(Tree {
                    total_action_value: result as f64,
                    game_result,
                    children: None,
                })
                .ok_or(MctsError::OOM)?,
        );

        Ok(result)
    }
}

const SIMD_WIDTH: usize = 4;

impl<const S: usize> Tree<S> {
    /// Perform one iteration of monte carlo tree search.
    ///
    /// Moves done on the board are not reversed.
    pub fn select(
        &mut self,
        position: &mut Position<S>,
        settings: &MctsSetting<S>,
        temp_vectors: &mut TempVectors<S>,
        arena: &Arena,
        parent_visits: u32,
    ) -> Result<f32, MctsError> {
        // TODO: Assume node has already had 1 visit before?
        if let Some(game_result) = self.game_result {
            let result = game_result.score();
            self.total_action_value += result as f64;
            return Ok(result);
        }
        let Some(children) = self.children.as_mut() else {
            let result = self.expand_child(position, settings, temp_vectors, arena)?;
            self.total_action_value += result as f64;
            return Ok(result);
        };

        let result = arena.get_mut(children).select(
            position,
            settings,
            temp_vectors,
            arena,
            parent_visits,
        )?;
        self.total_action_value += result as f64;
        Ok(result)
    }

    /// Do not initialize children in the expansion phase, for better performance
    /// Never inline, for profiling purposes
    #[inline(never)]
    fn expand_child(
        &mut self,
        position: &mut Position<S>,
        settings: &MctsSetting<S>,
        temp_vectors: &mut TempVectors<S>,
        arena: &Arena,
    ) -> Result<f32, MctsError> {
        assert!(self.children.is_none());
        // TODO: This assert is to ensure that this method is only called on the node's second visit, but it may trigger through normal execution
        assert!(self.total_action_value != 0.0);
        let group_data = position.group_data();
        assert!(temp_vectors.simple_moves.is_empty());
        assert!(temp_vectors.moves.is_empty());
        assert!(temp_vectors.fcd_per_move.is_empty());
        position.generate_moves_with_params(
            match settings.policy_params.as_ref() {
                Some(params) => params,
                None => <Position<S>>::policy_params(position.komi()),
            },
            &group_data,
            &mut temp_vectors.simple_moves,
            &mut temp_vectors.moves,
            &mut temp_vectors.fcd_per_move,
            &mut temp_vectors.policy_feature_sets,
        );

        let num_children = temp_vectors.moves.len();
        let padding = (SIMD_WIDTH - (num_children % SIMD_WIDTH)) % SIMD_WIDTH;

        let mut tree_edge = TreeBridge {
            children: arena
                .add_slice((0..(num_children + padding)).map(|_| TreeEdge { child: None }))
                .ok_or(MctsError::OOM)?,
            moves: arena
                .add_slice(
                    (0..(num_children + padding))
                        .map(|i| temp_vectors.moves.get(i).map(|(mv, _)| *mv)),
                )
                .ok_or(MctsError::OOM)?,
            mean_action_values: arena
                .add_slice(
                    (0..(num_children + padding)).map(|_| settings.initial_mean_action_value()),
                )
                .ok_or(MctsError::OOM)?,
            visitss: arena
                .add_slice((0..(num_children + padding)).map(|i| {
                    if i < num_children {
                        0
                    } else {
                        u32::MAX - 1 // Avoid overflow, because the exploration formula uses `visits + 1`
                    }
                }))
                .ok_or(MctsError::OOM)?,
            heuristic_scores: arena
                .add_slice((0..(num_children + padding)).map(|i| {
                    temp_vectors
                        .moves
                        .get(i)
                        .map(|(_, score)| *score)
                        .unwrap_or(f16::NEG_INFINITY) // Ensure that this move never actually gets selected
                }))
                .ok_or(MctsError::OOM)?,
        };
        temp_vectors.moves.clear();

        // Select child edge before writing the child node into the tree, in case we OOM inside this call
        let result = tree_edge.select(position, settings, temp_vectors, arena, 1)?;

        self.children = Some(arena.add(tree_edge).ok_or(MctsError::OOM)?);

        Ok(result)
    }
}

pub struct Pv<'a, const S: usize> {
    arena: &'a Arena,
    edge: &'a TreeEdge<S>,
}

impl<'a, const S: usize> Pv<'a, S> {
    pub fn new(edge: &'a TreeEdge<S>, arena: &'a Arena) -> Pv<'a, S> {
        let mut pv = Pv { edge, arena };
        // Skip the dummy move on the top of the tree
        pv.next();
        pv
    }
}

impl<'a, const S: usize> Iterator for Pv<'a, S> {
    type Item = Move<S>;

    fn next(&mut self) -> Option<Self::Item> {
        self.edge
            .child
            .as_ref()
            .and_then(|child_index| {
                let child = self.arena.get(child_index);
                child.children.as_ref()
            })
            .and_then(|index| {
                let bridge = self.arena.get(index);
                let (_, (mv, child)) = self
                    .arena
                    .get_slice(&bridge.visitss)
                    .iter()
                    .zip(
                        self.arena
                            .get_slice(&bridge.moves)
                            .iter()
                            .zip(self.arena.get_slice(&bridge.children)),
                    )
                    .filter(|(_, (mv, _))| mv.is_some())
                    .max_by_key(|(visits, _)| **visits)?;
                self.edge = child;
                *mv
            })
    }
}

/// Do a mcts rollout up to `depth` plies, before doing a static evaluation.
/// Depth is 0 on default settings, in which case it immediately does a static evaluation
/// Higher depths are mainly used for playing with reduced difficulty
// Never inline, for profiling purposes
#[inline(never)]
pub fn rollout<const S: usize>(
    position: &mut Position<S>,
    settings: &MctsSetting<S>,
    depth: u16,
    temp_vectors: &mut TempVectors<S>,
) -> (f32, Option<GameResultForUs>) {
    let group_data = position.group_data();

    if let Some(game_result) = position.game_result_with_group_data(&group_data) {
        let game_result_for_us = match (game_result, position.side_to_move()) {
            (GameResult::Draw, _) => GameResultForUs::Draw,
            (GameResult::WhiteWin, Color::Black) => GameResultForUs::Loss, // The side to move has lost
            (GameResult::BlackWin, Color::White) => GameResultForUs::Loss, // The side to move has lost
            (GameResult::WhiteWin, Color::White) => GameResultForUs::Win, // The side to move has lost
            (GameResult::BlackWin, Color::Black) => GameResultForUs::Win, // The side to move has lost
        };

        (game_result_for_us.score(), Some(game_result_for_us))
    } else if depth == 0 {
        let centipawn_score = position.static_eval_with_params_and_data(
            &group_data,
            match settings.value_params.as_ref() {
                Some(params) => params,
                None => <Position<S>>::value_params(position.komi()),
            },
        );
        let static_eval = if let Some(static_eval_variance) = settings.static_eval_variance {
            let mut rng = rand::thread_rng();
            cp_to_win_percentage(
                centipawn_score + rng.gen_range((-static_eval_variance)..static_eval_variance),
            )
        } else {
            cp_to_win_percentage(centipawn_score)
        };
        match position.side_to_move() {
            Color::White => (static_eval, None),
            Color::Black => (1.0 - static_eval, None),
        }
    } else {
        position.generate_moves_with_probabilities(
            &group_data,
            &mut temp_vectors.simple_moves,
            &mut temp_vectors.moves,
            &mut temp_vectors.fcd_per_move,
            match settings.policy_params.as_ref() {
                Some(params) => params,
                None => <Position<S>>::policy_params(position.komi()),
            },
            &mut temp_vectors.policy_feature_sets,
        );

        let mut rng = rand::thread_rng();
        let best_move = best_move(&mut rng, settings.rollout_temperature, &temp_vectors.moves);

        position.do_move(best_move);

        temp_vectors.moves.clear();
        let (score, _) = rollout(position, settings, depth - 1, temp_vectors);
        (1.0 - score, None)
    }
}

/// A game result from one side's perspective
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameResultForUs {
    Win,
    Loss,
    Draw,
}

impl ops::Not for GameResultForUs {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            GameResultForUs::Win => GameResultForUs::Loss,
            GameResultForUs::Loss => GameResultForUs::Win,
            GameResultForUs::Draw => GameResultForUs::Draw,
        }
    }
}

impl GameResultForUs {
    fn score(self) -> f32 {
        match self {
            GameResultForUs::Win => 1.0,
            GameResultForUs::Loss => 0.0,
            GameResultForUs::Draw => 0.5,
        }
    }
}

/// Selects a move from the move_scores vector,
/// tending towards the highest-scoring moves, but with a random component
/// If temperature is low (e.g. 0.1), it tends to choose the highest-scoring move
/// If temperature is 1.0, it chooses a move proportional to its score
pub fn best_move<R: Rng, const S: usize>(
    rng: &mut R,
    temperature: Option<f64>,
    move_scores: &[(Move<S>, f16)],
) -> Move<S> {
    if let Some(temperature) = temperature {
        let mut move_probabilities = Vec::with_capacity(move_scores.len());
        let mut cumulative_prob = 0.0;

        for (mv, individual_prob) in move_scores.iter() {
            cumulative_prob += (individual_prob.to_f64()).powf(1.0 / temperature);
            move_probabilities.push((mv, cumulative_prob));
        }

        let p = rng.gen_range(0.0..cumulative_prob);
        for (mv, cumulative_prob) in move_probabilities {
            if cumulative_prob > p {
                return *mv;
            }
        }
        unreachable!()
    } else {
        return move_scores
            .iter()
            .max_by(|(_, score1), (_, score2)| score1.total_cmp(score2))
            .unwrap()
            .0;
    }
}

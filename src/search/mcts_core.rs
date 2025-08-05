use std::f32;
use std::mem;
use std::ops;

use arrayvec::ArrayVec;
use board_game_traits::{Color, GameResult, Position as PositionTrait};
use half::f16;
use half::slice::HalfFloatSliceExt;
use rand::Rng;
use rand_distr::Distribution;

use crate::evaluation::parameters::IncrementalPolicy;
use crate::evaluation::topaz_eval::{BoardData, NNUE6};
use crate::position::Move;
/// This module contains the core of the MCTS search algorithm
use crate::position::Position;
use crate::search::{cp_to_win_percentage, MctsSetting};

use super::Error;

/// A Monte Carlo Search Tree, containing every node that has been seen in search.
#[derive(PartialEq, Debug)]
pub struct Tree<const S: usize> {
    pub total_action_value: f64,
    pub game_result: Option<GameResultForUs>,
    pub children: Option<Box<TreeChild<S>>>,
}

#[derive(PartialEq, Debug)]
pub enum TreeChild<const S: usize> {
    Small(SmallBridge<S>),
    Large(TreeBridge<S>),
}

#[derive(PartialEq, Debug)]
pub struct SmallBridge<const S: usize> {
    pub moves: Box<[Option<Move<S>>]>,
    pub heuristic_scores: Box<[f16]>,
    pub children: ArrayVec<(Tree<S>, Move<S>, u32), 4>,
}

#[derive(PartialEq, Debug)]
pub struct TreeBridge<const S: usize> {
    pub children: Box<[TreeEdge<S>]>,
    pub moves: Box<[Option<Move<S>>]>,
    pub mean_action_values: Box<[f32]>,
    pub visitss: Box<[u32]>,
    pub heuristic_scores: Box<[f16]>,
}

#[derive(PartialEq, Debug)]
pub struct TreeEdge<const S: usize> {
    pub child: Option<Tree<S>>,
}

/// Temporary vectors that are continually re-used during search to avoid unnecessary allocations
pub struct TempVectors<const S: usize> {
    simple_moves: Vec<Move<S>>,
    moves: Vec<(Move<S>, f16)>,
    fcd_per_move: Vec<i8>,
    policy_feature_sets: Vec<IncrementalPolicy<S>>,
    unpacked_heuristic_scores: Vec<f32>,
    topaz_evaluator: NNUE6,
}

impl<const S: usize> Default for TempVectors<S> {
    fn default() -> Self {
        TempVectors {
            simple_moves: vec![],
            moves: vec![],
            fcd_per_move: vec![],
            policy_feature_sets: vec![],
            unpacked_heuristic_scores: vec![0.0; 65536],
            topaz_evaluator: NNUE6::default(),
        }
    }
}

impl<const S: usize> TempVectors<S> {
    pub fn clear(&mut self) {
        self.simple_moves.clear();
        self.moves.clear();
        self.fcd_per_move.clear();
        self.policy_feature_sets.clear();
        self.unpacked_heuristic_scores.fill(0.0);
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
    cpuct * heuristic_score * parent_visits_sqrt / (1 + child_visits) as f32 - mean_action_value
}

// A fast approximation for ln(x) invented by ChatGPT. Seems to be accurate to within +/- 5%
fn fast_ln(f: f32) -> f32 {
    assert!(f > 0.0);
    let result = f.to_bits() as f32 * 1.192_092_9e-7;
    (result - 127.0) * f32::consts::LN_2
}

impl<const S: usize> TreeChild<S> {
    pub fn small_bridge(&mut self) -> Option<&mut SmallBridge<S>> {
        match self {
            TreeChild::Small(small_bridge) => Some(small_bridge),
            TreeChild::Large(_) => None,
        }
    }

    pub fn select(
        &mut self,
        position: &mut Position<S>,
        settings: &MctsSetting<S>,
        temp_vectors: &mut TempVectors<S>,
        our_visits: u32,
    ) -> Result<f32, Error> {
        match self {
            TreeChild::Small(small_bridge) => {
                // If this select returns None, it failed, and we need to grow the bridge into a full one
                let result = small_bridge.select(position, settings, temp_vectors, our_visits)?;
                if let Some(result) = result {
                    return Ok(result);
                }

                let num_child_nodes = self.small_bridge().unwrap().moves.len();

                // Allocate the box slices via Vec, so that we can recover from OOM with try_reserve_exact()
                let mut children: Vec<_> = vec![];
                children.try_reserve_exact(num_child_nodes)?;
                let mut mean_action_values: Vec<_> = vec![];
                mean_action_values.try_reserve_exact(num_child_nodes)?;
                let mut visitss: Vec<_> = vec![];
                visitss.try_reserve_exact(num_child_nodes)?;

                assert_eq!(children.capacity(), num_child_nodes);
                assert_eq!(mean_action_values.capacity(), num_child_nodes);
                assert_eq!(visitss.capacity(), num_child_nodes);

                for _ in 0..num_child_nodes {
                    children.push(TreeEdge { child: None });
                    mean_action_values.push(settings.initial_mean_action_value());
                    visitss.push(0);
                }

                let mut tree_edge = TreeBridge {
                    children: children.into_boxed_slice(),
                    moves: mem::take(&mut self.small_bridge().unwrap().moves),
                    mean_action_values: mean_action_values.into_boxed_slice(),
                    visitss: visitss.into_boxed_slice(),
                    heuristic_scores: mem::take(&mut self.small_bridge().unwrap().heuristic_scores),
                };

                for (child, mv, visits) in mem::take(&mut self.small_bridge().unwrap().children) {
                    let index = tree_edge.moves.iter().position(|m| m == &Some(mv)).unwrap();
                    tree_edge.mean_action_values[index] =
                        child.total_action_value as f32 / visits as f32;
                    tree_edge.children[index] = TreeEdge { child: Some(child) };
                    tree_edge.visitss[index] = visits;
                }

                *self = TreeChild::Large(tree_edge);

                self.select(position, settings, temp_vectors, our_visits)
            }
            TreeChild::Large(tree_bridge) => {
                tree_bridge.select(position, settings, temp_vectors, our_visits)
            }
        }
    }

    /// Apply Dirichlet noise to the heuristic scores of the child node
    /// The noise is given `epsilon` weight.
    /// `alpha` is used to generate the noise, lower values generate more varied noise.
    /// Values above 1 are less noisy, and tend towards uniform outputs
    pub fn apply_dirichlet(&mut self, epsilon: f32, alpha: f32) {
        let mut rng = rand::thread_rng();
        let heuristic_scores = match self {
            TreeChild::Small(small_bridge) => &mut small_bridge.heuristic_scores,
            TreeChild::Large(tree_bridge) => &mut tree_bridge.heuristic_scores,
        };
        let dirichlet =
            rand_distr::Dirichlet::new_with_size(alpha, heuristic_scores.len()).unwrap();
        let noise_vec = dirichlet.sample(&mut rng);
        for (child_prior, eta) in heuristic_scores.iter_mut().zip(noise_vec) {
            *child_prior = f16::from_f32(child_prior.to_f32() * (1.0 - epsilon) + epsilon * eta);
        }
    }
}

impl<const S: usize> SmallBridge<S> {
    pub fn new(
        position: &mut Position<S>,
        settings: &MctsSetting<S>,
        temp_vectors: &mut TempVectors<S>,
    ) -> Result<Self, Error> {
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

        // Allocate the box slices via Vec, so that we can recover from OOM with try_reserve_exact()
        let mut moves = vec![];
        moves.try_reserve_exact(num_children + padding)?;
        let mut heuristic_scores = vec![];
        heuristic_scores.try_reserve_exact(num_children + padding)?;

        for (mv, score) in temp_vectors.moves.drain(..) {
            moves.push(Some(mv));
            heuristic_scores.push(score);
        }
        for _ in 0..padding {
            moves.push(None);
            heuristic_scores.push(f16::NEG_INFINITY); // Ensure that this move never actually gets selected
        }

        let small_edge = SmallBridge {
            moves: moves.into_boxed_slice(),
            heuristic_scores: heuristic_scores.into_boxed_slice(),
            children: ArrayVec::new(),
        };
        assert!(temp_vectors.moves.is_empty());

        Ok(small_edge)
    }

    #[inline(never)]
    pub fn best_child(&self, settings: &MctsSetting<S>, our_visits: u32) -> (usize, bool) {
        let visits_sqrt = (our_visits as f32).sqrt();
        let dynamic_cpuct = settings.c_puct_init()
            + fast_ln((1.0 + our_visits as f32 + settings.c_puct_base()) / settings.c_puct_base());

        let mut best_child_node_index = 0;
        let mut best_score = f32::NEG_INFINITY;
        let mut best_is_initialized = false;

        for (i, (mv, heuristic_score)) in self
            .moves
            .iter()
            .flatten()
            .zip(&self.heuristic_scores)
            .enumerate()
        {
            if let Some((edge, _, visits)) =
                self.children.iter().find(|(_, child_mv, _)| child_mv == mv)
            {
                let mean_action_value = edge.total_action_value as f32 / *visits as f32;
                let score = exploration_value(
                    mean_action_value,
                    heuristic_score.to_f32(),
                    *visits,
                    visits_sqrt,
                    dynamic_cpuct,
                );
                if score > best_score {
                    best_score = score;
                    best_child_node_index = i;
                    best_is_initialized = true;
                }
            } else {
                let score = exploration_value(
                    settings.initial_mean_action_value(),
                    heuristic_score.to_f32(),
                    0,
                    visits_sqrt,
                    dynamic_cpuct,
                );
                if score > best_score {
                    best_score = score;
                    best_child_node_index = i;
                    best_is_initialized = false;
                }
            }
        }

        (best_child_node_index, best_is_initialized)
    }

    pub fn select(
        &mut self,
        position: &mut Position<S>,
        settings: &MctsSetting<S>,
        temp_vectors: &mut TempVectors<S>,
        our_visits: u32,
    ) -> Result<Option<f32>, Error> {
        let (best_child_node_index, is_initialized) = self.best_child(settings, our_visits);

        let child_move = self.moves[best_child_node_index].unwrap();

        if !is_initialized {
            if self.children.remaining_capacity() == 0 {
                return Ok(None);
            }

            position.do_move(child_move);

            let (result, game_result) =
                rollout(position, settings, settings.rollout_depth, temp_vectors);
            self.children.push((
                Tree {
                    total_action_value: result as f64,
                    game_result,
                    children: None,
                },
                child_move,
                1, // Initialize with 1 visit,
            ));
            return Ok(Some(1.0 - result));
        }

        let (child_tree, _, child_visits) = self
            .children
            .iter_mut()
            .find(|(_, mv, _)| mv == &child_move)
            .unwrap();

        position.do_move(child_move);

        let result = child_tree.select(position, settings, temp_vectors, *child_visits)?;

        *child_visits += 1;

        Ok(Some(1.0 - result))
    }
}

impl<const S: usize> TreeBridge<S> {
    #[inline(never)]
    pub fn best_child(
        &mut self,
        settings: &MctsSetting<S>,
        temp_vectors: &mut TempVectors<S>,
        our_visits: u32,
    ) -> usize {
        let visits_sqrt = (our_visits as f32).sqrt();
        let dynamic_cpuct = settings.c_puct_init()
            + fast_ln((1.0 + our_visits as f32 + settings.c_puct_base()) / settings.c_puct_base());

        let unpacked_heuristic_scores =
            &mut temp_vectors.unpacked_heuristic_scores[0..self.heuristic_scores.len()];
        self.heuristic_scores
            .convert_to_f32_slice(unpacked_heuristic_scores);
        let heuristic_scores = unpacked_heuristic_scores;

        assert_eq!(heuristic_scores.len() % SIMD_WIDTH, 0);
        assert_eq!(heuristic_scores.len(), self.mean_action_values.len());
        assert_eq!(heuristic_scores.len(), self.visitss.len());

        for (i, heuristic_score) in heuristic_scores.iter_mut().enumerate() {
            let mean_action_value = &self.mean_action_values[i];
            let child_visits = &self.visitss[i];

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
            .max_by(|(_, a), (_, b)| {
                // Safety: The values in `maxes` are guaranteed to be larger than `f32::NEG_INFINITY`, so cannot be NaN
                unsafe { a.partial_cmp(b).unwrap_unchecked() }
            })
            .unwrap();

        best_child_node_index as usize
    }

    pub fn select(
        &mut self,
        position: &mut Position<S>,
        settings: &MctsSetting<S>,
        temp_vectors: &mut TempVectors<S>,
        our_visits: u32,
    ) -> Result<f32, Error> {
        assert_ne!(
            self.children.len(),
            0,
            "No legal moves in position\n{:?}",
            position
        );

        let best_child_node_index = self.best_child(settings, temp_vectors, our_visits);

        let child_edge = self.children.get_mut(best_child_node_index).unwrap();
        let child_visits = self.visitss.get_mut(best_child_node_index).unwrap();
        let child_move = self
            .moves
            .get_mut(best_child_node_index)
            .unwrap()
            .unwrap_or_else(|| {
                panic!(
                    "Move has {} visits from {} parent vists",
                    child_visits, our_visits
                )
            });

        position.do_move(child_move);

        let result = 1.0 - child_edge.select(position, settings, temp_vectors, *child_visits)?;

        *self.visitss.get_mut(best_child_node_index).unwrap() += 1;

        *self
            .mean_action_values
            .get_mut(best_child_node_index)
            .unwrap() = child_edge.child.as_ref().unwrap().total_action_value as f32
            / *self.visitss.get_mut(best_child_node_index).unwrap() as f32;
        Ok(result)
    }
}

impl<const S: usize> TreeEdge<S> {
    pub fn select(
        &mut self,
        position: &mut Position<S>,
        settings: &MctsSetting<S>,
        temp_vectors: &mut TempVectors<S>,
        parent_visits: u32,
    ) -> Result<f32, Error> {
        if let Some(child) = self.child.as_mut() {
            return child.select(position, settings, temp_vectors, parent_visits);
        }

        let (result, game_result) =
            rollout(position, settings, settings.rollout_depth, temp_vectors);
        self.child = Some(Tree {
            total_action_value: result as f64,
            game_result,
            children: None,
        });

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
        parent_visits: u32,
    ) -> Result<f32, Error> {
        // TODO: Assume node has already had 1 visit before?
        if let Some(game_result) = self.game_result {
            let result = game_result.score();
            self.total_action_value += result as f64;
            return Ok(result);
        }
        let Some(children) = self.children.as_mut() else {
            let result = self.expand_child(position, settings, temp_vectors)?;
            self.total_action_value += result as f64;
            return Ok(result);
        };

        let result = children.select(position, settings, temp_vectors, parent_visits)?;
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
    ) -> Result<f32, Error> {
        let mut small_bridge = SmallBridge::new(position, settings, temp_vectors)?;

        // Select child edge before writing the child node into the tree, in case we OOM inside this call
        let result = small_bridge
            .select(position, settings, temp_vectors, 1)?
            .unwrap();

        self.children = Some(trybox::new(TreeChild::Small(small_bridge))?);

        Ok(result)
    }
}

pub struct Pv<'a, const S: usize> {
    tree: Option<&'a TreeChild<S>>,
}

impl<'a, const S: usize> Pv<'a, S> {
    pub fn new(tree: &'a TreeChild<S>) -> Pv<'a, S> {
        Pv { tree: Some(tree) }
    }
}

impl<const S: usize> Iterator for Pv<'_, S> {
    type Item = Move<S>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.tree {
            None => None,
            Some(TreeChild::Small(small_bridge)) => {
                if let Some((child, mv, _)) = small_bridge
                    .children
                    .iter()
                    .max_by_key(|(_, _, visits)| *visits)
                {
                    self.tree = child.children.as_ref().map(|bx| bx.as_ref());
                    Some(*mv)
                } else {
                    None
                }
            }
            Some(TreeChild::Large(tree_bridge)) => {
                let (_, (mv, child)) = tree_bridge
                    .visitss
                    .iter()
                    .zip(tree_bridge.moves.iter().zip(&tree_bridge.children))
                    .filter(|(_, (mv, _))| mv.is_some())
                    .max_by_key(|(visits, _)| **visits)?;
                self.tree = child
                    .child
                    .as_ref()
                    .and_then(|bx| bx.children.as_ref())
                    .map(|bx| bx.as_ref());
                *mv
            }
        }
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

        let centipawn_score = temp_vectors
            .topaz_evaluator
            .incremental_eval(BoardData::from(position.clone()))
            as f32;

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
        move_scores
            .iter()
            .max_by(|(_, score1), (_, score2)| score1.total_cmp(score2))
            .unwrap()
            .0
    }
}

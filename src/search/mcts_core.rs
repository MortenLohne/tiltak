use std::f32;
use std::mem;
use std::ops;

use board_game_traits::{Color, GameResult, Position as PositionTrait};
use half::f16;
use half::slice::HalfFloatSliceExt;
use rand::Rng;
use rand_distr::Distribution;
use size_of::SizeOf;

use crate::evaluation::parameters::IncrementalPolicy;
use crate::position::Move;
/// This module contains the core of the MCTS search algorithm
use crate::position::Position;
use crate::search::{cp_to_win_percentage, MctsSetting};

use super::Error;

/// A Monte Carlo Search Tree, containing every node that has been seen in search.
#[derive(PartialEq, Debug, SizeOf)]
pub struct Tree<const S: usize> {
    pub total_action_value: f64,
    pub game_result: Option<GameResultForUs>,
    pub children: Option<Box<TreeBridge<S>>>,
}

#[derive(PartialEq, Debug)]
pub struct TreeBridge<const S: usize> {
    pub children: Box<[TreeEdge<S>]>,
    pub moves: Box<[Option<Move<S>>]>,
    pub mean_action_values: Box<[f32]>,
    pub visitss: Box<[u32]>,
    pub heuristic_scores: Box<[f16]>,
}

impl<const S: usize> SizeOf for TreeBridge<S> {
    fn size_of_children(&self, context: &mut size_of::Context) {
        context.add_arraylike(self.moves.len(), mem::size_of::<Option<Move<S>>>());
        context.add_arraylike(self.mean_action_values.len(), mem::size_of::<f32>());
        context.add_arraylike(self.visitss.len(), mem::size_of::<u32>());
        context.add_arraylike(self.heuristic_scores.len(), mem::size_of::<f16>());

        for child in self.children.iter() {
            child.size_of_with_context(context);
        }
    }
}

#[derive(PartialEq, Debug, SizeOf)]
pub struct TreeEdge<const S: usize> {
    pub child: Option<Box<Tree<S>>>,
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
    cpuct * heuristic_score * parent_visits_sqrt / (1 + child_visits) as f32 - mean_action_value
}

// A fast approximation for ln(x) invented by ChatGPT. Seems to be accurate to within +/- 5%
fn fast_ln(f: f32) -> f32 {
    assert!(f > 0.0);
    let result = f.to_bits() as f32 * 1.192_092_9e-7;
    (result - 127.0) * f32::consts::LN_2
}

impl<const S: usize> TreeBridge<S> {
    #[inline(always)]
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

        for i in 0..heuristic_scores.len() {
            let heuristic_score = &mut heuristic_scores[i];
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

    /// Apply Dirichlet noise to the heuristic scores of the child node
    /// The noise is given `epsilon` weight.
    /// `alpha` is used to generate the noise, lower values generate more varied noise.
    /// Values above 1 are less noisy, and tend towards uniform outputs
    pub fn apply_dirichlet(&mut self, epsilon: f32, alpha: f32) {
        let mut rng = rand::thread_rng();
        let dirichlet = rand_distr::Dirichlet::new_with_size(alpha, self.children.len()).unwrap();
        let noise_vec = dirichlet.sample(&mut rng);
        for (child_prior, eta) in self.heuristic_scores.iter_mut().zip(noise_vec) {
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
        parent_visits: u32,
    ) -> Result<f32, Error> {
        if let Some(child) = self.child.as_mut() {
            return child.select(position, settings, temp_vectors, parent_visits);
        }

        let (result, game_result) =
            rollout(position, settings, settings.rollout_depth, temp_vectors);
        self.child = Some(Box::new(Tree {
            // TODO: OOM handling
            total_action_value: result as f64,
            game_result,
            children: None,
        }));

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
        assert!(self.children.is_none());
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
            children: (0..(num_children + padding))
                .map(|_| TreeEdge { child: None })
                .collect::<Box<_>>(), // TODO: OOM handling
            moves: (0..(num_children + padding))
                .map(|i| temp_vectors.moves.get(i).map(|(mv, _)| *mv))
                .collect(),
            // TODO: OOM handling
            mean_action_values: (0..(num_children + padding))
                .map(|_| settings.initial_mean_action_value())
                .collect(),
            // TODO: OOM handling
            visitss: (0..(num_children + padding)).map(|_| 0).collect(), // TODO: OOM handling

            heuristic_scores: (0..(num_children + padding))
                .map(|i| {
                    temp_vectors
                        .moves
                        .get(i)
                        .map(|(_, score)| *score)
                        .unwrap_or(f16::NEG_INFINITY) // Ensure that this move never actually gets selected
                })
                .collect(), // TODO: OOM handling
        };
        temp_vectors.moves.clear();

        // Select child edge before writing the child node into the tree, in case we OOM inside this call
        let result = tree_edge.select(position, settings, temp_vectors, 1)?;

        self.children = Some(Box::new(tree_edge)); // TODO: OOM handling

        Ok(result)
    }
}

pub struct Pv<'a, const S: usize> {
    edge: &'a TreeEdge<S>,
}

impl<'a, const S: usize> Pv<'a, S> {
    pub fn new(edge: &'a TreeEdge<S>) -> Pv<'a, S> {
        Pv { edge }
    }
}

impl<const S: usize> Iterator for Pv<'_, S> {
    type Item = Move<S>;

    fn next(&mut self) -> Option<Self::Item> {
        self.edge
            .child
            .as_ref()
            .and_then(|child_index| {
                let child = child_index;
                child.children.as_ref()
            })
            .and_then(|index| {
                let bridge = index;
                let (_, (mv, child)) = bridge
                    .visitss
                    .iter()
                    .zip(bridge.moves.iter().zip(&bridge.children))
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
#[derive(Clone, Copy, Debug, Eq, PartialEq, SizeOf)]
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

use std::ops;

use board_game_traits::{Color, GameResult, Position as PositionTrait};
use half::f16;
use rand::distributions::Distribution;
use rand::Rng;

use crate::evaluation::parameters::IncrementalPolicy;
use crate::position::Move;
/// This module contains the core of the MCTS search algorithm
use crate::position::{GroupData, Position};
use crate::search::{cp_to_win_percentage, MctsSetting, Score};

use super::{arena, Arena};

/// A Monte Carlo Search Tree, containing every node that has been seen in search.
#[derive(PartialEq, Debug)]
pub struct Tree<const S: usize> {
    pub total_action_value: f64,
    pub is_terminal: bool,
    pub children: arena::SliceIndex<TreeEdge<S>>, // This is only `None` if the node is confirmed to be a terminal node. Uninitialized nodes will have `Some(SliceIndex::default())`
}

#[derive(PartialEq, Debug)]
pub struct TreeEdge<const S: usize> {
    pub child: Option<arena::Index<Tree<S>>>,
    pub mv: Move<S>,
    pub mean_action_value: Score,
    pub visits: u32,
    pub heuristic_score: f16,
}

/// Temporary vectors that are continually re-used during search to avoid unnecessary allocations
#[derive(Debug)]
pub struct TempVectors<const S: usize> {
    simple_moves: Vec<Move<S>>,
    moves: Vec<(Move<S>, f16)>,
    fcd_per_move: Vec<i8>,
    policy_feature_sets: Vec<IncrementalPolicy<S>>,
}

impl<const S: usize> Default for TempVectors<S> {
    fn default() -> Self {
        TempVectors {
            simple_moves: vec![],
            moves: vec![],
            fcd_per_move: vec![],
            policy_feature_sets: vec![],
        }
    }
}

impl<const S: usize> TreeEdge<S> {
    pub fn new(mv: Move<S>, heuristic_score: f16, mean_action_value: Score) -> Self {
        TreeEdge {
            child: None,
            mv,
            mean_action_value,
            visits: 0,
            heuristic_score,
        }
    }

    pub fn shallow_clone(&self) -> Self {
        Self {
            child: None,
            mv: self.mv,
            mean_action_value: self.mean_action_value,
            visits: self.visits,
            heuristic_score: self.heuristic_score,
        }
    }

    /// Perform one iteration of monte carlo tree search.
    ///
    /// Moves done on the board are not reversed.
    #[must_use]
    pub fn select(
        &mut self,
        position: &mut Position<S>,
        settings: &MctsSetting<S>,
        temp_vectors: &mut TempVectors<S>,
        arena: &Arena,
    ) -> Option<Score> {
        if self.visits == 0 {
            self.expand(position, settings, temp_vectors, arena)
        } else if self.visits == u32::MAX {
            return None;
        } else if arena.get(self.child.as_ref().unwrap()).is_terminal() {
            self.visits += 1;
            arena
                .get_mut(self.child.as_mut().unwrap())
                .total_action_value += self.mean_action_value as f64;
            Some(self.mean_action_value)
        } else {
            let node = arena.get_mut(self.child.as_mut().unwrap());
            debug_assert_eq!(
                self.visits,
                arena
                    .get_slice(&node.children)
                    .iter()
                    .map(|edge| edge.visits)
                    .sum::<u32>()
                    + 1,
                "{} visits, {} total action value, {} mean action value",
                self.visits,
                node.total_action_value,
                self.mean_action_value
            );
            // Only generate child moves on the 2nd visit
            if self.visits == 1 {
                let group_data = position.group_data();
                node.init_children(position, &group_data, settings, temp_vectors, arena)?;
            }

            let visits_sqrt = (self.visits as Score).sqrt();
            let dynamic_cpuct = settings.c_puct_init()
                + Score::ln(
                    (1.0 + self.visits as Score + settings.c_puct_base()) / settings.c_puct_base(),
                );

            assert_ne!(
                arena.get_slice(&node.children).len(),
                0,
                "No legal moves in position\n{:?}",
                position
            );

            let mut best_exploration_value = 0.0;
            let mut best_child_node_index = 0;

            for (i, edge) in arena.get_slice(&node.children).iter().enumerate() {
                let child_exploration_value = edge.exploration_value(visits_sqrt, dynamic_cpuct);
                if child_exploration_value >= best_exploration_value {
                    best_child_node_index = i;
                    best_exploration_value = child_exploration_value;
                }
            }

            let child_edge = arena
                .get_slice_mut(&mut node.children)
                .get_mut(best_child_node_index)
                .unwrap();

            position.do_move(child_edge.mv);
            let result = 1.0 - child_edge.select(position, settings, temp_vectors, arena)?;
            self.visits += 1;

            node.total_action_value += result as f64;

            self.mean_action_value = (node.total_action_value / self.visits as f64) as f32;
            Some(result)
        }
    }

    // Never inline, for profiling purposes
    #[inline(never)]
    #[must_use]
    fn expand(
        &mut self,
        position: &mut Position<S>,
        settings: &MctsSetting<S>,
        temp_vectors: &mut TempVectors<S>,
        arena: &Arena,
    ) -> Option<Score> {
        debug_assert!(self.child.is_none());
        self.child = Some(arena.add(Tree::new_node())?);

        let child = arena.get_mut(self.child.as_mut().unwrap());

        let (eval, is_terminal) = rollout(position, settings, settings.rollout_depth, temp_vectors);

        self.visits = 1;
        child.total_action_value = eval as f64;
        self.mean_action_value = eval;
        child.is_terminal = is_terminal;
        Some(eval)
    }

    #[inline]
    pub fn exploration_value(&self, parent_visits_sqrt: Score, cpuct: Score) -> Score {
        (1.0 - self.mean_action_value)
            + cpuct * self.heuristic_score.to_f32() * parent_visits_sqrt
                / (1 + self.visits) as Score
    }
}

impl<const S: usize> Tree<S> {
    fn is_terminal(&self) -> bool {
        self.is_terminal
    }

    /// Do not initialize children in the expansion phase, for better performance
    /// Never inline, for profiling purposes
    #[inline(never)]
    #[must_use]
    fn init_children(
        &mut self,
        position: &Position<S>,
        group_data: &GroupData<S>,
        settings: &MctsSetting<S>,
        temp_vectors: &mut TempVectors<S>,
        arena: &Arena,
    ) -> Option<()> {
        position.generate_moves_with_params(
            match settings.policy_params.as_ref() {
                Some(params) => params,
                None => <Position<S>>::policy_params(position.komi()),
            },
            group_data,
            &mut temp_vectors.simple_moves,
            &mut temp_vectors.moves,
            &mut temp_vectors.fcd_per_move,
            &mut temp_vectors.policy_feature_sets,
        );
        // let mut children_vec = arena.add_from_iter(length, source) Vec::with_capacity(temp_vectors.moves.len());
        let policy_sum: f32 = temp_vectors
            .moves
            .iter()
            .map(|(_, score)| score.to_f32())
            .sum();
        let inv_sum = 1.0 / policy_sum;

        let child_edges = temp_vectors.moves.drain(..).map(|(mv, heuristic_score)| {
            TreeEdge::new(
                mv,
                f16::from_f32(heuristic_score.to_f32() * inv_sum),
                settings.initial_mean_action_value(),
            )
        });

        self.children = arena.add_slice(child_edges)?;
        Some(())
    }

    fn new_node() -> Self {
        Tree {
            children: arena::SliceIndex::default(),
            is_terminal: false,
            total_action_value: 0.0,
        }
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
            .get_slice_mut(&mut self.children)
            .iter_mut()
            .map(|child| &mut child.heuristic_score)
            .zip(noise_vec)
        {
            *child_prior = f16::from_f32(child_prior.to_f32() * (1.0 - epsilon) + epsilon * eta);
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
) -> (Score, bool) {
    let group_data = position.group_data();

    if let Some(game_result) = position.game_result_with_group_data(&group_data) {
        let game_result_for_us = match (game_result, position.side_to_move()) {
            (GameResult::Draw, _) => GameResultForUs::Draw,
            (GameResult::WhiteWin, Color::Black) => GameResultForUs::Loss, // The side to move has lost
            (GameResult::BlackWin, Color::White) => GameResultForUs::Loss, // The side to move has lost
            (GameResult::WhiteWin, Color::White) => GameResultForUs::Win, // The side to move has lost
            (GameResult::BlackWin, Color::Black) => GameResultForUs::Win, // The side to move has lost
        };

        (game_result_for_us.score(), true)
    } else if depth == 0 {
        let static_eval = cp_to_win_percentage(position.static_eval_with_params_and_data(
            &group_data,
            match settings.value_params.as_ref() {
                Some(params) => params,
                None => <Position<S>>::value_params(position.komi()),
            },
        ));
        match position.side_to_move() {
            Color::White => (static_eval, false),
            Color::Black => (1.0 - static_eval, false),
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
        (1.0 - score, false)
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
    fn score(self) -> Score {
        match self {
            GameResultForUs::Win => 1.0,
            GameResultForUs::Loss => 0.0,
            GameResultForUs::Draw => 0.5,
        }
    }
}

pub struct Pv<'a, const S: usize> {
    arena: &'a Arena,
    edge: Option<&'a TreeEdge<S>>,
}

impl<'a, const S: usize> Pv<'a, S> {
    pub fn new(edge: &'a TreeEdge<S>, arena: &'a Arena) -> Pv<'a, S> {
        let mut pv = Pv {
            edge: Some(edge),
            arena,
        };
        // Skip the dummy move on the top of the tree
        pv.next();
        pv
    }
}

impl<'a, const S: usize> Iterator for Pv<'a, S> {
    type Item = Move<S>;

    fn next(&mut self) -> Option<Self::Item> {
        self.edge.map(|edge| {
            let mv = edge.mv;
            if let Some(child_index) = &edge.child {
                let child = self.arena.get(child_index);

                let best_edge = self
                    .arena
                    .get_slice(&child.children)
                    .iter()
                    .max_by_key(|edge| edge.visits);
                self.edge = best_edge;
            }
            mv
        })
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

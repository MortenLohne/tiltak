use std::ops;

use board_game_traits::{Color, GameResult, Position as PositionTrait};
use rand::distributions::Distribution;
use rand::seq::SliceRandom;
use rand::Rng;

use crate::position::Move;
/// This module contains the core of the MCTS search algorithm
use crate::position::{GroupData, Position, TunableBoard};
use crate::search::{cp_to_win_percentage, MctsSetting, Score};

/// A Monte Carlo Search Tree, containing every node that has been seen in search.
#[derive(Clone, PartialEq, Debug)]
pub struct Tree {
    pub children: Box<[TreeEdge]>,
    pub total_action_value: f64,
    pub is_terminal: bool,
}

#[derive(Clone, PartialEq, Debug)]
pub struct TreeEdge {
    pub child: Option<Box<Tree>>,
    pub mv: Move,
    pub mean_action_value: Score,
    pub visits: u64,
    pub heuristic_score: Score,
}

impl TreeEdge {
    pub fn new(mv: Move, heuristic_score: Score) -> Self {
        TreeEdge {
            child: None,
            mv,
            mean_action_value: 0.1,
            visits: 0,
            heuristic_score,
        }
    }

    /// Perform one iteration of monte carlo tree search.
    ///
    /// Moves done on the board are not reversed.
    pub fn select<const S: usize>(
        &mut self,
        position: &mut Position<S>,
        settings: &MctsSetting<S>,
        simple_moves: &mut Vec<Move>,
        moves: &mut Vec<(Move, Score)>,
    ) -> Score {
        if self.visits == 0 {
            self.expand(position, settings, simple_moves)
        } else if self.child.as_ref().unwrap().is_terminal {
            self.visits += 1;
            self.child.as_mut().unwrap().total_action_value += self.mean_action_value as f64;
            self.mean_action_value
        } else {
            let node = self.child.as_mut().unwrap();
            debug_assert_eq!(
                self.visits,
                node.children.iter().map(|edge| edge.visits).sum::<u64>() + 1,
                "{} visits, {} total action value, {} mean action value",
                self.visits,
                node.total_action_value,
                self.mean_action_value
            );
            // Only generate child moves on the 2nd visit
            if self.visits == 1 {
                let group_data = position.group_data();
                node.init_children(
                    &position,
                    &group_data,
                    simple_moves,
                    &settings.policy_params,
                    moves,
                );
            }

            let visits_sqrt = (self.visits as Score).sqrt();
            let dynamic_cpuct = settings.c_puct_init()
                + Score::ln(
                    (1.0 + self.visits as Score + settings.c_puct_base()) / settings.c_puct_base(),
                );

            assert_ne!(
                node.children.len(),
                0,
                "No legal moves in position\n{:?}",
                position
            );

            let mut best_exploration_value = 0.0;
            let mut best_child_node_index = 0;

            for (i, edge) in node.children.iter().enumerate() {
                let child_exploration_value = edge.exploration_value(visits_sqrt, dynamic_cpuct);
                if child_exploration_value >= best_exploration_value {
                    best_child_node_index = i;
                    best_exploration_value = child_exploration_value;
                }
            }

            let child_edge = node.children.get_mut(best_child_node_index).unwrap();

            position.do_move(child_edge.mv.clone());
            let result = 1.0 - child_edge.select::<S>(position, settings, simple_moves, moves);
            self.visits += 1;

            node.total_action_value += result as f64;

            self.mean_action_value = (node.total_action_value / self.visits as f64) as f32;
            result
        }
    }

    // Never inline, for profiling purposes
    #[inline(never)]
    fn expand<const S: usize>(
        &mut self,
        position: &mut Position<S>,
        settings: &MctsSetting<S>,
        simple_moves: &mut Vec<Move>,
    ) -> Score {
        debug_assert!(self.child.is_none());
        self.child = Some(Box::new(Tree::new_node()));
        let child = self.child.as_mut().unwrap();

        let group_data = position.group_data();

        if let Some(game_result) = position.game_result_with_group_data(&group_data) {
            let game_result_for_us = match (game_result, position.side_to_move()) {
                (GameResult::Draw, _) => GameResultForUs::Draw,
                (GameResult::WhiteWin, Color::Black) => GameResultForUs::Loss, // The side to move has lost
                (GameResult::BlackWin, Color::White) => GameResultForUs::Loss, // The side to move has lost
                (GameResult::WhiteWin, Color::White) => GameResultForUs::Win, // The side to move has lost
                (GameResult::BlackWin, Color::Black) => GameResultForUs::Win, // The side to move has lost
            };
            self.visits = 1;
            child.is_terminal = true;

            let score = game_result_for_us.score();
            self.mean_action_value = score;
            child.total_action_value = score as f64;

            return score;
        }

        let eval = rollout(position, settings, 5, simple_moves);

        self.visits = 1;
        child.total_action_value = eval as f64;
        self.mean_action_value = eval;
        eval
    }

    #[inline]
    pub fn exploration_value(&self, parent_visits_sqrt: Score, cpuct: Score) -> Score {
        (1.0 - self.mean_action_value)
            + cpuct * self.heuristic_score * parent_visits_sqrt / (1 + self.visits) as Score
    }
}

impl Tree {
    /// Do not initialize children in the expansion phase, for better performance
    /// Never inline, for profiling purposes
    #[inline(never)]
    fn init_children<const S: usize>(
        &mut self,
        position: &Position<S>,
        group_data: &GroupData<S>,
        simple_moves: &mut Vec<Move>,
        policy_params: &[f32],
        moves: &mut Vec<(Move, Score)>,
    ) {
        position.generate_moves_with_params(policy_params, group_data, simple_moves, moves);
        let mut children_vec = Vec::with_capacity(moves.len());
        let policy_sum: f32 = moves.iter().map(|(_, score)| *score).sum();
        let inv_sum = 1.0 / policy_sum;
        for (mv, heuristic_score) in moves.drain(..) {
            children_vec.push(TreeEdge::new(mv.clone(), heuristic_score * inv_sum));
        }
        self.children = children_vec.into_boxed_slice();
    }

    fn new_node() -> Self {
        Tree {
            children: Box::new([]),
            total_action_value: 0.0,
            is_terminal: false,
        }
    }

    /// Apply Dirichlet noise to the heuristic scores of the child node
    /// The noise is given `epsilon` weight.
    /// `alpha` is used to generate the noise, lower values generate more varied noise.
    /// Values above 1 are less noisy, and tend towards uniform outputs
    pub fn apply_dirichlet(&mut self, epsilon: f32, alpha: f32) {
        let mut rng = rand::thread_rng();
        let dirichlet = rand_distr::Dirichlet::new_with_size(alpha, self.children.len()).unwrap();
        let noise_vec = dirichlet.sample(&mut rng);
        for (child_prior, eta) in self
            .children
            .iter_mut()
            .map(|child| &mut child.heuristic_score)
            .zip(noise_vec)
        {
            *child_prior = *child_prior * (1.0 - epsilon) + epsilon * eta;
        }
    }
}

// Never inline, for profiling purposes
#[inline(never)]
pub fn rollout<const S: usize>(
    position: &mut Position<S>,
    settings: &MctsSetting<S>,
    depth: usize,
    simple_moves: &mut Vec<Move>,
) -> Score {
    let group_data = position.group_data();

    if let Some(game_result) = position.game_result_with_group_data(&group_data) {
        let game_result_for_us = match (game_result, position.side_to_move()) {
            (GameResult::Draw, _) => GameResultForUs::Draw,
            (GameResult::WhiteWin, Color::Black) => GameResultForUs::Loss, // The side to move has lost
            (GameResult::BlackWin, Color::White) => GameResultForUs::Loss, // The side to move has lost
            (GameResult::WhiteWin, Color::White) => GameResultForUs::Win, // The side to move has lost
            (GameResult::BlackWin, Color::Black) => GameResultForUs::Win, // The side to move has lost
        };

        game_result_for_us.score()
    } else if depth == 0 {
        let static_eval = cp_to_win_percentage(
            position.static_eval_with_params_and_data(&group_data, &settings.value_params),
        );
        match position.side_to_move() {
            Color::White => static_eval,
            Color::Black => 1.0 - static_eval,
        }
    } else {
        position.generate_moves(simple_moves);

        let mut rng = rand::thread_rng();

        let best_move = simple_moves.choose(&mut rng).unwrap().clone();
        position.do_move(best_move);

        simple_moves.clear();
        1.0 - rollout(position, settings, depth - 1, simple_moves)
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

pub struct Pv<'a> {
    tree: &'a Tree,
}

impl<'a> Pv<'a> {
    pub fn new(tree: &'a Tree) -> Pv<'a> {
        Pv { tree }
    }
}

impl<'a> Iterator for Pv<'a> {
    type Item = Move;

    fn next(&mut self) -> Option<Self::Item> {
        self.tree
            .children
            .iter()
            .max_by_key(|edge| edge.visits)
            .and_then(|edge| {
                edge.child.as_ref().map(|child| {
                    self.tree = child;
                    edge.mv.clone()
                })
            })
    }
}

pub fn best_move<R: Rng>(rng: &mut R, temperature: f64, move_scores: &[(Move, Score)]) -> Move {
    let mut move_probabilities = vec![];
    let mut cumulative_prob = 0.0;

    for (mv, individual_prob) in move_scores.iter() {
        cumulative_prob += (*individual_prob as f64).powf(1.0 / temperature);
        move_probabilities.push((mv, cumulative_prob));
    }

    let p = rng.gen_range(0.0, cumulative_prob);
    for (mv, cumulative_prob) in move_probabilities {
        if cumulative_prob > p {
            return mv.clone();
        }
    }
    unreachable!()
}

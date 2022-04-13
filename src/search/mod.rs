//! A strong Tak AI, based on Monte Carlo Tree Search.
//!
//! This implementation does not use full Monte Carlo rollouts, relying on a heuristic evaluation when expanding new nodes instead.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::{mem, time};

use crate::position::Move;
use crate::position::Position;
use crate::position::{Role, Square};
pub use crate::search::mcts_core::best_move;
use crate::search::mcts_core::{TempVectors, Tree};

use self::mcts_core::{Pv, TreeEdge};

/// This module contains the public-facing convenience API for the search.
/// The implementation itself in in mcts_core.
mod mcts_core;

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, PartialEq, Clone)]
pub enum TimeControl {
    FixedNodes(u64),
    Time(time::Duration, time::Duration), // Total time left, increment
}

#[derive(Clone, PartialEq, Debug)]
pub struct MctsSetting<const S: usize> {
    value_params: Vec<f32>,
    policy_params: Vec<f32>,
    search_params: Vec<Score>,
    dirichlet: Option<f32>,
    excluded_moves: Vec<Move>,
    rollout_depth: u16,
    rollout_temperature: f64,
}

impl<const S: usize> Default for MctsSetting<S> {
    fn default() -> Self {
        MctsSetting {
            value_params: Vec::from(<Position<S>>::value_params()),
            policy_params: Vec::from(<Position<S>>::policy_params()),
            search_params: vec![1.43, 2800.0, 0.61],
            dirichlet: None,
            excluded_moves: vec![],
            rollout_depth: 0,
            rollout_temperature: 0.25,
        }
    }
}

impl<const N: usize> MctsSetting<N> {
    pub fn add_value_params(mut self, value_params: Vec<f32>) -> Self {
        self.value_params = value_params;
        self
    }

    pub fn add_policy_params(mut self, policy_params: Vec<f32>) -> Self {
        self.policy_params = policy_params;
        self
    }

    pub fn add_search_params(mut self, search_params: Vec<f32>) -> Self {
        self.search_params = search_params;
        self
    }

    pub fn add_dirichlet(mut self, alpha: f32) -> Self {
        self.dirichlet = Some(alpha);
        self
    }

    pub fn exclude_moves(mut self, excluded_moves: Vec<Move>) -> Self {
        self.excluded_moves = excluded_moves;
        self
    }

    /// The maximum depth of the MCTS rollouts. Defaults to 0, in which case no rollouts are done
    pub fn add_rollout_depth(mut self, rollout_depth: u16) -> Self {
        self.rollout_depth = rollout_depth;
        self
    }

    /// The degree of randomness when picking moves in MCTS rollouts
    /// A value of 1.0 is highly random, values around 0.2 give low randomness
    pub fn add_rollout_temperature(mut self, temperature: f64) -> Self {
        self.rollout_temperature = temperature;
        self
    }

    pub fn c_puct_init(&self) -> Score {
        self.search_params[0]
    }

    pub fn c_puct_base(&self) -> Score {
        self.search_params[1]
    }

    pub fn initial_mean_action_value(&self) -> Score {
        self.search_params[2]
    }
}

/// Type alias for winning probability, used for scoring positions.
pub type Score = f32;

/// Abstract representation of a Monte Carlo Search Tree.
/// Gives more fine-grained control of the search process compared to using the `mcts` function.
#[derive(Clone, PartialEq, Debug)]
pub struct MonteCarloTree<const S: usize> {
    edge: TreeEdge, // A virtual edge to the first node, with fake move and heuristic score
    position: Position<S>,
    settings: MctsSetting<S>,
    temp_vectors: TempVectors,
}

impl<const S: usize> MonteCarloTree<S> {
    pub fn new(position: Position<S>) -> Self {
        MonteCarloTree {
            edge: TreeEdge {
                child: None,
                mv: Move::Place(Role::Flat, Square(0)),
                mean_action_value: 0.0,
                visits: 0,
                heuristic_score: 0.0,
            },
            position,
            settings: MctsSetting::default(),
            temp_vectors: TempVectors::new::<S>(),
        }
    }

    pub fn with_settings(position: Position<S>, settings: MctsSetting<S>) -> Self {
        #[allow(unused_mut)]
        let mut tree = MonteCarloTree {
            edge: TreeEdge {
                child: None,
                mv: Move::Place(Role::Flat, Square(0)),
                mean_action_value: 0.0,
                visits: 0,
                heuristic_score: 0.0,
            },
            position,
            settings: settings.clone(),
            temp_vectors: TempVectors::new::<S>(),
        };

        if let Some(alpha) = tree.settings.dirichlet {
            tree.select();
            tree.select();
            (*tree.edge.child.as_mut().unwrap()).apply_dirichlet(0.25, alpha);
        }

        if !tree.settings.excluded_moves.is_empty() {
            tree.select();
            tree.select();
            let filtered_edges: Vec<TreeEdge> = tree
                .edge
                .child
                .as_ref()
                .unwrap()
                .children
                .iter()
                .filter(|edge| !settings.excluded_moves.contains(&edge.mv))
                .cloned()
                .collect();
            tree.edge.child.as_mut().unwrap().children = filtered_edges.into_boxed_slice();
        }

        tree
    }

    pub fn search_for_time<F>(&mut self, max_time: time::Duration, callback: F)
    where
        F: Fn(&Self),
    {
        let start_time = time::Instant::now();

        for i in 0.. {
            let nodes = (50.0 * 2.0_f32.powf(0.125).powi(i)) as u64;
            for _ in 0..nodes {
                self.select();
            }

            // Always return when we have less than 10ms left
            if max_time < (time::Duration::from_millis(10))
                || start_time.elapsed() > max_time - (time::Duration::from_millis(10))
                || self.children().len() == 1
            {
                callback(self);
                return;
            }

            let mut child_refs: Vec<&TreeEdge> = self.children().iter().collect();
            child_refs.sort_by_key(|edge| edge.visits);
            child_refs.reverse();

            let node_ratio = (1 + child_refs[1].visits) as f32 / (1 + child_refs[0].visits) as f32;
            let time_ratio = start_time.elapsed().as_secs_f32() / max_time.as_secs_f32();

            let visits_sqrt = (self.visits() as f32).sqrt();
            let dynamic_cpuct = self.settings.c_puct_init()
                + Score::ln(
                    (1.0 + self.visits() as Score + self.settings.c_puct_base())
                        / self.settings.c_puct_base(),
                );

            let best_edge = self
                .children()
                .iter()
                .max_by_key(|edge| edge.visits)
                .unwrap();

            let best_exploration_value = best_edge.exploration_value(visits_sqrt, dynamic_cpuct);

            if time_ratio.powf(2.0) > node_ratio / 2.0 {
                callback(self);
                // Do not stop if any other child nodes have better exploration value
                if self.children().iter().any(|edge| {
                    edge.mv != best_edge.mv
                        && edge.exploration_value(visits_sqrt, dynamic_cpuct)
                            > best_exploration_value + 0.01
                }) {
                    continue;
                }
                return;
            } else if i % 2 == 0 {
                callback(self);
            }
        }
    }

    /// Run one iteration of MCTS
    pub fn select(&mut self) -> f32 {
        self.edge.select::<S>(
            &mut self.position.clone(),
            &self.settings,
            &mut self.temp_vectors,
        )
    }

    /// Returns the best move, and its score (as winning probability) from the perspective of the side to move
    /// Panics if no search iterations have been run
    pub fn best_move(&self) -> (Move, f32) {
        self.edge
            .child
            .as_ref()
            .unwrap()
            .children
            .iter()
            .max_by_key(|edge| edge.visits)
            .map(|edge| (edge.mv.clone(), 1.0 - edge.mean_action_value))
            .unwrap_or_else(|| panic!("Couldn't find best move"))
    }

    pub fn node_edge_sizes(&self) -> (usize, usize) {
        pub fn edge_sizes(edge: &TreeEdge) -> (usize, usize) {
            if let Some(child) = &edge.child {
                let (child_nodes, child_edges) = node_sizes(child);
                (child_nodes, child_edges + 1)
            } else {
                (0, 1)
            }
        }
        pub fn node_sizes(node: &Tree) -> (usize, usize) {
            node.children.iter().map(edge_sizes).fold(
                (1, 0),
                |(acc_nodes, acc_edges), (child_nodes, child_edges)| {
                    (acc_nodes + child_nodes, acc_edges + child_edges)
                },
            )
        }
        edge_sizes(&self.edge)
    }

    fn children(&self) -> &[TreeEdge] {
        &self.edge.child.as_ref().unwrap().children
    }

    pub fn pv(&self) -> impl Iterator<Item = Move> + '_ {
        Pv::new(self.edge.child.as_ref().unwrap())
    }

    /// Print human-readable information of the search's progress.
    pub fn print_info(&self) {
        let mut best_children: Vec<&TreeEdge> = self.children().iter().collect();

        best_children.sort_by_key(|edge| edge.visits);
        best_children.reverse();
        let dynamic_cpuct = self.settings.c_puct_init()
            + Score::ln(
                (1.0 + self.visits() as Score + self.settings.c_puct_base())
                    / self.settings.c_puct_base(),
            );

        best_children.iter().take(8).for_each(|edge| {
            println!(
                "Move {}: {} visits, {:.2}% mean action value, {:.3}% static score, {:.3} exploration value, pv {}",
                edge.mv.to_string::<S>(), edge.visits, edge.mean_action_value * 100.0, edge.heuristic_score * 100.0,
                edge.exploration_value((self.visits() as Score).sqrt(), dynamic_cpuct),
                if edge.child.is_some() {
                    Pv::new(edge.child.as_ref().unwrap()).map(|mv| mv.to_string::<S>() + " ").collect::<String>()
                }
                else {
                    String::new()
                }
            )
        });
    }

    pub fn visits(&self) -> u64 {
        self.edge.visits
    }

    pub fn mean_action_value(&self) -> Score {
        self.edge.mean_action_value
    }
}

/// The simplest way to use the mcts module. Run Monte Carlo Tree Search for `nodes` nodes, returning the best move, and its estimated winning probability for the side to move.
pub fn mcts<const S: usize>(position: Position<S>, nodes: u64) -> (Move, Score) {
    let mut tree = MonteCarloTree::new(position);

    for _ in 0..nodes.max(2) {
        tree.select();
    }
    let (mv, score) = tree.best_move();
    (mv, score)
}

/// Play a move, calculating for a maximum duration.
/// It will usually spend much less time, especially if the move is obvious.
/// On average, it will spend around 20% of `max_time`, and rarely more than 50%.
pub fn play_move_time<const S: usize>(
    board: Position<S>,
    max_time: time::Duration,
    settings: MctsSetting<S>,
) -> (Move, Score) {
    let mut tree = MonteCarloTree::with_settings(board, settings);
    tree.search_for_time(max_time, |_| {});
    tree.best_move()
}

/// Run mcts with specific static evaluation parameters, for optimization the parameter set.
/// Also applies Dirichlet noise to the root node
pub fn mcts_training<const S: usize>(
    position: Position<S>,
    time_control: &TimeControl,
    settings: MctsSetting<S>,
) -> Vec<(Move, Score)> {
    let mut tree = MonteCarloTree::with_settings(position, settings);

    match time_control {
        TimeControl::FixedNodes(nodes) => {
            for _ in 0..*nodes {
                tree.select();
            }
        }
        TimeControl::Time(time, increment) => {
            let max_time = *time / 5 + *increment / 2;
            tree.search_for_time(max_time, |_| {});
        }
    }

    let child_visits: u64 = tree.children().iter().map(|edge| edge.visits).sum();
    tree.children()
        .iter()
        .map(|edge| (edge.mv.clone(), edge.visits as f32 / child_visits as f32))
        .collect()
}

/// Convert a static evaluation in centipawns to a winning probability between 0.0 and 1.0.
pub fn cp_to_win_percentage(cp: f32) -> Score {
    1.0 / (1.0 + Score::exp(-cp as Score))
}

// Utility for testing
pub fn edge_mem_usage() -> usize {
    mem::size_of::<TreeEdge>()
}

// Utility for testing
pub fn node_mem_usage() -> usize {
    mem::size_of::<Tree>()
}

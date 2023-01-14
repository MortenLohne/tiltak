//! A strong Tak AI, based on Monte Carlo Tree Search.
//!
//! This implementation does not use full Monte Carlo rollouts, relying on a heuristic evaluation when expanding new nodes instead.

use dfdx::prelude::LoadFromNpz;
use dfdx::prelude::ModuleBuilder;
use dfdx::tensor::Cpu;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::{mem, time};

use crate::evaluation::parameters::PolicyModel;
use crate::evaluation::parameters::ValueModel;
use crate::evaluation::parameters::NUM_POLICY_FEATURES_6S;
use crate::evaluation::parameters::NUM_VALUE_FEATURES_6S;
use crate::position::Move;
use crate::position::Position;
use crate::position::{Role, Square};
pub use crate::search::mcts_core::best_move;
use crate::search::mcts_core::{TempVectors, Tree, TreeEdge};

use self::mcts_core::Pv;

mod arena;
/// This module contains the public-facing convenience API for the search.
/// The implementation itself in in mcts_core.
mod mcts_core;
pub use arena::Arena;

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, PartialEq, Clone)]
pub enum TimeControl {
    FixedNodes(u64),
    Time(time::Duration, time::Duration), // Total time left, increment
}

#[derive(Clone, Debug)]
pub struct MctsSetting<const S: usize> {
    arena_size: u32,
    pub cpu: Cpu,
    value_params: ValueModel<NUM_VALUE_FEATURES_6S>, // TODO: Generic
    pub policy_model: PolicyModel<NUM_POLICY_FEATURES_6S>,
    policy_params: Vec<f32>,
    search_params: Vec<Score>,
    dirichlet: Option<f32>,
    excluded_moves: Vec<Move>,
    rollout_depth: u16,
    rollout_temperature: f64,
}

impl<const S: usize> Default for MctsSetting<S> {
    fn default() -> Self {
        let cpu: Cpu = Default::default(); // TODO: Hard-coded stuff
        let mut value_params: ValueModel<NUM_VALUE_FEATURES_6S> = cpu.build_module();
        value_params.load("model_49811.zip").unwrap();
        let mut policy_model: PolicyModel<NUM_POLICY_FEATURES_6S> = cpu.build_module();
        policy_model.load("policy_model_0037861.zip").unwrap();
        MctsSetting {
            arena_size: 2_u32.pow(26), // Default to 1.5GB max
            cpu,
            value_params,
            policy_model,
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
    /// Set a very liberal arena size, for searching a given amount of nodes
    pub fn arena_size_for_nodes(self, nodes: u32) -> Self {
        // For 6s, the toughest position I've found required 40 elements/node searched
        // This formula gives 108, which is hopefully plenty
        self.arena_size((N * N) as u32 * 3 * nodes)
    }

    pub fn mem_usage(self, mem_usage: usize) -> Self {
        assert!(
            mem_usage < u32::MAX as usize // Check for 32-bit platforms
            || mem_usage < 24 * 2_usize.pow(32) - 2
        );
        self.arena_size((mem_usage / ARENA_ELEMENT_SIZE) as u32)
    }

    pub fn arena_size(mut self, arena_size: u32) -> Self {
        assert!(arena_size < u32::MAX - 1);
        self.arena_size = arena_size;
        self
    }

    pub fn add_value_params(self, _value_params: Vec<f32>) -> Self {
        unimplemented!() // TODO: Repair
                         //self.value_params = value_params;
                         //self
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
pub const ARENA_ELEMENT_SIZE: usize = 24;

/// Abstract representation of a Monte Carlo Search Tree.
/// Gives more fine-grained control of the search process compared to using the `mcts` function.
// #[derive(Clone, PartialEq, Debug)]
pub struct MonteCarloTree<const S: usize> {
    edge: TreeEdge, // A virtual edge to the first node, with fake move and heuristic score
    position: Position<S>,
    settings: MctsSetting<S>,
    temp_vectors: TempVectors,
    arena: Arena,
}

impl<const S: usize> MonteCarloTree<S> {
    pub fn new(position: Position<S>) -> Self {
        Self::with_settings(position, MctsSetting::default())
    }

    pub fn with_settings(position: Position<S>, settings: MctsSetting<S>) -> Self {
        let arena = Arena::new(settings.arena_size).unwrap();
        let mut temp_vectors = TempVectors::new::<S>();
        let mut root_edge = TreeEdge {
            child: None,
            mv: Move::Place(Role::Flat, Square(0)),
            mean_action_value: 0.0,
            visits: 0,
            heuristic_score: 0.0,
        };

        root_edge
            .select(&mut position.clone(), &settings, &mut temp_vectors, &arena)
            .unwrap();
        root_edge
            .select(&mut position.clone(), &settings, &mut temp_vectors, &arena)
            .unwrap();

        if let Some(alpha) = settings.dirichlet {
            (arena.get_mut(root_edge.child.as_mut().unwrap())).apply_dirichlet(&arena, 0.25, alpha);
        }

        if !settings.excluded_moves.is_empty() {
            let mut filtered_edges: Vec<TreeEdge> = arena
                .get_slice_mut(&mut arena.get_mut(root_edge.child.as_mut().unwrap()).children)
                .iter_mut()
                .filter(|edge| !settings.excluded_moves.contains(&edge.mv))
                .map(|edge| TreeEdge {
                    child: edge.child.take(),
                    mv: edge.mv.clone(),
                    mean_action_value: edge.mean_action_value,
                    visits: edge.visits,
                    heuristic_score: edge.heuristic_score,
                })
                .collect();
            (*arena.get_mut(root_edge.child.as_mut().unwrap())).children =
                arena.add_slice(&mut filtered_edges).unwrap();
        }

        MonteCarloTree {
            edge: root_edge,
            position,
            settings,
            temp_vectors,
            arena,
        }
    }

    pub fn get_child(&self) -> &Tree {
        self.arena.get(self.edge.child.as_ref().unwrap())
    }

    pub fn get_child_mut(&mut self) -> &mut Tree {
        self.arena.get_mut(self.edge.child.as_mut().unwrap())
    }

    pub fn search_for_time<F>(&mut self, max_time: time::Duration, callback: F)
    where
        F: Fn(&Self),
    {
        let start_time = time::Instant::now();

        for i in 0.. {
            let nodes = (50.0 * 2.0_f32.powf(0.125).powi(i)) as u64;
            for _ in 0..nodes {
                if self.select().is_none() {
                    eprintln!("Warning: Search stopped early due to OOM");
                    callback(self);
                    return;
                };
            }

            // Always return when we have less than 10ms left
            if max_time < (time::Duration::from_millis(10))
                || start_time.elapsed() > max_time - (time::Duration::from_millis(10))
                || self.children().len() == 1
            {
                callback(self);
                return;
            }

            let child = self.get_child();
            let mut child_refs: Vec<&TreeEdge> =
                self.arena.get_slice(&child.children).iter().collect();

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
                .unwrap()
                .shallow_clone();

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
    #[must_use]
    pub fn select(&mut self) -> Option<f32> {
        self.edge.select::<S>(
            &mut self.position.clone(),
            &self.settings,
            &mut self.temp_vectors,
            &self.arena,
        )
    }

    /// Returns the best move, and its score (as winning probability) from the perspective of the side to move
    /// Panics if no search iterations have been run
    pub fn best_move(&self) -> (Move, f32) {
        self.arena
            .get_slice(&self.get_child().children)
            .iter()
            .max_by_key(|edge| edge.visits)
            .map(|edge| (edge.mv.clone(), 1.0 - edge.mean_action_value))
            .unwrap_or_else(|| panic!("Couldn't find best move"))
    }

    pub fn node_edge_sizes(&self, arena: &Arena) -> (usize, usize) {
        pub fn edge_sizes(edge: &TreeEdge, arena: &Arena) -> (usize, usize) {
            if let Some(child_index) = &edge.child {
                let (child_nodes, child_edges) = node_sizes(&*arena.get(child_index), arena);
                (child_nodes, child_edges + 1)
            } else {
                (0, 1)
            }
        }
        pub fn node_sizes(node: &Tree, arena: &Arena) -> (usize, usize) {
            arena
                .get_slice(&node.children)
                .iter()
                .map(|edge| edge_sizes(edge, arena))
                .fold(
                    (1, 0),
                    |(acc_nodes, acc_edges), (child_nodes, child_edges)| {
                        (acc_nodes + child_nodes, acc_edges + child_edges)
                    },
                )
        }
        edge_sizes(&self.edge, arena)
    }

    fn children(&self) -> Vec<TreeEdge> {
        self.arena
            .get_slice(&self.get_child().children)
            .iter()
            .map(|edge| edge.shallow_clone())
            .collect()
    }

    pub fn pv(&self) -> impl Iterator<Item = Move> + '_ {
        Pv::new(&self.edge, &self.arena)
    }

    /// Print human-readable information of the search's progress.
    pub fn print_info(&self) {
        let child = self.get_child();
        let mut best_children: Vec<&TreeEdge> =
            self.arena.get_slice(&child.children).iter().collect();

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
                Pv::new(edge, &self.arena).map(|mv| mv.to_string::<S>() + " ").collect::<String>()
            )
        });
    }

    pub fn visits(&self) -> u64 {
        self.edge.visits
    }

    pub fn mem_usage(&self) -> usize {
        self.arena.slots_used() as usize * 24
    }

    pub fn mean_action_value(&self) -> Score {
        self.edge.mean_action_value
    }
}

/// The simplest way to use the mcts module. Run Monte Carlo Tree Search for `nodes` nodes, returning the best move, and its estimated winning probability for the side to move.
pub fn mcts<const S: usize>(position: Position<S>, nodes: u64) -> (Move, Score) {
    let settings = MctsSetting::default().arena_size_for_nodes(nodes as u32);
    let mut tree = MonteCarloTree::with_settings(position, settings);

    for _ in 0..nodes.max(2) {
        tree.select().unwrap();
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
                if tree.select().is_none() {
                    eprintln!("Warning: Search stopped early due to OOM");
                    break;
                };
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

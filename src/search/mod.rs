//! A strong Tak AI, based on Monte Carlo Tree Search.
//!
//! This implementation does not use full Monte Carlo rollouts, relying on a heuristic evaluation when expanding new nodes instead.

use arrayvec::ArrayVec;
use board_game_traits::Position as _;
use half::f16;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::collections::TryReserveError;
use std::f32::consts::PI;
use std::fmt::Display;
use std::{mem, time};

use self::mcts_core::Pv;
use crate::position::Move;
use crate::position::Position;
pub use crate::search::mcts_core::best_move;
use crate::search::mcts_core::{SmallBridge, TempVectors, Tree, TreeBridge, TreeChild, TreeEdge};

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
    value_params: Option<&'static [f32]>,
    policy_params: Option<&'static [f32]>,
    search_params: Box<[f32]>,
    dirichlet: Option<f32>,
    excluded_moves: Vec<Move<S>>,
    static_eval_variance: Option<f32>,
    rollout_depth: u16,
    rollout_temperature: Option<f64>,
}

impl<const S: usize> Default for MctsSetting<S> {
    fn default() -> Self {
        MctsSetting {
            value_params: None,
            policy_params: None,
            search_params: vec![1.50, 2200.0, 0.61].into_boxed_slice(),
            dirichlet: None,
            excluded_moves: vec![],
            static_eval_variance: None,
            rollout_depth: 0,
            rollout_temperature: None,
        }
    }
}

impl<const S: usize> MctsSetting<S> {
    pub fn add_value_params(mut self, value_params: &'static [f32]) -> Self {
        self.value_params = Some(value_params);
        self
    }

    pub fn add_policy_params(mut self, policy_params: &'static [f32]) -> Self {
        self.policy_params = Some(policy_params);
        self
    }

    pub fn add_search_params(mut self, search_params: Box<[f32]>) -> Self {
        self.search_params = search_params;
        self
    }

    pub fn add_dirichlet(mut self, alpha: f32) -> Self {
        self.dirichlet = Some(alpha);
        self
    }

    pub fn exclude_moves(mut self, excluded_moves: Vec<Move<S>>) -> Self {
        self.excluded_moves = excluded_moves;
        self
    }

    pub fn add_static_eval_variance(mut self, static_eval_variance: f32) -> Self {
        self.static_eval_variance = Some(static_eval_variance);
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
        self.rollout_temperature = Some(temperature);
        self
    }

    pub fn c_puct_init(&self) -> f32 {
        self.search_params[0]
    }

    pub fn c_puct_base(&self) -> f32 {
        self.search_params[1]
    }

    pub fn initial_mean_action_value(&self) -> f32 {
        self.search_params[2]
    }
}

#[derive(Debug)]
pub enum Error {
    OOM,
    MaxVisits,
}

impl<T> From<trybox::ErrorWith<T>> for Error {
    fn from(_: trybox::ErrorWith<T>) -> Self {
        Error::OOM
    }
}

impl From<TryReserveError> for Error {
    fn from(_: TryReserveError) -> Self {
        Error::OOM
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::OOM => {
                write!(f, "Search stopped early due to OOM")
            }
            Error::MaxVisits => {
                write!(f, "Reached {} max visit count", u32::MAX)
            }
        }
    }
}

impl std::error::Error for Error {}

pub struct MonteCarloTree<const S: usize> {
    tree: TreeEdge<S>, // Fake edge to the root node
    visits: u32,
    position: Position<S>,
    temp_position: Position<S>,
    pub settings: MctsSetting<S>,
    temp_vectors: TempVectors<S>,
}

impl<const S: usize> MonteCarloTree<S> {
    pub fn reroot(self, moves: &[Move<S>]) -> Option<Self> {
        let mut new_edge = self.tree;
        let mut new_visits = self.visits;
        let mut position = self.position;
        for mv in moves {
            let child = new_edge.child?.children?;
            let TreeChild::Large(mut bridge) = *child else {
                return None;
            };

            let index = bridge.moves.iter().position(|m| *m == Some(*mv))?;

            new_edge = TreeEdge {
                child: bridge.children[index].child.take(),
            };
            new_visits = bridge.visitss[index];
            position.do_move(*mv);
        }

        let mut new_temp_position = self.temp_position;
        new_temp_position.clone_from(&position);

        Some(Self {
            tree: new_edge,
            visits: new_visits,
            position,
            temp_position: new_temp_position,
            settings: self.settings,
            temp_vectors: self.temp_vectors,
        })
    }

    /// Resets the tree to a new position and settings,
    /// re-using allocations from the previous tree
    pub fn reset_tree(&mut self, position: &Position<S>, settings: MctsSetting<S>) {
        self.tree.child = None;
        self.visits = 0;
        self.settings = settings;
        self.position.clone_from(position);
        self.temp_position.clone_from(position);
        self.temp_vectors.clear();
        self.initialize_tree();
    }

    /// Applies dirichlet noise and excludes moves
    /// which can only be done once two iterations of mcts have been done
    fn initialize_tree(&mut self) {
        self.select().unwrap();
        self.select().unwrap();

        if let Some(alpha) = self.settings.dirichlet {
            self.tree
                .child
                .as_mut()
                .unwrap()
                .children
                .as_mut()
                .unwrap()
                .apply_dirichlet(0.25, alpha);
        }

        if !self.settings.excluded_moves.is_empty() {
            let bridge = self.tree.child.as_mut().unwrap().children.as_mut().unwrap();
            for excluded_move in self.settings.excluded_moves.iter() {
                let TreeChild::Small(ref mut small_bridge) = **bridge else {
                    panic!()
                };
                let index = small_bridge
                    .moves
                    .iter()
                    .enumerate()
                    .find(|(_, mv)| **mv == Some(*excluded_move))
                    .unwrap()
                    .0;
                let moves = &mut small_bridge.moves;
                let heuristic_scores = &mut small_bridge.heuristic_scores;

                small_bridge
                    .children
                    .retain(|(_, mv, _)| *mv != *excluded_move);

                moves[index] = None;
                heuristic_scores[index] = f16::NEG_INFINITY; // TODO: Also set infinite visitss?
            }
        }
    }

    pub fn new(position: Position<S>, settings: MctsSetting<S>) -> MonteCarloTree<S> {
        let mut tree = MonteCarloTree {
            tree: TreeEdge { child: None },
            visits: 0,
            position: position.clone(),
            temp_position: position,
            settings,
            temp_vectors: TempVectors::default(),
        };
        tree.initialize_tree();
        tree
    }

    pub fn position(&self) -> &Position<S> {
        &self.position
    }

    pub fn search_for_time<F>(&mut self, max_time: time::Duration, callback: F)
    where
        F: Fn(&Self),
    {
        let start_time = time::Instant::now();

        for i in 0.. {
            let nodes = (50.0 * 2.0_f32.powf(0.125).powi(i)) as u64;
            for _ in 0..nodes {
                if let Err(err) = self.select() {
                    eprintln!("Warning: {err}");
                    callback(self);
                    return;
                };
            }

            let mut shallow_edges = self.shallow_edges().unwrap();

            // Always return when we have less than 10ms left
            if max_time < (time::Duration::from_millis(10))
                || start_time.elapsed() > max_time - (time::Duration::from_millis(10))
                || shallow_edges.len() == 1
            {
                callback(self);
                return;
            }

            shallow_edges.sort_by_key(|edge| edge.visits);
            shallow_edges.reverse();

            let node_ratio =
                (1 + shallow_edges[1].visits) as f32 / (1 + shallow_edges[0].visits) as f32;
            let time_ratio = start_time.elapsed().as_secs_f32() / max_time.as_secs_f32();

            let visits_sqrt = (self.visits() as f32).sqrt();
            let dynamic_cpuct = self.settings.c_puct_init()
                + f32::ln(
                    (1.0 + self.visits() as f32 + self.settings.c_puct_base())
                        / self.settings.c_puct_base(),
                );

            let best_edge = shallow_edges.iter().max_by_key(|edge| edge.visits).unwrap();

            let best_exploration_value = best_edge.exploration_value(visits_sqrt, dynamic_cpuct);

            if time_ratio.powf(2.0) > node_ratio / 2.0 {
                callback(self);
                // Do not stop if any other child nodes have better exploration value
                if shallow_edges.iter().any(|edge| {
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

    // TODO: Count up to u64 on root?
    pub fn visits(&self) -> u32 {
        self.visits
    }

    pub fn mean_action_value(&self) -> f32 {
        self.tree
            .child
            .as_ref()
            .map(|index| index.total_action_value as f32 / self.visits as f32)
            .unwrap_or(self.settings.initial_mean_action_value())
    }

    pub fn best_move(&self) -> Option<(Move<S>, f32)> {
        let root_child = self.tree.child.as_ref()?.children.as_ref()?;

        match **root_child {
            TreeChild::Small(ref small_bridge) => {
                let (grandchild, mv, visits) = small_bridge
                    .children
                    .iter()
                    .filter(|(_, _, visits)| *visits > 0)
                    .max_by_key(|(_, _, visits)| *visits)?;

                Some((
                    *mv,
                    1.0 - grandchild.total_action_value as f32 / *visits as f32,
                ))
            }
            TreeChild::Large(ref tree_bridge) => {
                let (best_index, _) = tree_bridge
                    .visitss
                    .iter()
                    .enumerate()
                    .filter(|(_, visits)| **visits > 0)
                    .max_by_key(|(_, visits)| *visits)?;

                Some((
                    tree_bridge.moves[best_index]?,
                    1.0 - tree_bridge.mean_action_values[best_index],
                ))
            }
        }
    }

    pub fn pv(&self) -> impl Iterator<Item = Move<S>> + '_ {
        Pv::new(self.tree.child.as_ref().unwrap().children.as_ref().unwrap())
            .map(|mv| mv.to_owned())
    }

    /// Print human-readable information of the search's progress.
    pub fn print_info(&self) {
        let mut best_children: Vec<ShallowEdge<S>> = self.shallow_edges().unwrap_or_default();

        best_children.sort_by_key(|edge| edge.visits);
        best_children.reverse();

        let dynamic_cpuct = self.settings.c_puct_init()
            + f32::ln(
                (1.0 + self.visits() as f32 + self.settings.c_puct_base())
                    / self.settings.c_puct_base(),
            );

        best_children.iter().take(8).for_each(|edge| {
            println!(
                "Move {}: {} visits, {:.2}% mean action value, {:.3}% static score, {:.3} exploration value, pv {}",
                edge.mv,
                edge.visits,
                edge.mean_action_value * 100.0,
                edge.policy.to_f32() * 100.0,
                1.0 + edge.exploration_value((self.visits() as f32).sqrt(), dynamic_cpuct), // The +1.0 doesn't matter, but positive numbers are easier to read
                if let Some(children) = edge.child.and_then(|c| c.children.as_ref()) {
                    Pv::new(children)
                    .map(|mv| mv.to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
                } else {
                    "".to_string()
                }
            )
        });
    }

    pub fn select(&mut self) -> Result<f32, Error> {
        if self.visits == u32::MAX {
            return Err(Error::MaxVisits);
        }
        self.temp_position.clone_from(&self.position);
        let result = self.tree.select(
            &mut self.temp_position,
            &self.settings,
            &mut self.temp_vectors,
            self.visits,
        )?;
        self.visits += 1;
        Ok(result)
    }

    pub fn shallow_edges(&self) -> Option<Vec<ShallowEdge<'_, S>>> {
        let child = self.tree.child.as_ref()?.children.as_ref()?;

        match **child {
            TreeChild::Small(ref bridge) => Some(
                bridge
                    .moves
                    .iter()
                    .zip(&bridge.heuristic_scores)
                    .filter_map(|(mv, policy)| {
                        let (initialized_child, _, visits) = bridge
                            .children
                            .iter()
                            .find(|(_, child_move, _)| Some(*child_move) == *mv)?;
                        let mean_action_value =
                            initialized_child.total_action_value as f32 / *visits as f32;
                        Some(ShallowEdge {
                            visits: *visits,
                            mv: (*mv)?,
                            mean_action_value,
                            child: Some(initialized_child),
                            policy: *policy,
                        })
                    })
                    .collect(),
            ),
            TreeChild::Large(ref bridge) => Some(
                bridge
                    .visitss
                    .iter()
                    .zip(
                        bridge.moves.iter().zip(
                            bridge
                                .mean_action_values
                                .iter()
                                .zip(bridge.children.iter().zip(&bridge.heuristic_scores)),
                        ),
                    )
                    .filter_map(|(visits, (mv, (score, (child, policy))))| {
                        Some(ShallowEdge {
                            visits: *visits,
                            mv: (*mv)?,
                            mean_action_value: *score,
                            child: child.child.as_ref(),
                            policy: *policy,
                        })
                    })
                    .collect(),
            ),
        }
    }

    // returns an iterator of the (up to 16) best moves (by visits)
    pub fn best_moves(&self) -> BestMoves<'_, S> {
        let Some(child) = self.tree.child.as_ref().and_then(|c| c.children.as_ref()) else {
            return BestMoves {
                node: None,
                best_indices: ArrayVec::new(),
                next: 0,
            };
        };

        match **child {
            TreeChild::Small(ref bridge) => {
                let mut best_indices: ArrayVec<_, _> = (0..bridge.children.len()).collect();
                best_indices.sort_by_key(|&index| bridge.children[index].2);
                best_indices.reverse();

                BestMoves {
                    node: Some(&**child),
                    best_indices,
                    next: 0,
                }
            }
            TreeChild::Large(ref bridge) => {
                let mut best_indices = ArrayVec::new();

                for index in 0..bridge.visitss.len() {
                    let visits = bridge.visitss[index];
                    let is_new = if best_indices.is_full() {
                        if bridge.visitss[*best_indices.last().unwrap()] < visits {
                            *best_indices.last_mut().unwrap() = index;
                            true
                        } else {
                            false
                        }
                    } else {
                        best_indices.push(index);
                        true
                    };

                    if is_new {
                        for i in (0..best_indices.len() - 1).rev() {
                            if bridge.visitss[best_indices[i]] < visits {
                                best_indices.swap(i, i + 1);
                            } else {
                                break;
                            }
                        }
                    }
                }

                BestMoves {
                    node: Some(&**child),
                    best_indices,
                    next: 0,
                }
            }
        }
    }
}
// More convenient edge representation, allowing them to be stored as array-of-structs rather than struct-of-arrays
pub struct ShallowEdge<'a, const S: usize> {
    pub visits: u32,
    pub mv: Move<S>,
    pub mean_action_value: f32,
    pub child: Option<&'a Tree<S>>,
    policy: f16,
}

impl<const S: usize> ShallowEdge<'_, S> {
    pub fn exploration_value(&self, parent_visits_sqrt: f32, dynamic_cpuct: f32) -> f32 {
        mcts_core::exploration_value(
            self.mean_action_value,
            self.policy.to_f32(),
            self.visits,
            parent_visits_sqrt,
            dynamic_cpuct,
        )
    }

    pub fn pv<'a>(&'a self) -> Option<Pv<'a, S>> {
        self.child
            .and_then(|child| child.children.as_ref())
            .map(|child| Pv::new(child))
    }
}

pub struct BestMoves<'a, const S: usize> {
    node: Option<&'a TreeChild<S>>,
    best_indices: ArrayVec<usize, 16>,
    next: usize,
}

impl<'a, const S: usize> Iterator for BestMoves<'a, S> {
    type Item = ShallowEdge<'a, S>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next < self.best_indices.len() {
            let index = self.best_indices[self.next];
            self.next += 1;
            self.node.and_then(|node| match *node {
                TreeChild::Small(ref bridge) => {
                    let (ref initialized_child, mv, visits) = bridge.children[index];
                    let mean_action_value =
                        initialized_child.total_action_value as f32 / visits as f32;
                    Some(ShallowEdge {
                        visits,
                        mv,
                        mean_action_value,
                        child: Some(initialized_child),
                        policy: f16::ZERO,
                    })
                }
                TreeChild::Large(ref bridge) => bridge.moves[index].map(|mv| ShallowEdge {
                    visits: bridge.visitss[index],
                    mv,
                    mean_action_value: bridge.mean_action_values[index],
                    child: bridge.children[index].child.as_ref(),
                    policy: f16::ZERO,
                }),
            })
        } else {
            None
        }
    }
}

/// The simplest way to use the mcts module. Run Monte Carlo Tree Search for `nodes` nodes, returning the best move, and its estimated winning probability for the side to move.
pub fn mcts<const S: usize>(position: Position<S>, nodes: u64) -> (Move<S>, f32) {
    let settings = MctsSetting::default();
    let mut tree = MonteCarloTree::new(position, settings);

    for _ in 0..nodes.max(2) {
        tree.select().unwrap();
    }
    let (mv, score) = tree.best_move().unwrap();
    (mv, score)
}

/// Play a move, calculating for a maximum duration.
/// It will usually spend much less time, especially if the move is obvious.
/// On average, it will spend around 20% of `max_time`, and rarely more than 50%.
pub fn play_move_time<const S: usize>(
    board: Position<S>,
    max_time: time::Duration,
    settings: MctsSetting<S>,
) -> (Move<S>, f32) {
    let mut tree = MonteCarloTree::new(board.clone(), settings);
    tree.search_for_time(max_time, |_| {});
    tree.best_move().unwrap()
}

/// Run mcts with specific static evaluation parameters, for optimization the parameter set.
/// Also applies Dirichlet noise to the root node
pub fn mcts_training<const S: usize>(
    position: Position<S>,
    time_control: &TimeControl,
    settings: MctsSetting<S>,
) -> Vec<(Move<S>, f16)> {
    let mut tree = MonteCarloTree::new(position, settings);

    match time_control {
        TimeControl::FixedNodes(nodes) => {
            for _ in 0..*nodes {
                if let Err(err) = tree.select() {
                    eprintln!("Warning: {err}");
                    break;
                }
            }
        }
        TimeControl::Time(time, increment) => {
            let max_time = *time / 5 + *increment / 2;
            tree.search_for_time(max_time, |_| {});
        }
    }
    let shallow_edges = tree.shallow_edges().unwrap();
    let child_visits: u32 = shallow_edges.iter().map(|edge| edge.visits).sum();
    shallow_edges
        .iter()
        .map(|edge| {
            (
                edge.mv,
                f16::from_f32(edge.visits as f32 / child_visits as f32),
            )
        })
        .collect()
}

/// Convert a static evaluation in centipawns to a winning probability between 0.0 and 1.0.
pub fn cp_to_win_percentage(cp: f32) -> f32 {
    0.5 + f32::atan(cp) / PI
}

// Utility for testing
pub fn edge_mem_usage<const S: usize>() -> usize {
    mem::size_of::<TreeEdge<S>>()
}

// Utility for testing
pub fn node_mem_usage<const S: usize>() -> usize {
    mem::size_of::<Tree<S>>()
}

// Utility for testing
pub fn tree_child_mem_usage<const S: usize>() -> usize {
    mem::size_of::<TreeChild<S>>()
}

// Utility for testing
pub fn large_bridge_mem_usage<const S: usize>() -> usize {
    mem::size_of::<TreeBridge<S>>()
}

// Utility for testing
pub fn small_bridge_mem_usage<const S: usize>() -> usize {
    mem::size_of::<SmallBridge<S>>()
}

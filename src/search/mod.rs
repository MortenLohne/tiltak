//! A strong Tak AI, based on Monte Carlo Tree Search.
//!
//! This implementation does not use full Monte Carlo rollouts, relying on a heuristic evaluation when expanding new nodes instead.

use half::f16;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;
use std::fmt::Display;
use std::{mem, time};
use std::{process, sync};

use crate::position::Move;
use crate::position::Position;
pub use crate::search::mcts_core::best_move;
use crate::search::mcts_core::{TempVectors, Tree, TreeEdge};

use self::arena::ArenaError;
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

#[derive(Clone, PartialEq, Debug)]
pub struct MctsSetting<const S: usize> {
    arena_size: u32,
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
            arena_size: 3 * 2_u32.pow(30), // Default to 48GB max
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
    /// Set a very liberal arena size, for searching a given amount of nodes
    pub fn arena_size_for_nodes(self, nodes: u32) -> Self {
        // For 6s, the toughest position I've found required 40 elements/node searched
        // This formula gives 108, which is hopefully plenty
        self.arena_size((S * S) as u32 * 3 * nodes)
    }

    // Useful on 32-bit platforms, where the arena's underlying memory allocation cannot be larger than isize::MAX
    pub fn max_arena_size(self) -> Self {
        self.mem_usage(isize::MAX as usize - 2 * ARENA_ELEMENT_SIZE)
    }

    pub fn mem_usage(self, mem_usage: usize) -> Self {
        assert!(
            mem_usage < u32::MAX as usize // Check for 32-bit platforms
            || mem_usage < ARENA_ELEMENT_SIZE * 2_usize.pow(32) - 2
        );
        self.arena_size((mem_usage / ARENA_ELEMENT_SIZE) as u32)
    }

    pub fn arena_size(mut self, arena_size: u32) -> Self {
        assert!(arena_size < u32::MAX - 1);
        self.arena_size = arena_size;
        self
    }

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

/// Type alias for winning probability, used for scoring positions.
pub const ARENA_ELEMENT_SIZE: usize = 16;

#[derive(Debug)]
pub enum Error {
    OOM,
    MaxVisits,
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
    settings: MctsSetting<S>,
    temp_vectors: TempVectors<S>,
    arena: Arena,
}

impl<const S: usize> MonteCarloTree<S> {
    pub fn new(position: Position<S>, settings: MctsSetting<S>) -> MonteCarloTree<S> {
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
                let heuristic_scores = arena.get_slice_mut(&mut bridge.heuristic_scores);

                moves[index] = None;
                heuristic_scores[index] = f16::NEG_INFINITY; // TODO: Also set infinite visitss?
            }
        }

        MonteCarloTree {
            tree,
            visits: 0,
            position: position.clone(),
            temp_position: position,
            settings,
            temp_vectors,
            arena,
        }
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
        let best_edge = self
            .shallow_edges()?
            .into_iter()
            .max_by_key(|edge| edge.visits)?;
        Some((best_edge.mv, 1.0 - best_edge.mean_action_value))
    }

    pub fn pv(&self) -> impl Iterator<Item = Move<S>> + '_ {
        Pv::new(&self.tree, &self.arena)
    }

    /// Print human-readable information of the search's progress.
    pub fn print_info(&self) {
        let mut best_children: Vec<ShallowEdge<S>> = self.shallow_edges().unwrap_or_default();

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
                Pv::new(edge.child, &self.arena)
                    .map(|mv| mv.to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
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
            &self.arena,
            self.visits,
        )?;
        self.visits += 1;
        Ok(result)
    }

    pub fn shallow_edges(&self) -> Option<Vec<ShallowEdge<'_, S>>> {
        let child = self.arena.get(
            self.arena
                .get(self.tree.child.as_ref()?)
                .children
                .as_ref()?,
        );

        Some(
            self.arena
                .get_slice(&child.visitss)
                .iter()
                .zip(
                    self.arena.get_slice(&child.moves).iter().zip(
                        self.arena.get_slice(&child.mean_action_values).iter().zip(
                            self.arena
                                .get_slice(&child.children)
                                .iter()
                                .zip(self.arena.get_slice(&child.heuristic_scores)),
                        ),
                    ),
                )
                .filter_map(|(visits, (mv, (score, (child, policy))))| {
                    Some(ShallowEdge {
                        visits: *visits,
                        mv: (*mv)?,
                        mean_action_value: *score,
                        child,
                        policy: *policy,
                    })
                })
                .collect(),
        )
    }
}
// More convenient edge representation, allowing them to be stored as array-of-structs rather than struct-of-arrays
pub struct ShallowEdge<'a, const S: usize> {
    visits: u32,
    mv: Move<S>,
    mean_action_value: f32,
    child: &'a TreeEdge<S>,
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
}

/// The simplest way to use the mcts module. Run Monte Carlo Tree Search for `nodes` nodes, returning the best move, and its estimated winning probability for the side to move.
pub fn mcts<const S: usize>(position: Position<S>, nodes: u64) -> (Move<S>, f32) {
    let settings = MctsSetting::default().arena_size_for_nodes(nodes as u32);
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

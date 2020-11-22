/// This module contains the core of the MCTS search algorithm
use crate::board::{Board, GroupData, Move, TunableBoard};
use crate::search::{cp_to_win_percentage, MctsSetting, Score};
use board_game_traits::board::{Board as BoardTrait, Color, GameResult};
use std::ops;

/// A Monte Carlo Search Tree, containing every node that has been seen in search.
#[derive(Clone, PartialEq, Debug)]
pub struct Tree {
    pub children: Vec<TreeEdge>,
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
    pub fn select(
        &mut self,
        board: &mut Board,
        settings: &MctsSetting,
        simple_moves: &mut Vec<Move>,
        moves: &mut Vec<(Move, Score)>,
    ) -> Score {
        if self.visits == 0 {
            self.expand(board, &settings.value_params)
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
                let group_data = board.group_data();
                node.init_children(
                    &board,
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
                "No legal moves on board\n{:?}",
                board
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

            board.do_move(child_edge.mv.clone());
            let result = 1.0 - child_edge.select(board, settings, simple_moves, moves);
            self.visits += 1;

            node.total_action_value += result as f64;

            self.mean_action_value = (node.total_action_value / self.visits as f64) as f32;
            result
        }
    }

    // Never inline, for profiling purposes
    #[inline(never)]
    fn expand(&mut self, board: &Board, params: &[f32]) -> Score {
        debug_assert!(self.child.is_none());
        self.child = Some(Box::new(Tree::new_node()));
        let child = self.child.as_mut().unwrap();

        let group_data = board.group_data();

        if let Some(game_result) = board.game_result_with_group_data(&group_data) {
            let game_result_for_us = match (game_result, board.side_to_move()) {
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

        let mut static_eval =
            cp_to_win_percentage(board.static_eval_with_params_and_data(&group_data, params));
        if board.side_to_move() == Color::Black {
            static_eval = 1.0 - static_eval;
        }
        self.visits = 1;
        child.total_action_value = static_eval as f64;
        self.mean_action_value = static_eval;
        static_eval
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
    fn init_children(
        &mut self,
        board: &Board,
        group_data: &GroupData,
        simple_moves: &mut Vec<Move>,
        policy_params: &[f32],
        moves: &mut Vec<(Move, Score)>,
    ) {
        board.generate_moves_with_params(policy_params, group_data, simple_moves, moves);
        self.children.reserve_exact(moves.len());
        for (mv, heuristic_score) in moves.drain(..) {
            self.children
                .push(TreeEdge::new(mv.clone(), heuristic_score));
        }
    }

    fn new_node() -> Self {
        Tree {
            children: vec![],
            total_action_value: 0.0,
            is_terminal: false,
        }
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

pub struct PV<'a> {
    tree: &'a Tree,
}

impl<'a> PV<'a> {
    pub fn new(tree: &'a Tree) -> PV<'a> {
        PV { tree }
    }
}

impl<'a> Iterator for PV<'a> {
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

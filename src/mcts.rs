use crate::board::{Board, Move};
use board_game_traits::board::{Board as BoardTrait, Color, EvalBoard};

const C_PUCT: f64 = 5.0;

pub struct Tree {
    children: Vec<(Tree, Move)>,
    pub visits: u64,
    pub total_action_value: f64,
    pub mean_action_value: f64,
    pub heuristic_score: f64,
}

// TODO: Winning percentage should be always be interpreted from the side to move's perspective

impl Tree {
    pub(crate) fn new_root() -> Self {
        Tree {
            children: vec![],
            visits: 0,
            total_action_value: 0.0,
            mean_action_value: 0.5,
            heuristic_score: 0.0,
        }
    }

    fn new_node(heuristic_score: f64) -> Self {
        Tree {
            children: vec![],
            visits: 0,
            total_action_value: 0.0,
            mean_action_value: 0.5,
            heuristic_score,
        }
    }

    pub fn select(&mut self, board: &mut Board) -> f64 {
        if self.children.is_empty() {
            self.expand(board)
        } else {
            let visits = self.visits;
            let (child, mv) = if board.side_to_move() == Color::White {
                self.children
                    .iter_mut()
                    .max_by(|(child1, _), (child2, _)| {
                        child1
                            .exploration_value(visits)
                            .partial_cmp(&child2.exploration_value(visits))
                            .unwrap()
                    })
                    .unwrap()
            } else {
                self.children
                    .iter_mut()
                    .max_by(|(child1, _), (child2, _)| {
                        child1
                            .exploration_value(visits)
                            .partial_cmp(&child2.exploration_value(visits))
                            .unwrap()
                    })
                    .unwrap()
            };
            board.do_move(mv.clone());
            let result = child.select(board);
            self.visits += 1;
            self.total_action_value += result;
            self.mean_action_value = self.total_action_value / self.visits as f64;
            result
        }
    }

    fn expand(&mut self, board: &mut Board) -> f64 {
        // TODO: Check for game termination
        debug_assert_eq!(self.visits, 0);
        debug_assert!(self.children.is_empty());

        let mut moves = vec![];
        board.generate_moves_with_probabilities(&mut moves);
        for (mv, heuristic_score) in moves {
            self.children
                .push((Tree::new_node(heuristic_score), mv.clone()));
        }
        let mut static_eval = cp_to_win_percentage(board.static_eval());
        if board.side_to_move() == Color::Black {
            static_eval = 1.0 - static_eval;
        }
        self.visits += 1;
        self.total_action_value = static_eval;
        self.mean_action_value = static_eval;
        static_eval
    }

    fn exploration_value(&self, parent_visits: u64) -> f64 {
        (1.0 - self.mean_action_value)
            + C_PUCT * self.heuristic_score * parent_visits as f64 / (1 + self.visits) as f64
    }
}

pub fn cp_to_win_percentage(cp: f32) -> f64 {
    1.0 / (1.0 + f64::powf(10.0, -cp as f64 / 4.0))
}

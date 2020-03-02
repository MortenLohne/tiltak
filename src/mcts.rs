use crate::board::{Board, Move};
use board_game_traits::board::{Board as BoardTrait, Color, EvalBoard, GameResult};

const C_PUCT: f64 = 2.0;

#[derive(Clone, PartialEq, Debug)]
pub struct Tree {
    pub children: Vec<(Tree, Move)>,
    pub visits: u64,
    pub total_action_value: f64,
    pub mean_action_value: f64,
    pub heuristic_score: f64,
    pub is_terminal: bool,
}

// TODO: Winning percentage should be always be interpreted from the side to move's perspective

pub(crate) fn mcts(board: Board, nodes: u64) -> (Move, f64) {
    let mut tree = Tree::new_root();
    let mut moves = vec![];
    let mut simple_moves = vec![];
    for _ in 0..nodes {
        tree.select(&mut board.clone(), &mut simple_moves, &mut moves);
    }
    tree.best_move()
}

impl Tree {
    pub(crate) fn new_root() -> Self {
        Tree {
            children: vec![],
            visits: 0,
            total_action_value: 0.0,
            mean_action_value: 0.5,
            heuristic_score: 0.0,
            is_terminal: false,
        }
    }

    /// Clones this node, but does not clone its children
    pub fn shallow_clone(&self) -> Self {
        Tree {
            children: vec![],
            visits: self.visits,
            total_action_value: self.total_action_value,
            mean_action_value: self.mean_action_value,
            heuristic_score: self.heuristic_score,
            is_terminal: self.is_terminal,
        }
    }

    pub fn print_info(&self) {
        let mut best_children: Vec<(Tree, Move)> = self
            .children
            .iter()
            .map(|(tree, mv)| (tree.shallow_clone(), mv.clone()))
            .collect();
        best_children.sort_by_key(|(child, _)| child.visits);
        best_children.reverse();
        let parent_visits = self.visits;

        best_children.iter().take(8).for_each(|(child, mv)| {
            println!(
                "Move {}: {} visits, {:.3} mean action value, {:.3} static score, {:.3} exploration value, best reply {:?}",
                mv, child.visits, child.mean_action_value, child.heuristic_score,
                child.exploration_value(parent_visits),
                if child.children.is_empty() { "".to_string() } else { format!("{:?}", child.best_move().0) }
            )
        });
    }

    pub fn best_move(&self) -> (Move, f64) {
        let (tree, mv) = self
            .children
            .iter()
            .max_by_key(|(child, _)| child.visits)
            .unwrap();
        (mv.clone(), tree.mean_action_value)
    }

    fn new_node(heuristic_score: f64) -> Self {
        Tree {
            children: vec![],
            visits: 0,
            total_action_value: 0.0,
            mean_action_value: 0.5,
            heuristic_score,
            is_terminal: false,
        }
    }

    pub fn select(
        &mut self,
        board: &mut Board,
        simple_moves: &mut Vec<Move>,
        moves: &mut Vec<(Move, f64)>,
    ) -> f64 {
        if self.is_terminal {
            self.visits += 1;
            self.total_action_value += self.mean_action_value;
            self.mean_action_value
        } else if self.visits == 0 {
            self.expand(board)
        } else {
            // Only generate child moves on the 2nd visit
            if self.visits == 1 {
                self.init_children(&board, simple_moves, moves);
            }
            let visits = self.visits;
            let (child, mv) = self
                .children
                .iter_mut()
                .max_by(|(child1, _), (child2, _)| {
                    child1
                        .exploration_value(visits)
                        .partial_cmp(&child2.exploration_value(visits))
                        .unwrap()
                })
                .unwrap();
            board.do_move(mv.clone());
            let result = 1.0 - child.select(board, simple_moves, moves);
            self.visits += 1;
            self.total_action_value += result;
            self.mean_action_value = self.total_action_value / self.visits as f64;
            result
        }
    }

    // Never inline, for profiling purposes
    #[inline(never)]
    fn expand(&mut self, board: &mut Board) -> f64 {
        debug_assert!(self.children.is_empty());

        if let Some(game_result) = board.game_result() {
            let result = match game_result {
                GameResult::Draw => 0.5,
                GameResult::WhiteWin => 0.0, // The side to move has lost
                GameResult::BlackWin => 0.0, // The side to move has lost
            };
            self.is_terminal = true;
            self.visits += 1;
            self.mean_action_value = result;
            self.total_action_value += result;
            return result;
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

    /// Do not initialize children in the expansion phase, for better fperformance
    /// Never inline, for profiling purposes
    #[inline(never)]
    fn init_children(
        &mut self,
        board: &Board,
        simple_moves: &mut Vec<Move>,
        moves: &mut Vec<(Move, f64)>,
    ) {
        board.generate_moves_with_probabilities(simple_moves, moves);
        self.children.reserve_exact(moves.len());
        for (mv, heuristic_score) in moves.drain(..) {
            self.children
                .push((Tree::new_node(heuristic_score), mv.clone()));
        }
    }

    fn exploration_value(&self, parent_visits: u64) -> f64 {
        (1.0 - self.mean_action_value)
            + C_PUCT * self.heuristic_score * (parent_visits as f64).sqrt()
                / (1 + self.visits) as f64
    }
}

pub fn cp_to_win_percentage(cp: f32) -> f64 {
    1.0 / (1.0 + f64::powf(10.0, -cp as f64 / 10.0))
}

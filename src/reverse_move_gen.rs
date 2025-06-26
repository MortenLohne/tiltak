use arrayvec::ArrayVec;
use board_game_traits::{Color, Position as _};
use pgn_traits::PgnPosition;

use crate::position::{
    squares_iterator, starting_capstones, starting_stones, AbstractBoard, Move, Movement, Piece,
    Position, ReverseMove, Role, Square, StackMovement,
};

#[derive(Debug)]
pub enum ReverseAnalysisResult<const S: usize> {
    Solution(Vec<Move<S>>),
    NoSolution(u16), // Depth of deepest possible line
    TimedOut,
}

impl<const S: usize> Position<S> {
    /// Perform a reverse analysis on the position,
    /// attempting to find a sequence of moves that start from the start position, and lead to the current position.
    pub fn reverse_analysis(&mut self) -> ReverseAnalysisResult<S> {
        let mut num_nodes = 0;
        self.recursive_analysis_rec(&mut vec![], &mut num_nodes, 0)
    }

    pub fn recursive_analysis_rec(
        &mut self,
        moves: &mut Vec<Move<S>>,
        num_nodes: &mut u64,
        num_ply_without_placements: u8,
    ) -> ReverseAnalysisResult<S> {
        *num_nodes += 1;
        if *num_nodes > 10_000 {
            return ReverseAnalysisResult::TimedOut;
        }

        if *self.stack_heights() == AbstractBoard::new_with_value(0) {
            let mut solution = moves.clone();
            solution.reverse();
            return ReverseAnalysisResult::Solution(solution);
        }

        let mut reverse_moves = Vec::with_capacity(S * S * S / 2);
        self.generate_partial_reverse_moves(&mut reverse_moves);

        reverse_moves.sort_by_cached_key(|reverse_move| match reverse_move {
            // Prefer placements, especially cap/wall placements
            ReverseMove::Place(sq) if self.top_stones()[*sq].unwrap().role() == Role::Flat => 1,
            ReverseMove::Place(_) => 0,
            ReverseMove::Move(from, _, _, _, _) => {
                let mut score = 10;
                // Prefer movements that left no pieces behind
                if self.stack_heights()[*from] == 0 {
                    score -= 2;
                }
                // Leaving one piece behind is also good
                if self.stack_heights()[*from] == 1 {
                    score -= 1;
                }
                score
            }
        });
        let mut highest_no_solution_depth = 0;
        for best_move in &reverse_moves {
            let original_move = match &best_move {
                ReverseMove::Place(square) => {
                    Move::placement(self.top_stones()[*square].unwrap().role(), *square)
                }
                ReverseMove::Move(square, direction, stack_movement, _, _) => {
                    Move::movement(*square, *direction, *stack_movement)
                }
            };

            if !original_move.is_placement() && num_ply_without_placements >= 8 {
                continue;
            }

            let old_position = self.clone();
            self.reverse_move(best_move.clone());

            assert!(
                self.move_is_legal(original_move),
                "Move {} not legal on {}",
                original_move,
                self.to_fen()
            );

            if self.game_result().is_some() {
                self.do_move(original_move);
                continue;
            }

            moves.push(original_move);

            let child_ply_without_placements = if original_move.is_placement() {
                0
            } else {
                num_ply_without_placements + 1
            };
            let result =
                self.recursive_analysis_rec(moves, num_nodes, child_ply_without_placements);
            match result {
                ReverseAnalysisResult::Solution(_) | ReverseAnalysisResult::TimedOut => {
                    return result;
                }
                ReverseAnalysisResult::NoSolution(depth) => {
                    highest_no_solution_depth = highest_no_solution_depth.max(depth + 1);
                    self.do_move(original_move);
                    assert_eq!(
                        *self, old_position,
                        "Got broken board after doing {}, reverse move was {}",
                        original_move, best_move
                    );
                    moves.pop();
                }
            }
        }
        ReverseAnalysisResult::NoSolution(highest_no_solution_depth)
    }

    /// Partially generate reverse moves for the current position
    /// WARNING: This does not generate all possible reverse moves, it will skip spreads more than 1 square long for example.
    pub fn generate_partial_reverse_moves(&self, moves: &mut Vec<ReverseMove<S>>) {
        for square in squares_iterator::<S>() {
            self.generate_reverse_moves_for_square(moves, square)
        }
    }
    fn generate_reverse_moves_for_square(
        &self,
        moves: &mut Vec<ReverseMove<S>>,
        square: Square<S>,
    ) {
        let last_side_to_move = !self.side_to_move();

        let Some(top_stone) = self.top_stones()[square] else {
            return;
        };

        let mut num_white_walls = 0;
        let mut num_black_walls = 0;
        for top_stone in squares_iterator().flat_map(|sq| self.top_stones()[sq]) {
            match top_stone {
                Piece::WhiteWall => num_white_walls += 1,
                Piece::BlackWall => num_black_walls += 1,
                _ => (),
            }
        }

        let white_flats_on_board =
            starting_stones(S) - self.white_reserves_left() - num_white_walls;

        let black_flats_on_board =
            starting_stones(S) - self.black_reserves_left() - num_black_walls;

        let has_one_flat_left = last_side_to_move == Color::White && white_flats_on_board == 1
            || last_side_to_move == Color::Black && black_flats_on_board == 1;

        let has_one_piece_left = last_side_to_move == Color::Black
            && self.black_reserves_left() == starting_stones(S) - 1
            && self.black_caps_left() == starting_capstones(S)
            || last_side_to_move == Color::White
                && self.white_reserves_left() == starting_stones(S) - 1
                && self.white_caps_left() == starting_capstones(S);

        // If we're undoing a move that could be the 2nd move of the game,
        // we could be undoing a placement of a white piece, or undoing a simple movement
        // However, other reverse moves are also possible
        if last_side_to_move == Color::Black && has_one_piece_left {
            // If both players have exactly one un-stacked piece on the board, we could be undoing a white placement
            if self.white_reserves_left() == starting_stones(S) - 1
                && self.white_caps_left() == starting_capstones(S)
                && top_stone.color() == Color::White
                && self.stack_heights()[square] == 1
            {
                moves.push(ReverseMove::Place(square));
                return;
            }
        }
        // If it's the first ply of the game, undo black's placement. No other moves are legal
        if last_side_to_move == Color::White
            && self.white_reserves_left() == starting_stones(S)
            && self.white_caps_left() == starting_capstones(S)
            && self.black_reserves_left() == starting_stones(S) - 1
            && self.black_caps_left() == starting_capstones(S)
        {
            if top_stone.color() == Color::Black {
                moves.push(ReverseMove::Place(square));
            }
            return;
        }

        if top_stone.color() != last_side_to_move {
            return;
        }

        if self.stack_heights()[square] == 1
            && (!has_one_flat_left || top_stone.role() != Role::Flat)
        {
            moves.push(ReverseMove::Place(square));
        }
        // Only generate movements where we move a single square, for now
        for (direction, neighbor) in square.direction_neighbors() {
            if self.top_stones()[neighbor].is_none_or(|piece| piece.role() == Role::Flat) {
                // for pieces_to_take in 1..=1 {
                for pieces_to_take in 1..=(S as u8).min(self.stack_heights()[square]) {
                    let mut stack_movement = StackMovement::new();
                    stack_movement.push(Movement { pieces_to_take }, S as u8);
                    stack_movement.push(Movement { pieces_to_take: 0 }, pieces_to_take);

                    let mut pieces_left_behind = ArrayVec::new();
                    pieces_left_behind.push(pieces_to_take);

                    moves.push(ReverseMove::Move(
                        neighbor,
                        direction.reverse(),
                        stack_movement,
                        pieces_left_behind.clone(),
                        false,
                    ));
                    // If we moved the cap, we may have flattened a wall
                    if pieces_to_take == 1
                        && top_stone.role() == Role::Cap
                        && self.stack_heights()[square] > 1
                        && !(last_side_to_move == Color::Black && white_flats_on_board == 1)
                        && !(last_side_to_move == Color::White && black_flats_on_board == 1)
                    {
                        moves.push(ReverseMove::Move(
                            neighbor,
                            direction.reverse(),
                            stack_movement,
                            pieces_left_behind,
                            true,
                        ));
                    }
                }
            }
        }
    }
}

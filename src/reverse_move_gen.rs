use arrayvec::ArrayVec;
use board_game_traits::{Color, Position as _};
use pgn_traits::PgnPosition;

use crate::position::{
    squares_iterator, starting_capstones, starting_stones, Move, Movement, Position, ReverseMove,
    Role, Square, StackMovement,
};

impl<const S: usize> Position<S> {
    pub fn reverse_analysis(&mut self) -> Option<Vec<Move<S>>> {
        let mut moves = vec![];
        'game_loop: for _ in 0..10000 {
            let mut reverse_moves = vec![];
            self.generate_reverse_moves(&mut reverse_moves);
            reverse_moves.sort_by_key(|reverse_move| match reverse_move {
                // Prefer placements, especially cap/wall placements
                ReverseMove::Place(sq) if self.top_stones()[*sq].unwrap().role() == Role::Flat => 1,
                ReverseMove::Place(_) => 0,
                ReverseMove::Move(from, direction, _, _, _) => {
                    let mut score = 10;
                    // Prefer movements that left no pieces behind
                    if self.stack_heights()[*from] == 0 {
                        score -= 3;
                    }
                    // Leaving one piece behind is also good
                    if self.stack_heights()[*from] == 1 {
                        score -= 1;
                    }
                    // Prefer spreads to squares that were empty
                    if self.stack_heights()[from.go_direction(*direction).unwrap()] == 1 {
                        score -= 1;
                    }
                    score
                }
            });

            for best_move in &reverse_moves {
                let original_move = match &best_move {
                    ReverseMove::Place(square) => {
                        Move::placement(self.top_stones()[*square].unwrap().role(), *square)
                    }
                    ReverseMove::Move(square, direction, stack_movement, _, _) => {
                        Move::movement(*square, *direction, *stack_movement)
                    }
                };
                let old_position = self.clone();
                self.reverse_move(best_move.clone());

                let mut original_moves = vec![];
                self.generate_moves(&mut original_moves);
                assert!(
                    original_moves.contains(&original_move),
                    "Move {} not legal on {}",
                    original_move,
                    self.to_fen()
                );

                // If we reverse into a decided game, try the next move
                if self.game_result().is_some() {
                    // println!("Reversing a reverse move due to reversing into a win");
                    self.do_move(original_move);
                    assert_eq!(
                        *self, old_position,
                        "Got broken board after doing {}, reverse move was {}",
                        original_move, best_move
                    );
                    continue;
                }

                if self.group_data().all_pieces().is_empty() {
                    moves.push(original_move);
                    moves.reverse();
                    // println!("Successfully reversed game!");
                    return Some(moves);
                }

                // If we reverse into a position where there are no further moves, undo and try the next move
                let mut next_reverse_moves = vec![];
                self.generate_reverse_moves(&mut next_reverse_moves);
                if next_reverse_moves.is_empty() {
                    // println!(
                    //     "Reversing move {} due to dead end on {}",
                    //     original_move,
                    //     self.to_fen()
                    // );
                    self.do_move(original_move);
                    assert_eq!(
                        *self, old_position,
                        "Got broken board after doing {}, reverse move was {}",
                        original_move, best_move
                    );
                    continue;
                }

                moves.push(original_move);
                continue 'game_loop;
            }
            println!(
                "Failed to find a move to reverse into! Current TPS: {}, current moves: {}, current reverse moves: {}",
                self.to_fen(),
                moves
                    .iter()
                    .rev()
                    .map(|mv| mv.to_string())
                    .collect::<Vec<_>>()
                    .join(" "),
                reverse_moves
                    .iter()
                    .map(|mv| mv.to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            return None;
        }
        println!(
            "Took too many moves to make progress! Current TPS: {}",
            self.to_fen(),
        );
        None
    }

    pub fn generate_reverse_moves(&self, moves: &mut Vec<ReverseMove<S>>) {
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

        let group_data = self.group_data();

        let white_flats_on_board =
            starting_stones(S) - self.white_reserves_left() - group_data.white_walls.count();

        let black_flats_on_board =
            starting_stones(S) - self.black_reserves_left() - group_data.black_walls.count();

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

use crate::position::color_trait::ColorTr;
use crate::position::mv::Move;
use crate::position::utils::Role::*;
use crate::position::utils::{Piece, Square};
use crate::position::{utils, Board, Direction, Movement, StackMovement};

impl<const S: usize> Board<S> {
    pub(crate) fn generate_moves_colortr<Us: ColorTr, Them: ColorTr>(
        &self,
        moves: &mut Vec<<Board<S> as board_game_traits::Position>::Move>,
    ) {
        for square in utils::squares_iterator::<S>() {
            match self[square].top_stone() {
                None => {
                    if Us::stones_left(&self) > 0 {
                        moves.push(Move::Place(Flat, square));
                        moves.push(Move::Place(Wall, square));
                    }
                    if Us::caps_left(&self) > 0 {
                        moves.push(Move::Place(Cap, square));
                    }
                }
                Some(piece) if Us::piece_is_ours(piece) => {
                    for direction in square.directions::<S>() {
                        let mut movements = vec![];
                        if piece == Us::cap_piece() {
                            self.generate_moving_moves_cap::<Us>(
                                direction,
                                square,
                                square,
                                self[square].len() as u8,
                                StackMovement::new(),
                                &mut movements,
                            );
                        } else if Us::piece_is_ours(piece) {
                            self.generate_moving_moves_non_cap::<Us>(
                                direction,
                                square,
                                square,
                                self[square].len() as u8,
                                StackMovement::new(),
                                &mut movements,
                            );
                        }
                        for movements in movements.into_iter().filter(|mv| !mv.is_empty()) {
                            let mv = Move::Move(square, direction, movements);
                            moves.push(mv);
                        }
                    }
                }
                Some(_) => (),
            }
        }
    }

    fn generate_moving_moves_cap<Colorr: ColorTr>(
        &self,
        direction: Direction,
        origin_square: Square,
        square: Square,
        pieces_carried: u8,
        partial_movement: StackMovement,
        movements: &mut Vec<StackMovement>,
    ) {
        if let Some(neighbour) = square.go_direction::<S>(direction) {
            let max_pieces_to_take = if square == origin_square {
                pieces_carried.min(S as u8)
            } else {
                (pieces_carried - 1).min(S as u8)
            };
            let neighbour_piece = self[neighbour].top_stone();
            if neighbour_piece.map(Piece::role) == Some(Cap) {
                return;
            }
            if neighbour_piece.map(Piece::role) == Some(Wall) && max_pieces_to_take > 0 {
                let mut new_movement = partial_movement;
                new_movement.push(Movement { pieces_to_take: 1 });
                movements.push(new_movement);
            } else {
                for pieces_to_take in 1..=max_pieces_to_take {
                    let mut new_movement = partial_movement;
                    new_movement.push(Movement { pieces_to_take });

                    self.generate_moving_moves_cap::<Colorr>(
                        direction,
                        origin_square,
                        neighbour,
                        pieces_to_take,
                        new_movement,
                        movements,
                    );
                    movements.push(new_movement);
                }
            }
        }
    }

    fn generate_moving_moves_non_cap<Colorr: ColorTr>(
        &self,
        direction: Direction,
        origin_square: Square,
        square: Square,
        pieces_carried: u8,
        partial_movement: StackMovement,
        movements: &mut Vec<StackMovement>,
    ) {
        if let Some(neighbour) = square.go_direction::<S>(direction) {
            let neighbour_piece = self[neighbour].top_stone();
            if neighbour_piece.is_some() && neighbour_piece.unwrap().role() != Flat {
                return;
            }

            let neighbour = square.go_direction::<S>(direction).unwrap();
            let max_pieces_to_take = if square == origin_square {
                pieces_carried.min(S as u8)
            } else {
                (pieces_carried - 1).min(S as u8)
            };
            for pieces_to_take in 1..=max_pieces_to_take {
                let mut new_movement = partial_movement;
                new_movement.push(Movement { pieces_to_take });

                self.generate_moving_moves_non_cap::<Colorr>(
                    direction,
                    origin_square,
                    neighbour,
                    pieces_to_take,
                    new_movement,
                    movements,
                );
                movements.push(new_movement);
            }
        }
    }
}

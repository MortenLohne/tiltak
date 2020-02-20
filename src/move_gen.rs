use crate::board::Role::*;
use crate::board::{board_iterator, Board, ColorTr, Direction, Move, Movement, Piece, Square, BOARD_SIZE};
use board_game_traits::board::Board as BoardTrait;
use board_game_traits::board::{Color, GameResult};
use smallvec::SmallVec;

impl Board {
    pub fn generate_moves_colortr<Colorr: ColorTr>(
        &self,
        moves: &mut Vec<<Board as board_game_traits::board::Board>::Move>,
    ) {
        for square in board_iterator() {
            match self[square].last() {
                None => {
                    if Colorr::stones_left(&self) > 0 {
                        moves.push(Move::Place(Colorr::flat_piece(), square));
                        moves.push(Move::Place(Colorr::standing_piece(), square));
                    }
                    if Colorr::capstones_left(&self) > 0 {
                        moves.push(Move::Place(Colorr::cap_piece(), square));
                    }
                }
                Some(&piece) if Colorr::piece_is_ours(piece) => {
                    for direction in square.directions() {
                        let mut movements = vec![];
                        if piece == Colorr::cap_piece() {
                            self.generate_moving_moves_cap::<Colorr>(
                                direction,
                                square,
                                square,
                                self[square].len() as u8,
                                &smallvec![],
                                &mut movements,
                            );
                        } else if Colorr::piece_is_ours(piece) {
                            self.generate_moving_moves_non_cap::<Colorr>(
                                direction,
                                square,
                                square,
                                self[square].len() as u8,
                                &smallvec![],
                                &mut movements,
                            );
                        }
                        for movement in movements.into_iter().filter(|mv| !mv.is_empty()) {
                            let mv = Move::Move(square, direction, movement);
                            // Check that moves are not suicide moves
                            if self[square]
                                .iter()
                                .any(|piece: &Piece| !Colorr::piece_is_ours(*piece))
                            {
                                let mut new_board = self.clone();
                                new_board.do_move(mv.clone());
                                match new_board.game_result() {
                                    Some(GameResult::WhiteWin) => {
                                        if Colorr::color() == Color::White {
                                            moves.push(mv);
                                        }
                                    }
                                    Some(GameResult::BlackWin) => {
                                        if Colorr::color() == Color::Black {
                                            moves.push(mv);
                                        }
                                    }
                                    Some(GameResult::Draw) => moves.push(mv),
                                    None => moves.push(mv),
                                };
                            } else {
                                moves.push(mv);
                            }
                        }
                    }
                }
                Some(_) => (),
            }
        }
    }

    pub fn generate_moving_moves_cap<Colorr: ColorTr>(
        &self,
        direction: Direction,
        origin_square: Square,
        square: Square,
        pieces_carried: u8,
        partial_movement: &SmallVec<[Movement; 5]>,
        movements: &mut Vec<SmallVec<[Movement; 5]>>,
    ) {
        if let Some(neighbour) = square.go_direction(direction) {
            let max_pieces_to_take = if square == origin_square {
                pieces_carried.min(BOARD_SIZE as u8)
            } else {
                (pieces_carried - 1).min(BOARD_SIZE as u8)
            };
            let neighbour_piece = self[neighbour].last().cloned();
            if neighbour_piece.map(Piece::role) == Some(Cap) {
                return;
            }
            if neighbour_piece.map(Piece::role) == Some(Standing) && max_pieces_to_take > 0 {
                let mut new_movement = partial_movement.clone();
                new_movement.push(Movement { pieces_to_take: 1 });
                movements.push(new_movement);
            } else {
                for pieces_to_take in 1..=max_pieces_to_take {
                    let mut new_movement = partial_movement.clone();
                    new_movement.push(Movement { pieces_to_take });

                    self.generate_moving_moves_cap::<Colorr>(
                        direction,
                        origin_square,
                        neighbour,
                        pieces_to_take,
                        &new_movement,
                        movements,
                    );
                    movements.push(new_movement);
                }
            }
        }
    }

    pub fn generate_moving_moves_non_cap<Colorr: ColorTr>(
        &self,
        direction: Direction,
        origin_square: Square,
        square: Square,
        pieces_carried: u8,
        partial_movement: &SmallVec<[Movement; 5]>,
        movements: &mut Vec<SmallVec<[Movement; 5]>>,
    ) {
        if let Some(neighbour) = square.go_direction(direction) {
            let neighbour_piece = self[neighbour].last().cloned();
            if neighbour_piece.is_some() && neighbour_piece.unwrap().role() != Flat {
                return;
            }

            let neighbour = square.go_direction(direction).unwrap();
            let max_pieces_to_take = if square == origin_square {
                pieces_carried.min(BOARD_SIZE as u8)
            } else {
                (pieces_carried - 1).min(BOARD_SIZE as u8)
            };
            for pieces_to_take in 1..=max_pieces_to_take {
                let mut new_movement = partial_movement.clone();
                new_movement.push(Movement { pieces_to_take });

                self.generate_moving_moves_non_cap::<Colorr>(
                    direction,
                    origin_square,
                    neighbour,
                    pieces_to_take,
                    &new_movement,
                    movements,
                );
                movements.push(new_movement);
            }
        }
    }

    pub fn count_all_stones(&self) -> u8 {
        self.cells.iter().flatten().flatten().count() as u8
    }

    pub fn all_top_stones(&self) -> impl Iterator<Item = &Piece> {
        self.cells.iter().flatten().filter_map(|cell| cell.last())
    }
}

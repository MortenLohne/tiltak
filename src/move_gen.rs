use crate::position::color_trait::ColorTr;
use crate::position::{
    squares_iterator, Direction, Movement, Position, Role::*, Square, StackMovement,
};
use crate::position::{Move, Piece};
use arrayvec::ArrayVec;
use board_game_traits::Position as PositionTrait;
use std::iter;

impl<const S: usize> Position<S> {
    pub(crate) fn generate_moves_colortr<
        E: Extend<<Self as PositionTrait>::Move>,
        Us: ColorTr,
        Them: ColorTr,
    >(
        &self,
        moves: &mut E,
    ) {
        for square in squares_iterator::<S>() {
            self.generate_moves_for_square_colortr::<E, Us, Them>(moves, square)
        }
    }

    pub(crate) fn generate_moves_for_square_colortr<
        E: Extend<<Self as PositionTrait>::Move>,
        Us: ColorTr,
        Them: ColorTr,
    >(
        &self,
        moves: &mut E,
        square: Square<S>,
    ) {
        match self.top_stones()[square] {
            None => {
                if Us::stones_left(self) > 0 {
                    moves.extend(iter::once(Move::placement(Flat, square)));
                    moves.extend(iter::once(Move::placement(Wall, square)));
                }
                if Us::caps_left(self) > 0 {
                    moves.extend(iter::once(Move::placement(Cap, square)));
                }
            }
            Some(piece) if Us::piece_is_ours(piece) => {
                for direction in square.directions() {
                    let mut movements = ArrayVec::new();
                    if piece == Us::cap_piece() {
                        self.generate_moving_moves_cap::<Us>(
                            direction,
                            square,
                            square,
                            self.stack_heights()[square],
                            StackMovement::new(),
                            &mut movements,
                        );
                    } else if Us::piece_is_ours(piece) {
                        self.generate_moving_moves_non_cap::<Us>(
                            direction,
                            square,
                            square,
                            self.stack_heights()[square],
                            StackMovement::new(),
                            &mut movements,
                        );
                    }
                    for movements in movements.into_iter().filter(|mv| !mv.is_empty()) {
                        let mv = Move::movement(square, direction, movements);
                        moves.extend(iter::once(mv));
                    }
                }
            }
            Some(_) => (),
        }
    }

    fn generate_moving_moves_cap<Colorr: ColorTr>(
        &self,
        direction: Direction,
        origin_square: Square<S>,
        square: Square<S>,
        pieces_carried: u8,
        partial_movement: StackMovement<S>,
        movements: &mut ArrayVec<StackMovement<S>, 255>, // 2^max_size - 1
    ) {
        if let Some(neighbour) = square.go_direction(direction) {
            let pieces_held = if square == origin_square {
                S as u8
            } else {
                pieces_carried
            };
            let max_pieces_to_take = if square == origin_square {
                pieces_carried.min(S as u8)
            } else {
                (pieces_carried - 1).min(S as u8)
            };
            let neighbour_piece = self.top_stones()[neighbour];
            if neighbour_piece.map(Piece::role) == Some(Cap) {
                return;
            }
            if neighbour_piece.map(Piece::role) == Some(Wall) && max_pieces_to_take > 0 {
                let mut new_movement = partial_movement;
                new_movement.push(Movement { pieces_to_take: 1 }, pieces_held);
                movements.push(new_movement);
            } else {
                for pieces_to_take in 1..=max_pieces_to_take {
                    let mut new_movement = partial_movement;
                    new_movement.push(Movement { pieces_to_take }, pieces_held);

                    self.generate_moving_moves_cap::<Colorr>(
                        direction,
                        origin_square,
                        neighbour,
                        pieces_to_take,
                        new_movement,
                        movements,
                    );

                    // Finish the movement
                    new_movement.push(Movement { pieces_to_take: 0 }, pieces_to_take);

                    movements.push(new_movement);
                }
            }
        }
    }

    fn generate_moving_moves_non_cap<Colorr: ColorTr>(
        &self,
        direction: Direction,
        origin_square: Square<S>,
        square: Square<S>,
        pieces_carried: u8,
        partial_movement: StackMovement<S>,
        movements: &mut ArrayVec<StackMovement<S>, 255>, // 2^max_size - 1
    ) {
        if let Some(neighbour) = square.go_direction(direction) {
            let neighbour_piece = self.top_stones()[neighbour];
            if neighbour_piece.is_some_and(|piece| piece.role() != Flat) {
                return;
            }

            let pieces_held = if square == origin_square {
                S as u8
            } else {
                pieces_carried
            };
            let max_pieces_to_take = if square == origin_square {
                pieces_carried.min(S as u8)
            } else {
                (pieces_carried - 1).min(S as u8)
            };
            for pieces_to_take in 1..=max_pieces_to_take {
                let mut new_movement = partial_movement;
                new_movement.push(Movement { pieces_to_take }, pieces_held);

                self.generate_moving_moves_non_cap::<Colorr>(
                    direction,
                    origin_square,
                    neighbour,
                    pieces_to_take,
                    new_movement,
                    movements,
                );

                // Finish the movement
                new_movement.push(Movement { pieces_to_take: 0 }, pieces_to_take);

                movements.push(new_movement);
            }
        }
    }
}

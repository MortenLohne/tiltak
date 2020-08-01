use crate::board::Role::*;
use crate::board::{
    Board, ColorTr, Direction, Move, Movement, Piece, Square, StackMovement, TunableBoard,
    BOARD_SIZE,
};
use crate::{board, mcts};
use arrayvec::ArrayVec;

pub fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + f32::exp(-x))
}

pub fn inverse_sigmoid(x: f32) -> f32 {
    assert!(x > 0.0 && x < 1.0, "Tried to inverse sigmoid {}", x);
    f32::ln(x / (1.0 - x))
}

impl Board {
    pub(crate) fn generate_moves_with_probabilities_colortr<Us: ColorTr, Them: ColorTr>(
        &self,
        params: &[f32],
        simple_moves: &mut Vec<Move>,
        moves: &mut Vec<(Move, mcts::Score)>,
    ) {
        let num_moves = simple_moves.len();
        moves.extend(simple_moves.drain(..).map(|mv| {
            (
                mv.clone(),
                self.probability_for_move_colortr::<Us, Them>(params, &mv, num_moves),
            )
        }));
    }

    fn probability_for_move_colortr<Us: ColorTr, Them: ColorTr>(
        &self,
        params: &[f32],
        mv: &Move,
        num_moves: usize,
    ) -> f32 {
        let mut coefficients = vec![0.0; Self::POLICY_PARAMS.len()];
        self.coefficients_for_move_colortr::<Us, Them>(&mut coefficients, mv, num_moves);
        let total_value: f32 = coefficients.iter().zip(params).map(|(c, p)| c * p).sum();

        sigmoid(total_value)
    }

    pub(crate) fn coefficients_for_move_colortr<Us: ColorTr, Them: ColorTr>(
        &self,
        coefficients: &mut [f32],
        mv: &Move,
        num_legal_moves: usize,
    ) {
        use crate::board::SQUARE_SYMMETRIES;

        const MOVE_COUNT: usize = 0;
        const FLAT_STONE_PSQT: usize = MOVE_COUNT + 1;
        const STANDING_STONE_PSQT: usize = FLAT_STONE_PSQT + 6;
        const CAPSTONE_PSQT: usize = STANDING_STONE_PSQT + 6;
        const ROAD_STONES_IN_RANK_FILE: usize = CAPSTONE_PSQT + 1;
        const EXTEND_GROUP: usize = ROAD_STONES_IN_RANK_FILE + 15;
        const MERGE_TWO_GROUPS: usize = EXTEND_GROUP + 1;
        const BLOCK_MERGER: usize = MERGE_TWO_GROUPS + 1;
        const NEXT_TO_OUR_LAST_STONE: usize = BLOCK_MERGER + 1;
        const NEXT_TO_THEIR_LAST_STONE: usize = NEXT_TO_OUR_LAST_STONE + 1;
        const DIAGONAL_TO_OUR_LAST_STONE: usize = NEXT_TO_THEIR_LAST_STONE + 1;
        const DIAGONAL_TO_THEIR_LAST_STONE: usize = DIAGONAL_TO_OUR_LAST_STONE + 1;
        const FLAT_PIECE_NEXT_TO_TWO_FLAT_PIECES: usize = DIAGONAL_TO_THEIR_LAST_STONE + 1;
        const ATTACK_FLATSTONE: usize = FLAT_PIECE_NEXT_TO_TWO_FLAT_PIECES + 1;
        const ATTACK_STRONG_FLATSTONE: usize = ATTACK_FLATSTONE + 3;

        const BLOCKING_STONE_NEXT_TO_TWO_OF_THEIR_FLATS: usize = ATTACK_STRONG_FLATSTONE + 1;
        const BLOCKING_STONE_BLOCKS_EXTENSIONS_OF_TWO_FLATS: usize =
            BLOCKING_STONE_NEXT_TO_TWO_OF_THEIR_FLATS + 1;

        const STACK_MOVEMENT_THAT_GIVES_US_TOP_PIECES: usize =
            BLOCKING_STONE_BLOCKS_EXTENSIONS_OF_TWO_FLATS + 1;
        const STACK_CAPTURED_BY_MOVEMENT: usize = STACK_MOVEMENT_THAT_GIVES_US_TOP_PIECES + 9;
        const _NEXT_CONST: usize = STACK_CAPTURED_BY_MOVEMENT + 1;

        assert_eq!(coefficients.len(), _NEXT_CONST);

        let initial_move_prob = 1.0 / num_legal_moves.max(2) as f32;

        coefficients[MOVE_COUNT] = inverse_sigmoid(initial_move_prob);

        // If it's the first move, give every move equal probability
        if self.half_moves_played() < 2 {
            return;
        }

        match mv {
            Move::Place(role, square) if *role == Flat => {
                // Apply PSQT
                coefficients[FLAT_STONE_PSQT + SQUARE_SYMMETRIES[square.0 as usize]] = 1.0;

                // Bonus for laying stones in files/ranks where we already have road stones
                // Implemented as a 2D table. Because of symmetries,
                // only 15 squares are needed, not all 25
                let road_stones_in_rank = Us::road_stones(&self).rank(square.rank()).count();
                let road_stones_in_file = Us::road_stones(&self).file(square.file()).count();

                let n_low = u8::min(road_stones_in_file, road_stones_in_rank);
                let n_high = u8::max(road_stones_in_file, road_stones_in_rank);
                let i = (11 * n_low - n_low * n_low) / 2 + n_high - n_low;
                debug_assert!(i < 15);
                coefficients[ROAD_STONES_IN_RANK_FILE + i as usize] += 1.0;

                // If square is next to a group
                let mut our_unique_neighbour_groups: ArrayVec<[(Square, u8); 4]> = ArrayVec::new();
                let mut their_unique_neighbour_groups: ArrayVec<[(Square, u8); 4]> =
                    ArrayVec::new();
                for neighbour in square.neighbours().filter(|sq| !self[*sq].is_empty()) {
                    let neighbour_group_id = self.groups()[neighbour];
                    if Us::piece_is_ours(self[neighbour].top_stone().unwrap()) {
                        if our_unique_neighbour_groups
                            .iter()
                            .all(|(_sq, id)| *id != neighbour_group_id)
                        {
                            our_unique_neighbour_groups.push((neighbour, neighbour_group_id));
                        }
                    } else if their_unique_neighbour_groups
                        .iter()
                        .all(|(_sq, id)| *id != neighbour_group_id)
                    {
                        their_unique_neighbour_groups.push((neighbour, neighbour_group_id));
                    }
                }
                if our_unique_neighbour_groups.len() > 1 {
                    coefficients[MERGE_TWO_GROUPS] += 1.0;
                }

                if their_unique_neighbour_groups.len() > 1 {
                    coefficients[BLOCK_MERGER] += 1.0;
                }

                for (_, group_id) in our_unique_neighbour_groups {
                    coefficients[EXTEND_GROUP] += self.amount_in_group()[group_id as usize] as f32;
                }

                // If square is next to a road stone laid on our last turn
                if let Some(Move::Place(last_role, last_square)) =
                    self.moves().get(self.moves().len() - 2)
                {
                    if *last_role == Flat || *last_role == Cap {
                        if square.neighbours().any(|neigh| neigh == *last_square) {
                            coefficients[NEXT_TO_OUR_LAST_STONE] = 1.0;
                        } else if (square.rank() as i8 - last_square.rank() as i8).abs() == 1
                            && (square.file() as i8 - last_square.file() as i8).abs() == 1
                        {
                            coefficients[DIAGONAL_TO_OUR_LAST_STONE] = 1.0;
                        }
                    }
                }

                // If square is next to a road stone laid on their last turn
                if let Some(Move::Place(last_role, last_square)) = self.moves().last() {
                    if *last_role == Flat {
                        if square.neighbours().any(|neigh| neigh == *last_square) {
                            coefficients[NEXT_TO_THEIR_LAST_STONE] = 1.0;
                        } else if (square.rank() as i8 - last_square.rank() as i8).abs() == 1
                            && (square.file() as i8 - last_square.file() as i8).abs() == 1
                        {
                            coefficients[DIAGONAL_TO_THEIR_LAST_STONE] = 1.0;
                        }
                    }
                }

                // If square has two or more of your own pieces around it
                if square
                    .neighbours()
                    .filter_map(|neighbour| self[neighbour].top_stone())
                    .filter(|neighbour_piece| Us::is_road_stone(*neighbour_piece))
                    .count()
                    >= 2
                {
                    coefficients[FLAT_PIECE_NEXT_TO_TWO_FLAT_PIECES] = 1.0;
                }

                // Bonus for "attacking" an enemy flatstone
                let enemy_flatstone_neighbours = square
                    .neighbours()
                    .filter(|sq| self[*sq].top_stone() == Some(Them::flat_piece()))
                    .count();

                if enemy_flatstone_neighbours > 0 {
                    coefficients[ATTACK_FLATSTONE + usize::max(enemy_flatstone_neighbours, 3)] =
                        1.0;
                }

                // Bonus for attacking a flatstone in a rank/file where we are strong
                for neighbour in square.neighbours() {
                    if self[neighbour].top_stone() == Some(Them::flat_piece()) {
                        let our_road_stones = Us::road_stones(self).rank(neighbour.rank()).count()
                            + Us::road_stones(self).file(neighbour.file()).count();
                        if our_road_stones >= 2 {
                            coefficients[ATTACK_STRONG_FLATSTONE] += (our_road_stones - 1) as f32;
                        }
                    }
                }
            }
            Move::Place(role, square) => {
                // Apply PSQT:
                if *role == Standing {
                    coefficients[STANDING_STONE_PSQT + SQUARE_SYMMETRIES[square.0 as usize]] = 1.0;
                } else if *role == Cap {
                    coefficients[CAPSTONE_PSQT] = 1.0;
                } else {
                    unreachable!(
                        "Tried to place {:?} with move {} on board\n{:?}",
                        role, mv, self
                    );
                };
                // If square has two or more opponent flatstones around it
                if square
                    .neighbours()
                    .filter_map(|neighbour| self[neighbour].top_stone())
                    .filter(|neighbour_piece| *neighbour_piece == Them::flat_piece())
                    .count()
                    >= 2
                {
                    coefficients[BLOCKING_STONE_NEXT_TO_TWO_OF_THEIR_FLATS] = 1.0;
                }
                for direction in square.directions() {
                    let neighbour = square.go_direction(direction).unwrap();
                    if self[neighbour]
                        .top_stone()
                        .map(Them::is_road_stone)
                        .unwrap_or_default()
                        && neighbour
                            .go_direction(direction)
                            .and_then(|sq| self[sq].top_stone())
                            .map(Them::is_road_stone)
                            .unwrap_or_default()
                    {
                        coefficients[BLOCKING_STONE_BLOCKS_EXTENSIONS_OF_TWO_FLATS] += 1.0;
                    }
                }
            }
            Move::Move(square, direction, stack_movement) => {
                let mut destination_square =
                    if stack_movement.movements[0].pieces_to_take == self[*square].len() {
                        square.go_direction(*direction).unwrap()
                    } else {
                        *square
                    };

                let mut our_pieces = 0;
                let mut their_pieces = 0;

                // This iterator skips the first square if we move the whole stack
                for piece in self
                    .top_stones_left_behind_by_move(*square, stack_movement)
                    .flatten()
                {
                    if Us::piece_is_ours(piece) {
                        our_pieces += 1;
                    } else {
                        their_pieces += 1;
                    }

                    let destination_stack = &self[destination_square];
                    if let Some(destination_top_stone) = destination_stack.top_stone() {
                        // When a stack gets captured, give a linear bonus or malus depending on
                        // whether it's captured by us or them
                        if piece.color() != destination_top_stone.color() {
                            if Us::piece_is_ours(piece) {
                                coefficients[STACK_CAPTURED_BY_MOVEMENT] +=
                                    destination_stack.len() as f32;
                            } else {
                                coefficients[STACK_CAPTURED_BY_MOVEMENT] -=
                                    destination_stack.len() as f32;
                            }
                        }
                    }

                    destination_square = destination_square
                        .go_direction(*direction)
                        .unwrap_or(destination_square);
                }

                if their_pieces == 0 {
                    coefficients[STACK_MOVEMENT_THAT_GIVES_US_TOP_PIECES] = 1.0;
                    coefficients[STACK_MOVEMENT_THAT_GIVES_US_TOP_PIECES + 1] = our_pieces as f32;
                    coefficients[STACK_MOVEMENT_THAT_GIVES_US_TOP_PIECES + 2] =
                        (our_pieces * our_pieces) as f32;
                } else if their_pieces == 1 {
                    coefficients[STACK_MOVEMENT_THAT_GIVES_US_TOP_PIECES + 3] = 1.0;
                    coefficients[STACK_MOVEMENT_THAT_GIVES_US_TOP_PIECES + 4] = our_pieces as f32;
                    coefficients[STACK_MOVEMENT_THAT_GIVES_US_TOP_PIECES + 5] =
                        (our_pieces * our_pieces) as f32;
                } else {
                    coefficients[STACK_MOVEMENT_THAT_GIVES_US_TOP_PIECES + 6] = 1.0;
                    coefficients[STACK_MOVEMENT_THAT_GIVES_US_TOP_PIECES + 7] = our_pieces as f32;
                    coefficients[STACK_MOVEMENT_THAT_GIVES_US_TOP_PIECES + 8] =
                        (our_pieces * our_pieces) as f32;
                }
            }
        }
    }

    pub(crate) fn generate_moves_colortr<Us: ColorTr, Them: ColorTr>(
        &self,
        moves: &mut Vec<<Board as board_game_traits::board::Board>::Move>,
    ) {
        for square in board::squares_iterator() {
            match self[square].top_stone() {
                None => {
                    if Us::stones_left(&self) > 0 {
                        moves.push(Move::Place(Flat, square));
                        moves.push(Move::Place(Standing, square));
                    }
                    if Us::capstones_left(&self) > 0 {
                        moves.push(Move::Place(Cap, square));
                    }
                }
                Some(piece) if Us::piece_is_ours(piece) => {
                    for direction in square.directions() {
                        let mut movements = vec![];
                        if piece == Us::cap_piece() {
                            self.generate_moving_moves_cap::<Us>(
                                direction,
                                square,
                                square,
                                self[square].len() as u8,
                                &ArrayVec::new(),
                                &mut movements,
                            );
                        } else if Us::piece_is_ours(piece) {
                            self.generate_moving_moves_non_cap::<Us>(
                                direction,
                                square,
                                square,
                                self[square].len() as u8,
                                &ArrayVec::new(),
                                &mut movements,
                            );
                        }
                        for movements in movements.into_iter().filter(|mv| !mv.is_empty()) {
                            let stack_movement = StackMovement { movements };
                            let mv = Move::Move(square, direction, stack_movement.clone());
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
        partial_movement: &ArrayVec<[Movement; BOARD_SIZE - 1]>,
        movements: &mut Vec<ArrayVec<[Movement; BOARD_SIZE - 1]>>,
    ) {
        if let Some(neighbour) = square.go_direction(direction) {
            let max_pieces_to_take = if square == origin_square {
                pieces_carried.min(BOARD_SIZE as u8)
            } else {
                (pieces_carried - 1).min(BOARD_SIZE as u8)
            };
            let neighbour_piece = self[neighbour].top_stone();
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

    fn generate_moving_moves_non_cap<Colorr: ColorTr>(
        &self,
        direction: Direction,
        origin_square: Square,
        square: Square,
        pieces_carried: u8,
        partial_movement: &ArrayVec<[Movement; BOARD_SIZE - 1]>,
        movements: &mut Vec<ArrayVec<[Movement; BOARD_SIZE - 1]>>,
    ) {
        if let Some(neighbour) = square.go_direction(direction) {
            let neighbour_piece = self[neighbour].top_stone();
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
}

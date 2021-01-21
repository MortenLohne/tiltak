use crate::bitboard::BitBoard;
use crate::board::Role::{Cap, Flat, Wall};
use crate::board::{
    Board, ColorTr, Direction::*, GroupData, Move, Square, TunableBoard, BOARD_SIZE,
    NUM_SQUARE_SYMMETRIES, SQUARE_SYMMETRIES,
};
use crate::search;
use arrayvec::ArrayVec;

pub fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + f32::exp(-x))
}

pub fn inverse_sigmoid(x: f32) -> f32 {
    assert!(x > 0.0 && x < 1.0, "Tried to inverse sigmoid {}", x);
    f32::ln(x / (1.0 - x))
}

const MOVE_COUNT: usize = 0;
const FLAT_PSQT: usize = MOVE_COUNT + 1;
const WALL_PSQT: usize = FLAT_PSQT + NUM_SQUARE_SYMMETRIES;
const CAP_PSQT: usize = WALL_PSQT + NUM_SQUARE_SYMMETRIES;
const OUR_ROAD_STONES_IN_LINE: usize = CAP_PSQT + NUM_SQUARE_SYMMETRIES;
const THEIR_ROAD_STONES_IN_LINE: usize = OUR_ROAD_STONES_IN_LINE + BOARD_SIZE * 3;
const EXTEND_GROUP: usize = THEIR_ROAD_STONES_IN_LINE + BOARD_SIZE * 3;
const MERGE_TWO_GROUPS: usize = EXTEND_GROUP + 3;
const BLOCK_MERGER: usize = MERGE_TWO_GROUPS + 3;
const PLACE_CRITICAL_SQUARE: usize = BLOCK_MERGER + 3;
const IGNORE_CRITICAL_SQUARE: usize = PLACE_CRITICAL_SQUARE + 5;
const NEXT_TO_OUR_LAST_STONE: usize = IGNORE_CRITICAL_SQUARE + 2;
const NEXT_TO_THEIR_LAST_STONE: usize = NEXT_TO_OUR_LAST_STONE + 1;
const DIAGONAL_TO_OUR_LAST_STONE: usize = NEXT_TO_THEIR_LAST_STONE + 1;
const DIAGONAL_TO_THEIR_LAST_STONE: usize = DIAGONAL_TO_OUR_LAST_STONE + 1;
const ATTACK_STRONG_FLATS: usize = DIAGONAL_TO_THEIR_LAST_STONE + 1;
const BLOCKING_STONE_BLOCKS_EXTENSIONS_OF_TWO_FLATS: usize = ATTACK_STRONG_FLATS + 1;

const MOVE_ROLE_BONUS: usize = BLOCKING_STONE_BLOCKS_EXTENSIONS_OF_TWO_FLATS + 1;
const STACK_MOVEMENT_THAT_GIVES_US_TOP_PIECES: usize = MOVE_ROLE_BONUS + 3;
const STACK_CAPTURED_BY_MOVEMENT: usize = STACK_MOVEMENT_THAT_GIVES_US_TOP_PIECES + 6;
const STACK_CAPTURE_IN_STRONG_LINE: usize = STACK_CAPTURED_BY_MOVEMENT + 1;
const STACK_CAPTURE_IN_STRONG_LINE_CAP: usize = STACK_CAPTURE_IN_STRONG_LINE + 2;
const MOVE_CAP_ONTO_STRONG_LINE: usize = STACK_CAPTURE_IN_STRONG_LINE_CAP + 2;
const MOVE_ONTO_CRITICAL_SQUARE: usize = MOVE_CAP_ONTO_STRONG_LINE + 4;
const _NEXT_CONST: usize = MOVE_ONTO_CRITICAL_SQUARE + 2;

impl<const S: usize> Board<S> {
    pub(crate) fn generate_moves_with_probabilities_colortr<Us: ColorTr, Them: ColorTr>(
        &self,
        params: &[f32],
        group_data: &GroupData<S>,
        simple_moves: &mut Vec<Move>,
        moves: &mut Vec<(Move, search::Score)>,
    ) {
        let num_moves = simple_moves.len();
        moves.extend(simple_moves.drain(..).map(|mv| {
            (
                mv.clone(),
                self.probability_for_move_colortr::<Us, Them>(params, &mv, group_data, num_moves),
            )
        }));
    }

    fn probability_for_move_colortr<Us: ColorTr, Them: ColorTr>(
        &self,
        params: &[f32],
        mv: &Move,
        group_data: &GroupData<S>,
        num_moves: usize,
    ) -> f32 {
        let mut coefficients = vec![0.0; Self::POLICY_PARAMS.len()];
        coefficients_for_move_colortr::<Us, Them, S>(
            self,
            &mut coefficients,
            mv,
            group_data,
            num_moves,
        );
        let total_value: f32 = coefficients.iter().zip(params).map(|(c, p)| c * p).sum();

        sigmoid(total_value)
    }
}
pub(crate) fn coefficients_for_move_colortr<Us: ColorTr, Them: ColorTr, const S: usize>(
    board: &Board<S>,
    coefficients: &mut [f32],
    mv: &Move,
    group_data: &GroupData<S>,
    num_legal_moves: usize,
) {
    assert_eq!(coefficients.len(), _NEXT_CONST);

    let initial_move_prob = 1.0 / num_legal_moves.max(2) as f32;

    coefficients[MOVE_COUNT] = inverse_sigmoid(initial_move_prob);

    // If it's the first move, give every move equal probability
    if board.half_moves_played() < 2 {
        return;
    }

    match mv {
        Move::Place(role, square) => {
            let their_open_critical_squares =
                Them::critical_squares(&*group_data) & (!group_data.all_pieces());

            // Apply PSQT
            match role {
                Flat => coefficients[FLAT_PSQT + SQUARE_SYMMETRIES[square.0 as usize]] = 1.0,
                Wall => coefficients[WALL_PSQT + SQUARE_SYMMETRIES[square.0 as usize]] = 1.0,
                Cap => coefficients[CAP_PSQT + SQUARE_SYMMETRIES[square.0 as usize]] = 1.0,
            }

            let role_id = match *role {
                Flat => 0,
                Wall => 1,
                Cap => 2,
            };

            for &line in BitBoard::lines_for_square::<S>(*square).iter() {
                let our_line_score = (Us::road_stones(&group_data) & line).count();
                let their_line_score = (Them::road_stones(&group_data) & line).count();
                coefficients
                    [OUR_ROAD_STONES_IN_LINE + BOARD_SIZE * role_id + our_line_score as usize] +=
                    1.0;
                coefficients[THEIR_ROAD_STONES_IN_LINE
                    + BOARD_SIZE * role_id
                    + their_line_score as usize] += 1.0;
            }

            // If square is next to a group
            let mut our_unique_neighbour_groups: ArrayVec<[(Square, u8); 4]> = ArrayVec::new();
            let mut their_unique_neighbour_groups: ArrayVec<[(Square, u8); 4]> = ArrayVec::new();
            for neighbour in square.neighbours::<S>().filter(|sq| !board[*sq].is_empty()) {
                let neighbour_group_id = group_data.groups[neighbour];
                if Us::piece_is_ours(board[neighbour].top_stone().unwrap()) {
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
                coefficients[MERGE_TWO_GROUPS + role_id] += 1.0;
            }

            if their_unique_neighbour_groups.len() > 1 {
                coefficients[BLOCK_MERGER + role_id] += 1.0;
            }

            for (_, group_id) in our_unique_neighbour_groups {
                coefficients[EXTEND_GROUP + role_id] +=
                    group_data.amount_in_group[group_id as usize].0 as f32;
            }

            if *role == Flat || *role == Cap {
                if Us::is_critical_square(&*group_data, *square) {
                    coefficients[PLACE_CRITICAL_SQUARE] += 1.0;
                } else if !their_open_critical_squares.is_empty() {
                    if their_open_critical_squares == BitBoard::empty().set(square.0) {
                        coefficients[PLACE_CRITICAL_SQUARE + 1] += 1.0;
                    } else {
                        coefficients[IGNORE_CRITICAL_SQUARE] += 1.0;
                    }
                }

                // If square is next to a road stone laid on our last turn
                if let Some(Move::Place(last_role, last_square)) =
                    board.moves().get(board.moves().len() - 2)
                {
                    if *last_role == Flat || *last_role == Cap {
                        if square.neighbours::<S>().any(|neigh| neigh == *last_square) {
                            coefficients[NEXT_TO_OUR_LAST_STONE] = 1.0;
                        } else if (square.rank::<S>() as i8 - last_square.rank::<S>() as i8).abs()
                            == 1
                            && (square.file::<S>() as i8 - last_square.file::<S>() as i8).abs() == 1
                        {
                            coefficients[DIAGONAL_TO_OUR_LAST_STONE] = 1.0;
                        }
                    }
                }

                // If square is next to a road stone laid on their last turn
                if let Some(Move::Place(last_role, last_square)) = board.moves().last() {
                    if *last_role == Flat {
                        if square.neighbours::<S>().any(|neigh| neigh == *last_square) {
                            coefficients[NEXT_TO_THEIR_LAST_STONE] = 1.0;
                        } else if (square.rank::<S>() as i8 - last_square.rank::<S>() as i8).abs()
                            == 1
                            && (square.file::<S>() as i8 - last_square.file::<S>() as i8).abs() == 1
                        {
                            coefficients[DIAGONAL_TO_THEIR_LAST_STONE] = 1.0;
                        }
                    }
                }

                // Bonus for attacking a flatstone in a rank/file where we are strong
                for neighbour in square.neighbours::<S>() {
                    if board[neighbour].top_stone() == Some(Them::flat_piece()) {
                        let our_road_stones = Us::road_stones(group_data)
                            .rank::<S>(neighbour.rank::<S>())
                            .count()
                            + Us::road_stones(group_data)
                                .file::<S>(neighbour.file::<S>())
                                .count();
                        if our_road_stones >= 2 {
                            coefficients[ATTACK_STRONG_FLATS] += (our_road_stones - 1) as f32;
                        }
                    }
                }
            }

            if *role == Wall {
                coefficients[WALL_PSQT + SQUARE_SYMMETRIES[square.0 as usize]] = 1.0;

                if !their_open_critical_squares.is_empty() {
                    if their_open_critical_squares == BitBoard::empty().set(square.0) {
                        coefficients[PLACE_CRITICAL_SQUARE + 2] += 1.0;
                    } else {
                        coefficients[IGNORE_CRITICAL_SQUARE] += 1.0;
                    }
                }
            } else if *role == Cap {
                if Us::is_critical_square(&*group_data, *square) {
                    coefficients[PLACE_CRITICAL_SQUARE] += 1.0;
                } else if !their_open_critical_squares.is_empty() {
                    if their_open_critical_squares == BitBoard::empty().set(square.0) {
                        coefficients[PLACE_CRITICAL_SQUARE + 3] += 1.0;
                    } else {
                        coefficients[IGNORE_CRITICAL_SQUARE] += 1.0;
                    }
                }
            }
            if *role == Wall || *role == Cap {
                // If square has two or more opponent flatstones around it
                for direction in square.directions::<S>() {
                    let neighbour = square.go_direction::<S>(direction).unwrap();
                    if board[neighbour]
                        .top_stone()
                        .map(Them::is_road_stone)
                        .unwrap_or_default()
                        && neighbour
                            .go_direction::<S>(direction)
                            .and_then(|sq| board[sq].top_stone())
                            .map(Them::is_road_stone)
                            .unwrap_or_default()
                    {
                        coefficients[BLOCKING_STONE_BLOCKS_EXTENSIONS_OF_TWO_FLATS] += 1.0;
                    }
                }
            }
        }

        Move::Move(square, direction, stack_movement) => {
            let role_id = match board[*square].top_stone().unwrap().role() {
                Flat => 0,
                Wall => 1,
                Cap => 2,
            };

            coefficients[MOVE_ROLE_BONUS + role_id] += 1.0;

            let mut destination_square =
                if stack_movement.movements[0].pieces_to_take == board[*square].len() {
                    square.go_direction::<S>(*direction).unwrap()
                } else {
                    *square
                };
            let mut gets_critical_square = false;

            let mut our_pieces = 0;
            let mut their_pieces = 0;
            let mut their_pieces_captured = 0;

            // This iterator skips the first square if we move the whole stack
            for piece in board
                .top_stones_left_behind_by_move(*square, stack_movement)
                .flatten()
            {
                if Us::piece_is_ours(piece) {
                    our_pieces += 1;
                } else {
                    their_pieces += 1;
                }

                // Bonus for moving our cap to a strong line
                // Extra bonus if it lands next to our critical square
                if piece == Us::cap_piece() {
                    let destination_line =
                        match direction {
                            North => Us::road_stones(group_data)
                                .rank::<S>(destination_square.rank::<S>()),
                            West => Us::road_stones(group_data)
                                .file::<S>(destination_square.file::<S>()),
                            East => Us::road_stones(group_data)
                                .file::<S>(destination_square.file::<S>()),
                            South => Us::road_stones(group_data)
                                .rank::<S>(destination_square.rank::<S>()),
                        };
                    let road_piece_count = destination_line.count() as usize;
                    if road_piece_count > 2 {
                        coefficients[MOVE_CAP_ONTO_STRONG_LINE + road_piece_count - 3] += 1.0;
                        if destination_square
                            .neighbours::<S>()
                            .any(|n| Us::is_critical_square(group_data, n))
                        {
                            coefficients[MOVE_CAP_ONTO_STRONG_LINE + road_piece_count - 1] += 1.0;
                        }
                    }
                }

                let destination_stack = &board[destination_square];
                if let Some(destination_top_stone) = destination_stack.top_stone() {
                    // When a stack gets captured, give a linear bonus or malus depending on
                    // whether it's captured by us or them
                    if piece.color() != destination_top_stone.color() {
                        if Us::piece_is_ours(piece) {
                            coefficients[STACK_CAPTURED_BY_MOVEMENT] +=
                                destination_stack.len() as f32;
                            their_pieces_captured += 1;
                            if Us::is_critical_square(&*group_data, destination_square) {
                                gets_critical_square = true;
                            }
                        } else {
                            coefficients[STACK_CAPTURED_BY_MOVEMENT] -=
                                destination_stack.len() as f32;
                        }
                    }
                    for &line in BitBoard::lines_for_square::<S>(destination_square).iter() {
                        let our_road_stones = (line & Us::road_stones(group_data)).count() as usize;
                        let color_factor = if Us::piece_is_ours(piece) { 1.0 } else { -1.0 };
                        if our_road_stones > 2 {
                            if piece.role() == Cap {
                                coefficients
                                    [STACK_CAPTURE_IN_STRONG_LINE_CAP + our_road_stones - 3] +=
                                    color_factor * destination_stack.len() as f32;
                            } else {
                                coefficients[STACK_CAPTURE_IN_STRONG_LINE + our_road_stones - 3] +=
                                    color_factor * destination_stack.len() as f32;
                            }
                        }
                    }
                }

                destination_square = destination_square
                    .go_direction::<S>(*direction)
                    .unwrap_or(destination_square);
            }

            if their_pieces == 0 {
                coefficients[STACK_MOVEMENT_THAT_GIVES_US_TOP_PIECES] = 1.0;
                coefficients[STACK_MOVEMENT_THAT_GIVES_US_TOP_PIECES + 1] = our_pieces as f32;
            } else if their_pieces == 1 {
                coefficients[STACK_MOVEMENT_THAT_GIVES_US_TOP_PIECES + 2] = 1.0;
                coefficients[STACK_MOVEMENT_THAT_GIVES_US_TOP_PIECES + 3] = our_pieces as f32;
            } else {
                coefficients[STACK_MOVEMENT_THAT_GIVES_US_TOP_PIECES + 4] = 1.0;
                coefficients[STACK_MOVEMENT_THAT_GIVES_US_TOP_PIECES + 5] = our_pieces as f32;
            }

            let their_open_critical_squares =
                Them::critical_squares(&*group_data) & (!group_data.all_pieces());

            if !their_open_critical_squares.is_empty() {
                if their_pieces_captured == 0 {
                    // Move ignores their critical threat, but might win for us
                    coefficients[IGNORE_CRITICAL_SQUARE + 1] += 1.0;
                } else {
                    // Move captures at least one stack, which might save us
                    coefficients[PLACE_CRITICAL_SQUARE + 4] += their_pieces_captured as f32;
                }
            }

            if gets_critical_square {
                if their_pieces == 0
                    && stack_movement.movements[0].pieces_to_take == board[*square].len()
                {
                    coefficients[MOVE_ONTO_CRITICAL_SQUARE] += 1.0;
                } else {
                    coefficients[MOVE_ONTO_CRITICAL_SQUARE + 1] += 1.0;
                }
            }
        }
    }
}

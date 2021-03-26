use arrayvec::ArrayVec;

use crate::position::bitboard::BitBoard;
use crate::position::color_trait::ColorTr;
use crate::position::Direction::*;
use crate::position::Move;
use crate::position::Role::{Cap, Flat, Wall};
use crate::position::Square;
use crate::position::{
    num_square_symmetries, square_symmetries, GroupData, Position, TunableBoard,
};
use crate::search;

pub fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + f32::exp(-x))
}

pub fn inverse_sigmoid(x: f32) -> f32 {
    assert!(x > 0.0 && x < 1.0, "Tried to inverse sigmoid {}", x);
    f32::ln(x / (1.0 - x))
}

impl<const S: usize> Position<S> {
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
        let mut coefficients = vec![0.0; Self::policy_params().len()];
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
    position: &Position<S>,
    coefficients: &mut [f32],
    mv: &Move,
    group_data: &GroupData<S>,
    num_legal_moves: usize,
) {
    let move_count: usize = 0;
    let flat_psqt: usize = move_count + 1;
    let wall_psqt: usize = flat_psqt + num_square_symmetries::<S>();
    let cap_psqt: usize = wall_psqt + num_square_symmetries::<S>();
    let our_road_stones_in_line: usize = cap_psqt + num_square_symmetries::<S>();
    let their_road_stones_in_line: usize = our_road_stones_in_line + S * 3;
    let extend_group: usize = their_road_stones_in_line + S * 3;
    let merge_two_groups: usize = extend_group + 3;
    let block_merger: usize = merge_two_groups + 3;
    let place_critical_square: usize = block_merger + 3;
    let ignore_critical_square: usize = place_critical_square + 5;
    let next_to_our_last_stone: usize = ignore_critical_square + 2;
    let next_to_their_last_stone: usize = next_to_our_last_stone + 1;
    let diagonal_to_our_last_stone: usize = next_to_their_last_stone + 1;
    let diagonal_to_their_last_stone: usize = diagonal_to_our_last_stone + 1;
    let attack_strong_flats: usize = diagonal_to_their_last_stone + 1;
    let blocking_stone_blocks_extensions_of_two_flats: usize = attack_strong_flats + 1;

    let move_role_bonus: usize = blocking_stone_blocks_extensions_of_two_flats + 1;
    let stack_movement_that_gives_us_top_pieces: usize = move_role_bonus + 3;
    let stack_captured_by_movement: usize = stack_movement_that_gives_us_top_pieces + 6;
    let stack_capture_in_strong_line: usize = stack_captured_by_movement + 1;
    let stack_capture_in_strong_line_cap: usize = stack_capture_in_strong_line + 2;
    let move_cap_onto_strong_line: usize = stack_capture_in_strong_line_cap + 2;
    let move_onto_critical_square: usize = move_cap_onto_strong_line + 4;
    let _next_const: usize = move_onto_critical_square + 4;

    assert_eq!(coefficients.len(), _next_const);

    let initial_move_prob = 1.0 / num_legal_moves.max(2) as f32;

    coefficients[move_count] = inverse_sigmoid(initial_move_prob);

    // If it's the first move, give every move equal probability
    if position.half_moves_played() < 2 {
        return;
    }

    match mv {
        Move::Place(role, square) => {
            let their_open_critical_squares =
                Them::critical_squares(&*group_data) & (!group_data.all_pieces());

            // Apply PSQT
            match role {
                Flat => coefficients[flat_psqt + square_symmetries::<S>()[square.0 as usize]] = 1.0,
                Wall => coefficients[wall_psqt + square_symmetries::<S>()[square.0 as usize]] = 1.0,
                Cap => coefficients[cap_psqt + square_symmetries::<S>()[square.0 as usize]] = 1.0,
            }

            let role_id = match *role {
                Flat => 0,
                Wall => 1,
                Cap => 2,
            };

            for &line in BitBoard::lines_for_square::<S>(*square).iter() {
                let our_line_score = (Us::road_stones(&group_data) & line).count();
                let their_line_score = (Them::road_stones(&group_data) & line).count();
                coefficients[our_road_stones_in_line + S * role_id + our_line_score as usize] +=
                    1.0;
                coefficients
                    [their_road_stones_in_line + S * role_id + their_line_score as usize] += 1.0;
            }

            // If square is next to a group
            let mut our_unique_neighbour_groups: ArrayVec<[(Square, u8); 4]> = ArrayVec::new();
            let mut their_unique_neighbour_groups: ArrayVec<[(Square, u8); 4]> = ArrayVec::new();
            for neighbour in square
                .neighbours::<S>()
                .filter(|sq| !position[*sq].is_empty())
            {
                let neighbour_group_id = group_data.groups[neighbour];
                if Us::piece_is_ours(position[neighbour].top_stone().unwrap()) {
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
                coefficients[merge_two_groups + role_id] += 1.0;
            }

            if their_unique_neighbour_groups.len() > 1 {
                coefficients[block_merger + role_id] += 1.0;
            }

            for (_, group_id) in our_unique_neighbour_groups {
                coefficients[extend_group + role_id] +=
                    group_data.amount_in_group[group_id as usize].0 as f32;
            }

            if *role == Flat || *role == Cap {
                if Us::is_critical_square(&*group_data, *square) {
                    coefficients[place_critical_square] += 1.0;
                } else if !their_open_critical_squares.is_empty() {
                    if their_open_critical_squares == BitBoard::empty().set(square.0) {
                        coefficients[place_critical_square + 1] += 1.0;
                    } else {
                        coefficients[ignore_critical_square] += 1.0;
                    }
                }

                // If square is next to a road stone laid on our last turn
                if let Some(Move::Place(last_role, last_square)) =
                    position.moves().get(position.moves().len() - 2)
                {
                    if *last_role == Flat || *last_role == Cap {
                        if square.neighbours::<S>().any(|neigh| neigh == *last_square) {
                            coefficients[next_to_our_last_stone] = 1.0;
                        } else if (square.rank::<S>() as i8 - last_square.rank::<S>() as i8).abs()
                            == 1
                            && (square.file::<S>() as i8 - last_square.file::<S>() as i8).abs() == 1
                        {
                            coefficients[diagonal_to_our_last_stone] = 1.0;
                        }
                    }
                }

                // If square is next to a road stone laid on their last turn
                if let Some(Move::Place(last_role, last_square)) = position.moves().last() {
                    if *last_role == Flat {
                        if square.neighbours::<S>().any(|neigh| neigh == *last_square) {
                            coefficients[next_to_their_last_stone] = 1.0;
                        } else if (square.rank::<S>() as i8 - last_square.rank::<S>() as i8).abs()
                            == 1
                            && (square.file::<S>() as i8 - last_square.file::<S>() as i8).abs() == 1
                        {
                            coefficients[diagonal_to_their_last_stone] = 1.0;
                        }
                    }
                }

                // Bonus for attacking a flatstone in a rank/file where we are strong
                for neighbour in square.neighbours::<S>() {
                    if position[neighbour].top_stone() == Some(Them::flat_piece()) {
                        let our_road_stones = Us::road_stones(group_data)
                            .rank::<S>(neighbour.rank::<S>())
                            .count()
                            + Us::road_stones(group_data)
                                .file::<S>(neighbour.file::<S>())
                                .count();
                        if our_road_stones >= 2 {
                            coefficients[attack_strong_flats] += (our_road_stones - 1) as f32;
                        }
                    }
                }
            }

            if *role == Wall {
                coefficients[wall_psqt + square_symmetries::<S>()[square.0 as usize]] = 1.0;

                if !their_open_critical_squares.is_empty() {
                    if their_open_critical_squares == BitBoard::empty().set(square.0) {
                        coefficients[place_critical_square + 2] += 1.0;
                    } else {
                        coefficients[ignore_critical_square] += 1.0;
                    }
                }
            } else if *role == Cap {
                if Us::is_critical_square(&*group_data, *square) {
                    coefficients[place_critical_square] += 1.0;
                } else if !their_open_critical_squares.is_empty() {
                    if their_open_critical_squares == BitBoard::empty().set(square.0) {
                        coefficients[place_critical_square + 3] += 1.0;
                    } else {
                        coefficients[ignore_critical_square] += 1.0;
                    }
                }
            }
            if *role == Wall || *role == Cap {
                // If square has two or more opponent flatstones around it
                for direction in square.directions::<S>() {
                    let neighbour = square.go_direction::<S>(direction).unwrap();
                    if position[neighbour]
                        .top_stone()
                        .map(Them::is_road_stone)
                        .unwrap_or_default()
                        && neighbour
                            .go_direction::<S>(direction)
                            .and_then(|sq| position[sq].top_stone())
                            .map(Them::is_road_stone)
                            .unwrap_or_default()
                    {
                        coefficients[blocking_stone_blocks_extensions_of_two_flats] += 1.0;
                    }
                }
            }
        }

        Move::Move(square, direction, stack_movement) => {
            let role_id = match position[*square].top_stone().unwrap().role() {
                Flat => 0,
                Wall => 1,
                Cap => 2,
            };

            coefficients[move_role_bonus + role_id] += 1.0;

            let mut destination_square =
                if stack_movement.get(0).pieces_to_take == position[*square].len() {
                    square.go_direction::<S>(*direction).unwrap()
                } else {
                    *square
                };
            let mut gets_critical_square = false;

            let mut our_pieces = 0;
            let mut their_pieces = 0;
            let mut their_pieces_captured = 0;

            // This iterator skips the first square if we move the whole stack
            for piece in position
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
                        coefficients[move_cap_onto_strong_line + road_piece_count - 3] += 1.0;
                        if destination_square
                            .neighbours::<S>()
                            .any(|n| Us::is_critical_square(group_data, n))
                        {
                            coefficients[move_cap_onto_strong_line + road_piece_count - 1] += 1.0;
                        }
                    }
                }

                let destination_stack = &position[destination_square];
                if let Some(destination_top_stone) = destination_stack.top_stone() {
                    // When a stack gets captured, give a linear bonus or malus depending on
                    // whether it's captured by us or them
                    if piece.color() != destination_top_stone.color() {
                        if Us::piece_is_ours(piece) {
                            coefficients[stack_captured_by_movement] +=
                                destination_stack.len() as f32;
                            their_pieces_captured += 1;
                        } else {
                            coefficients[stack_captured_by_movement] -=
                                destination_stack.len() as f32;
                        }
                    }
                    if Us::is_critical_square(&*group_data, destination_square) {
                        gets_critical_square = true;
                    }

                    for &line in BitBoard::lines_for_square::<S>(destination_square).iter() {
                        let our_road_stones = (line & Us::road_stones(group_data)).count() as usize;
                        let color_factor = if Us::piece_is_ours(piece) { 1.0 } else { -1.0 };
                        if our_road_stones > 2 {
                            if piece.role() == Cap {
                                coefficients
                                    [stack_capture_in_strong_line_cap + our_road_stones - 3] +=
                                    color_factor * destination_stack.len() as f32;
                            } else {
                                coefficients[stack_capture_in_strong_line + our_road_stones - 3] +=
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
                coefficients[stack_movement_that_gives_us_top_pieces] = 1.0;
                coefficients[stack_movement_that_gives_us_top_pieces + 1] = our_pieces as f32;
            } else if their_pieces == 1 {
                coefficients[stack_movement_that_gives_us_top_pieces + 2] = 1.0;
                coefficients[stack_movement_that_gives_us_top_pieces + 3] = our_pieces as f32;
            } else {
                coefficients[stack_movement_that_gives_us_top_pieces + 4] = 1.0;
                coefficients[stack_movement_that_gives_us_top_pieces + 5] = our_pieces as f32;
            }

            let their_open_critical_squares =
                Them::critical_squares(&*group_data) & (!group_data.all_pieces());

            if !their_open_critical_squares.is_empty() {
                if their_pieces_captured == 0 {
                    // Move ignores their critical threat, but might win for us
                    coefficients[ignore_critical_square + 1] += 1.0;
                } else {
                    // Move captures at least one stack, which might save us
                    coefficients[place_critical_square + 4] += their_pieces_captured as f32;
                }
            }

            // Bonus for moving onto a critical square
            if gets_critical_square {
                let moves_our_whole_stack =
                    stack_movement.get(0).pieces_to_take == position[*square].len();

                match (their_pieces == 0, moves_our_whole_stack) {
                    (false, false) => coefficients[move_onto_critical_square] += 1.0,
                    (false, true) => coefficients[move_onto_critical_square + 1] += 1.0,
                    // Only this option is a guaranteed win:
                    (true, false) => coefficients[move_onto_critical_square + 2] += 1.0,
                    (true, true) => coefficients[move_onto_critical_square + 3] += 1.0,
                }
            }
        }
    }
}

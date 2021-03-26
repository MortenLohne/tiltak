use board_game_traits::{Color, Position as EvalPosition};

use crate::position::bitboard::BitBoard;
use crate::position::color_trait::{BlackTr, ColorTr, WhiteTr};
use crate::position::{
    num_square_symmetries, square_symmetries, squares_iterator, GroupData, Piece::*, Position,
    Role::*, Square,
};

pub(crate) fn static_eval_game_phase<const S: usize>(
    position: &Position<S>,
    group_data: &GroupData<S>,
    coefficients: &mut [f32],
) {
    let flat_psqt: usize = 0;
    let wall_psqt: usize = flat_psqt + num_square_symmetries::<S>();
    let cap_psqt: usize = wall_psqt + num_square_symmetries::<S>();
    let our_stack_psqt: usize = cap_psqt + num_square_symmetries::<S>();
    let their_stack_psqt: usize = our_stack_psqt + num_square_symmetries::<S>();

    let mut white_flat_count = 0;
    let mut black_flat_count = 0;

    for square in squares_iterator::<S>() {
        let stack = &position[square];
        if let Some(piece) = position[square].top_stone() {
            let i = square.0 as usize;
            match piece {
                WhiteFlat => {
                    coefficients[flat_psqt + square_symmetries::<S>()[i]] += 1.0;
                    white_flat_count += 1;
                }
                BlackFlat => {
                    coefficients[flat_psqt + square_symmetries::<S>()[i]] -= 1.0;
                    black_flat_count += 1;
                }
                WhiteWall => coefficients[wall_psqt + square_symmetries::<S>()[i]] += 1.0,
                BlackWall => coefficients[wall_psqt + square_symmetries::<S>()[i]] -= 1.0,
                WhiteCap => coefficients[cap_psqt + square_symmetries::<S>()[i]] += 1.0,
                BlackCap => coefficients[cap_psqt + square_symmetries::<S>()[i]] -= 1.0,
            }
            if stack.height > 1 {
                let controlling_player = piece.color();
                let color_factor = piece.color().multiplier() as f32;
                for piece in stack.clone().into_iter().take(stack.height as usize - 1) {
                    if piece.color() == controlling_player {
                        coefficients[our_stack_psqt + square_symmetries::<S>()[i]] += color_factor;
                    } else {
                        coefficients[their_stack_psqt + square_symmetries::<S>()[i]] -=
                            color_factor;
                    }
                }
            }
        }
    }

    let side_to_move: usize = their_stack_psqt + num_square_symmetries::<S>();
    let flatstone_lead: usize = side_to_move + 3;
    let i_number_of_groups: usize = flatstone_lead + 3;

    // Give the side to move a bonus/malus depending on flatstone lead
    let white_flatstone_lead = white_flat_count - black_flat_count;

    // Bonus/malus depending on the number of groups each side has
    let mut seen_groups = vec![false; S * S + 1]; // TODO: Can be an array with full const-generics
    seen_groups[0] = true;

    let number_of_groups = squares_iterator::<S>()
        .map(|square| {
            let group_id = group_data.groups[square] as usize;
            if !seen_groups[group_id] {
                seen_groups[group_id] = true;
                position[square].top_stone().unwrap().color().multiplier()
            } else {
                0
            }
        })
        .sum::<isize>() as f32;

    let opening_scale_factor = f32::min(
        f32::max((24.0 - position.half_moves_played() as f32) / 12.0, 0.0),
        1.0,
    );
    let endgame_scale_factor = f32::min(
        f32::max((position.half_moves_played() as f32 - 24.0) / 24.0, 0.0),
        1.0,
    );
    let middlegame_scale_factor = 1.0 - opening_scale_factor - endgame_scale_factor;

    debug_assert!(middlegame_scale_factor <= 1.0);
    debug_assert!(opening_scale_factor == 0.0 || endgame_scale_factor == 0.0);

    coefficients[side_to_move] = position.side_to_move().multiplier() as f32 * opening_scale_factor;
    coefficients[flatstone_lead] = white_flatstone_lead as f32 * opening_scale_factor;
    coefficients[i_number_of_groups] = number_of_groups * opening_scale_factor;

    coefficients[side_to_move + 1] =
        position.side_to_move().multiplier() as f32 * middlegame_scale_factor;
    coefficients[flatstone_lead + 1] = white_flatstone_lead as f32 * middlegame_scale_factor;
    coefficients[i_number_of_groups + 1] = number_of_groups * middlegame_scale_factor;

    coefficients[side_to_move + 2] =
        position.side_to_move().multiplier() as f32 * endgame_scale_factor;
    coefficients[flatstone_lead + 2] = white_flatstone_lead as f32 * endgame_scale_factor;
    coefficients[i_number_of_groups + 2] = number_of_groups * endgame_scale_factor;

    let critical_squares: usize = i_number_of_groups + 3;

    for critical_square in group_data.critical_squares(Color::White) {
        critical_squares_eval::<WhiteTr, BlackTr, S>(
            position,
            critical_square,
            coefficients,
            critical_squares,
        );
    }

    for critical_square in group_data.critical_squares(Color::Black) {
        critical_squares_eval::<BlackTr, WhiteTr, S>(
            position,
            critical_square,
            coefficients,
            critical_squares,
        );
    }

    let capstone_over_own_piece: usize = critical_squares + 6;
    let capstone_on_stack: usize = capstone_over_own_piece + 1;
    let standing_stone_on_stack: usize = capstone_on_stack + 1;
    let flat_stone_next_to_our_stack: usize = standing_stone_on_stack + 1;
    let standing_stone_next_to_our_stack: usize = flat_stone_next_to_our_stack + 1;
    let capstone_next_to_our_stack: usize = standing_stone_next_to_our_stack + 1;

    squares_iterator::<S>()
        .map(|sq| (sq, &position[sq]))
        .filter(|(_, stack)| stack.len() > 1)
        .for_each(|(square, stack)| {
            let top_stone = stack.top_stone().unwrap();
            let controlling_player = top_stone.color();
            let color_factor = top_stone.color().multiplier() as f32;

            // Extra bonus for having your capstone over your own piece
            if top_stone.role() == Cap
                && stack.get(stack.len() - 2).unwrap().color() == controlling_player
            {
                coefficients[capstone_over_own_piece] += color_factor;
            }

            match top_stone.role() {
                Cap => coefficients[capstone_on_stack] += color_factor,
                Flat => (),
                Wall => coefficients[standing_stone_on_stack] += color_factor,
            }

            // Malus for them having stones next to our stack with flat stones on top
            for neighbour in square.neighbours::<S>() {
                if let Some(neighbour_top_stone) = position[neighbour].top_stone() {
                    if top_stone.role() == Flat && neighbour_top_stone.color() != controlling_player
                    {
                        match neighbour_top_stone.role() {
                            Flat => {
                                coefficients[flat_stone_next_to_our_stack] +=
                                    color_factor * stack.len() as f32
                            }
                            Wall => {
                                coefficients[standing_stone_next_to_our_stack] +=
                                    color_factor * stack.len() as f32
                            }
                            Cap => {
                                coefficients[capstone_next_to_our_stack] +=
                                    color_factor * stack.len() as f32
                            }
                        }
                    }
                }
            }
        });

    // Number of pieces in each line
    let num_lines_occupied: usize = capstone_next_to_our_stack + 1;
    // Number of lines with at least one road stone
    let line_control: usize = num_lines_occupied + S + 1;

    let mut num_ranks_occupied_white = 0;
    let mut num_files_occupied_white = 0;
    let mut num_ranks_occupied_black = 0;
    let mut num_files_occupied_black = 0;

    for line in BitBoard::all_lines::<S>().iter() {
        line_score::<WhiteTr, BlackTr, S>(&group_data, *line, coefficients, line_control);
        line_score::<BlackTr, WhiteTr, S>(&group_data, *line, coefficients, line_control);
    }

    for i in 0..S as u8 {
        if !WhiteTr::road_stones(&group_data).rank::<S>(i).is_empty() {
            num_ranks_occupied_white += 1;
        }
        if !BlackTr::road_stones(&group_data).rank::<S>(i).is_empty() {
            num_ranks_occupied_black += 1;
        }
    }

    for i in 0..S as u8 {
        if !WhiteTr::road_stones(&group_data).file::<S>(i).is_empty() {
            num_files_occupied_white += 1;
        }
        if !BlackTr::road_stones(&group_data).file::<S>(i).is_empty() {
            num_files_occupied_black += 1;
        }
    }

    coefficients[num_lines_occupied + num_ranks_occupied_white] += 1.0;
    coefficients[num_lines_occupied + num_files_occupied_white] += 1.0;
    coefficients[num_lines_occupied + num_ranks_occupied_black] -= 1.0;
    coefficients[num_lines_occupied + num_files_occupied_black] -= 1.0;

    let _next_const = line_control + 2 * (S + 1);

    assert_eq!(_next_const, coefficients.len());
}

/// Give bonus for our critical squares
fn critical_squares_eval<Us: ColorTr, Them: ColorTr, const S: usize>(
    position: &Position<S>,
    critical_square: Square,
    coefficients: &mut [f32],
    critical_squares: usize,
) {
    let top_stone = position[critical_square].top_stone;
    if top_stone.is_none() {
        coefficients[critical_squares] += Us::color().multiplier() as f32;
    } else if top_stone == Some(Us::wall_piece()) {
        coefficients[critical_squares + 1] += Us::color().multiplier() as f32;
    } else if top_stone == Some(Them::flat_piece()) {
        coefficients[critical_squares + 2] += Us::color().multiplier() as f32;
    }
    // Their capstone or wall
    else {
        coefficients[critical_squares + 3] += Us::color().multiplier() as f32
    }

    // Bonus for having our cap next to our critical square
    for neighbour in critical_square.neighbours::<S>() {
        if position[neighbour].top_stone() == Some(Us::cap_piece()) {
            coefficients[critical_squares + 4] += Us::color().multiplier() as f32;
            // Further bonus for a capped stack next to our critical square
            for piece in position[neighbour].clone().into_iter() {
                if piece == Us::flat_piece() {
                    coefficients[critical_squares + 5] += Us::color().multiplier() as f32;
                }
            }
        }
    }
}

fn line_score<Us: ColorTr, Them: ColorTr, const S: usize>(
    group_data: &GroupData<S>,
    line: BitBoard,
    coefficients: &mut [f32],
    line_control: usize,
) {
    let road_pieces_in_line = (Us::road_stones(group_data) & line).count();

    coefficients[line_control + road_pieces_in_line as usize] += Us::color().multiplier() as f32;

    let block_their_line = line_control + S + 1;

    // Bonus for blocking their lines
    if road_pieces_in_line >= 3 {
        coefficients[block_their_line + road_pieces_in_line as usize - 3] +=
            ((Them::flats(group_data) & line).count() as isize * Them::color().multiplier()) as f32;
        coefficients[block_their_line + 2 + road_pieces_in_line as usize - 3] +=
            ((Them::walls(group_data) & line).count() as isize * Them::color().multiplier()) as f32;
        coefficients[block_their_line + 4 + road_pieces_in_line as usize - 3] +=
            ((Them::caps(group_data) & line).count() as isize * Them::color().multiplier()) as f32;
    }
}

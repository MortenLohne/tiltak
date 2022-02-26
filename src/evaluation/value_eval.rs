use board_game_traits::{Color, Position as EvalPosition};

use crate::evaluation::parameters::ValueFeatures;
use crate::position::bitboard::BitBoard;
use crate::position::color_trait::{BlackTr, ColorTr, WhiteTr};
use crate::position::{
    line_symmetries, square_symmetries, squares_iterator, GroupData, Piece, Piece::*, Position,
    Role::*, Square,
};

pub(crate) fn static_eval_game_phase<const S: usize>(
    position: &Position<S>,
    group_data: &GroupData<S>,
    value_features: &mut ValueFeatures,
) {
    let mut white_flat_count = 0;
    let mut black_flat_count = 0;

    for square in squares_iterator::<S>() {
        let stack = &position[square];
        if let Some(piece) = position[square].top_stone() {
            let i = square.0 as usize;
            match piece {
                WhiteFlat => {
                    value_features.flat_psqt[square_symmetries::<S>()[i]] += 1.0;
                    white_flat_count += 1;
                }
                BlackFlat => {
                    value_features.flat_psqt[square_symmetries::<S>()[i]] -= 1.0;
                    black_flat_count += 1;
                }
                WhiteWall => value_features.wall_psqt[square_symmetries::<S>()[i]] += 1.0,
                BlackWall => value_features.wall_psqt[square_symmetries::<S>()[i]] -= 1.0,
                WhiteCap => {
                    value_features.cap_psqt[square_symmetries::<S>()[i]] += 1.0;
                    cap_activity::<WhiteTr, BlackTr, S>(position, square, value_features);
                }
                BlackCap => {
                    value_features.cap_psqt[square_symmetries::<S>()[i]] -= 1.0;
                    cap_activity::<BlackTr, WhiteTr, S>(position, square, value_features);
                }
            }
            if stack.height > 1 {
                let controlling_player = piece.color();
                let color_factor = piece.color().multiplier() as f32;
                for (stack_index, stack_piece) in stack
                    .into_iter()
                    .enumerate()
                    .take(stack.height as usize - 1)
                {
                    // Position in the stack. Top stone is 1
                    let depth = stack.height as usize - stack_index;
                    let is_support = stack_piece.color() == controlling_player;
                    let top_role_index = match piece.role() {
                        Flat => 0,
                        Wall => 1,
                        Cap if stack.get(stack.height - 2).unwrap().color()
                            == controlling_player =>
                        {
                            2
                        }
                        Cap => 3,
                    };
                    // Separate non-psqt bonus based on the role of the top stone,
                    // and whether the stack piece is below the carry limit in the stack
                    match (is_support, depth > S + 1) {
                        (true, true) => {
                            value_features.deep_supports_per_piece[top_role_index] += color_factor
                        }
                        (true, false) => {
                            value_features.shallow_supports_per_piece[top_role_index] +=
                                color_factor
                        }
                        (false, true) => {
                            value_features.deep_captives_per_piece[top_role_index] += color_factor
                        }
                        (false, false) => {
                            value_features.shallow_captives_per_piece[top_role_index] +=
                                color_factor
                        }
                    }
                    if is_support {
                        value_features.supports_psqt[square_symmetries::<S>()[i]] += color_factor;
                    } else {
                        value_features.captives_psqt[square_symmetries::<S>()[i]] -= color_factor;
                    }
                }
            }
        }
    }

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

    value_features.side_to_move[0] =
        position.side_to_move().multiplier() as f32 * opening_scale_factor;
    value_features.flatstone_lead[0] = white_flatstone_lead as f32 * opening_scale_factor;
    value_features.i_number_of_groups[0] = number_of_groups * opening_scale_factor;

    value_features.side_to_move[1] =
        position.side_to_move().multiplier() as f32 * middlegame_scale_factor;
    value_features.flatstone_lead[1] = white_flatstone_lead as f32 * middlegame_scale_factor;
    value_features.i_number_of_groups[1] = number_of_groups * middlegame_scale_factor;

    value_features.side_to_move[2] =
        position.side_to_move().multiplier() as f32 * endgame_scale_factor;
    value_features.flatstone_lead[2] = white_flatstone_lead as f32 * endgame_scale_factor;
    value_features.i_number_of_groups[2] = number_of_groups * endgame_scale_factor;

    for critical_square in group_data.critical_squares(Color::White) {
        critical_squares_eval::<WhiteTr, BlackTr, S>(position, critical_square, value_features);
    }

    for critical_square in group_data.critical_squares(Color::Black) {
        critical_squares_eval::<BlackTr, WhiteTr, S>(position, critical_square, value_features);
    }

    squares_iterator::<S>()
        .map(|sq| (sq, &position[sq]))
        .filter(|(_, stack)| stack.len() > 1)
        .for_each(|(square, stack)| {
            let top_stone = stack.top_stone().unwrap();
            let controlling_player = top_stone.color();
            let color_factor = top_stone.color().multiplier() as f32;

            // Malus for them having stones next to our stack with flat stones on top
            for neighbour in square.neighbours::<S>() {
                if let Some(neighbour_top_stone) = position[neighbour].top_stone() {
                    if top_stone.role() == Flat && neighbour_top_stone.color() != controlling_player
                    {
                        match neighbour_top_stone.role() {
                            Flat => {
                                value_features.flat_next_to_our_stack[0] +=
                                    color_factor * stack.len() as f32
                            }
                            Wall => {
                                value_features.wall_next_to_our_stack[0] +=
                                    color_factor * stack.len() as f32
                            }
                            Cap => {
                                value_features.cap_next_to_our_stack[0] +=
                                    color_factor * stack.len() as f32
                            }
                        }
                    }
                }
            }
        });

    let mut num_ranks_occupied_white = 0;
    let mut num_files_occupied_white = 0;
    let mut num_ranks_occupied_black = 0;
    let mut num_files_occupied_black = 0;

    for i in 0..(S as u8) {
        let rank = BitBoard::full().rank::<S>(i);
        let file = BitBoard::full().file::<S>(i);
        line_score::<WhiteTr, BlackTr, S>(group_data, rank, i, value_features);
        line_score::<BlackTr, WhiteTr, S>(group_data, rank, i, value_features);
        line_score::<WhiteTr, BlackTr, S>(group_data, file, i, value_features);
        line_score::<BlackTr, WhiteTr, S>(group_data, file, i, value_features);
    }

    for i in 0..S as u8 {
        if !WhiteTr::road_stones(group_data).rank::<S>(i).is_empty() {
            num_ranks_occupied_white += 1;
        }
        if !BlackTr::road_stones(group_data).rank::<S>(i).is_empty() {
            num_ranks_occupied_black += 1;
        }
    }

    for i in 0..S as u8 {
        if !WhiteTr::road_stones(group_data).file::<S>(i).is_empty() {
            num_files_occupied_white += 1;
        }
        if !BlackTr::road_stones(group_data).file::<S>(i).is_empty() {
            num_files_occupied_black += 1;
        }
    }

    value_features.num_lines_occupied[num_ranks_occupied_white] += 1.0;
    value_features.num_lines_occupied[num_files_occupied_white] += 1.0;
    value_features.num_lines_occupied[num_ranks_occupied_black] -= 1.0;
    value_features.num_lines_occupied[num_files_occupied_black] -= 1.0;
}

fn cap_activity<Us: ColorTr, Them: ColorTr, const S: usize>(
    position: &Position<S>,
    square: Square,
    value_features: &mut ValueFeatures,
) {
    let stack = position[square];
    let height_index = stack.height.min(3) as usize - 1;

    // Malus if our capstone's line towards the center is blocked
    if square.neighbours::<S>().any(|neighbour| {
        square_symmetries::<S>()[neighbour.0 as usize] > square_symmetries::<S>()[square.0 as usize]
            && position[neighbour].top_stone().map(Piece::role) == Some(Cap)
    }) {
        value_features.sidelined_cap[height_index] += Us::color().multiplier() as f32
    }

    let is_soft_cap = stack
        .get(stack.height.overflowing_sub(2).0)
        .map(Them::piece_is_ours)
        == Some(true);
    if square.neighbours::<S>().all(|neighbour| {
        matches!(
            position[neighbour].top_stone(),
            Some(WhiteCap) | Some(BlackCap) | None
        )
    }) {
        value_features.fully_isolated_cap[height_index] += Us::color().multiplier() as f32
    } else if square.neighbours::<S>().all(|neighbour| {
        if let Some(neighbour_top_stone) = position[neighbour].top_stone() {
            if neighbour_top_stone == Them::wall_piece() {
                is_soft_cap
            } else {
                neighbour_top_stone != Them::flat_piece()
            }
        } else {
            true
        }
    }) {
        value_features.semi_isolated_cap[height_index] += Us::color().multiplier() as f32
    }
}

/// Give bonus for our critical squares
fn critical_squares_eval<Us: ColorTr, Them: ColorTr, const S: usize>(
    position: &Position<S>,
    critical_square: Square,
    value_features: &mut ValueFeatures,
) {
    let top_stone = position[critical_square].top_stone;
    if top_stone.is_none() {
        value_features.critical_squares[0] += Us::color().multiplier() as f32;
    } else if top_stone == Some(Us::wall_piece()) {
        value_features.critical_squares[1] += Us::color().multiplier() as f32;
    } else if top_stone == Some(Them::flat_piece()) {
        value_features.critical_squares[2] += Us::color().multiplier() as f32;
    }
    // Their capstone or wall
    else {
        value_features.critical_squares[3] += Us::color().multiplier() as f32
    }

    // Bonus for having our cap next to our critical square
    for neighbour in critical_square.neighbours::<S>() {
        if position[neighbour].top_stone() == Some(Us::cap_piece()) {
            value_features.critical_squares[4] += Us::color().multiplier() as f32;
            // Further bonus for a capped stack next to our critical square
            for piece in position[neighbour].into_iter() {
                if piece == Us::flat_piece() {
                    value_features.critical_squares[5] += Us::color().multiplier() as f32;
                }
            }
        }
    }
}

fn line_score<Us: ColorTr, Them: ColorTr, const S: usize>(
    group_data: &GroupData<S>,
    line: BitBoard,
    i: u8,
    value_features: &mut ValueFeatures,
) {
    let road_pieces_in_line = (Us::road_stones(group_data) & line).count() as usize;
    let index = road_pieces_in_line + line_symmetries::<S>()[i as usize] * S;

    if !(Them::blocking_stones(group_data) & line).is_empty() {
        value_features.line_control_their_blocking_piece[index] += Us::color().multiplier() as f32;
    } else if !((Us::walls(group_data) | Them::flats(group_data)) & line).is_empty() {
        value_features.line_control_other[index] += Us::color().multiplier() as f32;
    } else {
        value_features.line_control_empty[index] += Us::color().multiplier() as f32;
    }
}

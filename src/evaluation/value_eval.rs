use std::f32::consts::PI;

use arrayvec::ArrayVec;
use board_game_traits::{Color, Position as EvalPosition};
use half::f16;
use num_traits::FromPrimitive;

use crate::evaluation::parameters::value_indexes;
use crate::position::bitboard::BitBoard;
use crate::position::color_trait::{BlackTr, ColorTr, WhiteTr};
use crate::position::{
    line_symmetries, lookup_square_symmetries, squares_iterator, AbstractBoard, GroupData,
    Piece::{self, *},
    Position,
    Role::*,
    Square,
};
use crate::position::{starting_capstones, starting_stones, Direction, Stack};

use super::parameters::ValueApplier;

pub fn sigmoid(x: f32) -> f32 {
    0.5 + f32::atan(x) / PI
}

pub fn sigmoid_derived(x: f32) -> f32 {
    1.0 / (PI * (1.0 + x.powi(2)))
}

pub fn static_eval_game_phase<const S: usize, V: ValueApplier>(
    position: &Position<S>,
    group_data: &GroupData<S>,
    white_value: &mut V,
    black_value: &mut V,
) {
    let indexes = value_indexes::<S>();
    let all_pieces = group_data.all_pieces();
    if all_pieces.count() == 0 {
        white_value.eval(indexes.first_ply, 0, f16::ONE);
        return;
    } else if all_pieces.count() == 1 {
        for square in squares_iterator::<S>() {
            if position.top_stones()[square].is_some() {
                white_value.eval(
                    indexes.second_ply,
                    lookup_square_symmetries::<S>(square),
                    f16::ONE,
                );
                return;
            }
        }
        unreachable!()
    }

    let mut white_flat_count = 0;
    let mut black_flat_count = 0;

    let lowest_reserves_fraction = u8::min(
        position.white_reserves_left() + position.white_caps_left(),
        position.black_reserves_left() + position.black_caps_left(),
    ) as f32
        / (starting_stones(S) + starting_capstones(S)) as f32;

    let opening_scale_factor =
        f16::from_f32((2.0 * lowest_reserves_fraction - 1.0).clamp(0.0, 1.0));
    let endgame_scale_factor =
        f16::from_f32((1.0 - (2.0 * lowest_reserves_fraction)).clamp(0.0, 1.0));

    let middlegame_scale_factor = f16::ONE - opening_scale_factor - endgame_scale_factor;

    debug_assert!(middlegame_scale_factor <= f16::ONE);
    debug_assert!(opening_scale_factor == f16::ZERO || endgame_scale_factor == f16::ZERO);

    #[derive(Default, Clone, Copy, Debug)]
    struct StackData {
        shallow_supports: u8,
        deep_supports: u8,
        shallow_captives: u8,
        deep_captives: u8,
    }

    let mut stack_data: AbstractBoard<StackData, S> = AbstractBoard::default();

    for (square, top_stone) in
        squares_iterator::<S>().filter_map(|sq| position.top_stones()[sq].map(|piece| (sq, piece)))
    {
        let mut data = StackData::default();
        let stack = position.get_stack(square);
        let controlling_player = top_stone.color();
        for (stack_index, stack_piece) in stack
            .into_iter()
            .enumerate()
            .take(stack.height as usize - 1)
        // Skip last element, so that we don't count top stone as a support
        {
            // Position in the stack. Top stone is 1
            let depth = stack.height as usize - stack_index;

            match (stack_piece.color() == controlling_player, depth > S + 1) {
                (true, true) => data.deep_supports += 1,
                (true, false) => data.shallow_supports += 1,
                (false, true) => data.deep_captives += 1,
                (false, false) => data.shallow_captives += 1,
            }
        }
        stack_data[square] = data;
    }

    for (square, piece) in
        squares_iterator::<S>().filter_map(|sq| position.top_stones()[sq].map(|piece| (sq, piece)))
    {
        let stack = position.get_stack(square);
        match piece {
            WhiteFlat => {
                white_value.eval(
                    indexes.flat_psqt_opening,
                    lookup_square_symmetries::<S>(square),
                    opening_scale_factor,
                );
                white_value.eval(
                    indexes.flat_psqt_middlegame,
                    lookup_square_symmetries::<S>(square),
                    middlegame_scale_factor,
                );
                white_value.eval(
                    indexes.flat_psqt_endgame,
                    lookup_square_symmetries::<S>(square),
                    endgame_scale_factor,
                );
                white_flat_count += 1;
            }
            BlackFlat => {
                black_value.eval(
                    indexes.flat_psqt_opening,
                    lookup_square_symmetries::<S>(square),
                    opening_scale_factor,
                );
                black_value.eval(
                    indexes.flat_psqt_middlegame,
                    lookup_square_symmetries::<S>(square),
                    middlegame_scale_factor,
                );
                black_value.eval(
                    indexes.flat_psqt_endgame,
                    lookup_square_symmetries::<S>(square),
                    endgame_scale_factor,
                );
                black_flat_count += 1;
            }
            WhiteWall => {
                white_value.eval(
                    indexes.wall_psqt_opening,
                    lookup_square_symmetries::<S>(square),
                    opening_scale_factor,
                );
                white_value.eval(
                    indexes.wall_psqt_middlegame,
                    lookup_square_symmetries::<S>(square),
                    middlegame_scale_factor,
                );
                white_value.eval(
                    indexes.wall_psqt_endgame,
                    lookup_square_symmetries::<S>(square),
                    endgame_scale_factor,
                )
            }
            BlackWall => {
                black_value.eval(
                    indexes.wall_psqt_opening,
                    lookup_square_symmetries::<S>(square),
                    opening_scale_factor,
                );
                black_value.eval(
                    indexes.wall_psqt_middlegame,
                    lookup_square_symmetries::<S>(square),
                    middlegame_scale_factor,
                );
                black_value.eval(
                    indexes.wall_psqt_endgame,
                    lookup_square_symmetries::<S>(square),
                    endgame_scale_factor,
                )
            }
            WhiteCap => {
                white_value.eval(
                    indexes.cap_psqt_opening,
                    lookup_square_symmetries::<S>(square),
                    opening_scale_factor,
                );
                white_value.eval(
                    indexes.cap_psqt_middlegame,
                    lookup_square_symmetries::<S>(square),
                    middlegame_scale_factor,
                );
                white_value.eval(
                    indexes.cap_psqt_endgame,
                    lookup_square_symmetries::<S>(square),
                    endgame_scale_factor,
                );
                cap_activity::<WhiteTr, BlackTr, V, S>(position, square, white_value);
            }
            BlackCap => {
                black_value.eval(
                    indexes.cap_psqt_opening,
                    lookup_square_symmetries::<S>(square),
                    opening_scale_factor,
                );
                black_value.eval(
                    indexes.cap_psqt_middlegame,
                    lookup_square_symmetries::<S>(square),
                    middlegame_scale_factor,
                );
                black_value.eval(
                    indexes.cap_psqt_endgame,
                    lookup_square_symmetries::<S>(square),
                    endgame_scale_factor,
                );
                cap_activity::<BlackTr, WhiteTr, V, S>(position, square, black_value);
            }
        }
        if stack.height < 2 {
            continue;
        }
        let controlling_player = piece.color();

        // Count the number of squares that can be reached by a spread
        let mut num_reachable_squares = 0;
        for direction in square.directions() {
            let mut steps_in_direction = 0;
            let mut dest_square = square;
            while let Some(sq) = dest_square.go_direction(direction) {
                steps_in_direction += 1;
                if steps_in_direction > stack.height {
                    break;
                }
                dest_square = sq;

                let dest_role = position.top_stones()[dest_square].map(Piece::role);
                if dest_role == Some(Cap) {
                    break;
                }
                if dest_role == Some(Wall) {
                    // A wall square can be reached by squashing, but that still ends the spread
                    if piece.role() == Cap {
                        num_reachable_squares += 1;
                    }
                    break;
                }
                num_reachable_squares += 1;
            }
        }

        let top_role_index = match piece.role() {
            Flat => 0,
            Wall => 1,
            Cap if stack.get(stack.height - 2).unwrap().color() == controlling_player => 2,
            Cap => 3,
        };
        let data = stack_data[square];
        let shallow_pieces = (data.shallow_supports + data.shallow_captives + 1) as f32;
        let value_for_stack = match controlling_player {
            Color::White => &mut *white_value,
            Color::Black => &mut *black_value,
        };

        value_for_stack.eval(
            indexes.deep_supports_per_piece,
            top_role_index,
            data.deep_supports.into(),
        );
        value_for_stack.eval(
            indexes.shallow_supports_per_piece,
            top_role_index,
            data.shallow_supports.into(),
        );
        value_for_stack.eval(
            indexes.shallow_supports_per_piece_mobility,
            top_role_index,
            f16::from_f32((data.shallow_supports * num_reachable_squares) as f32 / 16.0), // Arbitrarily divide by 16, to avoid passing too large value to the tuner
        );
        value_for_stack.eval(
            indexes.shallow_supports_per_piece_mob_scaled,
            top_role_index,
            f16::from_f32((data.shallow_supports * num_reachable_squares) as f32 / shallow_pieces),
        );

        value_for_stack.eval(
            indexes.deep_captives_per_piece,
            top_role_index,
            data.deep_captives.into(),
        );
        value_for_stack.eval(
            indexes.shallow_captives_per_piece,
            top_role_index,
            data.shallow_captives.into(),
        );
        value_for_stack.eval(
            indexes.shallow_captives_per_piece_mobility,
            top_role_index,
            f16::from_f32((data.shallow_captives * num_reachable_squares) as f32 / 16.0), // Arbitrarily divide by 16, to avoid passing too large value to the tuner
        );
        value_for_stack.eval(
            indexes.shallow_captives_per_piece_mob_scaled,
            top_role_index,
            f16::from_f32((data.shallow_captives * num_reachable_squares) as f32 / shallow_pieces),
        );

        value_for_stack.eval(
            indexes.supports_psqt_opening,
            lookup_square_symmetries::<S>(square),
            f16::from(data.deep_supports + data.shallow_supports) * opening_scale_factor,
        );
        value_for_stack.eval(
            indexes.supports_psqt_middlegame,
            lookup_square_symmetries::<S>(square),
            f16::from(data.deep_supports + data.shallow_supports) * middlegame_scale_factor,
        );
        value_for_stack.eval(
            indexes.supports_psqt_endgame,
            lookup_square_symmetries::<S>(square),
            f16::from(data.deep_supports + data.shallow_supports) * endgame_scale_factor,
        );

        value_for_stack.eval(
            indexes.captives_psqt_opening,
            lookup_square_symmetries::<S>(square),
            f16::from(-(data.deep_captives as i8) - data.shallow_captives as i8)
                * opening_scale_factor,
        );
        value_for_stack.eval(
            indexes.captives_psqt_middlegame,
            lookup_square_symmetries::<S>(square),
            f16::from(-(data.deep_captives as i8) - data.shallow_captives as i8)
                * middlegame_scale_factor,
        );
        value_for_stack.eval(
            indexes.captives_psqt_endgame,
            lookup_square_symmetries::<S>(square),
            f16::from(-(data.deep_captives as i8) - data.shallow_captives as i8)
                * endgame_scale_factor,
        );

        // Check if a continuously pure spread can create a road
        let edge_connection = group_data.amount_in_group[group_data.groups[square] as usize].1;
        for (is_connected, direction) in [
            (edge_connection.is_connected_west(), Direction::East),
            (edge_connection.is_connected_north(), Direction::South),
            (edge_connection.is_connected_east(), Direction::West),
            (edge_connection.is_connected_south(), Direction::North),
        ] {
            if piece.role() == Wall || !is_connected {
                continue;
            }
            let Some(destination) = pure_winning_spread_to(
                position,
                group_data,
                square,
                stack,
                data.shallow_supports,
                direction,
            ) else {
                continue;
            };
            let index = (square.rank().abs_diff(destination.rank())
                + square.file().abs_diff(destination.file())) as usize
                - 1;
            match (controlling_player == position.side_to_move(), piece.role()) {
                (true, _) => {
                    value_for_stack.eval(indexes.winning_spread_to_move, index.min(1), f16::ONE)
                }
                (false, Flat) => value_for_stack.eval(
                    indexes.winning_flat_spread_not_to_move,
                    index.min(1),
                    f16::ONE,
                ),
                (false, Cap) => value_for_stack.eval(
                    indexes.winning_cap_spread_not_to_move,
                    index.min(1),
                    f16::ONE,
                ),
                (false, Wall) => unreachable!(),
            }
        }
    }

    // Bonus/malus depending on the number of groups each side has
    let mut seen_groups: ArrayVec<bool, 257> = ArrayVec::new();
    seen_groups.push(true);
    for _ in 1..S * S + 1 {
        seen_groups.push(false);
    }

    let mut num_white_groups = 0;
    let mut num_black_groups = 0;
    for square in squares_iterator::<S>() {
        let group_id = group_data.groups[square] as usize;
        if !seen_groups[group_id] {
            seen_groups[group_id] = true;
            match position.top_stones()[square].unwrap().color() {
                Color::White => num_white_groups += 1,
                Color::Black => num_black_groups += 1,
            }
        }
    }

    // Give the side to move a bonus/malus depending on flatstone lead
    let white_flatstone_lead = white_flat_count - black_flat_count;
    let black_flatstone_lead = black_flat_count - white_flat_count;

    if position.side_to_move() == Color::White {
        let index = (white_flatstone_lead + 4).clamp(0, 8) as usize;
        white_value.eval(
            indexes.to_move_opening_flatstone_lead,
            index,
            opening_scale_factor,
        );
        white_value.eval(
            indexes.to_move_middlegame_flatstone_lead,
            index,
            middlegame_scale_factor,
        );
        white_value.eval(
            indexes.to_move_endgame_flatstone_lead,
            index,
            endgame_scale_factor,
        );
    } else {
        let index = (black_flatstone_lead + 4).clamp(0, 8) as usize;
        black_value.eval(
            indexes.to_move_opening_flatstone_lead,
            index,
            opening_scale_factor,
        );
        black_value.eval(
            indexes.to_move_middlegame_flatstone_lead,
            index,
            middlegame_scale_factor,
        );
        black_value.eval(
            indexes.to_move_endgame_flatstone_lead,
            index,
            endgame_scale_factor,
        );
    }

    white_value.eval(
        indexes.i_number_of_groups,
        0,
        f16::from_i32(num_white_groups).unwrap() * opening_scale_factor,
    );
    black_value.eval(
        indexes.i_number_of_groups,
        0,
        f16::from_i32(num_black_groups).unwrap() * opening_scale_factor,
    );

    white_value.eval(
        indexes.i_number_of_groups,
        1,
        f16::from_i32(num_white_groups).unwrap() * middlegame_scale_factor,
    );
    black_value.eval(
        indexes.i_number_of_groups,
        1,
        f16::from_i32(num_black_groups).unwrap() * middlegame_scale_factor,
    );

    white_value.eval(
        indexes.i_number_of_groups,
        2,
        f16::from_i32(num_white_groups).unwrap() * endgame_scale_factor,
    );
    black_value.eval(
        indexes.i_number_of_groups,
        2,
        f16::from_i32(num_black_groups).unwrap() * endgame_scale_factor,
    );

    for critical_square in group_data.critical_squares(Color::White) {
        critical_squares_eval::<WhiteTr, BlackTr, V, S>(
            position,
            group_data,
            critical_square,
            white_value,
        );
    }

    for critical_square in group_data.critical_squares(Color::Black) {
        critical_squares_eval::<BlackTr, WhiteTr, V, S>(
            position,
            group_data,
            critical_square,
            black_value,
        );
    }

    // Bonuses for having flat win immediately available
    match position.side_to_move() {
        Color::White => flat_win::<WhiteTr, BlackTr, V, S>(
            position,
            white_flat_count,
            black_flat_count,
            white_value,
            black_value,
        ),
        Color::Black => flat_win::<BlackTr, WhiteTr, V, S>(
            position,
            white_flat_count,
            black_flat_count,
            black_value,
            white_value,
        ),
    }

    for square in group_data.white_flat_stones.into_iter() {
        let stack_height = position.stack_heights()[square];
        if stack_height < 2 {
            continue;
        }
        let neighbors = square.neighbors_bitboard();
        // Malus for them having stones next to our stack with flat stones on top
        white_value.eval(
            indexes.flat_next_to_our_stack,
            0,
            f16::from_u8(stack_height * (neighbors & group_data.black_flat_stones).count())
                .unwrap(),
        );
        white_value.eval(
            indexes.wall_next_to_our_stack,
            0,
            f16::from_u8(stack_height * (neighbors & group_data.black_walls).count()).unwrap(),
        );
        white_value.eval(
            indexes.cap_next_to_our_stack,
            0,
            f16::from_u8(stack_height * (neighbors & group_data.black_caps).count()).unwrap(),
        );
    }

    for square in group_data.black_flat_stones.into_iter() {
        let stack_height = position.stack_heights()[square];
        if stack_height < 2 {
            continue;
        }

        let neighbors = square.neighbors_bitboard();
        // Malus for them having stones next to our stack with flat stones on top
        black_value.eval(
            indexes.flat_next_to_our_stack,
            0,
            f16::from_u8(stack_height * (neighbors & group_data.white_flat_stones).count())
                .unwrap(),
        );
        black_value.eval(
            indexes.wall_next_to_our_stack,
            0,
            f16::from_u8(stack_height * (neighbors & group_data.white_walls).count()).unwrap(),
        );
        black_value.eval(
            indexes.cap_next_to_our_stack,
            0,
            f16::from_u8(stack_height * (neighbors & group_data.white_caps).count()).unwrap(),
        );
    }

    let mut num_ranks_occupied_white = 0;
    let mut num_files_occupied_white = 0;
    let mut num_ranks_occupied_black = 0;
    let mut num_files_occupied_black = 0;

    for i in 0..(S as u8) {
        let rank = BitBoard::full().rank::<S>(i);
        let file = BitBoard::full().file::<S>(i);
        line_score::<WhiteTr, BlackTr, V, S>(position, group_data, rank, i, white_value);
        line_score::<BlackTr, WhiteTr, V, S>(position, group_data, rank, i, black_value);
        line_score::<WhiteTr, BlackTr, V, S>(position, group_data, file, i, white_value);
        line_score::<BlackTr, WhiteTr, V, S>(position, group_data, file, i, black_value);
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

    white_value.eval(
        indexes.num_lines_occupied,
        num_ranks_occupied_white,
        f16::ONE,
    );
    white_value.eval(
        indexes.num_lines_occupied,
        num_files_occupied_white,
        f16::ONE,
    );
    black_value.eval(
        indexes.num_lines_occupied,
        num_ranks_occupied_black,
        f16::ONE,
    );
    black_value.eval(
        indexes.num_lines_occupied,
        num_files_occupied_black,
        f16::ONE,
    );
}

fn flat_win<Us: ColorTr, Them: ColorTr, V: ValueApplier, const S: usize>(
    position: &Position<S>,
    white_flat_count: i8,
    black_flat_count: i8,
    our_value: &mut V,
    their_value: &mut V,
) {
    let indexes = value_indexes::<S>();

    let white_flats_needed_for_win =
        1 + (black_flat_count + position.komi().half_komi() / 2) - white_flat_count;
    let black_flats_needed_for_win =
        1 + white_flat_count - (black_flat_count + position.komi().half_komi() / 2);

    let (our_flats_needed_for_win, their_flats_needed_for_win) = match Us::color() {
        Color::White => (white_flats_needed_for_win, black_flats_needed_for_win),
        Color::Black => (black_flats_needed_for_win, white_flats_needed_for_win),
    };

    if Us::stones_left(position) == 1 {
        if our_flats_needed_for_win <= 1 {
            // Bonus if we have an immediate flat win
            our_value.eval(indexes.flat_win_this_ply, 0, f16::ONE)
        } else if Them::stones_left(position) > 2 {
            // General malus for having 1 reserve left, but being behind on flats
            // Exclude the case where they're also close to flatting out,
            // since it's covered by other features below
            our_value.eval(
                indexes.one_reserve_left_us,
                our_flats_needed_for_win.min(5) as usize - 2,
                f16::ONE,
            )
        }
    }

    // Bonus if we have a flat lead, and two reserves left
    if Us::stones_left(position) == 2 {
        if our_flats_needed_for_win <= 0 {
            // Opponent must play a +3 fcd move next move, to stop gaelet
            our_value.eval(indexes.flat_win_two_ply, 1, f16::ONE)
        } else if our_flats_needed_for_win <= 1 {
            // A +2 fcd response is required
            our_value.eval(indexes.flat_win_two_ply, 0, f16::ONE)
        }
    }

    // Malus if they're threatening a flat win on the next ply
    if Them::stones_left(position) == 1 {
        // Extra bonus if they win despite a +2 fcd move from us
        if their_flats_needed_for_win < 0 {
            their_value.eval(indexes.flat_win_next_ply, 1, f16::ONE)
        } else if their_flats_needed_for_win < 1 {
            their_value.eval(indexes.flat_win_next_ply, 0, f16::ONE)
        } else if Us::stones_left(position) > 2 {
            // General malus for having 1 reserve left, but being behind on flats
            // Exclude the case where we are also close to flatting out,
            // since it's covered by other features above our_value.eval(
            their_value.eval(
                indexes.one_reserve_left_them,
                their_flats_needed_for_win.min(4) as usize - 1,
                f16::ONE,
            )
        }
    }

    // Malus if they're threatening a flat win on their next move
    if Them::stones_left(position) == 2 {
        if their_flats_needed_for_win < -1 {
            // Extra bonus if they win despite +4 total fcd moves from us
            their_value.eval(indexes.flat_win_three_ply, 2, f16::ONE)
        } else if their_flats_needed_for_win < 0 {
            // despite +3 fcd on our moves
            their_value.eval(indexes.flat_win_three_ply, 1, f16::ONE)
        } else if their_flats_needed_for_win < 1 {
            // despite +2 fcd is needed
            their_value.eval(indexes.flat_win_three_ply, 0, f16::ONE)
        }
    }
}

fn cap_activity<Us: ColorTr, Them: ColorTr, V: ValueApplier, const S: usize>(
    position: &Position<S>,
    square: Square<S>,
    our_value: &mut V,
) {
    let indexes = value_indexes::<S>();

    let stack = position.get_stack(square);
    let height_index = stack.height.min(3) as usize - 1;

    // Malus if our capstone's line towards the center is blocked
    if square.neighbors().any(|neighbour| {
        lookup_square_symmetries::<S>(neighbour) > lookup_square_symmetries::<S>(square)
            && position.top_stones()[neighbour].map(Piece::role) == Some(Cap)
    }) {
        our_value.eval(indexes.sidelined_cap, height_index, f16::ONE)
    }

    let is_soft_cap = stack
        .get(stack.height.overflowing_sub(2).0)
        .map(Them::is_our_piece)
        == Some(true);
    if square.neighbors().all(|neighbour| {
        matches!(
            position.top_stones()[neighbour],
            Some(WhiteCap) | Some(BlackCap) | None
        )
    }) {
        our_value.eval(indexes.fully_isolated_cap, height_index, f16::ONE)
    } else if square.neighbors().all(|neighbour| {
        if let Some(neighbour_top_stone) = position.top_stones()[neighbour] {
            if neighbour_top_stone == Them::wall_piece() {
                is_soft_cap
            } else {
                neighbour_top_stone != Them::flat_piece()
            }
        } else {
            true
        }
    }) {
        our_value.eval(indexes.semi_isolated_cap, height_index, f16::ONE)
    }
}

fn pure_winning_spread_to<const S: usize>(
    position: &Position<S>,
    group_data: &GroupData<S>,
    square: Square<S>,
    stack: Stack,
    num_shallow_supports: u8,
    direction: Direction,
) -> Option<Square<S>> {
    let mut edge_connection = group_data.amount_in_group[group_data.groups[square] as usize].1;

    let controlling_player = stack.top_stone.unwrap().color();
    let is_hard_cap = stack.top_stone.unwrap().role() == Cap
        && stack.get(stack.height - 2).unwrap().color() == controlling_player;

    let mut destination = square;
    for _ in 0..num_shallow_supports {
        let Some(dest) = destination.go_direction(direction) else {
            break;
        };
        let top_stone = position.top_stones()[dest];
        // The spread is blocked by a capstone or a wall, unless we're a hard cap
        if top_stone
            .is_some_and(|piece| piece.role() == Cap || (piece.role() == Wall && !is_hard_cap))
        {
            break;
        }
        destination = dest;
        edge_connection |= destination.group_edge_connection();
        for orth_dir in direction.orthogonal_directions() {
            if let Some(sq) = destination.go_direction(orth_dir) {
                if position.top_stones()[sq].is_some_and(|piece| {
                    piece.is_road_piece() && piece.color() == controlling_player
                }) {
                    edge_connection |= group_data.amount_in_group[group_data.groups[sq] as usize].1;
                }
            }
        }
        if edge_connection.is_winning() {
            break;
        }
        // Smashing a wall ends the spread
        if top_stone.is_some_and(|piece| piece.role() == Wall) {
            break;
        }
    }
    // Connect the square in front of us, if it's ours
    if let Some(sq) = destination.go_direction(direction) {
        if position.top_stones()[sq]
            .is_some_and(|piece| piece.is_road_piece() && piece.color() == controlling_player)
        {
            edge_connection |= group_data.amount_in_group[group_data.groups[sq] as usize].1;
        }
    }

    edge_connection.is_winning().then_some(destination)
}

/// Give bonus for our critical squares
fn critical_squares_eval<Us: ColorTr, Them: ColorTr, V: ValueApplier, const S: usize>(
    position: &Position<S>,
    group_data: &GroupData<S>,
    critical_square: Square<S>,
    our_value: &mut V,
) {
    let indexes = value_indexes::<S>();

    let top_stone = position.top_stones()[critical_square];
    let top_stone_role = top_stone.map(Piece::role);
    if top_stone.is_none() {
        our_value.eval(indexes.critical_squares, 0, f16::ONE);
    } else if top_stone == Some(Us::wall_piece()) {
        our_value.eval(indexes.critical_squares, 1, f16::ONE);
    } else if top_stone == Some(Them::flat_piece()) {
        our_value.eval(indexes.critical_squares, 2, f16::ONE);
    }
    // Their capstone or wall
    else {
        our_value.eval(indexes.critical_squares, 3, f16::ONE)
    }

    let rank = critical_square.rank();
    let file = critical_square.file();

    let capstone_square_in_line = {
        let capstone_in_rank = BitBoard::full().rank::<S>(rank) & Us::caps(group_data);
        let capstone_in_file = BitBoard::full().file::<S>(file) & Us::caps(group_data);
        capstone_in_rank
            .occupied_square()
            .or(capstone_in_file.occupied_square())
    };

    // Bonuses when our capstone can spread to the critical square
    // TODO: Don't give bonuses if walls/caps block the spread
    if let Some(capstone_square) = capstone_square_in_line {
        let distance =
            file.abs_diff(capstone_square.file()) + rank.abs_diff(capstone_square.rank());
        let cap_stack = position.get_stack(capstone_square);
        let is_hard_cap = cap_stack
            .get(cap_stack.len().saturating_sub(2))
            .is_some_and(Us::is_our_piece);
        let num_high_supports = cap_stack
            .into_iter()
            .skip((cap_stack.len() as usize).saturating_sub(S + 1))
            .filter(|piece| Us::is_our_piece(*piece))
            .count() as u8
            - 1;
        if top_stone_role != Some(Cap) && distance <= cap_stack.len() {
            let has_pure_spread =
                distance <= num_high_supports && (top_stone_role != Some(Wall) || is_hard_cap);
            if has_pure_spread {
                if position.side_to_move() == Us::color() {
                    our_value.eval(indexes.critical_square_cap_attack, 0, f16::ONE);
                } else {
                    our_value.eval(indexes.critical_square_cap_attack, 1, f16::ONE);
                }
            } else if position.side_to_move() == Us::color() {
                our_value.eval(indexes.critical_square_cap_attack, 2, f16::ONE);
            } else {
                our_value.eval(indexes.critical_square_cap_attack, 3, f16::ONE);
            }
        }
        if distance == 1 && top_stone_role != Some(Cap) {
            our_value.eval(indexes.critical_square_cap_attack, 4, f16::ONE);
            our_value.eval(
                indexes.critical_square_cap_attack,
                5,
                f16::from_u8(num_high_supports).unwrap(),
            );
        }
    }
}

#[inline(always)] // Force-inlining gives a 1.5% performance boost
fn line_score<Us: ColorTr, Them: ColorTr, V: ValueApplier, const S: usize>(
    position: &Position<S>,
    group_data: &GroupData<S>,
    line: BitBoard,
    i: u8,
    value: &mut V,
) {
    let indexes = value_indexes::<S>();

    let road_pieces_in_line = (Us::road_stones(group_data) & line).count() as usize;
    let index = road_pieces_in_line + line_symmetries::<S>()[i as usize] * S;

    if !(Them::blocking_stones(group_data) & line).is_empty() {
        value.eval(indexes.line_control_their_blocking_piece, index, f16::ONE);
    } else if !((Us::walls(group_data) | Them::flats(group_data)) & line).is_empty() {
        // Specific bonus for strong lines at the edge of the board,
        // if they have a flat in our line that is flanked by another one of their pieces
        if road_pieces_in_line >= S - 3 && (i == 0 || i == S as u8 - 1) {
            let mut guarded = false;
            // TODO: Also check protection for critical squares?
            for square in (Them::flats(group_data) & line).into_iter() {
                let (direction, neighbor) = square
                    .direction_neighbors()
                    .find(|(_, neigh)| !line.get_square(*neigh))
                    .unwrap();
                if let Some(neighbor_piece) = position.top_stones()[neighbor].filter(|piece| {
                    Them::is_our_piece(*piece)
                        && !direction
                            .orthogonal_directions()
                            .into_iter()
                            .flat_map(|dir| neighbor.go_direction(dir))
                            .all(|sq| {
                                position.top_stones()[sq].is_some_and(|p| Us::is_our_road_piece(p))
                            })
                }) {
                    let i = index + 3 - S;
                    match neighbor_piece.role() {
                        Flat => value.eval(indexes.line_control_guarded_flat, i, f16::ONE),
                        Wall => value.eval(indexes.line_control_guarded_wall, i, f16::ONE),
                        Cap => value.eval(indexes.line_control_guarded_cap, i, f16::ONE),
                    }
                    guarded = true;
                }
            }
            // The guarded bonus can be applied several times, but if it's applied at least once,
            // skip the regular `line_control_other` value
            if guarded {
                return;
            }
        }
        value.eval(indexes.line_control_other, index, f16::ONE);
    } else {
        value.eval(indexes.line_control_empty, index, f16::ONE);
    }
}

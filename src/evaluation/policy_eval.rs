use std::{array, sync};

use arrayvec::ArrayVec;
use board_game_traits::{Color, GameResult, Position as PositionTrait};
use half::f16;

use crate::position::bitboard::BitBoard;
use crate::position::color_trait::{BlackTr, ColorTr, WhiteTr};
use crate::position::Role::{Cap, Flat, Wall};
use crate::position::{
    lookup_square_symmetries, GroupData, MovementSynopsis, Piece, Position, Role,
};
use crate::position::{squares_iterator, Move};
use crate::position::{AbstractBoard, Direction};
use crate::position::{Direction::*, ExpMove};
use crate::position::{GroupEdgeConnection, Square};

use super::parameters::{policy_indexes, PolicyApplier};

const POLICY_BASELINE: f32 = 0.05;

static POLICY_OFFSET: sync::OnceLock<[f32; 512]> = sync::OnceLock::new();

/// Memoize policy offset for low move numbers, to avoid expensive floating-point operations
/// Gives a roughly 10% speedup
pub fn policy_offset(num_moves: usize) -> f32 {
    if num_moves >= 512 {
        inverse_sigmoid(1.0 / (num_moves + 1) as f32)
    } else {
        POLICY_OFFSET.get_or_init(|| {
            array::from_fn(|i| {
                if i == 0 {
                    // This is a dummy value, having 0 legal moves is impossible
                    0.0
                } else {
                    inverse_sigmoid(1.0 / (i + 1) as f32)
                }
            })
        })[num_moves]
    }
}

pub fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + f32::exp(-x))
}

fn inverse_sigmoid(x: f32) -> f32 {
    assert!(x > 0.0 && x < 1.0, "Tried to inverse sigmoid {}", x);
    f32::ln(x / (1.0 - x))
}

impl<const S: usize> Position<S> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn generate_moves_with_probabilities_colortr<
        Us: ColorTr,
        Them: ColorTr,
        P: PolicyApplier,
    >(
        &self,
        params_for_color: &'static [f32],
        group_data: &GroupData<S>,
        simple_moves: &mut Vec<Move<S>>,
        fcd_per_move: &mut Vec<i8>,
        moves: &mut Vec<(Move<S>, f16)>,
        policy_feature_sets: &mut Vec<P>,
    ) {
        let num_moves = simple_moves.len();

        while policy_feature_sets.len() < num_moves {
            policy_feature_sets.push(P::new(params_for_color));
        }

        self.features_for_moves(policy_feature_sets, simple_moves, fcd_per_move, group_data);

        moves.extend(
            simple_moves
                .drain(..)
                .zip(policy_feature_sets)
                .map(|(mv, features)| {
                    let eval = features.finish(num_moves);

                    (mv, eval)
                }),
        );

        fcd_per_move.clear();

        let score_sum: f32 = moves.iter().map(|(_mv, score)| score.to_f32()).sum();

        let score_factor = (1.0 - POLICY_BASELINE) / score_sum;
        for (_mv, score) in moves.iter_mut() {
            *score =
                f16::from_f32(score.to_f32() * score_factor + (POLICY_BASELINE / num_moves as f32));
        }
    }

    pub fn features_for_moves<P: PolicyApplier>(
        &self,
        policies: &mut [P],
        moves: &[Move<S>],
        fcd_per_move: &mut Vec<i8>,
        group_data: &GroupData<S>,
    ) {
        let indexes = policy_indexes::<S>();
        assert!(
            policies.len() >= moves.len(),
            "Got {} policies for {} moves",
            policies.len(),
            moves.len()
        );

        let mut immediate_win_exists = false;

        let mut highest_fcd_per_square = <AbstractBoard<i8, S>>::new_with_value(-1);
        let mut highest_fcd = -1;

        for mv in moves.iter() {
            let fcd = self.fcd_for_move(*mv);
            if fcd > highest_fcd {
                highest_fcd = fcd;
            }
            if fcd > highest_fcd_per_square[mv.origin_square()] {
                highest_fcd_per_square[mv.origin_square()] = fcd;
            }
            fcd_per_move.push(fcd);
        }

        for (policy, (mv, &mut fcd)) in policies.iter_mut().zip(moves.iter().zip(fcd_per_move)) {
            self.features_for_move(policy, mv, fcd, group_data);

            // FCD bonus for all movements
            if let ExpMove::Move(square, _, _) = mv.expand() {
                if fcd >= highest_fcd {
                    policy.eval_one(indexes.fcd_highest_board, fcd.clamp(1, 6) as usize - 1);
                } else if fcd >= highest_fcd_per_square[square] {
                    policy.eval_one(indexes.fcd_highest_stack, (fcd.clamp(-1, 4) + 1) as usize);
                } else {
                    policy.eval_one(indexes.fcd_other, (fcd.clamp(-3, 4) + 3) as usize);
                }
            }

            if policy.has_immediate_win() {
                immediate_win_exists = true;
            }
        }
        if immediate_win_exists {
            for policy in policies.iter_mut().take(moves.len()) {
                if !policy.has_immediate_win() {
                    policy.eval_one(indexes.decline_win, 0)
                }
            }
        }
    }

    fn features_for_move<P: PolicyApplier>(
        &self,
        policy: &mut P,
        mv: &Move<S>,
        fcd: i8,
        group_data: &GroupData<S>,
    ) {
        match self.side_to_move() {
            Color::White => features_for_move_colortr::<WhiteTr, BlackTr, P, S>(
                self, policy, mv, fcd, group_data,
            ),
            Color::Black => features_for_move_colortr::<BlackTr, WhiteTr, P, S>(
                self, policy, mv, fcd, group_data,
            ),
        }
    }
}

fn our_last_placement<const S: usize>(position: &Position<S>) -> Option<(Role, Square<S>)> {
    position
        .moves()
        .get(position.moves().len().overflowing_sub(2).0)
        .and_then(|mv| match mv.expand() {
            ExpMove::Place(role, square) => Some((role, square)),
            ExpMove::Move(_, _, _) => None,
        })
}

fn their_last_placement<const S: usize>(position: &Position<S>) -> Option<(Role, Square<S>)> {
    position
        .moves()
        .get(position.moves().len().overflowing_sub(1).0)
        .and_then(|mv| match mv.expand() {
            ExpMove::Place(role, square) => Some((role, square)),
            ExpMove::Move(_, _, _) => None,
        })
}

fn features_for_move_colortr<Us: ColorTr, Them: ColorTr, P: PolicyApplier, const S: usize>(
    position: &Position<S>,
    policy: &mut P,
    mv: &Move<S>,
    fcd: i8,
    group_data: &GroupData<S>,
) {
    let indexes = policy_indexes::<S>();
    // If it's the first move, give every move equal probability
    if position.half_moves_played() < 2 {
        return;
    }

    let our_flatcount = Us::flats(group_data).count() as i8;
    let their_flatcount = Them::flats(group_data).count() as i8;

    let our_flatcount_after_move = our_flatcount + fcd;

    match mv.expand() {
        ExpMove::Place(role, square) => {
            let our_flat_lead_after_move = our_flatcount_after_move - their_flatcount;

            // Apply special bonuses if the game ends on this move
            if Us::stones_left(position) + Us::caps_left(position) == 1
                || group_data.all_pieces().count() as usize == S * S - 1
            {
                check_flat_win::<Us, P, S>(
                    position,
                    our_flatcount_after_move,
                    their_flatcount,
                    policy,
                );
            }
            // Bonuses if our opponent can finish on flats next turn
            else if Them::stones_left(position) + Them::caps_left(position) == 1
                || group_data.all_pieces().count() as usize == S * S - 2
            {
                check_flat_win_next_move::<Us, P, S>(
                    position,
                    our_flatcount_after_move,
                    their_flatcount,
                    policy,
                );
            }
            // TODO: These two bonuses don't take komi into account, but they should
            else if Us::stones_left(position) == 2 && Us::caps_left(position) == 0 {
                policy.eval_one(indexes.two_flats_left, 0);
                policy.eval_i8(indexes.two_flats_left, 1, our_flat_lead_after_move);
            } else if Us::stones_left(position) == 3 && Us::caps_left(position) == 0 {
                policy.eval_one(indexes.three_flats_left, 0);
                policy.eval_i8(indexes.three_flats_left, 1, our_flat_lead_after_move);
            }

            let their_open_critical_squares =
                Them::critical_squares(group_data) & (!group_data.all_pieces());

            // Apply PSQT
            match (role, position.side_to_move()) {
                (Flat, Color::White) => policy.eval_one(
                    indexes.flat_psqt_white,
                    lookup_square_symmetries::<S>(square),
                ),
                (Flat, Color::Black) => policy.eval_one(
                    indexes.flat_psqt_black,
                    lookup_square_symmetries::<S>(square),
                ),
                (Wall, Color::White) => policy.eval_one(
                    indexes.wall_psqt_white,
                    lookup_square_symmetries::<S>(square),
                ),
                (Wall, Color::Black) => policy.eval_one(
                    indexes.wall_psqt_black,
                    lookup_square_symmetries::<S>(square),
                ),
                (Cap, Color::White) => policy.eval_one(
                    indexes.cap_psqt_white,
                    lookup_square_symmetries::<S>(square),
                ),
                (Cap, Color::Black) => policy.eval_one(
                    indexes.cap_psqt_black,
                    lookup_square_symmetries::<S>(square),
                ),
            }

            let role_id = match role {
                Flat => 0,
                Wall => 1,
                Cap => 2,
            };

            for &line in BitBoard::lines_for_square::<S>(square).iter() {
                let our_line_score = (Us::road_stones(group_data) & line).count();
                let their_line_score = (Them::road_stones(group_data) & line).count();
                policy.eval_one(
                    indexes.our_road_stones_in_line,
                    S * role_id + our_line_score as usize,
                );
                policy.eval_one(
                    indexes.their_road_stones_in_line,
                    S * role_id + their_line_score as usize,
                );
            }

            // If square is next to a group
            let mut our_unique_neighbour_groups: ArrayVec<(Square<S>, u8), 4> = ArrayVec::new();
            let mut their_unique_neighbour_groups: ArrayVec<(Square<S>, u8), 4> = ArrayVec::new();
            for neighbour in square
                .neighbors()
                .filter(|sq| position.stack_heights()[*sq] != 0)
            {
                let neighbour_group_id = group_data.groups[neighbour];
                if Us::piece_is_ours(position.top_stones()[neighbour].unwrap()) {
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
                let total_neighbours_group_size: u8 = our_unique_neighbour_groups
                    .iter()
                    .map(|(_, group_id)| group_data.amount_in_group[*group_id as usize].0)
                    .sum();

                policy.eval_one(indexes.merge_two_groups_base, role_id);
                // Divide by 10, as large values confuse the tuner
                policy.eval_f32(
                    indexes.merge_two_groups_linear,
                    role_id,
                    total_neighbours_group_size as f32 / 10.0,
                );
            }

            if their_unique_neighbour_groups.len() > 1 {
                let total_neighbours_group_size: u8 = their_unique_neighbour_groups
                    .iter()
                    .map(|(_, group_id)| group_data.amount_in_group[*group_id as usize].0)
                    .sum();

                policy.eval_one(indexes.block_merger_base, role_id);
                // Divide by 10, as large values confuse the tuner
                policy.eval_f32(
                    indexes.block_merger_linear,
                    role_id,
                    total_neighbours_group_size as f32 / 10.0,
                );
            }
            if our_unique_neighbour_groups.len() == 1 {
                let group_id = our_unique_neighbour_groups[0].1;
                let amount_in_group = group_data.amount_in_group[group_id as usize].0;

                policy.eval_one(indexes.extend_single_group_base, role_id);
                // Divide by 10, as large values confuse the tuner
                policy.eval_f32(
                    indexes.extend_single_group_linear,
                    role_id,
                    amount_in_group as f32 / 10.0,
                );

                // Apply a separate bonus if the piece expands the group to a new line
                if squares_iterator::<S>()
                    .filter(|sq| group_data.groups[*sq] == group_id)
                    .all(|sq| sq.file() != square.file())
                    || squares_iterator::<S>()
                        .filter(|sq| group_data.groups[*sq] == group_id)
                        .all(|sq| sq.rank() != square.rank())
                {
                    policy.eval_one(indexes.extend_single_group_to_new_line_base, role_id);
                    policy.eval_f32(
                        indexes.extend_single_group_to_new_line_linear,
                        role_id,
                        amount_in_group as f32 / 10.0,
                    );
                }
            }

            if role == Flat || role == Cap {
                if Us::is_critical_square(group_data, square) {
                    policy.eval_one(indexes.place_our_critical_square, 0);
                    policy.set_immediate_win();
                } else if !their_open_critical_squares.is_empty() {
                    if their_open_critical_squares == BitBoard::empty().set_square(square) {
                        policy.eval_one(indexes.place_their_critical_square, 0);
                    } else {
                        policy.eval_one(indexes.ignore_their_critical_square, 0);
                    }
                }

                // If square is next to a road stone laid on our last turn
                if let Some((last_role, last_square)) = our_last_placement(position) {
                    if last_role == Flat || last_role == Cap {
                        if square.neighbors().any(|neigh| neigh == last_square) {
                            policy.eval_one(indexes.next_to_our_last_stone, 0);
                        } else if (square.rank() as i8 - last_square.rank() as i8).abs() == 1
                            && (square.file() as i8 - last_square.file() as i8).abs() == 1
                        {
                            policy.eval_one(indexes.diagonal_to_our_last_stone, 0);
                        }
                    }
                }

                // If square is next to a road stone laid on their last turn
                if let Some((last_role, last_square)) = their_last_placement(position) {
                    if last_role == Flat {
                        if square.neighbors().any(|neigh| neigh == last_square) {
                            policy.eval_one(indexes.next_to_their_last_stone, 0);
                        } else if (square.rank() as i8 - last_square.rank() as i8).abs() == 1
                            && (square.file() as i8 - last_square.file() as i8).abs() == 1
                        {
                            policy.eval_one(indexes.diagonal_to_their_last_stone, 0);
                        }
                    }
                }

                // Bonus for attacking a flatstone in a rank/file where we are strong
                for neighbour in square.neighbors() {
                    if position.top_stones()[neighbour] == Some(Them::flat_piece()) {
                        let our_road_stones = Us::road_stones(group_data)
                            .rank::<S>(neighbour.rank())
                            .count()
                            + Us::road_stones(group_data)
                                .file::<S>(neighbour.file())
                                .count();
                        if our_road_stones >= 2 {
                            policy.eval_i8(
                                indexes.attack_strong_flats,
                                0,
                                our_road_stones as i8 - 1,
                            );
                        }
                    }
                }
            }

            if role == Wall {
                if !their_open_critical_squares.is_empty() {
                    if their_open_critical_squares == BitBoard::empty().set_square(square) {
                        policy.eval_one(indexes.place_their_critical_square, 1);
                    } else {
                        policy.eval_one(indexes.ignore_their_critical_square, 0);
                    }
                }
            } else if role == Cap {
                if Us::is_critical_square(group_data, square) {
                    policy.eval_one(indexes.place_our_critical_square, 0);
                    policy.set_immediate_win();
                } else if !their_open_critical_squares.is_empty() {
                    if their_open_critical_squares == BitBoard::empty().set_square(square) {
                        policy.eval_one(indexes.place_their_critical_square, 2);
                    } else {
                        policy.eval_one(indexes.ignore_their_critical_square, 0);
                    }
                }
            }
            if role == Wall || role == Cap {
                for (direction, neighbour) in square.direction_neighbors() {
                    // If square blocks an extension of two flats
                    if position.top_stones()[neighbour]
                        .map(Them::is_road_stone)
                        .unwrap_or_default()
                        && neighbour
                            .go_direction(direction)
                            .and_then(|sq| position.top_stones()[sq])
                            .map(Them::is_road_stone)
                            .unwrap_or_default()
                    {
                        policy.eval_one(indexes.blocking_stone_blocks_extensions_of_two_flats, 0);
                    }

                    if position.stack_heights()[neighbour] > 2
                        && Them::piece_is_ours(position.top_stones()[neighbour].unwrap())
                    {
                        let stack = position.get_stack(neighbour);
                        let top_stone = stack.top_stone().unwrap();
                        let mut captives: u8 = 0;
                        let mut reserves: u8 = 0;
                        for piece in stack.into_iter().take(stack.len() as usize - 1) {
                            if Us::piece_is_ours(piece) {
                                captives += 1;
                            } else {
                                reserves += 1;
                            }
                        }
                        let index = top_stone.role().disc() * 2;
                        match role {
                            Flat => unreachable!(),
                            Wall => {
                                policy.eval_i8(
                                    indexes.attack_strong_stack_with_wall,
                                    index,
                                    captives as i8,
                                );
                                policy.eval_i8(
                                    indexes.attack_strong_stack_with_wall,
                                    index + 1,
                                    reserves as i8,
                                );
                            }
                            Cap => {
                                policy.eval_i8(
                                    indexes.attack_strong_stack_with_cap,
                                    index,
                                    captives as i8,
                                );
                                policy.eval_i8(
                                    indexes.attack_strong_stack_with_cap,
                                    index + 1,
                                    reserves as i8,
                                );
                            }
                        }

                        if let Some(MovementSynopsis {
                            origin: _,
                            destination,
                        }) = group_data.last_movement()
                        {
                            if neighbour == destination {
                                policy.eval_i8(indexes.attack_last_movement, 0, captives as i8);
                                policy.eval_i8(indexes.attack_last_movement, 1, reserves as i8);
                            }
                        }
                    }
                }
            }

            // Bonus for placing on the square left behind by their movement
            if let Some(MovementSynopsis {
                origin,
                destination: _,
            }) = group_data.last_movement()
            {
                if square == origin {
                    policy.eval_one(indexes.place_last_movement, role_id);
                }
            }
        }

        ExpMove::Move(square, direction, stack_movement) => {
            let role_id = match position.top_stones()[square].unwrap().role() {
                Flat => 0,
                Wall => 1,
                Cap => 2,
            };
            match position.side_to_move() {
                Color::White => policy.eval_one(indexes.move_role_bonus_white, role_id),
                Color::Black => policy.eval_one(indexes.move_role_bonus_black, role_id),
            }

            if stack_movement.len() == 1
                && stack_movement.get_first().pieces_to_take == 1
                && position.stack_heights()[square] == 1
            {
                if let Some(piece) = position.top_stones()[square.go_direction(direction).unwrap()]
                {
                    match (piece.role(), piece.color() == Us::color()) {
                        (Flat, true) => policy.eval_one(indexes.simple_self_capture, role_id),
                        (Flat, false) => policy.eval_one(indexes.simple_capture, role_id),
                        (Wall, true) => policy.eval_one(indexes.simple_self_capture, 3),
                        (Wall, false) => policy.eval_one(indexes.simple_capture, 3),
                        _ => unreachable!(),
                    }
                } else {
                    policy.eval_one(indexes.simple_movement, role_id);
                }
            }

            let mut destination_square =
                if stack_movement.get_first().pieces_to_take == position.stack_heights()[square] {
                    square.go_direction(direction).unwrap()
                } else {
                    square
                };

            // Bonus for moving the piece we placed on our last turn
            if let Some((role, last_square)) = our_last_placement(position) {
                if square == last_square && position.stack_heights()[destination_square] != 0 {
                    policy.eval_one(indexes.move_last_placement, role.disc());
                }
            }

            let mut captures_our_critical_square = None;
            let mut captures_their_critical_square = None;
            let mut loses_their_critical_square = None;

            // The groups that become connected through this move
            let mut our_groups_joined = <ArrayVec<u8, 10>>::new();
            let mut their_piece_left_on_previous_square = false;
            // Edge connections created by this move
            let mut group_edge_connection = GroupEdgeConnection::default();

            // The groups where the move causes us to lose flats
            let mut our_groups_affected = <ArrayVec<u8, S>>::new();
            let mut our_squares_affected = <ArrayVec<Square<S>, S>>::new();
            let mut stack_recaptured_with = None;

            // Number of squares given to them
            let mut their_pieces = 0;
            // Number of squares captured by us, that were previously held by them
            let mut their_pieces_captured = 0;
            let mut num_squares_covered = group_data.all_pieces().count();

            // Special case for when we spread the whole stack
            if position.stack_heights()[square] == stack_movement.get_first().pieces_to_take {
                num_squares_covered -= 1;
                let top_stone: Piece = position.top_stones()[square].unwrap();
                if top_stone.is_road_piece() {
                    our_squares_affected.push(square);

                    if spread_damages_our_group::<S, Us>(position, square, direction) {
                        our_groups_affected.push(group_data.groups[square]);
                    }
                }
            }

            // This iterator skips the first square if we move the whole stack
            for piece in position
                .top_stones_left_behind_by_move(square, &stack_movement)
                .flatten()
            {
                if Us::piece_is_ours(piece) {
                    if Us::is_critical_square(group_data, destination_square)
                        && piece.is_road_piece()
                    {
                        captures_our_critical_square = Some(destination_square);
                    }
                    if Them::is_critical_square(group_data, destination_square) {
                        captures_their_critical_square = Some(destination_square);
                    }
                    if let Some(MovementSynopsis {
                        origin: _,
                        destination: last_capture,
                    }) = group_data.last_movement()
                    {
                        if destination_square == last_capture {
                            stack_recaptured_with = Some(piece.role());
                        }
                    }
                } else {
                    their_pieces += 1;
                    if Them::is_critical_square(group_data, destination_square) {
                        // TODO: Filling their critical square needs a malus
                        loses_their_critical_square = Some(destination_square);
                    }
                }

                if Us::piece_is_ours(piece) && piece.is_road_piece() {
                    let mut neighbour_group_ids = <ArrayVec<u8, S>>::new();

                    for neighbour in Square::neighbors(destination_square) {
                        if destination_square != square
                            && destination_square.go_direction(direction.reverse())
                                == Some(neighbour)
                        {
                            continue;
                        }
                        if let Some(neighbour_piece) = position.top_stones()[neighbour] {
                            if Us::piece_is_ours(neighbour_piece) && neighbour_piece.is_road_piece()
                            {
                                neighbour_group_ids.push(group_data.groups[neighbour]);
                            }
                        }
                    }

                    // If our stack spread doesn't form one continuous group,
                    // "disconnect" from previous groups
                    if their_piece_left_on_previous_square
                        && our_groups_joined
                            .iter()
                            .all(|g| !neighbour_group_ids.contains(g))
                    {
                        our_groups_joined.clear();
                        group_edge_connection = GroupEdgeConnection::default();
                    }
                    group_edge_connection =
                        group_edge_connection | destination_square.group_edge_connection();

                    for group_id in neighbour_group_ids {
                        if !our_groups_joined.contains(&group_id) {
                            our_groups_joined.push(group_id);
                        }
                    }
                    their_piece_left_on_previous_square = false;
                } else {
                    their_piece_left_on_previous_square = true;
                    // We may have joined this group on the previous iteration
                    // If so, remove it, since the group is now affected
                    our_groups_joined.retain(|id| *id != group_data.groups[destination_square]);
                }

                // Bonus for moving our cap to a strong line
                // Extra bonus if it lands next to our critical square
                if piece == Us::cap_piece() {
                    let destination_line = match direction {
                        North => Us::road_stones(group_data).rank::<S>(destination_square.rank()),
                        West => Us::road_stones(group_data).file::<S>(destination_square.file()),
                        East => Us::road_stones(group_data).file::<S>(destination_square.file()),
                        South => Us::road_stones(group_data).rank::<S>(destination_square.rank()),
                    };
                    let road_piece_count = destination_line.count() as usize;
                    if road_piece_count > 2 {
                        policy.eval_one(indexes.move_cap_onto_strong_line, road_piece_count - 3);
                        if destination_square
                            .neighbors()
                            .any(|n| Us::is_critical_square(group_data, n))
                        {
                            policy.eval_one(
                                indexes.move_cap_onto_strong_line_with_critical_square,
                                road_piece_count - 3,
                            );
                        }
                    }
                }

                let destination_top_stone = position.top_stones()[destination_square];

                if let Some(destination_top_stone) = destination_top_stone {
                    // When a stack gets captured, give a linear bonus or malus depending on
                    // whether it's captured by us or them
                    let destination_stack_height = position.stack_heights()[destination_square];
                    if piece.color() != destination_top_stone.color() {
                        if Us::piece_is_ours(piece) {
                            policy.eval_i8(
                                indexes.stack_captured_by_movement,
                                0,
                                destination_stack_height as i8,
                            );
                            their_pieces_captured += 1;
                        } else {
                            policy.eval_i8(
                                indexes.stack_captured_by_movement,
                                0,
                                -(destination_stack_height as i8),
                            );
                            our_squares_affected.push(destination_square);

                            if destination_square != square
                                || spread_damages_our_group::<S, Us>(
                                    position,
                                    destination_square,
                                    direction,
                                )
                            {
                                our_groups_affected.push(group_data.groups[destination_square]);
                            }
                        }
                    }
                    if Us::piece_is_ours(destination_top_stone) && piece.role() == Wall {
                        our_squares_affected.push(destination_square);
                        our_groups_affected.push(group_data.groups[destination_square]);
                    }

                    for &line in BitBoard::lines_for_square::<S>(destination_square).iter() {
                        let our_road_stones = (line & Us::road_stones(group_data)).count() as usize;
                        let color_factor: i8 = if Us::piece_is_ours(piece) { 1 } else { -1 };
                        if our_road_stones > 2 {
                            if piece.role() == Cap {
                                policy.eval_i8(
                                    indexes.stack_capture_in_strong_line_cap,
                                    our_road_stones - 3,
                                    color_factor * destination_stack_height as i8,
                                );
                            } else {
                                policy.eval_i8(
                                    indexes.stack_capture_in_strong_line,
                                    our_road_stones - 3,
                                    color_factor * destination_stack_height as i8,
                                );
                            }
                        }
                    }
                } else {
                    num_squares_covered += 1;
                }

                destination_square = destination_square
                    .go_direction(direction)
                    .unwrap_or(destination_square);
            }

            // Check for board fill on this move and the next
            if num_squares_covered == S as u8 * S as u8 && loses_their_critical_square.is_none() {
                // TODO: Maybe add separate policy features for this?
                // It's possible that the spread that board fills also makes them a road
                check_flat_win::<Us, P, S>(
                    position,
                    our_flatcount_after_move,
                    their_flatcount,
                    policy,
                );
            } else if num_squares_covered == S as u8 * S as u8 - 1 {
                check_flat_win_next_move::<Us, P, S>(
                    position,
                    our_flatcount_after_move,
                    their_flatcount,
                    policy,
                );
            }

            if their_pieces == 0 {
                policy.eval_one(indexes.pure_spread, 0);
            } else {
                policy.eval_one(indexes.pure_spread, 1);
            }

            // Continue spreading the stack (the piece, that is) we spread last turn, if any
            if let Some(MovementSynopsis {
                origin: _,
                destination,
            }) = group_data.second_to_last_movement()
            {
                if destination == square {
                    policy.eval_one(indexes.continue_spread, role_id);
                }
            }

            // Recapture the stack they moved on their last move
            if let Some(role) = stack_recaptured_with {
                if their_pieces == 0 {
                    policy.eval_one(indexes.recapture_stack_pure, role as u16 as usize);
                } else {
                    policy.eval_one(indexes.recapture_stack_impure, role as u16 as usize);
                }
            }

            let their_open_critical_squares =
                Them::critical_squares(group_data) & (!group_data.all_pieces());

            if !their_open_critical_squares.is_empty() {
                if their_pieces_captured == 0 && captures_their_critical_square.is_none() {
                    // Move ignores their critical threat, but might win for us
                    policy.eval_one(indexes.ignore_their_critical_square, 1)
                } else {
                    // Move captures at least one stack, which might save us
                    policy.eval_i8(
                        indexes.place_their_critical_square,
                        3,
                        their_pieces_captured,
                    );
                }
            }

            // Bonus for moving onto a critical square
            if let Some(critical_square) = captures_our_critical_square {
                // Start with a very simple check for throwing onto a straight road
                let our_road_stones = Us::road_stones(group_data);
                if our_road_stones.file::<S>(critical_square.file()).count() == S as u8 - 1
                    && (direction == East || direction == West)
                    || our_road_stones.rank::<S>(critical_square.rank()).count() == S as u8 - 1
                        && (direction == North || direction == South)
                {
                    // Only this option is a guaranteed win:
                    policy.eval_one(indexes.move_onto_critical_square, 0);
                    policy.set_immediate_win();
                } else {
                    // Check if reaching the critical square still wins, in case our
                    // stack spread lost some of our flats
                    let mut edge_connection = critical_square.group_edge_connection();
                    for neighbour in critical_square.neighbors() {
                        if let Some(neighbour_piece) = position.top_stones()[neighbour] {
                            if Us::piece_is_ours(neighbour_piece) {
                                let group_id = group_data.groups[neighbour];
                                if our_groups_affected.iter().all(|g| *g != group_id) {
                                    edge_connection = edge_connection
                                        | group_data.amount_in_group[group_id as usize].1;
                                }
                            }
                        }
                    }

                    if edge_connection.is_winning() {
                        // Only this option is a guaranteed win:
                        policy.eval_one(indexes.move_onto_critical_square, 0);
                        policy.set_immediate_win();
                    }
                    // If the critical square has two neighbours of the same group,
                    // and neither the origin square nor the critical square is a wall,
                    // at least one of the spreads onto the critical square will be a road win
                    else if our_squares_affected.len() == 1
                        && critical_square
                            .neighbors()
                            .any(|sq| sq == our_squares_affected[0])
                        && critical_square
                            .neighbors()
                            .filter(|sq| {
                                group_data.groups[*sq] == group_data.groups[our_squares_affected[0]]
                            })
                            .count()
                            > 1
                        && position.top_stones()[critical_square].map(Piece::role) != Some(Wall)
                    {
                        policy.eval_one(indexes.move_onto_critical_square, 1);
                        policy.set_immediate_win();
                    } else {
                        policy.eval_one(indexes.move_onto_critical_square, 2)
                    }
                }
            }

            for group_id in our_groups_joined {
                if !our_groups_affected.contains(&group_id) {
                    group_edge_connection =
                        group_edge_connection | group_data.amount_in_group[group_id as usize].1;
                }
            }

            if group_edge_connection.is_winning() {
                policy.eval_one(indexes.spread_that_connects_groups_to_win, 0);
                policy.set_immediate_win();
            }
        }
    }
}

fn check_flat_win_next_move<Us: ColorTr, P: PolicyApplier, const S: usize>(
    position: &Position<S>,
    our_flatcount_after_move: i8,
    their_flatcount: i8,
    policy: &mut P,
) {
    let indexes = policy_indexes::<S>();
    if Us::color() == Color::White {
        match position
            .komi()
            .game_result_with_flatcounts(our_flatcount_after_move, their_flatcount + 1)
        {
            GameResult::WhiteWin => policy.eval_one(indexes.place_to_allow_opponent_to_end, 2),
            GameResult::BlackWin => policy.eval_one(indexes.place_to_allow_opponent_to_end, 0),
            GameResult::Draw => policy.eval_one(indexes.place_to_allow_opponent_to_end, 1),
        }
    } else {
        match position
            .komi()
            .game_result_with_flatcounts(their_flatcount + 1, our_flatcount_after_move)
        {
            GameResult::WhiteWin => policy.eval_one(indexes.place_to_allow_opponent_to_end, 0),
            GameResult::BlackWin => policy.eval_one(indexes.place_to_allow_opponent_to_end, 2),
            GameResult::Draw => policy.eval_one(indexes.place_to_allow_opponent_to_end, 1),
        }
    }
}

fn check_flat_win<Us: ColorTr, P: PolicyApplier, const S: usize>(
    position: &Position<S>,
    our_flatcount_after_move: i8,
    their_flatcount: i8,
    policy: &mut P,
) {
    let indexes = policy_indexes::<S>();
    if Us::color() == Color::White {
        match position
            .komi()
            .game_result_with_flatcounts(our_flatcount_after_move, their_flatcount)
        {
            GameResult::WhiteWin => {
                policy.eval_one(indexes.place_to_win, 0);
                policy.set_immediate_win();
            }
            GameResult::BlackWin => policy.eval_one(indexes.place_to_loss, 0),
            GameResult::Draw => policy.eval_one(indexes.place_to_draw, 0),
        }
    } else {
        match position
            .komi()
            .game_result_with_flatcounts(their_flatcount, our_flatcount_after_move)
        {
            GameResult::WhiteWin => policy.eval_one(indexes.place_to_loss, 0),
            GameResult::BlackWin => {
                policy.eval_one(indexes.place_to_win, 0);
                policy.set_immediate_win();
            }
            GameResult::Draw => policy.eval_one(indexes.place_to_draw, 0),
        }
    }
}

/// For a spread that starts from this square, determine if the spread does not damage the group it's part of,
/// for example because of a citadel
fn spread_damages_our_group<const S: usize, Us: ColorTr>(
    position: &Position<S>,
    square: Square<S>,
    direction: Direction,
) -> bool {
    let behind_square = square.go_direction(direction.reverse());

    !direction
        .orthogonal_directions()
        .into_iter()
        .filter(|orthogonal| square.go_direction(*orthogonal).is_some())
        .any(|orthogonal| {
            let flank_square = square.go_direction(orthogonal).unwrap();
            let opposite_flank = square.go_direction(orthogonal.reverse());

            position.top_stones()[flank_square].is_some_and(Us::is_road_stone)
                && position.top_stones()[flank_square.go_direction(direction).unwrap()]
                    .is_some_and(Us::is_road_stone)
                && (opposite_flank.is_none() // This is probably not fully correct, it assumes the connection to the edge will be restored because the next piece dropped is ours
                || behind_square.is_none() // Ditto
                || !position.top_stones()[opposite_flank.unwrap()]
                    .is_some_and(Us::is_road_stone))
                && (behind_square.is_none()
                    || !position.top_stones()[behind_square.unwrap()]
                        .is_some_and(Us::is_road_stone)
                    || position.top_stones()
                        [behind_square.unwrap().go_direction(orthogonal).unwrap()]
                    .is_some_and(Us::is_road_stone))
        })
}

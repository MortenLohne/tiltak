use crate::evaluation::parameters;
use arrayvec::ArrayVec;
use board_game_traits::{Color, GameResult, Position as PositionTrait};

use crate::evaluation::parameters::PolicyFeatures;
use crate::position::bitboard::BitBoard;
use crate::position::color_trait::{BlackTr, ColorTr, WhiteTr};
use crate::position::Direction::*;
use crate::position::Role::{Cap, Flat, Wall};
use crate::position::{square_symmetries, GroupData, Piece, Position, Role};
use crate::position::{squares_iterator, Move};
use crate::position::{GroupEdgeConnection, Square};
use crate::search;

const POLICY_BASELINE: f32 = 0.05;

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
        feature_sets: &mut Vec<Box<[f32]>>,
    ) {
        let num_moves = simple_moves.len();

        while feature_sets.len() < num_moves {
            feature_sets.push(vec![0.0; parameters::num_policy_features::<S>()].into_boxed_slice());
        }

        let mut policy_feature_sets: Vec<PolicyFeatures> = feature_sets
            .iter_mut()
            .map(|feature_set| PolicyFeatures::new::<S>(feature_set))
            .collect();

        self.features_for_moves(&mut policy_feature_sets, simple_moves, group_data);

        moves.extend(
            simple_moves
                .drain(..)
                .zip(feature_sets)
                .map(|(mv, features)| {
                    let offset = inverse_sigmoid(1.0 / num_moves as f32);

                    let total_value: f32 =
                        features.iter().zip(params).map(|(c, p)| c * p).sum::<f32>() + offset;

                    for c in features.iter_mut() {
                        *c = 0.0;
                    }

                    (mv, sigmoid(total_value))
                }),
        );

        let score_sum: f32 = moves.iter().map(|(_mv, score)| *score).sum();

        let score_factor = (1.0 - POLICY_BASELINE) / score_sum;
        for (_mv, score) in moves.iter_mut() {
            *score = *score * score_factor + (POLICY_BASELINE / num_moves as f32);
        }
    }

    pub fn features_for_moves(
        &self,
        feature_sets: &mut [PolicyFeatures],
        moves: &[Move],
        group_data: &GroupData<S>,
    ) {
        assert!(feature_sets.len() >= moves.len());

        let mut immediate_win_exists = false;

        for (features_set, mv) in feature_sets.iter_mut().zip(moves) {
            self.features_for_move(features_set, mv, group_data);
            if has_immediate_win(features_set) {
                immediate_win_exists = true;
            }
        }
        if immediate_win_exists {
            for features_set in feature_sets {
                if !has_immediate_win(features_set) {
                    features_set.decline_win[0] = 1.0;
                }
            }
        }
    }

    fn features_for_move(
        &self,
        policy_features: &mut PolicyFeatures,
        mv: &Move,
        group_data: &GroupData<S>,
    ) {
        match self.side_to_move() {
            Color::White => features_for_move_colortr::<WhiteTr, BlackTr, S>(
                self,
                policy_features,
                mv,
                group_data,
            ),
            Color::Black => features_for_move_colortr::<BlackTr, WhiteTr, S>(
                self,
                policy_features,
                mv,
                group_data,
            ),
        }
    }
}

fn has_immediate_win(policy_features: &PolicyFeatures) -> bool {
    [
        policy_features.place_to_win[0],
        policy_features.place_our_critical_square[0],
        policy_features.move_onto_critical_square[0],
        policy_features.move_onto_critical_square[1],
        policy_features.spread_that_connects_groups_to_win[0],
    ]
    .iter()
    .any(|p| *p != 0.0)
}

struct MovementSynopsis {
    origin: Square,
    destination: Square,
}

fn our_last_placement<const S: usize>(position: &Position<S>) -> Option<(Role, Square)> {
    position
        .moves()
        .get(position.moves().len().overflowing_sub(2).0)
        .and_then(|mv| match mv {
            Move::Place(role, square) => Some((*role, *square)),
            Move::Move(_, _, _) => None,
        })
}

fn their_last_placement<const S: usize>(position: &Position<S>) -> Option<(Role, Square)> {
    position
        .moves()
        .get(position.moves().len().overflowing_sub(1).0)
        .and_then(|mv| match mv {
            Move::Place(role, square) => Some((*role, *square)),
            Move::Move(_, _, _) => None,
        })
}

fn our_last_movement<const S: usize>(position: &Position<S>) -> Option<MovementSynopsis> {
    get_movement_in_history(position, 2)
}

fn their_last_movement<const S: usize>(position: &Position<S>) -> Option<MovementSynopsis> {
    get_movement_in_history(position, 1)
}

fn get_movement_in_history<const S: usize>(
    position: &Position<S>,
    i: usize,
) -> Option<MovementSynopsis> {
    position
        .moves()
        .get(position.moves().len().overflowing_sub(i).0)
        .and_then(|mv| match mv {
            Move::Place(_, _) => None,
            Move::Move(origin, direction, stack_movement) => Some(MovementSynopsis {
                origin: *origin,
                destination: origin
                    .jump_direction::<S>(*direction, stack_movement.len() as u8)
                    .unwrap(),
            }),
        })
}

fn features_for_move_colortr<Us: ColorTr, Them: ColorTr, const S: usize>(
    position: &Position<S>,
    policy_features: &mut PolicyFeatures,
    mv: &Move,
    group_data: &GroupData<S>,
) {
    // If it's the first move, give every move equal probability
    if position.half_moves_played() < 2 {
        return;
    }

    match mv {
        Move::Place(role, square) => {
            let our_flatcount = Us::flats(group_data).count();
            let their_flatcount = Them::flats(group_data).count();

            let our_flatcount_after_move = match *role {
                Flat => our_flatcount + 1,
                Wall | Cap => our_flatcount,
            };

            let our_flat_lead_after_move = our_flatcount_after_move as i8 - their_flatcount as i8;

            // Apply special bonuses if the game ends on this move
            if Us::stones_left(position) == 1 && Us::caps_left(position) == 0
                || group_data.all_pieces().count() as usize == S * S - 1
            {
                if Us::color() == Color::White {
                    match position.komi().game_result_with_flatcounts(
                        our_flatcount_after_move as i8,
                        their_flatcount as i8,
                    ) {
                        GameResult::WhiteWin => policy_features.place_to_win[0] = 1.0,
                        GameResult::BlackWin => policy_features.place_to_loss[0] = 1.0,
                        GameResult::Draw => policy_features.place_to_draw[0] = 1.0,
                    }
                } else {
                    match position.komi().game_result_with_flatcounts(
                        their_flatcount as i8,
                        our_flatcount_after_move as i8,
                    ) {
                        GameResult::WhiteWin => policy_features.place_to_loss[0] = 1.0,
                        GameResult::BlackWin => policy_features.place_to_win[0] = 1.0,
                        GameResult::Draw => policy_features.place_to_draw[0] = 1.0,
                    }
                }
            }
            // Bonuses if our opponent can finish on flats next turn
            else if Them::stones_left(position) == 1 && Them::caps_left(position) == 0
                || group_data.all_pieces().count() as usize == S * S - 2
            {
                if Us::color() == Color::White {
                    match position.komi().game_result_with_flatcounts(
                        our_flatcount_after_move as i8,
                        their_flatcount as i8 + 1,
                    ) {
                        GameResult::WhiteWin => {
                            policy_features.place_to_allow_opponent_to_end[2] = 1.0
                        }
                        GameResult::BlackWin => {
                            policy_features.place_to_allow_opponent_to_end[0] = 1.0
                        }
                        GameResult::Draw => policy_features.place_to_allow_opponent_to_end[1] = 1.0,
                    }
                } else {
                    match position.komi().game_result_with_flatcounts(
                        their_flatcount as i8 + 1,
                        our_flatcount_after_move as i8,
                    ) {
                        GameResult::WhiteWin => {
                            policy_features.place_to_allow_opponent_to_end[0] = 1.0
                        }
                        GameResult::BlackWin => {
                            policy_features.place_to_allow_opponent_to_end[2] = 1.0
                        }
                        GameResult::Draw => policy_features.place_to_allow_opponent_to_end[1] = 1.0,
                    }
                }
            }
            // TODO: These two bonuses don't take komi into account, but they should
            else if Us::stones_left(position) == 2 && Us::caps_left(position) == 0 {
                policy_features.two_flats_left[0] = 1.0;
                policy_features.two_flats_left[1] = our_flat_lead_after_move as f32;
            } else if Us::stones_left(position) == 3 && Us::caps_left(position) == 0 {
                policy_features.three_flats_left[0] = 1.0;
                policy_features.three_flats_left[1] = our_flat_lead_after_move as f32;
            }

            let their_open_critical_squares =
                Them::critical_squares(&*group_data) & (!group_data.all_pieces());

            // Apply PSQT
            match role {
                Flat => {
                    policy_features.flat_psqt[square_symmetries::<S>()[square.0 as usize]] = 1.0
                }
                Wall => {
                    policy_features.wall_psqt[square_symmetries::<S>()[square.0 as usize]] = 1.0
                }
                Cap => policy_features.cap_psqt[square_symmetries::<S>()[square.0 as usize]] = 1.0,
            }

            let role_id = match *role {
                Flat => 0,
                Wall => 1,
                Cap => 2,
            };

            for &line in BitBoard::lines_for_square::<S>(*square).iter() {
                let our_line_score = (Us::road_stones(group_data) & line).count();
                let their_line_score = (Them::road_stones(group_data) & line).count();
                policy_features.our_road_stones_in_line[S * role_id + our_line_score as usize] +=
                    1.0;
                policy_features.their_road_stones_in_line
                    [S * role_id + their_line_score as usize] += 1.0;
            }

            // If square is next to a group
            let mut our_unique_neighbour_groups: ArrayVec<(Square, u8), 4> = ArrayVec::new();
            let mut their_unique_neighbour_groups: ArrayVec<(Square, u8), 4> = ArrayVec::new();
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
                let total_neighbours_group_size: f32 = our_unique_neighbour_groups
                    .iter()
                    .map(|(_, group_id)| group_data.amount_in_group[*group_id as usize].0 as f32)
                    .sum();

                policy_features.merge_two_groups_base[role_id] = 1.0;
                // Divide by 10, as large values confuse the tuner
                policy_features.merge_two_groups_linear[role_id] =
                    total_neighbours_group_size / 10.0;
            }

            if their_unique_neighbour_groups.len() > 1 {
                let total_neighbours_group_size: f32 = their_unique_neighbour_groups
                    .iter()
                    .map(|(_, group_id)| group_data.amount_in_group[*group_id as usize].0 as f32)
                    .sum();

                policy_features.block_merger_base[role_id] = 1.0;
                // Divide by 10, as large values confuse the tuner
                policy_features.block_merger_linear[role_id] = total_neighbours_group_size / 10.0;
            }
            if our_unique_neighbour_groups.len() == 1 {
                let group_id = our_unique_neighbour_groups[0].1;
                let amount_in_group = group_data.amount_in_group[group_id as usize].0 as f32;

                policy_features.extend_single_group_base[role_id] = 1.0;
                // Divide by 10, as large values confuse the tuner
                policy_features.extend_single_group_linear[role_id] = amount_in_group / 10.0;

                // Apply a separate bonus if the piece expands the group to a new line
                if squares_iterator::<S>()
                    .filter(|sq| group_data.groups[*sq] == group_id)
                    .all(|sq| sq.file::<S>() != square.file::<S>())
                    || squares_iterator::<S>()
                        .filter(|sq| group_data.groups[*sq] == group_id)
                        .all(|sq| sq.rank::<S>() != square.rank::<S>())
                {
                    policy_features.extend_single_group_to_new_line_base[role_id] = 1.0;
                    policy_features.extend_single_group_to_new_line_linear[role_id] =
                        amount_in_group / 10.0;
                }
            }

            if *role == Flat || *role == Cap {
                if Us::is_critical_square(&*group_data, *square) {
                    policy_features.place_our_critical_square[0] += 1.0;
                } else if !their_open_critical_squares.is_empty() {
                    if their_open_critical_squares == BitBoard::empty().set(square.0) {
                        policy_features.place_their_critical_square[0] += 1.0;
                    } else {
                        policy_features.ignore_their_critical_square[0] += 1.0;
                    }
                }

                // If square is next to a road stone laid on our last turn
                if let Some((last_role, last_square)) = our_last_placement(position) {
                    if last_role == Flat || last_role == Cap {
                        if square.neighbours::<S>().any(|neigh| neigh == last_square) {
                            policy_features.next_to_our_last_stone[0] = 1.0;
                        } else if (square.rank::<S>() as i8 - last_square.rank::<S>() as i8).abs()
                            == 1
                            && (square.file::<S>() as i8 - last_square.file::<S>() as i8).abs() == 1
                        {
                            policy_features.diagonal_to_our_last_stone[0] = 1.0;
                        }
                    }
                }

                // If square is next to a road stone laid on their last turn
                if let Some((last_role, last_square)) = their_last_placement(position) {
                    if last_role == Flat {
                        if square.neighbours::<S>().any(|neigh| neigh == last_square) {
                            policy_features.next_to_their_last_stone[0] = 1.0;
                        } else if (square.rank::<S>() as i8 - last_square.rank::<S>() as i8).abs()
                            == 1
                            && (square.file::<S>() as i8 - last_square.file::<S>() as i8).abs() == 1
                        {
                            policy_features.diagonal_to_their_last_stone[0] = 1.0;
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
                            policy_features.attack_strong_flats[0] += (our_road_stones - 1) as f32;
                        }
                    }
                }
            }

            if *role == Wall {
                policy_features.wall_psqt[square_symmetries::<S>()[square.0 as usize]] = 1.0;

                if !their_open_critical_squares.is_empty() {
                    if their_open_critical_squares == BitBoard::empty().set(square.0) {
                        policy_features.place_their_critical_square[1] += 1.0;
                    } else {
                        policy_features.ignore_their_critical_square[0] += 1.0;
                    }
                }
            } else if *role == Cap {
                if Us::is_critical_square(&*group_data, *square) {
                    policy_features.place_our_critical_square[0] += 1.0;
                } else if !their_open_critical_squares.is_empty() {
                    if their_open_critical_squares == BitBoard::empty().set(square.0) {
                        policy_features.place_their_critical_square[2] += 1.0;
                    } else {
                        policy_features.ignore_their_critical_square[0] += 1.0;
                    }
                }
            }
            if *role == Wall || *role == Cap {
                for direction in square.directions::<S>() {
                    let neighbour = square.go_direction::<S>(direction).unwrap();

                    // If square blocks an extension of two flats
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
                        policy_features.blocking_stone_blocks_extensions_of_two_flats[0] += 1.0;
                    }

                    if position[neighbour].len() > 2
                        && Them::piece_is_ours(position[neighbour].top_stone().unwrap())
                    {
                        let stack = position[neighbour];
                        let top_stone = stack.top_stone().unwrap();
                        let mut captives = 0;
                        let mut reserves = 0;
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
                                policy_features.attack_strong_stack_with_wall[index] +=
                                    captives as f32;
                                policy_features.attack_strong_stack_with_wall[index + 1] +=
                                    reserves as f32;
                            }
                            Cap => {
                                policy_features.attack_strong_stack_with_cap[index] +=
                                    captives as f32;
                                policy_features.attack_strong_stack_with_cap[index + 1] +=
                                    reserves as f32;
                            }
                        }

                        if let Some(MovementSynopsis {
                            origin: _,
                            destination,
                        }) = their_last_movement(position)
                        {
                            if neighbour == destination {
                                policy_features.attack_last_movement[0] += captives as f32;
                                policy_features.attack_last_movement[1] += reserves as f32;
                            }
                        }
                    }
                }
            }

            // Bonus for placing on the square left behind by their movement
            if let Some(MovementSynopsis {
                origin,
                destination: _,
            }) = their_last_movement(position)
            {
                if *square == origin {
                    policy_features.place_last_movement[role_id] += 1.0;
                }
            }
        }

        Move::Move(square, direction, stack_movement) => {
            let role_id = match position[*square].top_stone().unwrap().role() {
                Flat => 0,
                Wall => 1,
                Cap => 2,
            };

            policy_features.move_role_bonus[role_id] += 1.0;

            let mut destination_square =
                if stack_movement.get(0).pieces_to_take == position[*square].len() {
                    square.go_direction::<S>(*direction).unwrap()
                } else {
                    *square
                };

            // Bonus for moving the piece we placed on our last turn
            if let Some((role, last_square)) = our_last_placement(position) {
                if *square == last_square && !position[destination_square].is_empty() {
                    policy_features.move_last_placement[role.disc()] += 1.0;
                }
            }

            let mut captures_our_critical_square = None;
            let mut captures_their_critical_square = None;

            // The groups that become connected through this move
            let mut our_groups_joined = <ArrayVec<u8, 10>>::new();
            let mut their_piece_left_on_previous_square = false;
            // Edge connections created by this move
            let mut group_edge_connection = GroupEdgeConnection::default();

            // The groups where the move causes us to lose flats
            let mut our_groups_affected = <ArrayVec<u8, S>>::new();
            let mut our_squares_affected = <ArrayVec<Square, S>>::new();
            let mut stack_recaptured_with = None;

            // Number of squares captured by us
            let mut our_pieces = 0;
            // Number of squares given to them
            let mut their_pieces = 0;
            // Number of squares captured by us, that were previously held by them
            let mut their_pieces_captured = 0;
            let mut fcd = 0;

            // Special case for when we spread the whole stack
            if position[*square].len() == stack_movement.get(0).pieces_to_take {
                let top_stone = position[*square].top_stone.unwrap();
                if top_stone.is_road_piece() {
                    our_squares_affected.push(*square);
                    our_groups_affected.push(group_data.groups[*square]);
                }
                if top_stone.role() == Flat {
                    fcd -= 1;
                }
            }

            // This iterator skips the first square if we move the whole stack
            for piece in position
                .top_stones_left_behind_by_move(*square, stack_movement)
                .flatten()
            {
                let destination_stack = &position[destination_square];

                if let Some(captured_piece) = destination_stack.top_stone() {
                    if captured_piece.role() == Flat {
                        if Us::piece_is_ours(captured_piece) {
                            fcd -= 1;
                        } else {
                            fcd += 1;
                        }
                    }
                }

                if Us::piece_is_ours(piece) {
                    our_pieces += 1;
                    if piece.role() == Flat {
                        fcd += 1;
                    }

                    if Us::is_critical_square(&*group_data, destination_square)
                        && piece.is_road_piece()
                    {
                        captures_our_critical_square = Some(destination_square);
                    }
                    if Them::is_critical_square(&*group_data, destination_square) {
                        captures_their_critical_square = Some(destination_square);
                    }
                    if let Some(MovementSynopsis {
                        origin: _,
                        destination: last_capture,
                    }) = their_last_movement(position)
                    {
                        if destination_square == last_capture {
                            stack_recaptured_with = Some(piece.role());
                        }
                    }
                } else {
                    their_pieces += 1;
                    if piece.role() == Flat {
                        fcd -= 1;
                    }
                }

                if Us::piece_is_ours(piece) && piece.is_road_piece() {
                    let mut neighbour_group_ids = <ArrayVec<u8, S>>::new();

                    for neighbour in Square::neighbours::<S>(destination_square) {
                        if destination_square != *square
                            && destination_square.go_direction::<S>(direction.reverse())
                                == Some(neighbour)
                        {
                            continue;
                        }
                        if let Some(neighbour_piece) = position[neighbour].top_stone() {
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
                        group_edge_connection.connect_square::<S>(destination_square);

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
                        policy_features.move_cap_onto_strong_line[road_piece_count - 3] += 1.0;
                        if destination_square
                            .neighbours::<S>()
                            .any(|n| Us::is_critical_square(group_data, n))
                        {
                            policy_features.move_cap_onto_strong_line_with_critical_square
                                [road_piece_count - 3] += 1.0;
                        }
                    }
                }

                if let Some(destination_top_stone) = destination_stack.top_stone() {
                    // When a stack gets captured, give a linear bonus or malus depending on
                    // whether it's captured by us or them
                    if piece.color() != destination_top_stone.color() {
                        if Us::piece_is_ours(piece) {
                            policy_features.stack_captured_by_movement[0] +=
                                destination_stack.len() as f32;
                            their_pieces_captured += 1;
                        } else {
                            policy_features.stack_captured_by_movement[0] -=
                                destination_stack.len() as f32;
                            our_squares_affected.push(destination_square);
                            our_groups_affected.push(group_data.groups[destination_square]);
                        }
                    }
                    if Us::piece_is_ours(destination_top_stone) && piece.role() == Wall {
                        our_squares_affected.push(destination_square);
                        our_groups_affected.push(group_data.groups[destination_square]);
                    }

                    for &line in BitBoard::lines_for_square::<S>(destination_square).iter() {
                        let our_road_stones = (line & Us::road_stones(group_data)).count() as usize;
                        let color_factor = if Us::piece_is_ours(piece) { 1.0 } else { -1.0 };
                        if our_road_stones > 2 {
                            if piece.role() == Cap {
                                policy_features.stack_capture_in_strong_line_cap
                                    [our_road_stones - 3] +=
                                    color_factor * destination_stack.len() as f32;
                            } else {
                                policy_features.stack_capture_in_strong_line
                                    [our_road_stones - 3] +=
                                    color_factor * destination_stack.len() as f32;
                            }
                        }
                    }
                }

                destination_square = destination_square
                    .go_direction::<S>(*direction)
                    .unwrap_or(destination_square);
            }

            policy_features.fcd[fcd.clamp(-2, 5) as usize + 2] = 1.0;

            if their_pieces == 0 {
                policy_features.stack_movement_that_gives_us_top_pieces[0] = 1.0;
                policy_features.stack_movement_that_gives_us_top_pieces[1] = our_pieces as f32;
            } else if their_pieces == 1 {
                policy_features.stack_movement_that_gives_us_top_pieces[2] = 1.0;
                policy_features.stack_movement_that_gives_us_top_pieces[3] = our_pieces as f32;
            } else {
                policy_features.stack_movement_that_gives_us_top_pieces[4] = 1.0;
                policy_features.stack_movement_that_gives_us_top_pieces[5] = our_pieces as f32;
            }

            // Continue spreading the stack (the piece, that is) we spread last turn, if any
            if let Some(MovementSynopsis {
                origin: _,
                destination,
            }) = our_last_movement(position)
            {
                if destination == *square {
                    policy_features.continue_spread[role_id] = 1.0;
                }
            }

            // Recapture the stack they moved on their last move
            if let Some(role) = stack_recaptured_with {
                if their_pieces == 0 {
                    policy_features.recapture_stack_pure[role as u16 as usize] = 1.0;
                } else {
                    policy_features.recapture_stack_impure[role as u16 as usize] = 1.0;
                }
            }

            let their_open_critical_squares =
                Them::critical_squares(&*group_data) & (!group_data.all_pieces());

            if !their_open_critical_squares.is_empty() {
                if their_pieces_captured == 0 && captures_their_critical_square.is_none() {
                    // Move ignores their critical threat, but might win for us
                    policy_features.ignore_their_critical_square[1] += 1.0;
                } else {
                    // Move captures at least one stack, which might save us
                    policy_features.place_their_critical_square[3] += their_pieces_captured as f32;
                }
            }

            // Bonus for moving onto a critical square
            if let Some(critical_square) = captures_our_critical_square {
                // Check if reaching the critical square still wins, in case our
                // stack spread lost some of our flats
                let mut edge_connection =
                    GroupEdgeConnection::default().connect_square::<S>(critical_square);
                for neighbour in critical_square.neighbours::<S>() {
                    if let Some(neighbour_piece) = position[neighbour].top_stone() {
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
                    policy_features.move_onto_critical_square[0] += 1.0;
                }
                // If the critical square has two neighbours of the same group,
                // and neither the origin square nor the critical square is a wall,
                // at least one of the spreads onto the critical square will be a road win
                else if our_squares_affected.len() == 1
                    && critical_square
                        .neighbours::<S>()
                        .any(|sq| sq == our_squares_affected[0])
                    && critical_square
                        .neighbours::<S>()
                        .filter(|sq| {
                            group_data.groups[*sq] == group_data.groups[our_squares_affected[0]]
                        })
                        .count()
                        > 1
                    && position[critical_square].top_stone().map(Piece::role) != Some(Wall)
                {
                    policy_features.move_onto_critical_square[1] += 1.0
                } else {
                    policy_features.move_onto_critical_square[2] += 1.0
                }
            }

            for group_id in our_groups_joined {
                if !our_groups_affected.contains(&group_id) {
                    group_edge_connection =
                        group_edge_connection | group_data.amount_in_group[group_id as usize].1;
                }
            }

            if group_edge_connection.is_winning() {
                policy_features.spread_that_connects_groups_to_win[0] = 1.0;
            }
        }
    }
}

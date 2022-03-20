use crate::evaluation::parameters;
use crate::evaluation::parameters::PolicyFeatures;
use board_game_traits::{Color, EvalPosition, GameResult::*, Position as PositionTrait};
use pgn_traits::PgnPosition;
use rand::seq::SliceRandom;

use crate::position::{squares_iterator, Role, Square};
use crate::position::{GroupData, Move};
use crate::position::{GroupEdgeConnection, Position};
use crate::tests::do_moves_and_check_validity;

#[test]
fn play_random_4s_games_test() {
    play_random_games_prop::<4>(200)
}

#[test]
fn play_random_5s_games_test() {
    play_random_games_prop::<5>(200)
}

#[test]
fn play_random_6s_games_test() {
    play_random_games_prop::<6>(200)
}

#[test]
#[ignore]
fn play_random_4s_games_test_long() {
    play_random_games_prop::<4>(10_000)
}

#[test]
#[ignore]
fn play_random_5s_games_test_long() {
    play_random_games_prop::<5>(10_000)
}

#[test]
#[ignore]
fn play_random_6s_games_test_long() {
    play_random_games_prop::<6>(10_000)
}

fn play_random_games_prop<const S: usize>(num_games: usize) {
    let mut white_wins = 0;
    let mut black_wins = 0;
    let mut draws = 0;
    let mut duration = 0;

    let mut rng = rand::thread_rng();
    for _ in 0..num_games {
        let mut position = <Position<S>>::default();
        let mut moves = vec![];

        for i in 0.. {
            let hash_from_scratch = position.zobrist_hash_from_scratch();
            assert_eq!(
                hash_from_scratch,
                position.zobrist_hash(),
                "Hash mismatch for board:\n{:?}\nMoves: {:?}",
                position,
                position.moves()
            );
            assert_eq!(position, position.flip_colors().flip_colors());

            assert_eq!(Position::from_fen(&position.to_fen()).unwrap(), position);

            let group_data: GroupData<S> = position.group_data();

            assert!((group_data.white_road_pieces() & group_data.black_road_pieces()).is_empty());
            assert!(
                (group_data.white_road_pieces() & group_data.white_blocking_pieces()).count() <= 1
            );

            let eval = position.static_eval();
            for rotation in position.symmetries_with_swapped_colors() {
                if position.side_to_move() == rotation.side_to_move() {
                    assert!(rotation.static_eval() - eval < 0.0001,
                            "Static eval changed with rotation from {} to {} on board\n{:?}Rotated board:\n{:?}", eval, rotation.static_eval(), position, rotation);
                } else {
                    assert!(rotation.static_eval() - eval.abs() < 0.0001);
                }
            }

            moves.clear();

            position.generate_moves(&mut moves);

            let mut feature_sets =
                vec![vec![0.0; parameters::num_policy_features::<S>()]; moves.len()];
            let mut policy_feature_sets: Vec<PolicyFeatures> = feature_sets
                .iter_mut()
                .map(|feature_set| PolicyFeatures::new::<S>(feature_set))
                .collect();

            position.features_for_moves(&mut policy_feature_sets, &moves, &group_data);

            // If the decline_win value is set, check that there really is a winning move
            if policy_feature_sets
                .iter()
                .any(|features| features.decline_win[0] != 0.0)
            {
                assert!(
                    moves.iter().any(|mv| {
                        let reverse_move = position.do_move(mv.clone());
                        let game_result = position.game_result();
                        position.reverse_move(reverse_move);
                        game_result == Some(WhiteWin) || game_result == Some(BlackWin)
                    }),
                    "TPS {} shows a win, but no winning move.",
                    position.to_fen()
                )
            }

            let mv = moves
                .choose(&mut rng)
                .unwrap_or_else(|| panic!("No legal moves on board\n{:?}", position))
                .clone();
            assert_eq!(
                mv,
                position.move_from_san(&position.move_to_san(&mv)).unwrap()
            );

            let white_flat_lead_before = group_data.white_flat_stones.count() as i8
                - group_data.black_flat_stones.count() as i8;

            let fcd = position.fcd_for_move(mv.clone());

            position.do_move(mv.clone());

            let new_group_data = position.group_data();

            let white_flat_lead_after = new_group_data.white_flat_stones.count() as i8
                - new_group_data.black_flat_stones.count() as i8;

            match !position.side_to_move() {
                Color::White => assert_eq!(white_flat_lead_after, white_flat_lead_before + fcd),
                Color::Black => assert_eq!(white_flat_lead_after, white_flat_lead_before - fcd),
            }

            assert_ne!(hash_from_scratch, position.zobrist_hash_from_scratch());

            let result = position.game_result();
            for rotation in position.symmetries_with_swapped_colors() {
                if position.side_to_move() == rotation.side_to_move() {
                    assert_eq!(rotation.game_result(), result);
                } else {
                    assert_eq!(rotation.game_result().map(|r| !r), result);
                }
            }

            if result.is_none() {
                let static_eval = position.static_eval();
                for rotation in position.symmetries_with_swapped_colors() {
                    assert!(
                        rotation.static_eval().abs() - static_eval.abs() < 0.0001,
                        "Original static eval {}, rotated static eval {}.Board:\n{:?}\nRotated board:\n{:?}",
                        static_eval,
                        rotation.static_eval(),
                        position,
                        rotation
                    );
                }
            }

            match result {
                None => (),
                Some(WhiteWin) => {
                    white_wins += 1;
                    duration += i;
                    break;
                }
                Some(BlackWin) => {
                    black_wins += 1;
                    duration += i;
                    break;
                }
                Some(Draw) => {
                    draws += 1;
                    duration += i;
                    break;
                }
            }
        }
    }
    println!(
        "{} white wins, {} black wins, {} draws, {} moves played.",
        white_wins, black_wins, draws, duration
    )
}

#[test]
fn go_in_directions_4s_test() {
    go_in_directions_prop::<4>()
}

#[test]
fn go_in_directions_5s_test() {
    go_in_directions_prop::<5>()
}

#[test]
fn go_in_directions_6s_test() {
    go_in_directions_prop::<6>()
}

fn go_in_directions_prop<const S: usize>() {
    for square in squares_iterator::<S>() {
        assert_eq!(
            square.directions::<S>().count(),
            square.neighbours::<S>().count()
        );
        for direction in square.directions::<S>() {
            assert!(
                square.go_direction::<S>(direction).is_some(),
                "Failed to go in direction {:?} from {:?}",
                direction,
                square
            )
        }
    }
}

#[test]
fn group_connection_4s_test() {
    group_connection_generic_prop::<4>()
}

#[test]
fn group_connection_5s_test() {
    group_connection_generic_prop::<5>()
}

#[test]
fn group_connection_6s_test() {
    group_connection_generic_prop::<6>()
}

fn group_connection_generic_prop<const S: usize>() {
    let group_connection = GroupEdgeConnection::default();

    let a1_connection =
        group_connection.connect_square::<S>(Square::parse_square::<S>("a1").unwrap());

    assert!(!a1_connection.is_connected_south());
    assert!(!a1_connection.is_connected_east());
    assert!(a1_connection.is_connected_north());
    assert!(a1_connection.is_connected_west());
}

#[test]
fn bitboard_full_board_file_rank_4s_test() {
    bitboard_full_board_file_rank_prop::<4>()
}

#[test]
fn bitboard_full_board_file_rank_5s_test() {
    bitboard_full_board_file_rank_prop::<5>()
}

#[test]
fn bitboard_full_board_file_rank_6s_test() {
    bitboard_full_board_file_rank_prop::<6>()
}

fn bitboard_full_board_file_rank_prop<const S: usize>() {
    let mut position = <Position<S>>::start_position();
    let move_strings: Vec<String> = squares_iterator::<S>()
        .map(|sq| sq.to_string::<S>())
        .collect();
    do_moves_and_check_validity(
        &mut position,
        &(move_strings.iter().map(AsRef::as_ref).collect::<Vec<_>>()),
    );
    if S % 2 == 0 {
        assert_eq!(position.game_result(), Some(BlackWin));
    } else {
        assert_eq!(position.game_result(), Some(WhiteWin));
    }

    let group_data = position.group_data();

    let road_pieces = group_data.white_road_pieces() | group_data.black_road_pieces();

    assert_eq!(road_pieces.count(), (S * S) as u8);

    for x in 0..S as u8 {
        assert_eq!(road_pieces.rank::<S>(x).count() as usize, S);
        assert_eq!(road_pieces.file::<S>(x).count() as usize, S);
        for y in 0..S as u8 {
            if x != y {
                assert!((road_pieces.rank::<S>(x) & road_pieces.rank::<S>(y)).is_empty());
                assert!((road_pieces.file::<S>(x) & road_pieces.file::<S>(y)).is_empty());
            }
        }
    }
}

#[test]
fn square_rank_file_test() {
    square_rank_file_prop::<4>();
    square_rank_file_prop::<5>();
    square_rank_file_prop::<6>();
    square_rank_file_prop::<7>();
    square_rank_file_prop::<8>();
}

fn square_rank_file_prop<const S: usize>() {
    let mut position = <Position<S>>::start_position();
    for rank_id in 0..S as u8 {
        for file_id in 0..S as u8 {
            let square = Square::from_rank_file::<S>(rank_id, file_id);
            assert_eq!(rank_id, square.rank::<S>());
            assert_eq!(file_id, square.file::<S>());

            let mv = Move::Place(Role::Flat, square);
            let reverse_move = position.do_move(mv);

            let group_data = position.group_data();

            assert_eq!(group_data.black_road_pieces().rank::<S>(rank_id).count(), 1);
            assert_eq!(group_data.black_road_pieces().file::<S>(file_id).count(), 1);
            position.reverse_move(reverse_move);
        }
    }
}

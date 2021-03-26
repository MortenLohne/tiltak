use board_game_traits::{EvalPosition, GameResult::*, Position as PositionTrait};
use pgn_traits::PgnPosition;
use rand::seq::SliceRandom;

use crate::position::utils::{squares_iterator, Role, Square};
use crate::position::{Board, GroupEdgeConnection, Move};
use crate::tests::do_moves_and_check_validity;

#[test]
fn play_random_4s_games_test() {
    play_random_games_prop::<4>()
}

#[test]
fn play_random_5s_games_test() {
    play_random_games_prop::<5>()
}

#[test]
fn play_random_6s_games_test() {
    play_random_games_prop::<6>()
}

fn play_random_games_prop<const S: usize>() {
    let mut white_wins = 0;
    let mut black_wins = 0;
    let mut draws = 0;
    let mut duration = 0;

    let mut rng = rand::thread_rng();
    for _ in 0..1_000 {
        let mut board = <Board<S>>::default();
        let mut moves = vec![];
        for i in 0.. {
            let hash_from_scratch = board.zobrist_hash_from_scratch();
            assert_eq!(
                hash_from_scratch,
                board.zobrist_hash(),
                "Hash mismatch for board:\n{:?}\nMoves: {:?}",
                board,
                board.moves()
            );
            assert_eq!(board, board.flip_colors().flip_colors());

            let group_data = board.group_data();

            assert!((group_data.white_road_pieces() & group_data.black_road_pieces()).is_empty());
            assert!(
                (group_data.white_road_pieces() & group_data.white_blocking_pieces()).count() <= 1
            );

            let eval = board.static_eval();
            for rotation in board.symmetries_with_swapped_colors() {
                if board.side_to_move() == rotation.side_to_move() {
                    assert!(rotation.static_eval() - eval < 0.0001,
                            "Static eval changed with rotation from {} to {} on board\n{:?}Rotated board:\n{:?}", eval, rotation.static_eval(), board, rotation);
                } else {
                    assert!(rotation.static_eval() - eval.abs() < 0.0001);
                }
            }

            moves.clear();
            board.generate_moves(&mut moves);
            let mv = moves
                .choose(&mut rng)
                .unwrap_or_else(|| panic!("No legal moves on board\n{:?}", board))
                .clone();
            assert_eq!(mv, board.move_from_san(&board.move_to_san(&mv)).unwrap());
            board.do_move(mv);

            assert_ne!(hash_from_scratch, board.zobrist_hash_from_scratch());

            let result = board.game_result();
            for rotation in board.symmetries_with_swapped_colors() {
                if board.side_to_move() == rotation.side_to_move() {
                    assert_eq!(rotation.game_result(), result);
                } else {
                    assert_eq!(rotation.game_result().map(|r| !r), result);
                }
            }

            if result.is_none() {
                let static_eval = board.static_eval();
                for rotation in board.symmetries_with_swapped_colors() {
                    assert!(
                        rotation.static_eval().abs() - static_eval.abs() < 0.0001,
                        "Original static eval {}, rotated static eval {}.Board:\n{:?}\nRotated board:\n{:?}",
                        static_eval,
                        rotation.static_eval(),
                        board,
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
    let mut board = <Board<S>>::start_position();
    let move_strings: Vec<String> = squares_iterator::<S>()
        .map(|sq| sq.to_string::<S>())
        .collect();
    do_moves_and_check_validity(
        &mut board,
        &(move_strings.iter().map(AsRef::as_ref).collect::<Vec<_>>()),
    );
    if S % 2 == 0 {
        assert_eq!(board.game_result(), Some(BlackWin));
    } else {
        assert_eq!(board.game_result(), Some(WhiteWin));
    }

    let group_data = board.group_data();

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
    let mut board = <Board<S>>::start_position();
    for rank_id in 0..S as u8 {
        for file_id in 0..S as u8 {
            let square = Square::from_rank_file::<S>(rank_id, file_id);
            assert_eq!(rank_id, square.rank::<S>());
            assert_eq!(file_id, square.file::<S>());

            let mv = Move::Place(Role::Flat, square);
            let reverse_move = board.do_move(mv);

            let group_data = board.group_data();

            assert_eq!(group_data.black_road_pieces().rank::<S>(rank_id).count(), 1);
            assert_eq!(group_data.black_road_pieces().file::<S>(file_id).count(), 1);
            board.reverse_move(reverse_move);
        }
    }
}

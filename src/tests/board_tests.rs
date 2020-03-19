use crate::board::Piece::{BlackCap, BlackFlat, WhiteFlat, WhiteStanding};
use crate::board::{squares_iterator, Board, Direction::*, Move, Piece, Role, Square, BOARD_SIZE};
use crate::tests::do_moves_and_check_validity;
use crate::{board as board_mod, board};
use board_game_traits::board::{Board as BoardTrait, EvalBoard};
use board_game_traits::board::{GameResult, GameResult::*};
use pgn_traits::pgn::PgnBoard;
use rand::seq::SliceRandom;

#[test]
fn default_board_test() {
    let board = board_mod::Board::default();
    for square in squares_iterator() {
        assert!(board[square].is_empty());
    }
}

#[test]
fn go_in_directions_test() {
    for square in squares_iterator() {
        assert_eq!(square.directions().count(), square.neighbours().count());
        for direction in square.directions() {
            assert!(
                square.go_direction(direction).is_some(),
                "Failed to go in direction {:?} from {:?}",
                direction,
                square
            )
        }
    }
}

#[test]
fn get_set_test() {
    let pieces = vec![WhiteFlat, BlackFlat, BlackFlat, WhiteStanding];
    let mut board = Board::default();
    for &piece in pieces.iter() {
        board[Square(12)].push(piece);
    }
    assert_eq!(board[Square(12)].len(), 4);
    assert_eq!(board[Square(12)].top_stone(), Some(WhiteStanding));

    for (i, &piece) in pieces.iter().enumerate() {
        assert_eq!(
            Some(piece),
            board[Square(12)].get(i as u8),
            "{:?}",
            board[Square(12)]
        );
    }

    for &piece in pieces.iter().rev() {
        assert_eq!(Some(piece), board[Square(12)].pop(), "{:?}", board);
    }

    assert!(board[Square(12)].is_empty());

    for &piece in pieces.iter() {
        board[Square(12)].push(piece);
    }

    for &piece in pieces.iter() {
        assert_eq!(
            piece,
            board[Square(12)].remove(0),
            "{:?}",
            board[Square(12)]
        );
    }
}

#[test]
fn flatten_stack_test() {
    let mut stack = board::Stack::default();
    stack.push(WhiteStanding);
    stack.push(BlackCap);
    assert_eq!(stack.get(0), Some(WhiteFlat));
    assert_eq!(stack.pop(), Some(BlackCap));
    assert_eq!(stack.pop(), Some(WhiteFlat));
}

#[test]
fn correct_number_of_direction_test() {
    assert_eq!(
        squares_iterator()
            .flat_map(|square| square.directions())
            .count(),
        4 * 2 + 12 * 3 + 9 * 4
    );
}

#[test]
fn correct_number_of_neighbours_test() {
    assert_eq!(
        squares_iterator()
            .flat_map(|square| square.neighbours())
            .count(),
        4 * 2 + 12 * 3 + 9 * 4
    );
}

#[test]
fn correct_number_of_legal_directions_test() {
    assert_eq!(
        squares_iterator()
            .flat_map(|square| [North, South, East, West]
                .iter()
                .filter_map(move |&direction| square.go_direction(direction)))
            .count(),
        4 * 2 + 12 * 3 + 9 * 4
    );
}

#[test]
fn stones_left_behind_by_stack_movement_test() {
    let mut board: Board = Board::default();

    do_moves_and_check_validity(&mut board, &["d3", "c3", "c4", "1d3<", "1c4-", "Sc4"]);

    let mv = board.move_from_san("2c3<11").unwrap();
    if let Move::Move(square, _direction, stack_movement) = mv {
        assert_eq!(
            board
                .top_stones_left_behind_by_move(square, &stack_movement)
                .collect::<Vec<_>>(),
            vec![
                Some(Piece::WhiteFlat),
                Some(Piece::BlackFlat),
                Some(Piece::WhiteFlat)
            ]
        );
    } else {
        panic!()
    }

    let mv = board.move_from_san("3c3<21").unwrap();
    if let Move::Move(square, _direction, stack_movement) = mv {
        assert_eq!(
            board
                .top_stones_left_behind_by_move(square, &stack_movement)
                .collect::<Vec<_>>(),
            vec![None, Some(Piece::BlackFlat), Some(Piece::WhiteFlat)]
        );
    } else {
        panic!()
    }
}

#[test]
fn black_can_win_with_road_test() {
    let mut board = board_mod::Board::default();
    let mut moves = vec![];

    for mv_san in [
        "e5", "c3", "c2", "d5", "c1", "c5", "d3", "a4", "e3", "b5", "b1", "a5",
    ]
    .iter()
    {
        let mv = board.move_from_san(&mv_san).unwrap();
        board.generate_moves(&mut moves);
        assert!(moves.contains(&mv));
        board.do_move(mv);
        moves.clear();
    }
    assert_eq!(board.game_result(), Some(GameResult::BlackWin));
}

#[test]
fn play_random_games_test() {
    let mut white_wins = 0;
    let mut black_wins = 0;
    let mut draws = 0;
    let mut duration = 0;

    let mut rng = rand::thread_rng();
    for _ in 0..5_000 {
        let mut board = board_mod::Board::default();
        let mut moves = vec![];
        for i in 0.. {
            assert_eq!(board, board.flip_colors().flip_colors());

            let eval = board.static_eval();
            for rotation in board.rotations_and_symmetries() {
                if board.side_to_move() == rotation.side_to_move() {
                    assert!(rotation.static_eval() - eval < 0.0001);
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

            let result = board.game_result();
            for rotation in board.rotations_and_symmetries() {
                if board.side_to_move() == rotation.side_to_move() {
                    assert_eq!(rotation.game_result(), result);
                } else {
                    assert_eq!(rotation.game_result().map(|r| !r), result);
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
fn game_win_test() {
    let mut board = board_mod::Board::default();
    for mv in [
        Move::Place(Piece::WhiteFlat, Square(12)),
        Move::Place(Piece::BlackFlat, Square(13)),
        Move::Place(Piece::WhiteFlat, Square(7)),
        Move::Place(Piece::BlackFlat, Square(14)),
        Move::Place(Piece::WhiteFlat, Square(2)),
        Move::Place(Piece::BlackFlat, Square(11)),
        Move::Place(Piece::WhiteFlat, Square(17)),
        Move::Place(Piece::BlackFlat, Square(10)),
    ]
    .iter()
    {
        board.do_move(mv.clone());
        assert!(board.game_result().is_none());
    }
    board.do_move(Move::Place(Piece::WhiteFlat, Square(22)));
    assert_eq!(board.game_result(), Some(GameResult::WhiteWin));
}

#[test]
fn game_win_test2() {
    let mut board = board_mod::Board::default();
    for mv in [
        Move::Place(Piece::WhiteFlat, Square(12)),
        Move::Place(Piece::BlackFlat, Square(7)),
        Move::Place(Piece::WhiteFlat, Square(14)),
        Move::Place(Piece::BlackFlat, Square(2)),
        Move::Place(Piece::WhiteFlat, Square(13)),
        Move::Place(Piece::BlackFlat, Square(17)),
        Move::Place(Piece::WhiteFlat, Square(11)),
        Move::Place(Piece::BlackFlat, Square(22)),
    ]
    .iter()
    {
        board.do_move(mv.clone());
        assert!(board.game_result().is_none());
    }
    board.do_move(Move::Place(Piece::WhiteFlat, Square(10)));
    assert_eq!(board.game_result(), Some(GameResult::WhiteWin));
}

#[test]
fn double_road_wins_test() {
    let mut board = Board::default();
    let mut moves = vec![];

    let move_strings = [
        "a4", "a5", "b5", "b4", "c5", "c4", "1c5-", "d4", "d5", "e4", "e5", "c3",
    ];
    do_moves_and_check_validity(&mut board, &move_strings);

    board.generate_moves(&mut moves);
    assert!(moves.contains(&board.move_from_san(&"1c4+").unwrap()));
    moves.clear();

    let reverse_move = board.do_move(board.move_from_san(&"1c4+").unwrap());
    assert_eq!(board.game_result(), Some(WhiteWin));
    board.reverse_move(reverse_move);

    board = board.flip_board_y();

    board.generate_moves(&mut moves);
    assert!(moves.contains(&board.move_from_san(&"1c2-").unwrap()));

    board.do_move(board.move_from_san(&"1c2-").unwrap());
    assert_eq!(board.game_result(), Some(WhiteWin));
}

// Black is behind by one point, with one stone left to place
// Check that placing it standing is suicide, but placing it flat is not
#[test]
fn cannot_suicide_into_points_loss_test() {
    let mut board = Board::start_board();
    let move_strings = [
        "a1", "e5", "e3", "Cc3", "e4", "e2", "d3", "c3>", "d4", "b2", "c3", "c2", "c4", "d2",
        "c3-", "a2", "c3", "c1", "2c2<", "c2", "c3-", "b1", "e3-", "e1", "2e2-", "d1", "2c2-",
        "a2>", "a4", "4b2+112", "a4>", "2b5-", "c4<", "b3+", "e2", "5b4>122", "3e1<", "e1", "e5-",
        "3d4>", "e2-", "b3", "c3", "c2", "b2", "a3", "c5", "c2+", "c4-", "2d3<", "Cc2", "b3-",
        "a2", "e2", "a2>", "b1>", "c2-", "e2-", "5c1>32", "Se2", "a2", "a1+", "2b2<", "c2", "c1",
        "b1", "b2-", "c2-", "4a2+13", "c2", "5e1<23",
    ];
    do_moves_and_check_validity(&mut board, &move_strings);

    let mut moves = vec![];
    board.generate_moves(&mut moves);
    for mv in moves.iter() {
        match mv {
            Move::Place(piece, _) => assert_ne!(
                piece.role(),
                Role::Standing,
                "Placing a standing stone is suicide"
            ),
            _ => (),
        }
    }

    assert!(moves.iter().any(|mv| if let Move::Place(_, _) = mv {
        true
    } else {
        false
    }));
}

#[test]
fn games_ends_when_board_is_full_test() {
    let mut board = Board::start_board();
    let move_strings: Vec<String> = squares_iterator()
        .skip(1)
        .map(|sq| sq.to_string())
        .collect();
    do_moves_and_check_validity(
        &mut board,
        &(move_strings.iter().map(AsRef::as_ref).collect::<Vec<_>>()),
    );
    assert!(board.game_result().is_none());
    board.do_move(board.move_from_san("a5").unwrap());
    assert_eq!(
        board.game_result(),
        Some(WhiteWin),
        "Board is full, game should have ended:\n{:?}",
        board
    );
}

#[test]
fn game_declared_loss_when_every_move_is_suicide() {
    let mut board = Board::start_board();

    do_moves_and_check_validity(&mut board, &["b3", "c4", "c4-", "b3>"]);

    for _ in 0..20 {
        do_moves_and_check_validity(&mut board, &["c4", "b3", "c4-", "b3>"]);
    }
    assert_eq!(board.game_result(), Some(BlackWin));
}

#[test]
fn bitboard_full_board_file_rank_test() {
    let mut board = Board::start_board();
    let move_strings: Vec<String> = squares_iterator().map(|sq| sq.to_string()).collect();
    do_moves_and_check_validity(
        &mut board,
        &(move_strings.iter().map(AsRef::as_ref).collect::<Vec<_>>()),
    );
    assert_eq!(board.game_result(), Some(WhiteWin));

    let road_pieces = board.white_road_pieces() | board.black_road_pieces();

    assert_eq!(road_pieces.count(), 25);

    for x in 0..BOARD_SIZE as u8 {
        assert_eq!(road_pieces.rank(x).count() as usize, BOARD_SIZE);
        assert_eq!(road_pieces.file(x).count() as usize, BOARD_SIZE);
        for y in 0..BOARD_SIZE as u8 {
            if x != y {
                assert!((road_pieces.rank(x) & road_pieces.rank(y)).is_empty());
                assert!((road_pieces.file(x) & road_pieces.file(y)).is_empty());
            }
        }
    }
}

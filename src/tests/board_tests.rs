use crate::board::Piece::{BlackCap, BlackFlat, WhiteFlat, WhiteStanding};
use crate::board::{board_iterator, Board, Move, Piece, Square};
use crate::tests::do_moves_and_check_validity;
use crate::{board as board_mod, board};
use board_game_traits::board::Board as BoardTrait;
use board_game_traits::board::{GameResult, GameResult::*};
use pgn_traits::pgn::PgnBoard;
use rand::seq::SliceRandom;

#[test]
fn default_board_test() {
    let board = board_mod::Board::default();
    for square in board_iterator() {
        assert!(board[square].is_empty());
    }
}

#[test]
fn go_in_directions_test() {
    for square in board_iterator() {
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
fn stones_left_behind_by_stack_movement_test() {
    let mut board: Board = Board::default();

    do_moves_and_check_validity(&mut board, &["c3", "d3", "c4", "1d3<", "1c4+", "Sc4"]);

    let mv = board.move_from_san("2c3<1").unwrap();
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

    let mv = board.move_from_san("3c3<1").unwrap();
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
        "c3", "e5", "c2", "d5", "c1", "c5", "d3", "a4", "e3", "b5", "b1", "a5",
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
    for _ in 0..2000 {
        let mut board = board_mod::Board::default();
        let mut moves = vec![];
        for i in 0.. {
            moves.clear();
            board.generate_moves(&mut moves);
            let mv = moves
                .choose(&mut rng)
                .unwrap_or_else(|| panic!("No legal moves on board\n{:?}", board))
                .clone();
            board.do_move(mv);
            match board.game_result() {
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

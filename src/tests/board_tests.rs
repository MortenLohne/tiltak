use crate::board as board_mod;
use crate::board::{board_iterator, Move, Piece, Square};
use board_game_traits::board::{Board, GameResult, GameResult::*};
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

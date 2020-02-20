use crate::board as board_mod;
use crate::board::{board_iterator, Direction, Move, Movement, Piece, Square};
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
fn start_board_move_gen_test() {
    let mut board = board_mod::Board::default();
    let mut moves = vec![];
    board.generate_moves(&mut moves);
    assert_eq!(moves.len(), 75);
    for mv in moves {
        let reverse_move = board.do_move(mv);
        let mut moves = vec![];
        board.generate_moves(&mut moves);
        assert_eq!(moves.len(), 72);
        board.reverse_move(reverse_move);
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
fn move_gen_test() {
    let mut board = board_mod::Board::default();
    let mut moves = vec![];

    for mv in [
        Move::Place(Piece::WhiteFlat, Square(12)),
        Move::Place(Piece::BlackFlat, Square(13)),
        Move::Place(Piece::WhiteFlat, Square(17)),
        Move::Move(
            Square(13),
            Direction::West,
            smallvec![Movement { pieces_to_take: 1 }],
        ),
        Move::Move(
            Square(17),
            Direction::North,
            smallvec![Movement { pieces_to_take: 1 }],
        ),
        Move::Place(Piece::BlackStanding, Square(17)),
    ]
    .iter()
    {
        board.generate_moves(&mut moves);
        assert!(moves.contains(mv));
        assert_eq!(*mv, board.move_from_san(&board.move_to_san(mv)).unwrap());
        board.do_move(mv.clone());
        moves.clear();
    }
    board.generate_moves(&mut moves);
    assert_eq!(
        moves.len(),
        69 + 18,
        "Generated wrong moves on board:\n{:?}\nExpected moves: {:?}\nExpected move moves:{:?}",
        board,
        moves,
        moves
            .iter()
            .filter(|mv| match mv {
                Move::Move(_, _, _) => true,
                _ => false,
            })
            .collect::<Vec<_>>()
    );
}

#[test]
fn respect_carry_limit_test() {
    let mut board = board_mod::Board::default();
    let mut moves = vec![];

    for move_string in [
        "c3", "c2", "d3", "b3", "c4", "1c2-", "1d3<", "1b3>", "1c4+", "Cc2", "a1", "1c2-", "a2",
    ]
    .iter()
    {
        let mv = board.move_from_san(move_string).unwrap();
        board.generate_moves(&mut moves);
        assert!(moves.contains(&mv));
        board.do_move(mv);
        moves.clear();
    }
    board.generate_moves(&mut moves);
    assert!(
        moves.contains(&board.move_from_san("5c3>").unwrap()),
        "5c3> was not a legal move among {:?} on board\n{:?}",
        moves,
        board
    );

    assert!(!moves.contains(&board.move_from_san("6c3>").unwrap()),
    "6c3> was a legal move among {:?} on board\n{:?}", moves, board);
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

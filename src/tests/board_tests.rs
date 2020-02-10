use crate::board as board_mod;
use crate::board::Piece::WhiteFlat;
use crate::board::{board_iterator, Direction, Move, Movement, Piece, Square};
use board_game_traits::board::{Board, GameResult::*};
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
fn play_random_games_test() {
    let mut white_wins = 0;
    let mut black_wins = 0;
    let mut draws = 0;
    let mut duration = 0;

    let mut rng = rand::thread_rng();
    for _ in 0..1000 {
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

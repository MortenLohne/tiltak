use crate::board as board_mod;
use crate::board::board_iterator;
use board_game_traits::board::{Board, GameResult::*};
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
fn play_random_games_test() {
    let mut white_wins = 0;
    let mut black_wins = 0;
    let mut draws = 0;
    let mut duration = 0;

    let mut rng = rand::thread_rng();
    for _ in 0..100000 {
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

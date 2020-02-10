extern crate board_game_traits;
extern crate pgn_traits;
extern crate rand;
#[macro_use]
extern crate smallvec;

mod board;
mod minmax;
mod tests;

use std::io;

use board as board_mod;
use board_game_traits::board::{Board, GameResult};
use pgn_traits::pgn::PgnBoard;
use rand::seq::SliceRandom;

fn main() {
    let mut board = board_mod::Board::default();
    for d in 1..4 {
        println!("{:?}", minmax::minmax(&mut board, d));
    }
    play_human(board);
}

fn play_random_game(mut board: board_mod::Board) {
    let mut rng = rand::thread_rng();
    let mut moves = vec![];
    for i in 0.. {
        moves.clear();
        board.generate_moves(&mut moves);
        println!("Moves: {:?}", moves);
        println!("Board:\n{:?}", board);
        let mv = moves
            .choose(&mut rng)
            .expect("No legal moves available")
            .clone();
        println!("Doing move {:?}", mv);
        board.do_move(mv);
        if board.game_result().is_some() {
            println!("Board:\n{:?}", board);
            println!(
                "Game ended with {:?} after {} moves",
                board.game_result(),
                i
            );
            break;
        }
    }
}

/// Play a game against the engine through stdin
fn play_human(mut board: board_mod::Board) {
    match board.game_result() {
        None => {
            use board_game_traits::board::Color::*;
            println!("Board:\n{:?}", board);
            // If black, play as human
            if board.side_to_move() == White {
                println!("Type your move as long algebraic notation (e2e4):");

                let reader = io::stdin();
                let mut input_str = "".to_string();
                let mut legal_moves = vec![];
                board.generate_moves(&mut legal_moves);
                // Loop until user enters a valid move
                loop {
                    input_str.clear();
                    reader
                        .read_line(&mut input_str)
                        .expect("Failed to read line");

                    match board.move_from_san(input_str.trim()) {
                        Ok(val) => {
                            if legal_moves.contains(&val) {
                                break;
                            }
                            println!("Move {:?} is illegal! Legal moves: {:?}", val, legal_moves);
                            println!("Try again: ");
                        }

                        Err(error) => {
                            println!("{}, try again.", error);
                        }
                    }
                }
                let c_move = board.move_from_san(input_str.trim()).unwrap();
                board.do_move(c_move);
                play_human(board);
            } else {
                let (best_move, score) = minmax::minmax(&mut board, 4);

                println!("Computer played {:?} with score {}", best_move, score);
                board.do_move(best_move.unwrap());
                play_human(board);
            }
        }

        Some(GameResult::WhiteWin) => println!("White won! Board:\n{:?}", board),
        Some(GameResult::BlackWin) => println!("Black won! Board:\n{:?}", board),
        Some(GameResult::Draw) => println!("The game was drawn! Board:\n{:?}", board),
    }
}

extern crate board_game_traits;
extern crate pgn_traits;
extern crate rand;
#[macro_use]
extern crate smallvec;

mod board;
mod mcts;
mod minmax;
mod move_gen;
mod tests;

use std::io;

use board as board_mod;
use board_game_traits::board::{Board, Color, GameResult};
use pgn_traits::pgn::PgnBoard;
use std::io::Write;

fn main() {
    // test_position();

    for i in 1..10 {
        mcts_vs_minmax(3, 10000 * i);
    }
}

fn mcts_vs_minmax(minmax_depth: u16, mcts_nodes: u64) {
    println!("Minmax depth {} vs mcts {} nodes", minmax_depth, mcts_nodes);
    let mut board = board_mod::Board::default();
    let mut moves = vec![];
    while board.game_result().is_none() {
        match board.side_to_move() {
            Color::White => {
                let (best_move, score) = minmax::minmax(&mut board, minmax_depth);
                board.do_move(best_move.clone().unwrap());
                moves.push(best_move.clone().unwrap());
                print!("{:6}: {:.2}, ", best_move.unwrap(), score);
                io::stdout().flush().unwrap();
            }

            Color::Black => {
                let (best_move, score) = mcts::mcts(board.clone(), mcts_nodes);
                board.do_move(best_move.clone());
                moves.push(best_move.clone());
                println!("{:6}: {:.3}", best_move, score);
                // println!("{:?}", board);
            }
        }
    }
    print!("\n[");
    for mv in moves {
        print!("\"{:?}\", ", mv);
    }
    print!("]");
    println!("\n{:?}\nResult: {:?}", board, board.game_result().unwrap());
}

fn test_position() {
    let mut board = board_mod::Board::default();
    let mut moves = vec![];

    for mv_san in [
        "c2", "c3", "d2", "d3", "1d2-", "c4", "d2", "b4", "1c2-", "1c4+", "2d3<", "d4", "b2", "a5",
        "c2", "a2", "b1", "1a2>", "1b1-", "1d4+", "5c3>3",
    ]
    .iter()
    {
        let mv = board.move_from_san(&mv_san).unwrap();
        board.generate_moves(&mut moves);
        assert!(moves.contains(&mv));
        board.do_move(mv);
        moves.clear();
    }

    println!("{:?}", board);

    let (best_move, score) = minmax::minmax(&mut board, 3);

    println!("Minmax played {:?} with score {}", best_move, score);

    let mut tree = mcts::Tree::new_root();
    for i in 0.. {
        tree.select(&mut board.clone());
        if i % 10000 == 0 {
            println!("{} visits, val={}", tree.visits, tree.mean_action_value);
            tree.print_info();
        }
    }
}

fn mcts(board: board_mod::Board) {
    let mut tree = mcts::Tree::new_root();
    for i in 0..100_000 {
        tree.select(&mut board.clone());
        if i % 10000 == 0 {
            println!("{} visits, val={}", tree.visits, tree.mean_action_value);
            tree.print_info();
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

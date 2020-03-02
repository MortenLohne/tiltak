extern crate board_game_traits;
extern crate pgn_traits;
extern crate rand;
#[macro_use]
extern crate smallvec;
extern crate arrayvec;

mod bitboard;
mod board;
mod mcts;
mod minmax;
mod move_gen;
mod tests;

use std::io;

use crate::tests::do_moves_and_check_validity;
use board::Board;
use board_game_traits::board::Board as BoardTrait;
use board_game_traits::board::{Color, GameResult};
use pgn_traits::pgn::PgnBoard;
use std::io::Write;

fn main() {
    println!("play: Play against the minmax AI");
    println!("aimatch: Watch the minmax and mcts AIs play");
    println!("analyze: Mcts analysis of a hardcoded position");

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    match input.trim() {
        "play" => {
            let board = Board::default();
            play_human(board);
        }
        "aimatch" => {
            for i in 1..10 {
                mcts_vs_minmax(3, 10000 * i);
            }
        }
        "analyze" => test_position(),
        "mem usage" => mem_usage(),
        "bench" => bench(),
        s => println!("Unknown option \"{}\"", s),
    }
}

fn mcts_vs_minmax(minmax_depth: u16, mcts_nodes: u64) {
    println!("Minmax depth {} vs mcts {} nodes", minmax_depth, mcts_nodes);
    let mut board = Board::default();
    let mut moves = vec![];
    while board.game_result().is_none() {
        match board.side_to_move() {
            Color::Black => {
                let (best_move, score) = mcts::mcts(board.clone(), mcts_nodes);
                board.do_move(best_move.clone());
                moves.push(best_move.clone());
                println!("{:6}: {:.3}", best_move, score);
                io::stdout().flush().unwrap();
            }

            Color::White => {
                let (best_move, score) = minmax::minmax(&mut board, minmax_depth);
                board.do_move(best_move.clone().unwrap());
                moves.push(best_move.clone().unwrap());
                print!("{:6}: {:.2}, ", best_move.unwrap(), score);
                io::stdout().flush().unwrap();
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
    let mut board = Board::default();
    let mut moves = vec![];

    for mv_san in [
        "c3", "c4", "b4", "1c4+", "d2", "b5", "b3", "1b5+", "1b3>", "d4", "2c3+", "c4", "d3",
        "1d4+", "d4", "1c4+", "b2", "c4", "1d4+", "2c3>", "1d2-", "Sb3", "5d3+3", "1b3+", "d4",
        "2b2>1", "3c2-1", "b3", "b2", "1b3+", "c2", "b3", "c5", "2b2>", "b2", "1b3+", "b3", "2b4+",
        "d5", "b4", "2c4<", "3b3-", "2c3+", "2b2>", "3d1<", "3c2+", "d1", "5b4+4",
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

    for d in 1..=3 {
        let (best_move, score) = minmax::minmax(&mut board, d);

        println!(
            "Depth {}: minmax played {:?} with score {}",
            d, best_move, score
        );
    }

    let mut tree = mcts::Tree::new_root();
    let mut simple_moves = vec![];
    let mut moves = vec![];
    for i in 0.. {
        tree.select(&mut board.clone(), &mut simple_moves, &mut moves);
        if i % 100_000 == 0 {
            println!("{} visits, val={}", tree.visits, tree.mean_action_value);
            tree.print_info();
        }
    }
}

/// Play a game against the engine through stdin
fn play_human(mut board: Board) {
    match board.game_result() {
        None => {
            use board_game_traits::board::Color::*;
            println!("Board:\n{:?}", board);
            // If black, play as human
            if board.side_to_move() == White {
                println!("Type your move in algebraic notation (c3):");

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
                let (best_move, score) = minmax::minmax(&mut board, 3);

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

fn bench() {
    use std::time;
    const NODES: u64 = 1000_000;
    let start_time = time::Instant::now();
    {
        let board = Board::default();

        let (_move, score) = mcts::mcts(board, NODES);
        print!("{:.3}, ", score);
    }

    {
        let mut board = Board::default();

        do_moves_and_check_validity(&mut board, &["c3", "d3", "c4", "1d3<", "1c4+", "Sc4"]);

        let (_move, score) = mcts::mcts(board, NODES);
        print!("{:.3}, ", score);
    }
    {
        let mut board = Board::default();

        do_moves_and_check_validity(
            &mut board,
            &[
                "c3", "c2", "d3", "b3", "c4", "1c2-", "1d3<", "1b3>", "1c4+", "Cc2", "a1", "1c2-",
                "a2",
            ],
        );

        let (_move, score) = mcts::mcts(board, NODES);
        println!("{:.3}", score);
    }
    let time_taken = start_time.elapsed();
    println!(
        "{} nodes in {} ms, {:.1} knps",
        NODES * 3,
        time_taken.as_millis(),
        NODES as f64 * 3.0 / (1000.0 * time_taken.as_secs_f64())
    );
}
/// Print memory usage of various data types in the project, for debugging purposes
fn mem_usage() {
    use std::mem;
    println!("Tak board: {} bytes", mem::size_of::<board::Board>());
    println!("Tak board cell: {} bytes", mem::size_of::<board::Stack>());
    println!("Tak move: {} bytes", mem::size_of::<board::Move>());

    println!("MCTS node: {} bytes.", mem::size_of::<mcts::Tree>());
    let mut board = board::Board::default();
    let mut tree = mcts::Tree::new_root();
    tree.select(&mut board, &mut vec![], &mut vec![]);
    println!(
        "MCTS node's children: {} bytes.",
        tree.children.len() * mem::size_of::<(mcts::Tree, board::Move)>()
    );
}

extern crate board_game_traits;
extern crate pgn_traits;
extern crate rand;
#[macro_use]
extern crate smallvec;
extern crate arrayvec;
#[macro_use]
extern crate nom;

mod bitboard;
mod board;
mod mcts;
mod minmax;
mod move_gen;
mod tests;
mod tune;

use std::{error, fs, io, sync};

use crate::tests::do_moves_and_check_validity;
use crate::tune::auto_tune::TunableBoard;
use crate::tune::pgn_parse::Game;
use board::Board;
use board_game_traits::board::Board as BoardTrait;
use board_game_traits::board::{Color, GameResult};
use pgn_traits::pgn::PgnBoard;
use rayon::prelude::*;
use std::io::{Read, Write};

fn main() {
    println!("play: Play against the mcts AI");
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
                mcts_vs_minmax(3, 50000 * i);
            }
        }
        "analyze" => test_position(),
        "mem usage" => mem_usage(),
        "bench" => bench(),
        "tune" => tune(),
        "tune_from_file" => tune_from_file().unwrap(),
        "pgn_to_move_list" => pgn_to_move_list(),
        s => println!("Unknown option \"{}\"", s),
    }
}

fn mcts_vs_minmax(minmax_depth: u16, mcts_nodes: u64) {
    println!("Minmax depth {} vs mcts {} nodes", minmax_depth, mcts_nodes);
    let mut board = Board::default();
    let mut moves = vec![];
    while board.game_result().is_none() {
        let num_moves = moves.len();
        if num_moves > 10 && (1..5).all(|i| moves[num_moves - i] == moves[num_moves - i - 4]) {
            break;
        }
        match board.side_to_move() {
            Color::Black => {
                let (best_move, score) = mcts::mcts(board.clone(), mcts_nodes, 0.1);
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
    for mv in moves.iter() {
        print!("\"{:?}\", ", mv);
    }
    println!("]");

    for (ply, mv) in moves.iter().enumerate() {
        if ply % 2 == 0 {
            print!("{}. {:?} ", ply / 2 + 1, mv);
        } else {
            println!("{:?}", mv);
        }
    }
    println!();

    println!("\n{:?}\nResult: {:?}", board, board.game_result());
}

fn test_position() {
    let mut board = Board::default();
    let mut moves = vec![];

    for mv_san in ["a5", "e1"].iter() {
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
            if i > 0 {
                println!("A good move: {}", tree.best_move(1.0).0);
            }
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
                let (best_move, score) = mcts::mcts(board.clone(), 100_000, 0.1);

                println!("Computer played {:?} with score {}", best_move, score);
                board.do_move(best_move);
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
    const NODES: u64 = 1_000_000;
    let start_time = time::Instant::now();
    {
        let board = Board::default();

        let (_move, score) = mcts::mcts(board, NODES, 0.1);
        print!("{:.3}, ", score);
    }

    {
        let mut board = Board::default();

        do_moves_and_check_validity(&mut board, &["c3", "d3", "c4", "1d3<", "1c4+", "Sc4"]);

        let (_move, score) = mcts::mcts(board, NODES, 0.1);
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

        let (_move, score) = mcts::mcts(board, NODES, 0.1);
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

fn tune() {
    let outfile = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("output3.ptn")
        .unwrap();
    let locked_writer = sync::Mutex::new(io::BufWriter::new(outfile));

    use std::sync::atomic::AtomicU64;
    use std::sync::atomic::Ordering;

    let mut white_wins: AtomicU64 = AtomicU64::new(0);
    let mut draws = AtomicU64::new(0);
    let mut black_wins = AtomicU64::new(0);
    let mut aborted = AtomicU64::new(0);
    loop {
        let games = tune::play_match::play_match();
        games.for_each(|ref game| {
            {
                let mut writer = locked_writer.lock().unwrap();
                tune::play_match::game_to_pgn(game, &mut *writer).unwrap();
            }
            match game.game_result {
                None => aborted.fetch_add(1, Ordering::Relaxed),
                Some(GameResult::WhiteWin) => white_wins.fetch_add(1, Ordering::Relaxed),
                Some(GameResult::BlackWin) => black_wins.fetch_add(1, Ordering::Relaxed),
                Some(GameResult::Draw) => draws.fetch_add(1, Ordering::Relaxed),
            };
        });
        println!(
            "{} white wins, {} draws, {} black wins, {} aborted.",
            white_wins.get_mut(),
            draws.get_mut(),
            black_wins.get_mut(),
            aborted.get_mut()
        );
    }
}

fn tune_from_file() -> Result<(), Box<dyn error::Error>> {
    let mut file = fs::File::open("output3.ptn")?;
    let mut input = String::new();
    file.read_to_string(&mut input)?;
    let games: Vec<Game<Board>> = tune::pgn_parse::parse_pgn(&input)?;

    let mut positions = vec![];
    let mut results = vec![];
    for game in games.into_iter().filter(|game| game.game_result.is_some()) {
        let mut board = game.start_board;
        for (mv, _) in game.moves {
            board.do_move(mv);
            positions.push(board.clone());
            results.push(game.game_result.unwrap());
        }
    }

    let middle_index = positions.len() / 2;

    let params = [0.01; Board::PARAMS.len()];

    println!(
        "Final parameters: {:?}",
        tune::auto_tune::gradient_descent(
            &positions[0..middle_index],
            &results[0..middle_index],
            &positions[middle_index..],
            &results[middle_index..],
            &params,
        )
    );

    Ok(())
}

fn pgn_to_move_list() {
    let mut file = fs::File::open("game.ptn").unwrap();
    let mut input = String::new();
    file.read_to_string(&mut input).unwrap();
    let games: Vec<Game<Board>> = tune::pgn_parse::parse_pgn(&input).unwrap();
    println!("Parsed {} games", games.len());
    print!("[");
    for (mv, _) in games[0].moves.iter() {
        print!("\"{}\", ", mv);
    }
    println!("]")
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

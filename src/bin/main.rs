#[cfg(feature = "constant-tuning")]
#[macro_use]
extern crate nom;
#[cfg(feature = "constant-tuning")]
#[macro_use]
extern crate log;

#[cfg(feature = "constant-tuning")]
mod tune;

use std::io;
use std::io::Write;
#[cfg(feature = "constant-tuning")]
use std::path::Path;

#[cfg(feature = "constant-tuning")]
use crate::tune::pgn_parse::Game;
#[cfg(feature = "constant-tuning")]
use crate::tune::play_match::play_match_between_params;
#[cfg(feature = "constant-tuning")]
use crate::tune::training::train_from_scratch;
use board_game_traits::board::Board as BoardTrait;
use board_game_traits::board::{Color, GameResult};
use pgn_traits::pgn::PgnBoard;

use taik::board;
use taik::board::Board;
use taik::board::TunableBoard;
use taik::minmax;
use taik::mcts;

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
        #[cfg(feature = "constant-tuning")]
        "train_from_scratch" => {
            for i in 0.. {
                let file_name = format!("games{}_batch0.ptn", i);
                if !Path::new(&file_name).exists() {
                    train_from_scratch(i).unwrap();
                    break;
                } else {
                    println!("File {} already exists, trying next.", file_name);
                }
            }
        }
        #[cfg(feature = "constant-tuning")]
        "tune_from_file" => tune::training::tune_from_file().unwrap(),
        #[cfg(feature = "constant-tuning")]
        "pgn_to_move_list" => pgn_to_move_list(),
        #[cfg(feature = "constant-tuning")]
        "play_params" => {
            #[allow(clippy::unreadable_literal)]
            let params1 = &[
                0.11546037,
                0.38993832,
                0.3594647,
                0.55473673,
                0.574743,
                0.54728144,
                0.8826678,
                1.2646897,
                1.1345893,
                1.4539466,
                1.5107378,
                0.7540873,
                -0.9577312,
                -0.41307563,
                -0.44581693,
                0.29796726,
                0.7496709,
                1.0882877,
                1.2875234,
                1.8512405,
                1.1104535,
                0.23242366,
                1.4878128,
                0.8031703,
                -2.1186066,
                -1.6040361,
                -0.6668861,
                0.58022684,
                2.0720365,
                -0.047600698,
                0.7756413,
                -1.160208,
                -0.7191107,
                -0.36372992,
                0.092443414,
                0.6003906,
            ];
            let params2 = Board::PARAMS;
            play_match_between_params(params1, params2);
        }
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

    let move_strings: &[&str] = &[
        "c3", "c2", "d2", "d3", "d2+", "c4", "d2", "b4", "c2+", "c4-", "2d3<", "d4", "b2", "a5",
        "c2", "a2", "b1", "a2>", "b1+", "d4-", "5c3>23",
    ];

    for mv_san in move_strings.iter() {
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
        tree.select(
            &mut board.clone(),
            Board::PARAMS,
            &mut simple_moves,
            &mut moves,
        );
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
            if board.side_to_move() == Black {
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
                let (best_move, score) = mcts::mcts(board.clone(), 1_000_000);

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

        let (_move, score) = mcts::mcts(board, NODES);
        print!("{:.3}, ", score);
    }

    {
        let mut board = Board::default();

        do_moves_and_check_validity(&mut board, &["d3", "c3", "c4", "1d3<", "1c4+", "Sc4"]);

        let (_move, score) = mcts::mcts(board, NODES);
        print!("{:.3}, ", score);
    }
    {
        let mut board = Board::default();

        do_moves_and_check_validity(
            &mut board,
            &[
                "c2", "c3", "d3", "b3", "c4", "1c2-", "1d3<", "1b3>", "1c4+", "Cc2", "a1", "1c2-",
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

#[cfg(feature = "constant-tuning")]
fn pgn_to_move_list() {
    use std::fs;
    use std::io::Read;

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
    tree.select(&mut board, Board::PARAMS, &mut vec![], &mut vec![]);
    println!(
        "MCTS node's children: {} bytes.",
        tree.children.len() * mem::size_of::<(mcts::Tree, board::Move)>()
    );
}

fn do_moves_and_check_validity(board: &mut Board, move_strings: &[&str]) {
    let mut moves = vec![];
    for mv_san in move_strings.iter() {
        let mv = board.move_from_san(&mv_san).unwrap();
        board.generate_moves(&mut moves);
        assert!(
            moves.contains(&mv),
            "Move {} was not among legal moves: {:?}\n{:?}",
            board.move_to_san(&mv),
            moves,
            board
        );
        board.do_move(mv);
        moves.clear();
    }
}
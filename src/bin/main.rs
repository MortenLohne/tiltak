#[cfg(test)]
mod tests;

pub mod playtak;
pub mod tei;

use std::io::{Read, Write};
use std::{io, time};

use board_game_traits::board::{Board as BoardTrait, EvalBoard};
use board_game_traits::board::{Color, GameResult};
use pgn_traits::pgn::PgnBoard;

#[cfg(feature = "constant-tuning")]
use rayon::prelude::*;
#[cfg(feature = "constant-tuning")]
use std::collections::HashSet;
use taik::board::Board;
use taik::board::TunableBoard;
#[cfg(feature = "constant-tuning")]
use taik::board::{Move, Role};
use taik::minmax;
use taik::pgn_writer::Game;
use taik::search::MctsSetting;
use taik::{board, search};

fn main() {
    println!("play: Play against the engine through the command line");
    println!("aimatch: Watch the engine play against a very simple minmax implementation");
    println!("analyze: Analyze a given position, provided from a simple move list");
    loop {
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let words = input.split_whitespace().collect::<Vec<_>>();
        match words[0] {
            "play" => {
                let board = Board::default();
                play_human(board);
            }
            "aimatch" => {
                for i in 1..10 {
                    mcts_vs_minmax(3, 50000 * i);
                }
            }
            "analyze" => match words.get(1) {
                Some(&"4") => test_position::<4>(),
                Some(&"5") => test_position::<5>(),
                Some(&"6") => test_position::<6>(),
                Some(&"7") => test_position::<7>(),
                Some(&"8") => test_position::<8>(),
                _ => test_position::<5>(),
            },
            #[cfg(feature = "constant-tuning")]
            "openings" => {
                let depth = 4;
                let mut positions = HashSet::new();
                let openings = generate_openings::<6>(Board::start_board(), &mut positions, depth);

                let mut evaled_openings: Vec<_> = openings
                    .into_par_iter()
                    .filter(|position| position.len() == depth as usize)
                    .map(|position| {
                        let mut board = <Board<6>>::start_board();
                        for mv in position.iter() {
                            board.do_move(mv.clone());
                        }
                        (position, search::mcts(board, 100_000))
                    })
                    .collect();

                evaled_openings.sort_by(|(_, (_, score1)), (_, (_, score2))| {
                    score1.partial_cmp(score2).unwrap()
                });
                for (p, (mv, s)) in evaled_openings {
                    let mut board = <Board<6>>::start_board();
                    for mv in p {
                        print!("{} ", board.move_to_san(&mv));
                        board.do_move(mv);
                    }
                    print!(": ");
                    println!("{}, {}", board.move_to_san(&mv), s);
                }
                return;
            }
            "game" => {
                let mut input = String::new();
                io::stdin().read_to_string(&mut input).unwrap();

                match words.get(1) {
                    Some(&"6") => {
                        let games = taik::pgn_parser::parse_pgn(&input).unwrap();
                        if games.is_empty() {
                            println!("Couldn't parse any games")
                        }

                        analyze_game::<6>(games[0].clone());
                    }
                    None | Some(&"5") | _ => {
                        let games = taik::pgn_parser::parse_pgn(&input).unwrap();
                        if games.is_empty() {
                            println!("Couldn't parse any games")
                        }

                        analyze_game::<5>(games[0].clone());
                    }
                }
            }
            "mem_usage" => mem_usage(),
            "bench" => bench(),
            "selfplay" => mcts_selfplay(time::Duration::from_secs(10)),
            s => println!("Unknown option \"{}\"", s),
        }
    }
}

#[cfg(feature = "constant-tuning")]
fn generate_openings<const S: usize>(
    mut board: Board<S>,
    positions: &mut HashSet<Board<S>>,
    depth: u8,
) -> Vec<Vec<Move>> {
    let mut moves = vec![];
    board.generate_moves(&mut moves);
    moves = moves
        .into_iter()
        .filter(|mv| matches!(mv, Move::Place(Role::Flat, _)))
        .collect();
    moves
        .into_iter()
        .flat_map(|mv| {
            let reverse_move = board.do_move(mv.clone());
            let mut child_lines = if depth > 1 {
                if board
                    .symmetries()
                    .iter()
                    .all(|board_symmetry| !positions.contains(board_symmetry))
                {
                    positions.insert(board.clone());
                    generate_openings(board.clone(), positions, depth - 1)
                } else {
                    vec![vec![]]
                }
            } else {
                vec![vec![]]
            };
            board.reverse_move(reverse_move);
            for child_line in child_lines.iter_mut() {
                child_line.insert(0, mv.clone());
            }
            child_lines
        })
        .collect()
}

fn mcts_selfplay(max_time: time::Duration) {
    let mut board = <Board<5>>::default();
    let mut moves = vec![];

    let mut white_elapsed = time::Duration::default();
    let mut black_elapsed = time::Duration::default();

    while board.game_result().is_none() {
        let start_time = time::Instant::now();
        let (best_move, score) =
            search::play_move_time::<5>(board.clone(), max_time, MctsSetting::default());

        match board.side_to_move() {
            Color::White => white_elapsed += start_time.elapsed(),
            Color::Black => black_elapsed += start_time.elapsed(),
        }

        board.do_move(best_move.clone());
        moves.push(best_move.clone());
        println!(
            "{:6}: {:.3}, {:.1}s",
            best_move.to_string::<5>(),
            score,
            start_time.elapsed().as_secs_f32()
        );
        io::stdout().flush().unwrap();
    }

    println!(
        "{:.1} used by white, {:.1} for black",
        white_elapsed.as_secs_f32(),
        black_elapsed.as_secs_f32()
    );

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

fn mcts_vs_minmax(minmax_depth: u16, mcts_nodes: u64) {
    println!("Minmax depth {} vs mcts {} nodes", minmax_depth, mcts_nodes);
    let mut board = <Board<5>>::default();
    let mut moves = vec![];
    while board.game_result().is_none() {
        let num_moves = moves.len();
        if num_moves > 10 && (1..5).all(|i| moves[num_moves - i] == moves[num_moves - i - 4]) {
            break;
        }
        match board.side_to_move() {
            Color::Black => {
                let (best_move, score) = search::mcts::<5>(board.clone(), mcts_nodes);
                board.do_move(best_move.clone());
                moves.push(best_move.clone());
                println!("{:6}: {:.3}", best_move.to_string::<5>(), score);
                io::stdout().flush().unwrap();
            }

            Color::White => {
                let (best_move, score) = minmax::minmax(&mut board, minmax_depth);
                board.do_move(best_move.clone().unwrap());
                moves.push(best_move.clone().unwrap());
                print!("{:6}: {:.2}, ", best_move.unwrap().to_string::<5>(), score);
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

fn test_position<const S: usize>() {
    let mut board = <Board<S>>::default();
    let mut moves = vec![];

    println!("Enter moves:");

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();

    for mv_san in input.split_whitespace() {
        let mv = board.move_from_san(&mv_san).unwrap();
        board.generate_moves(&mut moves);
        assert!(moves.contains(&mv));
        board.do_move(mv);
        moves.clear();
    }

    println!("{:?}", board);

    let mut simple_moves = vec![];
    let mut moves = vec![];

    board.generate_moves_with_probabilities(&board.group_data(), &mut simple_moves, &mut moves);
    moves.sort_by_key(|(_mv, score)| -(score * 1000.0) as i64);

    println!("Top 10 heuristic moves:");
    for (mv, score) in moves.iter().take(10) {
        println!("{}: {:.3}", mv.to_string::<S>(), score);
        let mut coefficients = vec![0.0; <Board<S>>::policy_params().len()];
        board.coefficients_for_move(&mut coefficients, mv, &board.group_data(), moves.len());
        for coefficient in coefficients {
            print!("{:.1}, ", coefficient);
        }
        println!();
    }

    let mut tree = search::MonteCarloTree::new(board.clone());
    for i in 1.. {
        tree.select();
        if i % 100_000 == 0 {
            println!(
                "{} visits, val={:.2}%, static eval={:.4}, static winning probability={:.2}%",
                tree.visits(),
                tree.mean_action_value() * 100.0,
                board.static_eval(),
                search::cp_to_win_percentage(board.static_eval()) * 100.0
            );
            tree.print_info();
            println!("Best move: {:?}", tree.best_move())
        }
    }
}

fn analyze_game<const S: usize>(game: Game<Board<S>>) {
    let mut board = game.start_board.clone();
    let mut ply_number = 2;
    for (mv, _) in game.moves {
        board.do_move(mv.clone());
        if board.game_result().is_some() {
            break;
        }
        let (best_move, score) = search::mcts::<S>(board.clone(), 1_000_000);
        if ply_number % 2 == 0 {
            print!(
                "{}. {} {{{:.2}%, best reply {}}} ",
                ply_number / 2,
                board.move_to_san(&mv),
                (1.0 - score) * 100.0,
                best_move.to_string::<S>()
            );
        } else {
            println!(
                "{}... {} {{{:.2}%, best reply {}}}",
                ply_number / 2,
                board.move_to_san(&mv),
                (1.0 - score) * 100.0,
                best_move.to_string::<S>()
            );
        }
        ply_number += 1;
    }
}

/// Play a game against the engine through stdin
fn play_human(mut board: Board<5>) {
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
                let (best_move, score) = search::mcts::<5>(board.clone(), 1_000_000);

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
    const NODES: u64 = 1_000_000;
    let start_time = time::Instant::now();
    {
        let board = <Board<5>>::default();

        let (_move, score) = search::mcts::<5>(board, NODES);
        print!("{:.3}, ", score);
    }

    {
        let mut board = Board::default();

        do_moves_and_check_validity(&mut board, &["d3", "c3", "c4", "1d3<", "1c4+", "Sc4"]);

        let (_move, score) = search::mcts::<5>(board, NODES);
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

        let (_move, score) = search::mcts::<5>(board, NODES);
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
    println!("Tak board: {} bytes", mem::size_of::<board::Board<5>>());
    println!("Tak board cell: {} bytes", mem::size_of::<board::Stack>());
    println!("Tak move: {} bytes", mem::size_of::<board::Move>());
    println!("Zobrist keys: {}", mem::size_of::<board::ZobristKeys<5>>())
}

fn do_moves_and_check_validity(board: &mut Board<5>, move_strings: &[&str]) {
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

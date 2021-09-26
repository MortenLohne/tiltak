#[cfg(feature = "constant-tuning")]
use std::collections::HashSet;
use std::io::{Read, Write};
use std::{io, time};

use board_game_traits::{Color, GameResult};
use board_game_traits::{EvalPosition, Position as PositionTrait};
use pgn_traits::PgnPosition;
#[cfg(feature = "constant-tuning")]
use rayon::prelude::*;

use tiltak::evaluation::parameters;
use tiltak::minmax;
use tiltak::position::Move;
#[cfg(feature = "constant-tuning")]
use tiltak::position::Role;
use tiltak::position::TunableBoard;
use tiltak::position::{Position, Stack};
use tiltak::ptn::{Game, PtnMove};
use tiltak::search::MctsSetting;
use tiltak::{position, search};

#[cfg(test)]
mod tests;

pub mod playtak;
pub mod tei;

fn main() {
    println!("play: Play against the engine through the command line");
    println!("aimatch: Watch the engine play against a very simple minmax implementation");
    println!("analyze <size>: Analyze a given position, provided from a PTN or a simple move list");
    println!("tps <size>: Analyze a given position, provided from a tps string");
    println!("game <size>: Analyze a whole game, provided from a PTN or a simple move list");
    loop {
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.is_empty() {
            continue;
        }
        match words[0] {
            "play" => {
                let position = Position::default();
                play_human(position);
            }
            "aimatch" => {
                for i in 1..10 {
                    mcts_vs_minmax(3, 50000 * i);
                }
            }
            "analyze" => match words.get(1) {
                Some(&"4") => analyze_position_from_ptn::<4>(),
                Some(&"5") => analyze_position_from_ptn::<5>(),
                Some(&"6") => analyze_position_from_ptn::<6>(),
                Some(&"7") => analyze_position_from_ptn::<7>(),
                Some(&"8") => analyze_position_from_ptn::<8>(),
                _ => analyze_position_from_ptn::<5>(),
            },
            "tps" => match words.get(1) {
                Some(&"4") => analyze_position_from_tps::<4>(),
                Some(&"5") => analyze_position_from_tps::<5>(),
                Some(&"6") => analyze_position_from_tps::<6>(),
                Some(&"7") => analyze_position_from_tps::<7>(),
                Some(&"8") => analyze_position_from_tps::<8>(),
                _ => analyze_position_from_tps::<5>(),
            },
            #[cfg(feature = "constant-tuning")]
            "openings" => {
                let depth = 4;
                let mut positions = HashSet::new();
                let openings =
                    generate_openings::<6>(Position::start_position(), &mut positions, depth);
                println!("{} openings generated, evaluating...", openings.len());

                let mut evaled_openings: Vec<_> = openings
                    .into_par_iter()
                    .filter(|opening| opening.len() == depth as usize)
                    .map(|opening| {
                        let mut position = <Position<6>>::start_position();
                        for mv in opening.iter() {
                            position.do_move(mv.clone());
                        }
                        (opening, search::mcts(position, 100_000))
                    })
                    .collect();

                evaled_openings.sort_by(|(_, (_, score1)), (_, (_, score2))| {
                    score1.partial_cmp(score2).unwrap()
                });
                for (p, (mv, s)) in evaled_openings {
                    let mut position = <Position<6>>::start_position();
                    for mv in p {
                        print!("{} ", position.move_to_san(&mv));
                        position.do_move(mv);
                    }
                    print!(": ");
                    println!("{}, {}", position.move_to_san(&mv), s);
                }
                return;
            }
            "analyze_openings" => analyze_openings::<6>(6_000_000),
            "game" => {
                println!("Enter move list or a full PTN, then press enter followed by CTRL+D");
                let mut input = String::new();

                match words.get(1) {
                    Some(&"6") => {
                        io::stdin().read_to_string(&mut input).unwrap();
                        let games = tiltak::ptn::ptn_parser::parse_ptn(&input).unwrap();
                        if games.is_empty() {
                            continue;
                        }
                        println!("Analyzing 1 game: ");

                        analyze_game::<6>(games[0].clone());
                    }
                    None | Some(&"5") => {
                        io::stdin().read_to_string(&mut input).unwrap();
                        let games = tiltak::ptn::ptn_parser::parse_ptn(&input).unwrap();
                        if games.is_empty() {
                            println!("Couldn't parse any games");
                            continue;
                        }
                        println!("Analyzing 1 game: ");

                        analyze_game::<5>(games[0].clone());
                    }
                    Some(s) => println!("Game analysis at size {} not available", s),
                }
            }
            "mem_usage" => mem_usage(),
            "bench" => bench(),
            "selfplay" => mcts_selfplay(time::Duration::from_secs(10)),
            s => println!("Unknown option \"{}\"", s),
        }
    }
}

fn analyze_openings<const S: usize>(nodes: u64) {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    for line in input.lines() {
        let mut position = <Position<6>>::start_position();
        for word in line.split_whitespace() {
            let mv = position.move_from_san(word).unwrap();
            position.do_move(mv);
        }
        let start_time = time::Instant::now();
        let mut tree = search::MonteCarloTree::new(position.clone());
        for _ in 0..nodes {
            tree.select();
        }
        let pv: Vec<Move> = tree.pv().take(4).collect();
        print!(
            "{}, {:.3}, {:.1}s, ",
            line.trim(),
            tree.best_move().1,
            start_time.elapsed().as_secs_f32()
        );
        for mv in pv {
            print!("{} ", position.move_to_san(&mv));
            position.do_move(mv);
        }
        println!();
    }
}

#[cfg(feature = "constant-tuning")]
fn generate_openings<const S: usize>(
    mut position: Position<S>,
    positions: &mut HashSet<Position<S>>,
    depth: u8,
) -> Vec<Vec<Move>> {
    let mut moves = vec![];
    position.generate_moves(&mut moves);
    moves = moves
        .into_iter()
        .filter(|mv| matches!(mv, Move::Place(Role::Flat, _)))
        .collect();
    moves
        .into_iter()
        .flat_map(|mv| {
            let reverse_move = position.do_move(mv.clone());
            let mut child_lines = if position
                .symmetries()
                .iter()
                .all(|board_symmetry| !positions.contains(board_symmetry))
            {
                positions.insert(position.clone());
                if depth > 1 {
                    generate_openings(position.clone(), positions, depth - 1)
                } else {
                    vec![vec![]]
                }
            } else {
                vec![]
            };
            position.reverse_move(reverse_move);
            for child_line in child_lines.iter_mut() {
                child_line.insert(0, mv.clone());
            }
            child_lines
        })
        .collect()
}

fn mcts_selfplay(max_time: time::Duration) {
    let mut position = <Position<5>>::default();
    let mut moves = vec![];

    let mut white_elapsed = time::Duration::default();
    let mut black_elapsed = time::Duration::default();

    while position.game_result().is_none() {
        let start_time = time::Instant::now();
        let (best_move, score) =
            search::play_move_time::<5>(position.clone(), max_time, MctsSetting::default());

        match position.side_to_move() {
            Color::White => white_elapsed += start_time.elapsed(),
            Color::Black => black_elapsed += start_time.elapsed(),
        }

        position.do_move(best_move.clone());
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

    println!("\n{:?}\nResult: {:?}", position, position.game_result());
}

fn mcts_vs_minmax(minmax_depth: u16, mcts_nodes: u64) {
    println!("Minmax depth {} vs mcts {} nodes", minmax_depth, mcts_nodes);
    let mut position = <Position<5>>::default();
    let mut moves = vec![];
    while position.game_result().is_none() {
        let num_moves = moves.len();
        if num_moves > 10 && (1..5).all(|i| moves[num_moves - i] == moves[num_moves - i - 4]) {
            break;
        }
        match position.side_to_move() {
            Color::Black => {
                let (best_move, score) = search::mcts::<5>(position.clone(), mcts_nodes);
                position.do_move(best_move.clone());
                moves.push(best_move.clone());
                println!("{:6}: {:.3}", best_move.to_string::<5>(), score);
                io::stdout().flush().unwrap();
            }

            Color::White => {
                let (best_move, score) = minmax::minmax(&mut position, minmax_depth);
                position.do_move(best_move.clone().unwrap());
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

    println!("\n{:?}\nResult: {:?}", position, position.game_result());
}

fn analyze_position_from_ptn<const S: usize>() {
    println!("Enter move list or a full PTN, then press enter followed by CTRL+D");

    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let games: Vec<Game<Position<S>>> = tiltak::ptn::ptn_parser::parse_ptn(&input).unwrap();
    if games.is_empty() {
        println!("Couldn't parse any games");
        return;
    }

    let mut position: Position<S> = games[0].start_position.clone();

    for PtnMove { mv, .. } in games[0].moves.clone() {
        position.do_move(mv);
    }
    analyze_position(&position)
}

fn analyze_position_from_tps<const S: usize>() {
    println!("Enter TPS");
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let position = <Position<S>>::from_fen(&input).unwrap();
    analyze_position(&position)
}

fn analyze_position<const S: usize>(position: &Position<S>) {
    println!("TPS {}", position.to_fen());
    println!("{:?}", position);

    assert_eq!(position.game_result(), None, "Cannot analyze finished game");

    let mut simple_moves = vec![];
    let mut moves = vec![];
    let mut features = vec![
        0.0;
        match S {
            4 => parameters::NUM_POLICY_FEATURES_4S,
            5 => parameters::NUM_POLICY_FEATURES_5S,
            6 => parameters::NUM_POLICY_FEATURES_6S,
            _ => unimplemented!(),
        }
    ];

    position.generate_moves_with_probabilities(
        &position.group_data(),
        &mut simple_moves,
        &mut moves,
        &mut features,
    );
    moves.sort_by_key(|(_mv, score)| -(score * 1000.0) as i64);

    println!("Top 10 heuristic moves:");
    for (mv, score) in moves.iter().take(10) {
        println!("{}: {:.3}%", mv.to_string::<S>(), score * 100.0);
        let mut features = vec![0.0; <Position<S>>::policy_params().len()];
        position.features_for_move(&mut features, mv, &position.group_data());
        for feature in features {
            print!("{:.1}, ", feature);
        }
        println!();
    }
    let settings: MctsSetting<S> = search::MctsSetting::default().exclude_moves(vec![]);
    let start_time = time::Instant::now();

    let mut tree = search::MonteCarloTree::with_settings(position.clone(), settings);
    for i in 1.. {
        tree.select();
        if i % 100_000 == 0 {
            println!(
                "{} visits, val={:.2}%, static eval={:.4}, static winning probability={:.2}%, {:.2}s",
                tree.visits(),
                tree.mean_action_value() * 100.0,
                position.static_eval(),
                search::cp_to_win_percentage(position.static_eval()) * 100.0,
                start_time.elapsed().as_secs_f64()
            );
            tree.print_info();
            println!("Best move: {:?}", tree.best_move())
        }
    }
}

fn analyze_game<const S: usize>(game: Game<Position<S>>) {
    let mut position = game.start_position.clone();
    let mut ply_number = 2;
    for PtnMove { mv, .. } in game.moves {
        position.do_move(mv.clone());
        if let Some(game_result) = position.game_result() {
            let result_string = match game_result {
                GameResult::WhiteWin => "1-0",
                GameResult::BlackWin => "0-1",
                GameResult::Draw => "1/2-1/2",
            };
            if ply_number % 2 == 0 {
                print!(
                    "{}. {} {}",
                    ply_number / 2,
                    mv.to_string::<S>(),
                    result_string
                );
                io::stdout().flush().unwrap();
            } else {
                println!(
                    "{}... {} {}",
                    ply_number / 2,
                    mv.to_string::<S>(),
                    result_string
                );
            }
        } else {
            let (best_move, score) = search::mcts::<S>(position.clone(), 1_000_000);
            if ply_number % 2 == 0 {
                print!(
                    "{}. {} {{{:.2}%, best reply {}}} ",
                    ply_number / 2,
                    position.move_to_san(&mv),
                    (1.0 - score) * 100.0,
                    best_move.to_string::<S>()
                );
                io::stdout().flush().unwrap();
            } else {
                println!(
                    "{}... {} {{{:.2}%, best reply {}}}",
                    ply_number / 2,
                    position.move_to_san(&mv),
                    (1.0 - score) * 100.0,
                    best_move.to_string::<S>()
                );
            }
        }
        ply_number += 1;
    }
}

/// Play a game against the engine through stdin
fn play_human(mut position: Position<5>) {
    match position.game_result() {
        None => {
            use board_game_traits::Color::*;
            println!("Position:\n{:?}", position);
            // If black, play as human
            if position.side_to_move() == Black {
                println!("Type your move in algebraic notation (c3):");

                let reader = io::stdin();
                let mut input_str = "".to_string();
                let mut legal_moves = vec![];
                position.generate_moves(&mut legal_moves);
                // Loop until user enters a valid move
                loop {
                    input_str.clear();
                    reader
                        .read_line(&mut input_str)
                        .expect("Failed to read line");

                    match position.move_from_san(input_str.trim()) {
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
                let c_move = position.move_from_san(input_str.trim()).unwrap();
                position.do_move(c_move);
            } else {
                let (best_move, score) = search::mcts::<5>(position.clone(), 1_000_000);

                println!("Computer played {:?} with score {}", best_move, score);
                position.do_move(best_move);
            }
            play_human(position);
        }

        Some(GameResult::WhiteWin) => println!("White won! Board:\n{:?}", position),
        Some(GameResult::BlackWin) => println!("Black won! Board:\n{:?}", position),
        Some(GameResult::Draw) => println!("The game was drawn! Board:\n{:?}", position),
    }
}

fn bench() {
    const NODES: u64 = 1_000_000;
    let start_time = time::Instant::now();
    {
        let position = <Position<5>>::default();

        let (_move, score) = search::mcts::<5>(position, NODES);
        print!("{:.3}, ", score);
    }

    {
        let mut position = Position::default();

        do_moves_and_check_validity(&mut position, &["d3", "c3", "c4", "1d3<", "1c4+", "Sc4"]);

        let (_move, score) = search::mcts::<5>(position, NODES);
        print!("{:.3}, ", score);
    }
    {
        let mut position = Position::default();

        do_moves_and_check_validity(
            &mut position,
            &[
                "c2", "c3", "d3", "b3", "c4", "1c2-", "1d3<", "1b3>", "1c4+", "Cc2", "a1", "1c2-",
                "a2",
            ],
        );

        let (_move, score) = search::mcts::<5>(position, NODES);
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
    println!(
        "Tak board: {} bytes",
        mem::size_of::<position::Position<5>>()
    );
    println!("Tak board cell: {} bytes", mem::size_of::<Stack>());
    println!("Tak move: {} bytes", mem::size_of::<Move>());
    println!("MCTS edge 6s: {} bytes", search::edge_mem_usage());
    println!("MCTS node 6s: {} bytes", search::node_mem_usage());
    println!(
        "Zobrist keys 5s: {} bytes",
        mem::size_of::<position::ZobristKeys<5>>()
    );
    println!(
        "Zobrist keys 6s: {} bytes",
        mem::size_of::<position::ZobristKeys<6>>()
    );
}

fn do_moves_and_check_validity(position: &mut Position<5>, move_strings: &[&str]) {
    let mut moves = vec![];
    for mv_san in move_strings.iter() {
        let mv = position.move_from_san(mv_san).unwrap();
        position.generate_moves(&mut moves);
        assert!(
            moves.contains(&mv),
            "Move {} was not among legal moves: {:?}\n{:?}",
            position.move_to_san(&mv),
            moves,
            position
        );
        position.do_move(mv);
        moves.clear();
    }
}

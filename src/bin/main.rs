#[cfg(feature = "constant-tuning")]
use std::collections::HashSet;
use std::io::{Read, Write};
use std::str::FromStr;
#[cfg(feature = "constant-tuning")]
use std::sync::atomic::{self, AtomicU64};
use std::{fs, io, time};

use board_game_traits::{Color, GameResult};
use board_game_traits::{EvalPosition, Position as PositionTrait};
use half::f16;
use pgn_traits::PgnPosition;
#[cfg(feature = "constant-tuning")]
use rayon::prelude::*;

use tiltak::evaluation::parameters::{self, IncrementalPolicy, PolicyIndexes, ValueIndexes};
#[cfg(feature = "sqlite")]
use tiltak::policy_sqlite;
use tiltak::position::Role;
use tiltak::position::{
    squares_iterator, AbstractBoard, Direction, Komi, Move, Square, SquareCacheEntry,
};
use tiltak::position::{Position, Stack};
use tiltak::ptn::{Game, PtnMove};
use tiltak::search::MctsSetting;
use tiltak::{minmax, ptn};
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
    println!(
        "perft <size>: Generate perft numbers of a given position, provided from a tps string"
    );
    #[cfg(feature = "sqlite")]
    println!("test_policy: Test how well policy scores find immediate wins in real games");
    loop {
        let mut input = String::new();
        let bytes_read = io::stdin().read_line(&mut input).unwrap();
        if bytes_read == 0 {
            break;
        }
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.is_empty() {
            continue;
        }
        let komi = words
            .get(2)
            .map(|komi_str| Komi::from_str(komi_str).unwrap())
            .unwrap_or_default();
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
                Some(&"4") => analyze_position_from_ptn::<4>(komi),
                Some(&"5") => analyze_position_from_ptn::<5>(komi),
                Some(&"6") => analyze_position_from_ptn::<6>(komi),
                Some(&"7") => analyze_position_from_ptn::<7>(komi),
                Some(&"8") => analyze_position_from_ptn::<8>(komi),
                Some(s) => println!("Unsupported size {}", s),
                None => analyze_position_from_ptn::<5>(komi),
            },
            "tps" => match words.get(1) {
                Some(&"4") => analyze_position_from_tps::<4>(komi),
                Some(&"5") => analyze_position_from_tps::<5>(komi),
                Some(&"6") => analyze_position_from_tps::<6>(komi),
                Some(&"7") => analyze_position_from_tps::<7>(komi),
                Some(&"8") => analyze_position_from_tps::<8>(komi),
                Some(s) => println!("Unsupported size {}", s),
                None => analyze_position_from_tps::<5>(komi),
            },
            "perft" => match words.get(1) {
                Some(&"3") => perft_from_tps::<3>(),
                Some(&"4") => perft_from_tps::<4>(),
                Some(&"5") => perft_from_tps::<5>(),
                Some(&"6") => perft_from_tps::<6>(),
                Some(&"7") => perft_from_tps::<7>(),
                Some(&"8") => perft_from_tps::<8>(),
                Some(s) => println!("Unsupported size {}", s),
                None => perft_from_tps::<5>(),
            },
            #[cfg(feature = "constant-tuning")]
            "openings" => {
                let depth = 4;
                let mut positions = HashSet::new();
                let openings = generate_openings::<6>(
                    &mut Position::start_position_with_komi(komi),
                    &mut positions,
                    depth,
                );
                println!("{} openings generated, evaluating...", openings.len());

                let start_time = time::Instant::now();
                let evaled: AtomicU64 = AtomicU64::default();

                let mut evaled_openings: Vec<_> = openings
                    .into_par_iter()
                    .filter(|opening| opening.len() == depth as usize)
                    .map(|opening| {
                        let mut position = Position::start_position_with_komi(komi);
                        for mv in opening.iter() {
                            position.do_move(*mv);
                        }
                        let result = (opening, search::mcts(position, 100_000));
                        let total = evaled.fetch_add(1, atomic::Ordering::Relaxed);
                        if total % 1000 == 0 {
                            eprintln!(
                                "Evaluted {} openings in {}s",
                                total,
                                start_time.elapsed().as_secs()
                            );
                        }
                        result
                    })
                    .collect();

                evaled_openings.sort_by(|(_, (_, score1)), (_, (_, score2))| {
                    score1.partial_cmp(score2).unwrap()
                });
                for (p, (mv, s)) in evaled_openings {
                    let mut position = Position::start_position_with_komi(komi);
                    for mv in p {
                        print!("{} ", position.move_to_san(&mv));
                        position.do_move(mv);
                    }
                    print!(": ");
                    println!("{}, {}", position.move_to_san(&mv), s);
                }
                return;
            }
            #[cfg(feature = "constant-tuning")]
            "analyze_openings" => analyze_openings::<6>(komi, 500_000),
            #[cfg(feature = "sqlite")]
            "test_policy" => policy_sqlite::check_all_games(),
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
            "mem_usage" => mem_usage::<6>(),
            "bench" => bench::<6>(),
            "bench_old" => bench_old(),
            "selfplay" => mcts_selfplay(time::Duration::from_secs(10)),
            "process_ptn" => process_ptn::<6>("games_6s_2komi_all.ptn"),
            "value_params" => {
                if words.len() < 3 {
                    println!("Error: format is 'value_params <size> <komi>'");
                    continue;
                }
                let komi = Komi::from_str(words[2]).unwrap();
                match words[1] {
                    "4" => value_params::<4>(komi),
                    "5" => value_params::<5>(komi),
                    "6" => value_params::<6>(komi),
                    s => panic!("Unsupported size {}", s),
                }
            }
            "policy_params" => {
                if words.len() < 3 {
                    println!("Error: format is 'policy_params <size> <komi>'");
                    continue;
                }
                let komi = Komi::from_str(words[2]).unwrap();
                match words[1] {
                    "4" => policy_params::<4>(komi),
                    "5" => policy_params::<5>(komi),
                    "6" => policy_params::<6>(komi),
                    s => panic!("Unsupported size {}", s),
                }
            }
            s => println!("Unknown option \"{}\"", s),
        }
    }
}

fn value_params<const S: usize>(komi: Komi) {
    let indexes: ValueIndexes<S> = parameters::value_indexes();
    let indexes_string = format!("{:?}", indexes);

    let params = match S {
        4 => parameters::value_features_4s(komi).as_slice(),
        5 => parameters::value_features_5s(komi).as_slice(),
        6 => parameters::value_features_6s(komi).as_slice(),
        _ => panic!("Unsupported size {}", S),
    };
    let black_start_index = params.len() / 2;

    println!("\nValue features:\n");

    for part in indexes_string
        .strip_prefix("ValueIndexes { ")
        .unwrap()
        .split("},")
    {
        let words: Vec<&str> = part.split_whitespace().collect();
        let name = words[0].strip_suffix(':').unwrap_or_default();
        let start: usize = words[4]
            .strip_suffix(',')
            .unwrap()
            .parse()
            .unwrap_or_default();
        let length: usize = words[6].parse().unwrap_or_default();

        println!("{} (white): {:?}", name, &params[start..(start + length)]);
        println!(
            "{} (black): {:?}",
            name,
            &params[(start + black_start_index)..(start + black_start_index + length)]
        );
        println!();
    }
}

fn policy_params<const S: usize>(komi: Komi) {
    let indexes: PolicyIndexes<S> = parameters::policy_indexes();
    let indexes_string = format!("{:?}", indexes);

    let params = match S {
        4 => parameters::policy_features_4s(komi).as_slice(),
        5 => parameters::policy_features_5s(komi).as_slice(),
        6 => parameters::policy_features_6s(komi).as_slice(),
        _ => panic!("Unsupported size {}", S),
    };

    println!("\nPolicy features:\n");

    for part in indexes_string
        .strip_prefix("PolicyIndexes { ")
        .unwrap()
        .split("},")
    {
        let words: Vec<&str> = part.split_whitespace().collect();
        let name = words[0].strip_suffix(':').unwrap_or_default();
        let start: usize = words[4]
            .strip_suffix(',')
            .unwrap()
            .parse()
            .unwrap_or_default();
        let length: usize = words[6].parse().unwrap_or_default();

        println!("{}: {:?}", name, &params[start..(start + length)]);
    }
}

fn process_ptn<const S: usize>(path: &str) {
    let ptn_contents = fs::read_to_string(path).unwrap();
    let games = ptn::ptn_parser::parse_ptn::<Position<S>>(&ptn_contents).unwrap();
    println!("Processing {} games", games.len());

    #[derive(Default, Clone, Copy, Debug)]
    struct MoveStats {
        flat_placements: u64,
        wall_placements: u64,
        cap_placements: u64,
        movements: u64,
        legal_placements: u64,
        legal_movements: u64,
        legal_moves_from_stack: u64,
    }

    impl MoveStats {
        fn total_played(&self) -> u64 {
            self.flat_placements + self.wall_placements + self.cap_placements + self.movements
        }

        fn total_legal(&self) -> u64 {
            self.legal_placements + self.legal_movements
        }
    }

    let mut per_move_stats = [MoveStats::default(); 400];

    for game in games {
        let mut position = game.start_position;
        for (i, mv) in game.moves.into_iter().enumerate() {
            let mut moves = vec![];
            position.generate_moves(&mut moves);
            let num_legal_placements = moves.iter().filter(|mv| mv.is_placement()).count();
            per_move_stats[i].legal_placements += num_legal_placements as u64;
            per_move_stats[i].legal_movements += moves.len() as u64 - num_legal_placements as u64;
            match mv.mv.expand() {
                position::ExpMove::Place(Role::Flat, _) => per_move_stats[i].flat_placements += 1,
                position::ExpMove::Place(Role::Wall, _) => per_move_stats[i].wall_placements += 1,
                position::ExpMove::Place(Role::Cap, _) => per_move_stats[i].cap_placements += 1,
                position::ExpMove::Move(_, _, _) => per_move_stats[i].movements += 1,
            }
            if !mv.mv.is_placement() {
                let origin_square = mv.mv.origin_square();
                let moves_in_stack = moves
                    .iter()
                    .filter(|mv| mv.origin_square() == origin_square)
                    .count();
                per_move_stats[i].legal_moves_from_stack += moves_in_stack as u64;
            }
            position.do_move(mv.mv);
        }
    }

    let total_stats = per_move_stats
        .iter()
        .cloned()
        .reduce(|a, b| MoveStats {
            flat_placements: a.flat_placements + b.flat_placements,
            wall_placements: a.wall_placements + b.wall_placements,
            cap_placements: a.cap_placements + b.cap_placements,
            movements: a.movements + b.movements,
            legal_placements: a.legal_placements + b.legal_placements,
            legal_movements: a.legal_movements + b.legal_movements,
            legal_moves_from_stack: a.legal_moves_from_stack + b.legal_moves_from_stack,
        })
        .unwrap();

    for (i, stats) in per_move_stats.iter().take(150).enumerate() {
        let total = stats.total_played();
        let placements = stats.total_played() - stats.movements;
        let average_placement_likelihood = placements as f32 / stats.legal_placements as f32;
        let average_movement_likelihood = stats.movements as f32 / stats.legal_movements as f32;
        let color = if i % 2 == 0 {
            Color::White
        } else {
            Color::Black
        };
        println!("{}'s move {}:", color, i / 2 + 1);
        println!("{:.1}% placements, {:.1}% movements, {:.1}% placements among legal moves, ({:.2}%, {:.2}%) chance for each (placement, movement), {:.2} moves in the played stack, {:.1} legal moves per position",
            (100 * placements) as f32 / total as f32,
            (100 * stats.movements) as f32 / total as f32,
            (100 * stats.legal_placements) as f32 / stats.total_legal() as f32,
            (100 * placements) as f32 / stats.legal_placements as f32,
            (100 * stats.movements) as f32 / stats.legal_movements as f32,
            stats.legal_moves_from_stack as f32 / stats.movements as f32,
            stats.total_legal() as f32 / total as f32
        );
        println!(
            "{:.2}% placement likelihood, {:.2}% movement likelihood, {:.2} ratio, {} total games",
            100.0 * average_placement_likelihood,
            100.0 * average_movement_likelihood,
            average_placement_likelihood / average_movement_likelihood,
            total
        );
    }
    let total_played = total_stats.total_played();
    println!(
        "Total stats: {:.1}% placement, {:.1}% movements, {} total games",
        (100 * (total_played - total_stats.movements)) as f32 / total_played as f32,
        (100 * total_stats.movements) as f32 / total_played as f32,
        total_played
    )
}

#[cfg(feature = "constant-tuning")]
fn analyze_openings<const S: usize>(komi: Komi, nodes: u32) {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();

    let openings: Vec<(Position<S>, Vec<&str>)> = input
        .lines()
        .map(|line| {
            let mut position = <Position<S>>::start_position_with_komi(komi);
            let words: Vec<&str> = line
                .split_whitespace()
                .take_while(|word| !word.contains(':'))
                .collect();
            for word in words.iter() {
                let mv = position.move_from_san(word).unwrap();
                position.do_move(mv);
            }
            (position, words)
        })
        .collect();

    eprintln!("Read {} openings. Check for duplicates...", openings.len());

    let mut unique_openings: Vec<(String, Vec<&str>)> = vec![];

    for (position, opening_moves) in openings {
        if position
            .symmetries_with_swapped_colors()
            .into_iter()
            .all(|rotation| {
                unique_openings
                    .iter()
                    .all(|(unique_tps, _)| *unique_tps != rotation.to_fen())
            })
        {
            unique_openings.push((position.to_fen(), opening_moves));
        }
    }

    eprintln!("Got {} truly unique openings", unique_openings.len());

    input
        .lines()
        .flat_map(|line| line.split(':').take(1))
        .par_bridge()
        .for_each(|line| {
            let mut position = <Position<S>>::start_position_with_komi(komi);
            for word in line
                .split_whitespace()
                .take_while(|word| !word.contains(':'))
            {
                let mv = position.move_from_san(word).unwrap();
                position.do_move(mv);
            }
            let start_time = time::Instant::now();
            let settings = search::MctsSetting::default().arena_size_for_nodes(nodes);
            let mut tree = search::MonteCarloTree::with_settings(position.clone(), settings);
            for _ in 0..nodes {
                if tree.select().is_none() {
                    eprintln!("Warning: Search stopped early due to OOM");
                    break;
                };
            }
            let pv: Vec<Move<S>> = tree.pv().take(4).collect();
            print!(
                "{}: {:.4}, {:.1}s, ",
                line.trim(),
                tree.best_move().1,
                start_time.elapsed().as_secs_f32()
            );
            for mv in pv {
                print!("{} ", position.move_to_san(&mv));
                position.do_move(mv);
            }
            println!();
        });
}

#[cfg(feature = "constant-tuning")]
fn generate_openings<const S: usize>(
    position: &mut Position<S>,
    positions: &mut HashSet<Position<S>>,
    depth: u8,
) -> Vec<Vec<Move<S>>> {
    use tiltak::position::ExpMove;

    let mut moves = vec![];
    position.generate_moves(&mut moves);
    moves.retain(|mv| matches!(mv.expand(), ExpMove::Place(Role::Flat, _)));
    moves
        .into_iter()
        .flat_map(|mv| {
            let reverse_move = position.do_move(mv);
            let mut child_lines = if position
                .symmetries()
                .iter()
                .all(|board_symmetry| !positions.contains(board_symmetry))
            {
                positions.insert(position.clone());
                if depth > 1 {
                    generate_openings(position, positions, depth - 1)
                } else {
                    vec![vec![]]
                }
            } else {
                vec![]
            };
            position.reverse_move(reverse_move);
            for child_line in child_lines.iter_mut() {
                child_line.insert(0, mv);
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

        position.do_move(best_move);
        moves.push(best_move);
        println!(
            "{:6}: {:.3}, {:.1}s",
            best_move.to_string(),
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
                position.do_move(best_move);
                moves.push(best_move);
                println!("{:6}: {:.3}", best_move.to_string(), score);
                io::stdout().flush().unwrap();
            }

            Color::White => {
                let (best_move, score) = minmax::minmax(&mut position, minmax_depth);
                position.do_move(best_move.unwrap());
                moves.push(best_move.unwrap());
                print!("{:6}: {:.2}, ", best_move.unwrap().to_string(), score);
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

fn analyze_position_from_ptn<const S: usize>(komi: Komi) {
    println!("Enter move list or a full PTN, then press enter followed by CTRL+D");

    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let games: Vec<Game<Position<S>>> = tiltak::ptn::ptn_parser::parse_ptn(&input).unwrap();
    if games.is_empty() {
        println!("Couldn't parse any games");
        return;
    }

    let mut position: Position<S> = games[0].start_position.clone();
    position.set_komi(komi);

    for PtnMove { mv, .. } in games[0].moves.clone() {
        position.do_move(mv);
    }
    analyze_position(&position)
}

fn analyze_position_from_tps<const S: usize>(komi: Komi) {
    println!("Enter TPS");
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let position = <Position<S>>::from_fen_with_komi(&input, komi).unwrap();
    analyze_position(&position)
}

fn analyze_position<const S: usize>(position: &Position<S>) {
    println!("TPS {}", position.to_fen());
    println!("{:?}", position);
    println!("Komi: {}", position.komi());

    // Change which sets of eval parameters to use in search
    // Can be different from the komi used to determine the game result at terminal nodes
    let eval_komi = position.komi();

    assert_eq!(position.game_result(), None, "Cannot analyze finished game");

    let mut simple_moves = vec![];
    let mut moves = vec![];
    let mut fcd_per_move = vec![];

    position.generate_moves_with_probabilities::<IncrementalPolicy<S>>(
        &position.group_data(),
        &mut simple_moves,
        &mut moves,
        &mut fcd_per_move,
        <Position<S>>::policy_params(eval_komi),
        &mut vec![],
    );
    moves.sort_by(|(_mv, score1), (_, score2)| score1.partial_cmp(score2).unwrap().reverse());

    let settings: MctsSetting<S> = search::MctsSetting::default()
        .arena_size(2_u32.pow(30) * 3)
        .exclude_moves(vec![]);
    let start_time = time::Instant::now();

    let mut tree = search::MonteCarloTree::with_settings(position.clone(), settings);
    for i in 1.. {
        if tree.select().is_none() {
            println!("Search stopped due to OOM");
            break;
        };
        if i % 100_000 == 0 {
            let static_eval: f32 =
                position.static_eval() * position.side_to_move().multiplier() as f32;
            println!(
                "{} visits, eval: {:.2}%, Wilem-style eval: {:+.2}, static eval: {:.4}, static winning probability: {:.2}%, {:.2}s",
                tree.visits(),
                tree.mean_action_value() * 100.0,
                tree.mean_action_value() * 2.0 - 1.0,
                static_eval,
                search::cp_to_win_percentage(static_eval) * 100.0,
                start_time.elapsed().as_secs_f64()
            );
            tree.print_info();
            let (mv, value) = tree.best_move();
            println!("Best move: ({}, {})", mv, value);
        }
    }
}

fn perft_from_tps<const S: usize>() {
    println!("Enter TPS (or leave empty for initial)");
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let mut position = if input.trim().is_empty() {
        <Position<S>>::default()
    } else {
        <Position<S>>::from_fen(&input).unwrap()
    };
    perft(&mut position);
}

fn perft<const S: usize>(position: &mut Position<S>) {
    for depth in 0.. {
        let start_time = time::Instant::now();
        let result = position.bulk_perft(depth);
        println!(
            "{}: {}, {:.2}s, {:.1} Mnps",
            depth,
            result,
            start_time.elapsed().as_secs_f32(),
            result as f32 / start_time.elapsed().as_micros() as f32
        );
    }
}

fn analyze_game<const S: usize>(game: Game<Position<S>>) {
    let mut position = game.start_position.clone();
    let mut ply_number = 2;
    for PtnMove { mv, .. } in game.moves {
        position.do_move(mv);
        if let Some(game_result) = position.game_result() {
            let result_string = match game_result {
                GameResult::WhiteWin => "1-0",
                GameResult::BlackWin => "0-1",
                GameResult::Draw => "1/2-1/2",
            };
            if ply_number % 2 == 0 {
                print!("{}. {} {}", ply_number / 2, mv, result_string);
                io::stdout().flush().unwrap();
            } else {
                println!("{}... {} {}", ply_number / 2, mv, result_string);
            }
        } else {
            let (best_move, score) = search::mcts(position.clone(), 1_000_000);
            if ply_number % 2 == 0 {
                print!(
                    "{}. {} {{{:.2}%, best reply {}}} ",
                    ply_number / 2,
                    position.move_to_san(&mv),
                    (1.0 - score) * 100.0,
                    best_move
                );
                io::stdout().flush().unwrap();
            } else {
                println!(
                    "{}... {} {{{:.2}%, best reply {}}}",
                    ply_number / 2,
                    position.move_to_san(&mv),
                    (1.0 - score) * 100.0,
                    best_move
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

fn bench<const S: usize>() {
    println!("Starting benchmark");
    const NODES: u32 = 5_000_000;
    let start_time = time::Instant::now();

    let mut position = <Position<S>>::default();

    // Start the benchmark from opposite corners opening
    let corner = squares_iterator().next().unwrap();
    let opposite_corner = squares_iterator().last().unwrap();
    position.do_move(Move::placement(Role::Flat, corner));
    position.do_move(Move::placement(Role::Flat, opposite_corner));

    let settings = search::MctsSetting::default().arena_size_for_nodes(NODES);
    let mut tree = search::MonteCarloTree::with_settings(position, settings);
    let mut last_iteration_start_time = time::Instant::now();
    for n in 1..=NODES {
        tree.select().unwrap();
        if n % 500_000 == 0 {
            let knps = 500.0 / last_iteration_start_time.elapsed().as_secs_f32();
            last_iteration_start_time = time::Instant::now();
            println!(
                "n={}, {:.2}s, {:.1} knps",
                n,
                start_time.elapsed().as_secs_f32(),
                knps
            );
        }
    }

    let (mv, score) = tree.best_move();
    let knps = 5000.0 / start_time.elapsed().as_secs_f32();

    println!(
        "{}: {:.2}%, {:.2}s, {:.1} knps",
        mv,
        score * 100.0,
        start_time.elapsed().as_secs_f32(),
        knps,
    );
}

fn bench_old() {
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
fn mem_usage<const S: usize>() {
    use std::mem;
    println!(
        "{}s tak board: {} bytes",
        S,
        mem::size_of::<position::Position<S>>()
    );
    println!("Tak board cell: {} bytes", mem::size_of::<Stack>());
    println!("Tak move: {} bytes", mem::size_of::<Move<S>>());
    println!("MCTS edge {}s: {} bytes", S, search::edge_mem_usage::<S>());
    println!("MCTS node {}s: {} bytes", S, search::node_mem_usage::<S>());
    println!("f16: {} bytes", mem::size_of::<f16>());
    println!(
        "Zobrist keys 5s: {} bytes",
        mem::size_of::<position::ZobristKeys<5>>()
    );
    println!(
        "Zobrist keys 6s: {} bytes",
        mem::size_of::<position::ZobristKeys<6>>()
    );
    println!(
        "Direction {} bytes, optional direction {} bytes",
        mem::size_of::<Direction>(),
        mem::size_of::<Option<Direction>>()
    );
    println!(
        "{}s square {} bytes, square cache entry: {} bytes, square cache table {} bytes",
        S,
        mem::size_of::<Square<S>>(),
        mem::size_of::<SquareCacheEntry<S>>(),
        mem::size_of::<AbstractBoard<SquareCacheEntry<6>, 6>>(),
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

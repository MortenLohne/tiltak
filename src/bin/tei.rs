#![allow(clippy::uninlined_format_args)]

use board_game_traits::{Color, Position as PositionTrait};
use pgn_traits::PgnPosition;
use std::io::{BufRead, BufReader};
use std::str::FromStr;
use std::sync::atomic::{self, AtomicBool};
use std::sync::{mpsc, Arc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use std::{env, io, mem, process};
use tiltak::position::{Komi, Move, Position};

use tiltak::search::{MctsSetting, MonteCarloTree};

pub fn tei_game<const S: usize>(
    input: mpsc::Receiver<String>,
    mcts_settings: MctsSetting<S>,
    komi: Komi,
) {
    let mut position: Option<SearchPosition<S>> = None;

    let mut last_position_searched: Option<SearchPosition<S>> = None;
    let mut search_tree: Option<MonteCarloTree<S>> = None;

    let should_stop: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    let mut calculating_handle: Option<JoinHandle<MonteCarloTree<S>>> = None;

    while let Ok(line) = input.recv() {
        let mut words = line.split_whitespace();
        match words.next().unwrap() {
            "quit" => {
                should_stop.store(true, atomic::Ordering::Relaxed);
                process::exit(0);
            }
            "stop" => {
                should_stop.store(true, atomic::Ordering::Relaxed);
                if let Some(handle) = calculating_handle.take() {
                    search_tree = Some(handle.join().unwrap());
                }
                should_stop.store(false, atomic::Ordering::Relaxed);
            }
            "isready" => println!("readyok"),
            "setoption" | "teinewgame" => {
                unreachable!() // This should be handled by the main loop
            }
            "position" => {
                position = Some(parse_position_string::<S>(&line, komi));
            }
            "go" => {
                let should_stop_clone = should_stop.clone();

                if let Some(handle) = calculating_handle.take() {
                    assert!(handle.is_finished());
                    search_tree = Some(handle.join().unwrap());
                }

                calculating_handle = {
                    let Some(current_position) = position.clone() else {
                        eprintln!("Error: Received go without receiving position string");
                        process::exit(1);
                    };

                    let tree = if let Some(old_position) = last_position_searched.as_ref() {
                        // eprintln!("Found previous position, updating search tree from it");
                        update_search_tree(
                            old_position,
                            &current_position,
                            search_tree.take(),
                            mcts_settings.clone(),
                        )
                    } else {
                        MonteCarloTree::new(current_position.position(), mcts_settings.clone())
                    };

                    last_position_searched = Some(current_position.clone());

                    Some(thread::spawn(move || {
                        parse_go_string::<S>(
                            &line,
                            current_position.clone(),
                            tree,
                            should_stop_clone,
                        )
                    }))
                }
            }
            s => {
                eprintln!("Unknown command \"{}\"", s);
                process::exit(1);
            }
        }
    }
    if let Some(handle) = calculating_handle.take() {
        assert!(handle.is_finished());
    }
}

pub fn main() {
    let is_slatebot = env::args().any(|arg| arg == "--slatebot");
    let is_cobblebot = env::args().any(|arg| arg == "--cobblebot");

    loop {
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        if input.trim() == "tei" {
            break;
        }
    }

    println!("id name Tiltak");
    println!("id author Morten Lohne");
    println!("option name HalfKomi type spin default 0 min -10 max 10");
    println!("teiok");

    let mut sender = None;

    let mut game_thread: Option<JoinHandle<()>> = None;
    let mut komi = Komi::default();

    for line in BufReader::new(io::stdin()).lines().map(Result::unwrap) {
        let mut words = line.split_whitespace();
        match words.next().unwrap() {
            "teinewgame" => {
                let size_string = words.next();
                let size: usize = size_string.and_then(|s| usize::from_str(s).ok()).unwrap();

                // Close the existing channel if it exists
                if let Some(sender) = sender.take() {
                    mem::drop(sender);
                }

                if let Some(handle) = game_thread.take() {
                    handle.join().unwrap();
                }

                let (snd, receiver) = mpsc::channel();
                sender = Some(snd);

                match size {
                    4 => {
                        game_thread = Some(thread::spawn(move || {
                            tei_game::<4>(receiver, mcts_settings(is_slatebot, is_cobblebot), komi);
                        }))
                    }
                    5 => {
                        game_thread = Some(thread::spawn(move || {
                            tei_game::<5>(receiver, mcts_settings(is_slatebot, is_cobblebot), komi);
                        }))
                    }
                    6 => {
                        game_thread = Some(thread::spawn(move || {
                            tei_game::<6>(receiver, mcts_settings(is_slatebot, is_cobblebot), komi);
                        }))
                    }
                    _ => panic!("Error: Unsupported size {}", size),
                }
            }
            "setoption" => {
                if game_thread
                    .as_ref()
                    .is_some_and(|handle| !handle.is_finished())
                {
                    eprintln!(
                        "Warning: Changes to komi need a 'teinewgame' command to take effect."
                    );
                }
                if [
                    words.next().unwrap_or_default(),
                    words.next().unwrap_or_default(),
                    words.next().unwrap_or_default(),
                ]
                .join(" ")
                    == "name HalfKomi value"
                {
                    if let Some(k) = words
                        .next()
                        .and_then(|komi_string| komi_string.parse::<i8>().ok())
                        .and_then(Komi::from_half_komi)
                    {
                        komi = k;
                    } else {
                        panic!("Invalid komi setting \"{}\"", line);
                    }
                } else {
                    panic!("Invalid setoption string \"{}\"", line);
                }
            }
            "quit" => {
                process::exit(0);
            }
            s => {
                let Some(sender) = sender.as_ref() else {
                    if s == "isready" {
                        println!("readyok");
                        continue;
                    } else {
                        panic!("Unknown command \"{}\"", s);
                    }
                };
                sender.send(line.trim().to_string()).unwrap()
            }
        }
    }
}

fn parse_position_string<const S: usize>(line: &str, komi: Komi) -> SearchPosition<S> {
    let mut words_iter = line.split_whitespace();
    words_iter.next(); // position
    let position = match words_iter.next() {
        Some("startpos") => Position::start_position_with_komi(komi),
        Some("tps") => {
            let tps: String = (&mut words_iter).take(3).collect::<Vec<_>>().join(" ");
            <Position<S>>::from_fen_with_komi(&tps, komi).unwrap()
        }
        _ => panic!("Expected \"startpos\" or \"tps\" to specify position."),
    };

    let mut moves = vec![];

    match words_iter.next() {
        Some("moves") => {
            for move_string in words_iter {
                moves.push(position.move_from_san(move_string).unwrap());
            }
        }
        Some(s) => panic!("Expected \"moves\" in \"{}\", got \"{}\".", line, s),
        None => (),
    }
    SearchPosition {
        root_position: position,
        moves,
    }
}

#[derive(Clone)]
struct SearchPosition<const S: usize> {
    root_position: Position<S>,
    moves: Vec<Move<S>>,
}

impl<const S: usize> SearchPosition<S> {
    fn position(&self) -> Position<S> {
        let mut position = self.root_position.clone();
        for mv in &self.moves {
            position.do_move(*mv);
        }
        position
    }

    fn move_difference(&self, new_position: &SearchPosition<S>) -> Option<Vec<Move<S>>> {
        if self.root_position != new_position.root_position {
            return None;
        }
        let mut old_moves_iter = self.moves.iter();
        let mut new_moves_iter = new_position.moves.iter();

        while let Some(old_move) = old_moves_iter.next() {
            if old_move != new_moves_iter.next()? {
                return None;
            }
        }

        let difference = new_moves_iter.cloned().collect();
        Some(difference)
    }
}

fn update_search_tree<const S: usize>(
    old_position: &SearchPosition<S>,
    new_position: &SearchPosition<S>,
    search_tree: Option<MonteCarloTree<S>>,
    mcts_settings: MctsSetting<S>,
) -> MonteCarloTree<S> {
    let tree = search_tree.unwrap_or(MonteCarloTree::new(
        new_position.position(),
        mcts_settings.clone(),
    ));

    if let Some(move_difference) = old_position.move_difference(new_position) {
        let tree_position = tree.position();
        tree.reroot(&move_difference).unwrap_or_else(|| {
            eprintln!(
                "Failed to reroot tree, creating new tree. Old tree was at {}",
                tree_position.to_fen()
            );
            MonteCarloTree::new(new_position.position(), mcts_settings)
        })
    } else {
        MonteCarloTree::new(new_position.position(), mcts_settings)
    }
}

fn mcts_settings<const S: usize>(is_slatebot: bool, is_cobblebot: bool) -> MctsSetting<S> {
    if is_slatebot {
        MctsSetting::default()
            .add_rollout_depth(200)
            .add_rollout_temperature(0.2)
    } else if is_cobblebot {
        MctsSetting::default()
            .add_rollout_depth(200)
            .add_rollout_temperature(0.2)
            .add_dirichlet(0.25)
    } else {
        MctsSetting::default()
    }
}

fn parse_go_string<const S: usize>(
    line: &str,
    position: SearchPosition<S>,
    mut tree: MonteCarloTree<S>,
    should_stop: Arc<AtomicBool>,
) -> MonteCarloTree<S> {
    let mut words = line.split_whitespace();
    words.next(); // go

    match words.next() {
        Some(word @ "movetime") | Some(word @ "infinite") => {
            let movetime = if word == "movetime" {
                Duration::from_millis(u64::from_str(words.next().unwrap()).unwrap())
            } else {
                Duration::MAX // 'go infinite' is just movetime with a very long duration
            };
            let start_time = Instant::now();

            for i in 0.. {
                let nodes_to_search = (200.0 * f64::powf(1.26, i as f64)) as u64;
                let mut oom = false;
                for _ in 0..nodes_to_search {
                    if should_stop.load(atomic::Ordering::Relaxed) {
                        break;
                    }
                    if let Err(err) = tree.select() {
                        eprintln!("Warning: {err}");
                        oom = true;
                        break;
                    }
                }
                let (best_move, best_score) = tree.best_move().unwrap();
                let pv: Vec<_> = tree.pv().collect();
                println!(
                    "info depth {} seldepth {} nodes {} score cp {} time {} nps {:.0} pv {}",
                    ((tree.visits() as f64 / 10.0).log2()) as u64,
                    pv.len(),
                    tree.visits(),
                    (best_score * 200.0 - 100.0) as i64,
                    start_time.elapsed().as_millis(),
                    tree.visits() as f32 / start_time.elapsed().as_secs_f32(),
                    pv.iter()
                        .map(|mv| mv.to_string())
                        .collect::<Vec<String>>()
                        .join(" ")
                );
                if oom
                    || should_stop.load(atomic::Ordering::Relaxed)
                    || start_time.elapsed().as_secs_f64() > movetime.as_secs_f64() * 0.7
                {
                    println!("bestmove {}", best_move.to_string());
                    break;
                }
            }
            tree
        }
        Some("wtime") | Some("btime") | Some("winc") | Some("binc") => {
            let parse_time = |s: Option<&str>| {
                Duration::from_millis(
                    s.and_then(|w| w.parse().ok())
                        .unwrap_or_else(|| panic!("Incorrect go command {}", line)),
                )
            };
            let mut words = line.split_whitespace().skip(1).peekable();
            let mut white_time = Duration::default();
            let mut white_inc = Duration::default();
            let mut black_time = Duration::default();
            let mut black_inc = Duration::default();

            while let Some(word) = words.next() {
                match word {
                    "wtime" => white_time = parse_time(words.next()),
                    "winc" => white_inc = parse_time(words.next()),
                    "btime" => black_time = parse_time(words.next()),
                    "binc" => black_inc = parse_time(words.next()),
                    _ => (),
                }
            }

            let max_time = match position.position().side_to_move() {
                Color::White => white_time / 5 + white_inc / 2,
                Color::Black => black_time / 5 + black_inc / 2,
            };

            let start_time = Instant::now();

            tree.search_for_time(max_time, |tree| {
                let best_score = tree.best_move().unwrap().1;
                let pv: Vec<_> = tree.pv().collect();
                println!(
                    "info depth {} seldepth {} nodes {} score cp {} time {} nps {:.0} pv {}",
                    ((tree.visits() as f64 / 10.0).log2()) as u64,
                    pv.len(),
                    tree.visits(),
                    (best_score * 200.0 - 100.0) as i64,
                    start_time.elapsed().as_millis(),
                    tree.visits() as f32 / start_time.elapsed().as_secs_f32(),
                    pv.iter()
                        .map(|mv| mv.to_string())
                        .collect::<Vec<String>>()
                        .join(" ")
                );
            });
            let best_move = tree.best_move().unwrap().0;

            println!("bestmove {}", best_move.to_string());
            tree
        }
        Some(_) | None => {
            panic!("Invalid go command \"{}\"", line);
        }
    }
}

use board_game_traits::{Color, Position as PositionTrait};
use pgn_traits::PgnPosition;
use std::io::{BufRead, BufReader};
use std::str::FromStr;
use std::time::{Duration, Instant};
use std::{env, io};
use tiltak::position::{Komi, Position};

use std::any::Any;
use tiltak::search::MctsSetting;
use tiltak::search::{self, MonteCarloTree};

pub fn main() {
    let is_slatebot = env::args().any(|arg| arg == "--slatebot");

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

    // Position stored in a `dyn Any` variable, because it can be any size
    let mut position: Option<Box<dyn Any>> = None;
    let mut size: Option<usize> = None;
    let mut komi = Komi::default();

    for line in BufReader::new(io::stdin()).lines().map(Result::unwrap) {
        let mut words = line.split_whitespace();
        match words.next().unwrap() {
            "quit" => break,
            "isready" => println!("readyok"),
            "setoption" => {
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
            "teinewgame" => {
                let size_string = words.next();
                size = size_string.and_then(|s| usize::from_str(s).ok());
                position = None;

                match size {
                    Some(4) | Some(5) | Some(6) => (),
                    _ => panic!("Error: Unsupported size {}", size.unwrap_or_default()),
                }
            }
            "position" => {
                position = match size {
                    None => panic!("Received position without receiving teinewgame string"),
                    Some(4) => Some(Box::new(parse_position_string::<4>(&line, komi))),
                    Some(5) => Some(Box::new(parse_position_string::<5>(&line, komi))),
                    Some(6) => Some(Box::new(parse_position_string::<6>(&line, komi))),
                    Some(s) => panic!("Unsupported size {}", s),
                }
            }
            "go" => match size {
                Some(4) => parse_go_string::<4>(
                    &line,
                    position.as_ref().and_then(|p| p.downcast_ref()).unwrap(),
                    is_slatebot,
                ),
                Some(5) => parse_go_string::<5>(
                    &line,
                    position.as_ref().and_then(|p| p.downcast_ref()).unwrap(),
                    is_slatebot,
                ),
                Some(6) => parse_go_string::<6>(
                    &line,
                    position.as_ref().and_then(|p| p.downcast_ref()).unwrap(),
                    is_slatebot,
                ),
                Some(s) => panic!("Error: Unsupported size {}", s),
                None => panic!("Error: Received go without receiving teinewgame string"),
            },
            s => panic!("Unknown command \"{}\"", s),
        }
    }
}

fn parse_position_string<const S: usize>(line: &str, komi: Komi) -> Position<S> {
    let mut words_iter = line.split_whitespace();
    words_iter.next(); // position
    let mut position = match words_iter.next() {
        Some("startpos") => Position::start_position_with_komi(komi),
        Some("tps") => {
            let tps: String = (&mut words_iter).take(3).collect::<Vec<_>>().join(" ");
            <Position<S>>::from_fen_with_komi(&tps, komi).unwrap()
        }
        _ => panic!("Expected \"startpos\" or \"tps\" to specify position."),
    };

    match words_iter.next() {
        Some("moves") => {
            for move_string in words_iter {
                position.do_move(position.move_from_san(move_string).unwrap());
            }
        }
        Some(s) => panic!("Expected \"moves\" in \"{}\", got \"{}\".", line, s),
        None => (),
    }
    position
}

fn parse_go_string<const S: usize>(line: &str, position: &Position<S>, is_slatebot: bool) {
    let mut words = line.split_whitespace();
    words.next(); // go

    let mcts_settings = if is_slatebot {
        MctsSetting::default().add_rollout_depth(200)
    } else {
        MctsSetting::default()
    };

    match words.next() {
        Some("movetime") => {
            let msecs = words.next().unwrap();
            let movetime = Duration::from_millis(u64::from_str(msecs).unwrap());
            let start_time = Instant::now();
            let mut tree = search::MonteCarloTree::with_settings(position.clone(), mcts_settings);

            for i in 0.. {
                let nodes_to_search = (200.0 * f64::powf(1.26, i as f64)) as u64;
                for _ in 0..nodes_to_search {
                    tree.select();
                }
                let (best_move, best_score) = tree.best_move();
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
                        .map(|mv| position.move_to_san(mv))
                        .collect::<Vec<String>>()
                        .join(" ")
                );
                if start_time.elapsed().as_secs_f64() > movetime.as_secs_f64() * 0.7 {
                    println!("bestmove {}", position.move_to_san(&best_move));
                    break;
                }
            }
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

            println!("{:?}, {:?}", white_time, black_time);

            let max_time = match position.side_to_move() {
                Color::White => white_time / 5 + white_inc / 2,
                Color::Black => black_time / 5 + black_inc / 2,
            };

            let start_time = Instant::now();

            let mut tree = MonteCarloTree::with_settings(position.clone(), mcts_settings);
            tree.search_for_time(max_time, |tree| {
                let best_score = tree.best_move().1;
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
                        .map(|mv| position.move_to_san(mv))
                        .collect::<Vec<String>>()
                        .join(" ")
                );
            });
            let best_move = tree.best_move().0;

            println!("bestmove {}", position.move_to_san(&best_move));
        }
        Some(_) | None => {
            panic!("Invalid go command \"{}\"", line);
        }
    }
}

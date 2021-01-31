use board_game_traits::board::{Board as BoardTrait, Color};
use pgn_traits::pgn::PgnBoard;
use std::io;
use std::io::{BufRead, BufReader};
use std::str::FromStr;
use std::time::{Duration, Instant};
use taik::board::Board;

use std::any::Any;
use taik::search;

pub fn main() {
    loop {
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        if input.trim() == "tei" {
            break;
        }
    }

    println!("id name taik");
    println!("id author Morten Lohne");
    println!("teiok");

    // Position stored in a `dyn Any` variable, because it can be any size
    let mut position: Option<Box<dyn Any>> = None;
    let mut size: Option<usize> = None;

    for line in BufReader::new(io::stdin()).lines().map(Result::unwrap) {
        let mut words = line.split_whitespace();
        match words.next().unwrap() {
            "quit" => break,
            "isready" => println!("readyok"),
            "setoption" => panic!("Unknown option \"{}\"", line),
            "teinewgame" => {
                let size_string = words.next();
                size = size_string.and_then(|s| usize::from_str(s).ok());

                if size.is_none() {
                    panic!("Error: Unsupported size {}", size.unwrap_or_default());
                }
            }
            "position" => {
                position = match size {
                    None => panic!("Received position without receiving teinewgame string"),
                    Some(4) => Some(Box::new(parse_position_string::<4>(&line))),
                    Some(5) => Some(Box::new(parse_position_string::<5>(&line))),
                    Some(6) => Some(Box::new(parse_position_string::<6>(&line))),
                    Some(s) => panic!("Unsupported size {}", s),
                }
            }
            "go" => match size {
                Some(4) => parse_go_string::<4>(
                    &line,
                    position.as_ref().and_then(|p| p.downcast_ref()).unwrap(),
                ),
                Some(5) => parse_go_string::<5>(
                    &line,
                    position.as_ref().and_then(|p| p.downcast_ref()).unwrap(),
                ),
                Some(6) => parse_go_string::<6>(
                    &line,
                    position.as_ref().and_then(|p| p.downcast_ref()).unwrap(),
                ),
                Some(s) => panic!("Error: Unsupported size {}", s),
                None => panic!("Error: Received go without receiving teinewgame string"),
            },
            s => panic!("Unknown command \"{}\"", s),
        }
    }
}

fn parse_position_string<const S: usize>(line: &str) -> Board<S> {
    let mut words_iter = line.split_whitespace();
    words_iter.next(); // position
    let mut position = match words_iter.next() {
        Some("startpos") => Board::default(),
        Some("tps") => {
            let tps: String = (&mut words_iter).take(3).collect::<Vec<_>>().join(" ");
            <Board<S>>::from_fen(&tps).unwrap()
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

fn parse_go_string<const S: usize>(line: &str, position: &Board<S>) {
    let mut words = line.split_whitespace();
    words.next(); // go
    match words.next() {
        Some("movetime") => {
            let msecs = words.next().unwrap();
            let movetime = Duration::from_millis(u64::from_str(msecs).unwrap());
            let start_time = Instant::now();

            let mut tree = search::MonteCarloTree::new(position.clone());
            let mut total_nodes = 0;
            for i in 0.. {
                let nodes_to_search = (1000.0 * f64::powf(1.26, i as f64)) as u64;
                for _ in 0..nodes_to_search {
                    tree.select();
                }
                total_nodes += nodes_to_search;
                let (best_move, score) = tree.best_move();
                println!(
                    "info depth {} seldepth {} score cp {} nodes {} time {} pv {}",
                    i / 2 + 1,
                    tree.pv().count(),
                    (score * 200.0 - 100.0) as i64,
                    total_nodes,
                    start_time.elapsed().as_millis(),
                    tree.pv()
                        .map(|mv| mv.to_string::<S>() + " ")
                        .collect::<String>()
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

            let max_time = match position.side_to_move() {
                Color::White => white_time / 5 + white_inc / 2,
                Color::Black => black_time / 5 + black_inc / 2,
            };

            let start_time = Instant::now();
            let (best_move, score) = search::play_move_time::<S>(position.clone(), max_time);

            println!(
                "info score cp {} time {} pv {}",
                (score * 200.0 - 100.0) as i64,
                start_time.elapsed().as_millis(),
                position.move_to_san(&best_move)
            );

            println!("bestmove {}", position.move_to_san(&best_move));
        }
        Some(_) | None => {
            panic!("Invalid go command \"{}\"", line);
        }
    }
}

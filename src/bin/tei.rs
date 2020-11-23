use board_game_traits::board::Board as BoardTrait;
use pgn_traits::pgn::PgnBoard;
use std::io;
use std::io::{BufRead, BufReader};
use std::str::FromStr;
use std::time::{Duration, Instant};
use taik::board::Board;

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

    let mut position = Board::default();

    for line in BufReader::new(io::stdin()).lines().map(Result::unwrap) {
        let mut words = line.split_whitespace();
        match words.next().unwrap() {
            "quit" => break,
            "isready" => println!("readyok"),
            "setoption" => {
                eprintln!("Unknown option \"{}\"", line);
                break;
            }
            "teinewgame" => {
                let size = words.next();
                if size.is_some() && size != Some("5") {
                    eprintln!("Error: Unsupported size {}, exiting...", size.unwrap());
                    break;
                }
            }
            "position" => {
                let mut words_iter = line.split_whitespace();
                assert_eq!(words_iter.next(), Some("position"));
                assert_eq!(words_iter.next(), Some("startpos"));

                position = Board::default();

                match words_iter.next() {
                    Some("moves") => {
                        for move_string in words_iter {
                            position.do_move(position.move_from_san(move_string).unwrap());
                        }
                    }
                    Some(s) => panic!("Expected \"moves\" in \"{}\", got \"{}\".", line, s),
                    None => (),
                }
            }
            "go" => match line.split_whitespace().nth(1) {
                Some("movetime") => {
                    let msecs = line.split_whitespace().nth(2).unwrap();
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
                            tree.pv().map(|mv| mv.to_string() + " ").collect::<String>()
                        );
                        if start_time.elapsed().as_secs_f64() > movetime.as_secs_f64() * 0.7 {
                            println!("bestmove {}", position.move_to_san(&best_move));
                            break;
                        }
                    }
                }
                Some(_) | None => {
                    eprintln!("Invalid go command \"{}\"", line);
                    break;
                }
            },
            s => {
                eprintln!("Unknown command \"{}\"", s);
                break;
            }
        }
    }
}

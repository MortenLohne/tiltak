use board_game_traits::board::Board as BoardTrait;
use pgn_traits::pgn::PgnBoard;
use std::io;
use std::io::{BufRead, BufReader};
use std::str::FromStr;
use std::time::{Duration, Instant};
use taik::board::{Board, TunableBoard};

use taik::mcts;

pub fn main() {
    println!("id name taik");
    println!("id author Morten Lohne");
    println!("utiok");

    let mut position = Board::default();

    for line in BufReader::new(io::stdin()).lines().map(Result::unwrap) {
        match line.split_whitespace().next().unwrap() {
            "quit" => break,
            "isready" => println!("readyok"),
            "setoption" => {
                eprintln!("Unknown option \"{}\"", line);
                break;
            }
            "utinewgame" => (),
            "position" => {
                let mut words_iter = line.split_whitespace();
                assert_eq!(words_iter.next(), Some("position"));
                assert_eq!(words_iter.next(), Some("startpos"));
                assert_eq!(words_iter.next(), Some("moves"));

                position = Board::default();
                for move_string in words_iter {
                    position.do_move(position.move_from_san(move_string).unwrap());
                }
            }
            "go" => match line.split_whitespace().nth(1) {
                Some("movetime") => {
                    let msecs = line.split_whitespace().nth(2).unwrap();
                    let movetime = Duration::from_millis(u64::from_str(msecs).unwrap());
                    let start_time = Instant::now();

                    let mut tree = mcts::Tree::new_root();
                    let mut simple_moves = vec![];
                    let mut moves = vec![];
                    loop {
                        if start_time.elapsed() > movetime + Duration::from_millis(100) {
                            let (best_move, _score) = tree.best_move(0.1);
                            println!("bestmove {}", position.move_to_san(&best_move));
                            break;
                        }
                        for _ in 0..1000 {
                            tree.select(
                                &mut position.clone(),
                                Board::VALUE_PARAMS,
                                Board::POLICY_PARAMS,
                                &mut simple_moves,
                                &mut moves,
                            );
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

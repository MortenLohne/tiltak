#![allow(clippy::uninlined_format_args)]

use crate::position::{Komi, Move, Position};
use async_channel::TryRecvError;
use board_game_traits::{Color, Position as PositionTrait};
use pgn_traits::PgnPosition;
use std::fmt::Write;
use std::process;
use std::str::FromStr;
use std::time::Duration;

use crate::search::{MctsSetting, MonteCarloTree};

pub trait Platform {
    type Instant;
    fn yield_fn() -> impl std::future::Future;
    fn current_time() -> Self::Instant;
    fn elapsed_time(start: &Self::Instant) -> Duration;
}

pub async fn tei_game<'a, const S: usize, Out: Fn(&str), P: Platform>(
    input: &async_channel::Receiver<String>,
    output: &Out,
    mcts_settings: MctsSetting<S>,
    komi: Komi,
) -> Result<usize, ()> {
    let mut position: Option<SearchPosition<S>> = None;

    let mut last_position_searched: SearchPosition<S> = SearchPosition::new(komi);
    let mut search_tree: MonteCarloTree<S> = MonteCarloTree::new(
        Position::<S>::start_position_with_komi(komi),
        mcts_settings.clone(),
    );

    while let Ok(line) = input.recv().await {
        let mut words = line.split_whitespace();
        match words.next().unwrap() {
            "quit" => {
                process::exit(0);
            }
            "stop" => {
                eprintln!("Got stop when not searching")
            }
            "isready" => output("readyok"),
            "teinewgame" => {
                let size_string = words.next();
                let size: usize = size_string.and_then(|s| usize::from_str(s).ok()).unwrap();
                return Ok(size);
            }
            "setoption" => {
                unreachable!() // This should be handled by the main loop
            }
            "position" => {
                position = Some(parse_position_string::<S>(&line, komi));
            }
            "go" => {
                let Some(current_position) = position.as_ref() else {
                    eprintln!("Error: Received go without receiving position string");
                    process::exit(1);
                };

                search_tree = update_search_tree(
                    &last_position_searched,
                    &current_position,
                    search_tree,
                    &mcts_settings,
                );

                last_position_searched.clone_from(&current_position);

                parse_go_string::<S, _, P>(
                    &input,
                    output,
                    &line,
                    &current_position,
                    &mut search_tree,
                )
                .await
            }
            s => {
                eprintln!("Unknown command \"{}\"", s);
                process::exit(1);
            }
        }
    }
    Err(())
}
pub async fn tei<Out: Fn(&str), P: Platform>(
    is_slatebot: bool,
    is_cobblebot: bool,
    input: async_channel::Receiver<String>,
    output: &Out,
) -> Result<(), ()> {
    loop {
        let Ok(input) = input.recv().await else {
            return Ok(());
        };
        if input.trim() == "tei" {
            break;
        }
    }

    output("id name Tiltak");
    output("id author Morten Lohne");
    output("option name HalfKomi type spin default 0 min -10 max 10");
    output("teiok");

    let mut komi = Komi::default();
    while let Ok(line) = input.recv().await {
        let mut words = line.split_whitespace();
        match words.next().unwrap() {
            "teinewgame" => {
                let size_string = words.next();
                let mut size: usize = size_string.and_then(|s| usize::from_str(s).ok()).unwrap();

                loop {
                    size = match size {
                        4 => {
                            tei_game::<4, _, P>(
                                &input,
                                output,
                                mcts_settings(is_slatebot, is_cobblebot),
                                komi,
                            )
                            .await?
                        }
                        5 => {
                            tei_game::<5, _, P>(
                                &input,
                                output,
                                mcts_settings(is_slatebot, is_cobblebot),
                                komi,
                            )
                            .await?
                        }
                        6 => {
                            tei_game::<6, _, P>(
                                &input,
                                output,
                                mcts_settings(is_slatebot, is_cobblebot),
                                komi,
                            )
                            .await?
                        }
                        _ => panic!("Error: Unsupported size {}", size),
                    };
                }
            }
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
            "quit" => {
                process::exit(0);
            }
            "isready" => output("readyok"),
            s => {
                eprintln!("Unknown command \"{}\"", s);
                process::exit(1);
            }
        }
    }
    Ok(())
}

fn parse_position_string<const S: usize>(line: &str, komi: Komi) -> SearchPosition<S> {
    let mut words_iter = line.split_whitespace();
    words_iter.next();
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
    fn new(komi: Komi) -> Self {
        Self {
            root_position: Position::<S>::start_position_with_komi(komi),
            moves: vec![],
        }
    }

    fn position(&self) -> Position<S> {
        let mut position = self.root_position.clone();
        for mv in &self.moves {
            position.do_move(*mv);
        }
        position
    }

    fn side_to_move(&self) -> Color {
        let root_side_to_move = self.root_position.side_to_move();
        if self.moves.len() % 2 == 0 {
            root_side_to_move
        } else {
            !root_side_to_move
        }
    }

    fn move_difference<'a>(&self, new_position: &'a SearchPosition<S>) -> Option<&'a [Move<S>]> {
        if self.root_position != new_position.root_position {
            return None;
        }

        let mut i = 0;
        while i < self.moves.len() {
            if self.moves[i] != *new_position.moves.get(i)? {
                return None;
            }
            i += 1;
        }

        Some(&new_position.moves[i..])
    }
}

fn update_search_tree<const S: usize>(
    old_position: &SearchPosition<S>,
    new_position: &SearchPosition<S>,
    search_tree: MonteCarloTree<S>,
    mcts_settings: &MctsSetting<S>,
) -> MonteCarloTree<S> {
    if let Some(move_difference) = old_position.move_difference(new_position) {
        search_tree.reroot(&move_difference).unwrap_or_else(|| {
            eprintln!("Failed to reroot tree, creating new tree");
            MonteCarloTree::new(new_position.position(), mcts_settings.clone())
        })
    } else {
        eprintln!("Failed to find move difference, creating new tree");
        MonteCarloTree::new(new_position.position(), mcts_settings.clone())
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

async fn parse_go_string<'a, const S: usize, Out: Fn(&str), P: Platform>(
    input: &async_channel::Receiver<String>,
    output: &Out,
    line: &str,
    position: &SearchPosition<S>,
    tree: &mut MonteCarloTree<S>,
) {
    let mut words = line.split_whitespace();
    words.next(); // go

    match words.next() {
        Some(word @ "movetime") | Some(word @ "infinite") => {
            let movetime = if word == "movetime" {
                Duration::from_millis(u64::from_str(words.next().unwrap()).unwrap())
            } else {
                Duration::MAX // 'go infinite' is just movetime with a very long duration
            };
            let start_time = P::current_time();

            for i in 0.. {
                let nodes_to_search = (200.0 * f64::powf(1.26, i as f64)) as u64;
                let mut oom = false;
                let mut should_stop = false;
                for n in 0..nodes_to_search {
                    if n % 1000 == 0 {
                        P::yield_fn().await;
                        match input.try_recv() {
                            Ok(line) => match line.trim() {
                                "stop" => {
                                    should_stop = true;
                                    break;
                                }
                                "quit" => process::exit(0),
                                "isready" => output("readyok"),
                                _ => {
                                    panic!("Warning: Ignoring input \"{}\" during search", line)
                                }
                            },
                            Err(TryRecvError::Empty) => {}
                            Err(TryRecvError::Closed) => {
                                eprintln!("Input channel closed, stopping search");
                                return;
                            }
                        }
                    }
                    if should_stop {
                        break;
                    }
                    if let Err(err) = tree.select() {
                        eprintln!("Warning: {err}");
                        oom = true;
                        break;
                    }
                }

                let elapsed = P::elapsed_time(&start_time);

                let info_string = info_string::<S, P>(&start_time, tree);

                output(&info_string);

                if oom || should_stop || elapsed.as_secs_f64() > movetime.as_secs_f64() * 0.7 {
                    let (best_move, _) = tree.best_move().unwrap();
                    output(&format!("bestmove {}", best_move));
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

            assert_eq!(position.side_to_move(), position.position().side_to_move());
            let max_time = match position.side_to_move() {
                Color::White => white_time / 5 + white_inc / 2,
                Color::Black => black_time / 5 + black_inc / 2,
            };

            let start_time = P::current_time();

            tree.search_for_time(max_time, |tree| {
                let info_string = info_string::<S, P>(&start_time, tree);

                output(&info_string);
            });
            let best_move = tree.best_move().unwrap().0;

            output(&format!("bestmove {}", best_move));
        }
        Some("nodes") => {
            let nodes = words.next().unwrap().parse::<u32>().unwrap();

            let start_time = P::current_time();

            while tree.visits() < nodes {
                tree.select().unwrap();
            }

            let info_string = info_string::<S, P>(&start_time, tree);

            output(&info_string);

            let (best_move, _) = tree.best_move().unwrap();

            output(&format!("bestmove {}", best_move));
        }
        Some(_) | None => {
            panic!("Invalid go command \"{}\"", line);
        }
    }
}

pub fn info_string<const S: usize, P: Platform>(
    start_time: &P::Instant,
    tree: &MonteCarloTree<S>,
) -> String {
    let (_, best_score) = tree.best_move().unwrap();

    let elapsed = P::elapsed_time(&start_time);

    let pv_length = tree.pv().count();

    let mut info_string = String::with_capacity(100 + 6 * pv_length);

    write!(
        info_string,
        "info depth {} seldepth {} nodes {} score cp {} time {} nps {:.0} pv",
        ((tree.visits() as f64 / 10.0).log2()) as u64,
        pv_length,
        tree.visits(),
        (best_score * 200.0 - 100.0) as i64,
        elapsed.as_millis(),
        tree.visits() as f32 / elapsed.as_secs_f32(),
    )
    .unwrap();

    for mv in tree.pv() {
        write!(info_string, " {}", mv).unwrap();
    }
    info_string
}

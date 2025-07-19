#![allow(clippy::uninlined_format_args)]

use crate::position::{Komi, Move, Position};
use arrayvec::ArrayString;
use async_channel::TryRecvError;
use board_game_traits::{Color, Position as PositionTrait};
use pgn_traits::PgnPosition;
use std::process;
use std::str::FromStr;
use std::time::Duration;
use std::{fmt::Write, mem};

use crate::search::{MctsSetting, MonteCarloTree};

pub trait Platform {
    type Instant;
    fn yield_fn() -> impl std::future::Future;
    fn current_time() -> Self::Instant;
    fn elapsed_time(start: &Self::Instant) -> Duration;
}

async fn tei_game<const S: usize, Out: Fn(&str), P: Platform>(
    input: &async_channel::Receiver<String>,
    output: &Out,
    mcts_settings: MctsSetting<S>,
    komi: &mut Komi,
) -> TeiResult {
    let mut position: Option<SearchPosition<S>> = None;

    let mut last_position_searched: SearchPosition<S> = SearchPosition::new(*komi);
    let mut search_tree: MonteCarloTree<S> = MonteCarloTree::new(
        Position::<S>::start_position_with_komi(*komi),
        mcts_settings.clone(),
    );
    // Dummy memory that can be de-allocated during OOM
    let dummy_memory = vec![0u8; 100 * 1024 * 1024];

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
                return TeiResult::SwitchSize(size);
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
                        *komi = k;
                    } else {
                        panic!("Invalid komi setting \"{}\"", line);
                    }
                } else {
                    panic!("Invalid setoption string \"{}\"", line);
                }
                if let Some(p) = position.as_mut() {
                    p.root_position.set_komi(*komi);
                }
                // If we receive a new komi, previous search is invalid
                last_position_searched = SearchPosition::new(*komi);
                search_tree = MonteCarloTree::new(
                    Position::<S>::start_position_with_komi(*komi),
                    mcts_settings.clone(),
                );
            }
            "position" => {
                position = Some(parse_position_string::<S>(&line, *komi));
            }
            "go" => {
                let Some(current_position) = position.as_ref() else {
                    eprintln!("Error: Received go without receiving position string");
                    process::exit(1);
                };

                search_tree = update_search_tree(
                    &last_position_searched,
                    current_position,
                    search_tree,
                    &mcts_settings,
                );

                last_position_searched.clone_from(current_position);

                match parse_go_string::<S, _, P>(
                    input,
                    output,
                    &line,
                    current_position,
                    &mut search_tree,
                )
                .await
                {
                    // Respond to OOM as if we received 'teinewgame'
                    // This will drop the whole search tree immediately
                    Some(TeiResult::Oom) => {
                        output("info recovering from OOM, search tree will be cleared");
                        mem::drop(dummy_memory);
                        output("dummy memory cleared");
                        mem::drop(search_tree);
                        output("info recovered from OOM, search tree has been dropped");
                        return TeiResult::Oom;
                    }
                    Some(TeiResult::Quit) => return TeiResult::Quit,
                    Some(TeiResult::NoInput) => return TeiResult::NoInput,
                    Some(TeiResult::SwitchSize(_)) => {
                        unreachable!("Cannot receive teinewgame during search")
                    }
                    None => (),
                }
            }
            s => {
                eprintln!("Unknown command \"{}\"", s);
                process::exit(1);
            }
        }
    }
    TeiResult::NoInput
}
pub async fn tei<Out: Fn(&str), P: Platform>(
    is_slatebot: bool,
    is_cobblebot: bool,
    input: async_channel::Receiver<String>,
    output: &Out,
) {
    loop {
        let Ok(input) = input.recv().await else {
            return;
        };
        if input.trim() == "tei" {
            break;
        }
    }

    output("id name Tiltak");
    output("id author Morten Lohne");
    output("option name HalfKomi type combo default 0 var 0 var 4");
    output("teiok");

    let mut komi = Komi::default();
    while let Ok(line) = input.recv().await {
        let mut words = line.split_whitespace();
        match words.next().unwrap() {
            "teinewgame" => {
                let size_string = words.next();
                let mut size: usize = size_string.and_then(|s| usize::from_str(s).ok()).unwrap();

                loop {
                    let result = match size {
                        4 => {
                            tei_game::<4, _, P>(
                                &input,
                                output,
                                mcts_settings(is_slatebot, is_cobblebot),
                                &mut komi,
                            )
                            .await
                        }
                        5 => {
                            tei_game::<5, _, P>(
                                &input,
                                output,
                                mcts_settings(is_slatebot, is_cobblebot),
                                &mut komi,
                            )
                            .await
                        }
                        6 => {
                            tei_game::<6, _, P>(
                                &input,
                                output,
                                mcts_settings(is_slatebot, is_cobblebot),
                                &mut komi,
                            )
                            .await
                        }
                        _ => panic!("Error: Unsupported size {}", size),
                    };
                    match result {
                        TeiResult::Quit => return,
                        TeiResult::SwitchSize(new_size) => size = new_size,
                        TeiResult::NoInput => return,
                        TeiResult::Oom => {
                            // If we faced OOM, this should be fine here, since the search tree has been deallocated
                            output("info recovering from OOM, search tree has been cleared");
                        }
                    }
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
        search_tree
            .reroot(move_difference)
            .unwrap_or_else(|| MonteCarloTree::new(new_position.position(), mcts_settings.clone()))
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

enum TeiResult {
    Oom,
    Quit,
    NoInput,
    SwitchSize(usize),
}

async fn parse_go_string<const S: usize, Out: Fn(&str), P: Platform>(
    input: &async_channel::Receiver<String>,
    output: &Out,
    line: &str,
    position: &SearchPosition<S>,
    tree: &mut MonteCarloTree<S>,
) -> Option<TeiResult> {
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
            let mut nodes_searched = 0;

            for i in 0.. {
                let nodes_to_search = (200.0 * f64::powf(1.26, i as f64)) as u64;
                let mut oom = false;
                let mut should_stop = false;
                'output_loop: for n in 0..nodes_to_search {
                    if n % 1000 == 0 {
                        if P::elapsed_time(&start_time) > movetime.mul_f32(0.9) {
                            should_stop = true;
                            break 'output_loop;
                        }
                        P::yield_fn().await;
                        match input.try_recv() {
                            Ok(line) => match line.trim() {
                                "stop" => {
                                    should_stop = true;
                                    break 'output_loop;
                                }
                                "quit" => return Some(TeiResult::Quit),
                                "isready" => output("readyok"),
                                _ => {
                                    panic!("Warning: Ignoring input \"{}\" during search", line)
                                }
                            },
                            Err(TryRecvError::Empty) => {}
                            Err(TryRecvError::Closed) => {
                                return Some(TeiResult::NoInput);
                            }
                        }
                    }
                    if let Err(err) = tree.select() {
                        eprintln!("Warning: {err}");
                        oom = true;
                        break;
                    }
                    nodes_searched += 1;
                }

                // Must not allocate memory here, because we may be in an OOM situation
                let info_string = info_string::<S, P>(&start_time, nodes_searched, tree);

                output(&info_string);

                if oom || should_stop {
                    let (best_move, _) = tree.best_move().unwrap();
                    let mut best_move_string: ArrayString<20> =
                        ArrayString::from_str("bestmove ").unwrap();
                    write!(best_move_string, "{}", best_move).unwrap();
                    output(&best_move_string);
                    if oom {
                        return Some(TeiResult::Oom);
                    } else {
                        return None;
                    }
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
            let nodes_searched_previously = tree.visits();

            tree.search_for_time(max_time, |tree| {
                let nodes_searched = tree.visits() - nodes_searched_previously;
                let info_string = info_string::<S, P>(&start_time, nodes_searched, tree);

                output(&info_string);
            });
            let best_move = tree.best_move().unwrap().0;

            output(&format!("bestmove {}", best_move));
        }
        Some("nodes") => {
            let nodes = words.next().unwrap().parse::<u32>().unwrap();
            let mut nodes_searched = 0;

            let start_time = P::current_time();

            while tree.visits() < nodes {
                tree.select().unwrap();
                nodes_searched += 1;
            }

            let info_string = info_string::<S, P>(&start_time, nodes_searched, tree);

            output(&info_string);

            let (best_move, _) = tree.best_move().unwrap();

            output(&format!("bestmove {}", best_move));
        }
        Some(_) | None => {
            panic!("Invalid go command \"{}\"", line);
        }
    }
    None
}

pub fn info_string<const S: usize, P: Platform>(
    start_time: &P::Instant,
    nodes_searched: u32,
    tree: &MonteCarloTree<S>,
) -> ArrayString<1024> {
    let (_, best_score) = tree.best_move().unwrap();

    let elapsed = P::elapsed_time(start_time);

    let pv_length = tree.pv().count();

    let mut info_string = ArrayString::new();

    write!(
        info_string,
        "info depth {} seldepth {} nodes {} score cp {} time {} nps {:.0} pv",
        ((tree.visits() as f64 / 10.0).log2()) as u64,
        pv_length,
        tree.visits(),
        (best_score * 200.0 - 100.0) as i64,
        elapsed.as_millis(),
        nodes_searched as f32 / elapsed.as_secs_f32(),
    )
    .unwrap();

    for mv in tree.pv() {
        if let Err(_) = write!(info_string, " {}", mv) {
            // If the info string grows too large, truncate the PV and return
            return info_string;
        }
    }
    info_string
}

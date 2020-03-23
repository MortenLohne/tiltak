use crate::tune::pgn_parse;
use crate::tune::pgn_parse::Game;
use board_game_traits::board::{Board as BoardTrait, Color, GameResult};
use rand::Rng;
use rayon::prelude::*;
use std::io;
use std::sync::atomic::{AtomicU64, Ordering};
use taik::board::{Board, Move};
use taik::mcts;
use taik::mcts::Score;

/// Play a single training game between the same parameter set
pub fn play_game(
    value_params: &[f32],
    policy_params: &[f32],
) -> (Game<Board>, Vec<Vec<(Move, Score)>>) {
    const MCTS_NODES: u64 = 20_000;
    const TEMPERATURE: f64 = 1.0;

    let mut board = Board::start_board();
    let mut game_moves = vec![];
    let mut move_scores = vec![];

    while board.game_result().is_none() {
        let num_plies = game_moves.len();
        if num_plies > 200 {
            break;
        }

        let moves_scores =
            mcts::mcts_training(board.clone(), MCTS_NODES, value_params, policy_params);
        // Turn off temperature in the middle-game, when all games are expected to be unique
        let best_move = if board.half_moves_played() < 20 {
            best_move(TEMPERATURE, &moves_scores[..])
        } else {
            best_move(0.1, &moves_scores[..])
        };
        board.do_move(best_move.clone());
        game_moves.push(best_move);
        move_scores.push(moves_scores);
    }
    (
        Game {
            start_board: Board::default(),
            moves: game_moves
                .into_iter()
                .map(|mv| (mv, String::new()))
                .collect::<Vec<_>>(),
            game_result: board.game_result(),
            tags: vec![],
        },
        move_scores,
    )
}

pub fn best_move(temperature: f64, move_scores: &[(Move, Score)]) -> Move {
    let mut rng = rand::thread_rng();
    let mut move_probabilities = vec![];
    let mut cumulative_prob = 0.0;

    for (mv, individual_prob) in move_scores.iter() {
        cumulative_prob += (*individual_prob as f64).powf(1.0 / temperature);
        move_probabilities.push((mv, cumulative_prob));
    }

    let p = rng.gen_range(0.0, cumulative_prob);
    for (mv, cumulative_prob) in move_probabilities {
        if cumulative_prob > p {
            return mv.clone();
        }
    }
    unreachable!()
}

/// Play an infinite match between two parameter sets
/// Prints the match score continuously
/// In each iteration, each side plays one white and one black game
pub fn play_match_between_params(
    value_params1: &[f32],
    value_params2: &[f32],
    policy_params1: &[f32],
    policy_params2: &[f32],
) -> ! {
    const NODES: u64 = 100_000;
    const TEMPERATURE: f64 = 0.5;

    let player1_wins = AtomicU64::new(0);
    let player2_wins = AtomicU64::new(0);
    let draws = AtomicU64::new(0);
    let aborted = AtomicU64::new(0);
    rayon::iter::repeat(()).for_each(|_| {
        let mut board = Board::start_board();

        while board.game_result().is_none() {
            if board.half_moves_played() > 200 {
                break;
            }
            let moves_scores = match board.side_to_move() {
                Color::White => {
                    mcts::mcts_training(board.clone(), NODES, value_params1, policy_params1)
                }
                Color::Black => {
                    mcts::mcts_training(board.clone(), NODES, value_params2, policy_params2)
                }
            };
            let best_move = best_move(TEMPERATURE, &moves_scores[..]);
            board.do_move(best_move);
        }

        match board.game_result() {
            None => aborted.fetch_add(1, Ordering::Relaxed),
            Some(GameResult::WhiteWin) => player1_wins.fetch_add(1, Ordering::Relaxed),
            Some(GameResult::BlackWin) => player2_wins.fetch_add(1, Ordering::Relaxed),
            Some(GameResult::Draw) => draws.fetch_add(1, Ordering::Relaxed),
        };

        board = Board::start_board();

        while board.game_result().is_none() {
            if board.half_moves_played() > 200 {
                break;
            }
            let moves_scores = match board.side_to_move() {
                Color::White => {
                    mcts::mcts_training(board.clone(), NODES, value_params2, policy_params2)
                }
                Color::Black => {
                    mcts::mcts_training(board.clone(), NODES, value_params1, policy_params1)
                }
            };
            let best_move = best_move(TEMPERATURE, &moves_scores[..]);
            board.do_move(best_move.clone());
        }

        match board.game_result() {
            None => aborted.fetch_add(1, Ordering::Relaxed),
            Some(GameResult::WhiteWin) => player2_wins.fetch_add(1, Ordering::Relaxed),
            Some(GameResult::BlackWin) => player1_wins.fetch_add(1, Ordering::Relaxed),
            Some(GameResult::Draw) => draws.fetch_add(1, Ordering::Relaxed),
        };
        let decided_games = player1_wins.load(Ordering::SeqCst)
            + player2_wins.load(Ordering::SeqCst)
            + draws.load(Ordering::SeqCst);
        println!(
            "+{}-{}={}, {:.1}% score. {} games aborted.",
            player1_wins.load(Ordering::SeqCst),
            player2_wins.load(Ordering::SeqCst),
            draws.load(Ordering::SeqCst),
            100.0
                * (player1_wins.load(Ordering::SeqCst) as f64
                    + draws.load(Ordering::SeqCst) as f64 / 2.0)
                / decided_games as f64,
            aborted.load(Ordering::SeqCst)
        );
    });
    unreachable!()
}

/// Write a single game in ptn format with the given writer
pub fn game_to_ptn<W: io::Write>(game: &Game<Board>, writer: &mut W) -> Result<(), io::Error> {
    let Game {
        start_board,
        moves,
        game_result,
        tags,
    } = game;
    pgn_parse::game_to_pgn(
        &mut start_board.clone(),
        &moves,
        "",
        "",
        "",
        "",
        tags.iter()
            .find_map(|(tag, val)| {
                if &tag.to_lowercase() == "white" {
                    Some(val)
                } else {
                    None
                }
            })
            .unwrap_or(&String::new()),
        "",
        *game_result,
        &[],
        writer,
    )
}

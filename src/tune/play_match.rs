use crate::board::Board;
use crate::mcts;
use crate::tune::pgn_parse;
use crate::tune::pgn_parse::Game;
use board_game_traits::board::{Board as BoardTrait, Color, GameResult};
use std::io;

pub fn play_game(params: &[f32]) -> Game<Board> {
    const MCTS_NODES: u64 = 20_000;
    const TEMPERATURE: f64 = 1.0;

    let mut board = Board::start_board();
    let mut game_moves = vec![];

    while board.game_result().is_none() {
        let num_plies = game_moves.len();
        if num_plies > 200 {
            break;
        }
        // Turn off temperature in the middle-game, when all games are expected to be unique
        let (best_move, _score) = if num_plies < 20 {
            mcts::mcts_training(board.clone(), MCTS_NODES, params, TEMPERATURE)
        } else {
            mcts::mcts_training(board.clone(), MCTS_NODES, params, 0.1)
        };
        board.do_move(best_move.clone());
        game_moves.push(best_move);
    }
    Game {
        start_board: Board::default(),
        moves: game_moves
            .into_iter()
            .map(|mv| (mv, String::new()))
            .collect::<Vec<_>>(),
        game_result: board.game_result(),
        tags: vec![],
    }
}

pub fn play_match_between_params(params1: &[f32], params2: &[f32]) -> ! {
    const NODES: u64 = 50_000;
    const TEMPERATURE: f64 = 0.5;

    let mut player1_wins = 0;
    let mut player2_wins = 0;
    let mut draws = 0;
    let mut aborted = 0;
    loop {
        let mut board = Board::start_board();

        while board.game_result().is_none() {
            if board.moves_played() > 200 {
                break;
            }
            let (best_move, _) = match board.side_to_move() {
                Color::White => mcts::mcts_training(board.clone(), NODES, params1, TEMPERATURE),
                Color::Black => mcts::mcts_training(board.clone(), NODES, params2, TEMPERATURE),
            };
            board.do_move(best_move.clone());
        }

        match board.game_result() {
            None => aborted += 1,
            Some(GameResult::WhiteWin) => player1_wins += 1,
            Some(GameResult::BlackWin) => player2_wins += 1,
            Some(GameResult::Draw) => draws += 1,
        }

        board = Board::start_board();

        while board.game_result().is_none() {
            if board.moves_played() > 200 {
                break;
            }
            let (best_move, _) = match board.side_to_move() {
                Color::White => mcts::mcts_training(board.clone(), NODES, params2, TEMPERATURE),
                Color::Black => mcts::mcts_training(board.clone(), NODES, params1, TEMPERATURE),
            };
            board.do_move(best_move.clone());
        }

        match board.game_result() {
            None => aborted += 1,
            Some(GameResult::WhiteWin) => player2_wins += 1,
            Some(GameResult::BlackWin) => player1_wins += 1,
            Some(GameResult::Draw) => draws += 1,
        }
        let decided_games = player1_wins + player2_wins + draws;
        println!(
            "+{}-{}={}, {:.1}% score. {} games aborted.",
            player1_wins,
            player2_wins,
            draws,
            100.0 * (player1_wins as f64 + draws as f64 / 2.0) / decided_games as f64,
            aborted
        );
    }
}

pub fn game_to_pgn<W: io::Write>(game: &Game<Board>, writer: &mut W) -> Result<(), io::Error> {
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

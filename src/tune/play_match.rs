use crate::board::Board;
use crate::mcts;
use crate::tune::pgn_parse;
use crate::tune::pgn_parse::Game;
use board_game_traits::board::Board as BoardTrait;
use std::io;

pub fn play_game(params: &[f32]) -> Game<Board> {
    const MCTS_NODES: u64 = 10_000;
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

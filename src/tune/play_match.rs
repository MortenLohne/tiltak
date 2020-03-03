use crate::board::Piece::{BlackFlat, WhiteFlat};
use crate::board::{board_iterator, Board, Move, Square};
use crate::mcts;
use crate::tune::pgn_parse;
use crate::tune::pgn_parse::Game;
use board_game_traits::board::Board as BoardTrait;
use board_game_traits::board::Color;
use rayon::prelude::*;
use std::io;

fn openings() -> impl Iterator<Item = [Move; 2]> {
    [0, 1, 2, 6, 7, 8].iter().flat_map(move |i| {
        let move1 = Move::Place(BlackFlat, Square(*i));
        board_iterator()
            .filter(move |square| *square != Square(*i))
            .map(|square| Move::Place(WhiteFlat, square))
            .map(move |move2| [move1.clone(), move2])
    })
}

pub fn play_match() -> impl ParallelIterator<Item = Game<Board>> {
    const MCTS_NODES: u64 = 10_000;
    const TEMPERATURE: f64 = 1.0;
    openings()
        .collect::<Vec<_>>()
        .into_par_iter()
        .map(|opening_moves| {
            let mut board = Board::start_board();
            let mut game_moves = opening_moves.to_vec();
            for mv in opening_moves.iter() {
                board.do_move(mv.clone());
            }
            while board.game_result().is_none() {
                let num_plies = game_moves.len();
                if num_plies > 10
                    && (1..5).all(|i| game_moves[num_plies - i] == game_moves[num_plies - i - 4])
                {
                    break;
                }
                match board.side_to_move() {
                    Color::Black => {
                        let (best_move, _score) =
                            mcts::mcts(board.clone(), MCTS_NODES, TEMPERATURE);
                        board.do_move(best_move.clone());
                        game_moves.push(best_move);
                    }

                    Color::White => {
                        let (best_move, _score) =
                            mcts::mcts(board.clone(), MCTS_NODES, TEMPERATURE);
                        board.do_move(best_move.clone());
                        game_moves.push(best_move);
                    }
                }
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
        })
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

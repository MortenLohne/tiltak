use crate::board::{Board, Move};
use crate::mcts;
use crate::tune::pgn_parse;
use crate::tune::pgn_parse::Game;
use board_game_traits::board::Board as BoardTrait;
use board_game_traits::board::Color;
use std::io;
use std::io::Write;

fn openings() -> impl Iterator<Item = Board> {
    let board = Board::default();
    let mut moves = vec![];
    board.generate_moves(&mut moves);
    moves.into_iter().flat_map(move |mv| {
        let mut board2 = board.clone();
        board2.do_move(mv);
        let mut moves2 = vec![];
        board2.generate_moves(&mut moves2);
        moves2.into_iter().map(move |mv| {
            let mut board3 = board2.clone();
            board3.do_move(mv);
            board3
        })
    })
}

pub fn play_match() -> impl Iterator<Item = Game<Board>> {
    const MCTS_NODES: u64 = 10_000;
    openings().map(|start_board| {
        let mut board = start_board.clone();
        let mut game_moves = vec![];
        while board.game_result().is_none() {
            let num_moves = game_moves.len();
            if num_moves > 10
                && (1..5).all(|i| game_moves[num_moves - i] == game_moves[num_moves - i - 4])
            {
                break;
            }
            match board.side_to_move() {
                Color::Black => {
                    let (best_move, score) = mcts::mcts(board.clone(), MCTS_NODES);
                    board.do_move(best_move.clone());
                    game_moves.push(best_move);
                }

                Color::White => {
                    let (best_move, score) = mcts::mcts(board.clone(), MCTS_NODES);
                    board.do_move(best_move.clone());
                    game_moves.push(best_move);
                }
            }
        }
        Game {
            start_board,
            moves: game_moves
                .into_iter()
                .map(|mv| (mv, String::new()))
                .collect::<Vec<_>>(),
            game_result: board.game_result(),
            tags: vec![],
        }
    })
}
pub fn game_to_pgn(game: Game<Board>) -> Result<(), io::Error> {
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
        game_result,
        &[],
        &mut io::stdout(),
    )
}

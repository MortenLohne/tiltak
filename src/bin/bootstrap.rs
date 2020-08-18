use board_game_traits::board::Board as EvalBoard;
use lambda_runtime::{error::HandlerError, lambda, Context};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use taik::board::{Board, Move};
use taik::mcts;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
struct Event {
    moves: Vec<Move>,
    time_left: Duration,
    increment: Duration,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
struct Output {
    best_move: Move,
    score: f32,
}

fn main() {
    lambda!(event_handler);
}

fn event_handler(e: Event, _c: Context) -> Result<Output, HandlerError> {
    let mut board = Board::default();
    for mv in e.moves {
        board.do_move(mv);
    }

    let max_time = Duration::min(e.time_left / 40 + e.increment, Duration::from_secs(60));

    let (best_move, score) = mcts::play_move_time(board, max_time);

    Ok(Output { best_move, score })
}

use serde::Serialize;
use serde::Deserialize;
use crate::board::{Move, Board};
use std::time::Duration;
use lambda_runtime::Context;
use lambda_runtime::error::HandlerError;
use board_game_traits::board::Board as EvalBoard;
use crate::mcts;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Event {
    pub moves: Vec<Move>,
    pub time_left: Duration,
    pub increment: Duration,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Output {
    pub best_move: Move,
    pub score: f32,
}

pub fn handle_aws_event(e: Event, _c: Context) -> Result<Output, HandlerError> {
    let mut board = Board::default();
    for mv in e.moves {
        board.do_move(mv);
    }

    let max_time = Duration::min(e.time_left / 40 + e.increment, Duration::from_secs(60));

    let (best_move, score) = mcts::play_move_time(board, max_time);

    Ok(Output { best_move, score })
}

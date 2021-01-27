use crate::aws::{Event, Output};
use crate::board::Board;
use crate::search;
use board_game_traits::board::Board as EvalBoard;
use lambda_runtime::{error::HandlerError, Context};
use std::time::Duration;

/// AWS serverside handler
pub fn handle_aws_event(e: Event, c: Context) -> Result<Output, HandlerError> {
    match e.size {
        4 => handle_aws_event_generic::<4>(e, c),
        5 => handle_aws_event_generic::<5>(e, c),
        s => panic!("Unsupported board size {}", s),
    }
}

pub fn handle_aws_event_generic<const S: usize>(
    e: Event,
    _c: Context,
) -> Result<Output, HandlerError> {
    let mut board = <Board<S>>::default();
    for mv in e.moves {
        board.do_move(mv);
    }

    let max_time = Duration::min(e.time_left / 40 + e.increment, Duration::from_secs(30));

    let (best_move, score) = search::play_move_time(board, max_time);

    Ok(Output { best_move, score })
}

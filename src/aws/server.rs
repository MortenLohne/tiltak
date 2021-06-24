use crate::aws::{Event, Output};
use crate::position::Position;
use crate::search;
use crate::search::MctsSetting;
use board_game_traits::Position as EvalPosition;
use lambda_runtime::{error::HandlerError, Context};
use std::time::Duration;

/// AWS serverside handler
pub fn handle_aws_event(e: Event, c: Context) -> Result<Output, HandlerError> {
    match e.size {
        4 => handle_aws_event_generic::<4>(e, c),
        5 => handle_aws_event_generic::<5>(e, c),
        6 => handle_aws_event_generic::<6>(e, c),
        s => panic!("Unsupported board size {}", s),
    }
}

pub fn handle_aws_event_generic<const S: usize>(
    e: Event,
    _c: Context,
) -> Result<Output, HandlerError> {
    let mut position = <Position<S>>::default();
    for mv in e.moves {
        position.do_move(mv);
    }

    let max_time = if position.half_moves_played() < 4 {
        Duration::min(e.time_left / 80 + e.increment / 4, Duration::from_secs(40))
    } else {
        Duration::min(e.time_left / 40 + e.increment / 2, Duration::from_secs(40))
    };

    let settings = MctsSetting::default().add_dirichlet(0.1);

    let (best_move, score) = search::play_move_time(position, max_time, settings);

    Ok(Output { best_move, score })
}

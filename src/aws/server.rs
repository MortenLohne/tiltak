use crate::aws::{Event, Output, TimeControl};
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

    let settings = if let Some(dirichlet) = e.dirichlet_noise {
        MctsSetting::default()
            .add_dirichlet(dirichlet)
            .add_rollout_depth(e.rollout_depth)
            .add_rollout_temperature(e.rollout_temperature)
    } else {
        MctsSetting::default()
            .add_rollout_depth(e.rollout_depth)
            .add_rollout_temperature(e.rollout_temperature)
    };

    match e.time_control {
        TimeControl::Time(time_left, increment) => {
            let max_time = if position.half_moves_played() < 4 {
                Duration::min(time_left / 80 + increment / 6, Duration::from_secs(40))
            } else {
                Duration::min(time_left / 40 + increment / 3, Duration::from_secs(40))
            };

            let (best_move, score) = search::play_move_time(position, max_time, settings);
            Ok(Output {
                pv: vec![best_move],
                score,
            })
        }
        TimeControl::FixedNodes(nodes) => {
            let mut tree = search::MonteCarloTree::with_settings(position, settings);
            for _ in 0..nodes {
                tree.select();
            }
            let score = tree.mean_action_value();
            let pv = tree.pv().collect();
            Ok(Output { pv, score })
        }
    }
}

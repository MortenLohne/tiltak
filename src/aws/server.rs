use crate::aws::{Event, Output, TimeControl};
use crate::position::{Komi, Position};
use crate::search;
use crate::search::MctsSetting;
use board_game_traits::{GameResult, Position as EvalPosition};
use lambda_runtime::Context;
use pgn_traits::PgnPosition;
use std::convert::TryFrom;
use std::time::Duration;

type Error = Box<dyn std::error::Error + Sync + Send>;

/// AWS serverside handler
pub async fn handle_aws_event(e: Event, c: Context) -> Result<Output, Error> {
    match e.size {
        4 => handle_aws_event_generic::<4>(e, c),
        5 => handle_aws_event_generic::<5>(e, c),
        6 => handle_aws_event_generic::<6>(e, c),
        s => panic!("Unsupported board size {}", s),
    }
}

pub fn handle_aws_event_generic<const S: usize>(e: Event, _c: Context) -> Result<Output, Error> {
    let komi = Komi::try_from(e.komi)?;
    let mut position = match e.tps {
        Some(tps) => <Position<S>>::from_fen_with_komi(&tps, komi)?,
        None => <Position<S>>::start_position_with_komi(komi),
    };
    for move_string in e.moves {
        let mv = position.move_from_san(&move_string)?;
        let mut legal_moves = vec![];
        position.generate_moves(&mut legal_moves);
        if !legal_moves.contains(&mv) {
            return Err(format!("Illegal {}s move {}", S, move_string).into());
        }
        position.do_move(mv);
    }

    match position.game_result() {
        Some(GameResult::Draw) => {
            return Ok(Output {
                pv: vec![],
                score: 0.5,
            })
        }
        Some(GameResult::WhiteWin | GameResult::BlackWin) => {
            return Ok(Output {
                pv: vec![],
                score: 1.0,
            })
        }
        None => (),
    }

    let settings = if let Some(dirichlet) = e.dirichlet_noise {
        MctsSetting::default().add_dirichlet(dirichlet)
    } else {
        MctsSetting::default()
    }
    .add_rollout_depth(e.rollout_depth)
    .add_rollout_temperature(e.rollout_temperature)
    .mem_usage(2_usize.pow(2));

    match e.time_control {
        TimeControl::Time(time_left, increment) => {
            let max_time = if position.half_moves_played() < 4 {
                Duration::min(time_left / 80 + increment / 6, Duration::from_secs(40))
            } else {
                Duration::min(time_left / 40 + increment / 3, Duration::from_secs(40))
            };

            let (best_move, score) = search::play_move_time(position, max_time, settings);
            Ok(Output {
                pv: vec![best_move.to_string::<S>()],
                score,
            })
        }
        TimeControl::FixedNodes(nodes) => {
            let mut tree = search::MonteCarloTree::with_settings(position, settings);
            for _ in 0..nodes {
                if tree.select().is_none() {
                    eprintln!("Warning: Search stopped early due to OOM");
                    break;
                };
            }
            let score = 1.0 - tree.best_move().1;
            let pv = tree.pv().map(|mv| mv.to_string::<S>()).collect();
            Ok(Output { pv, score })
        }
    }
}

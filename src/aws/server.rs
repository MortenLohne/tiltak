use crate::aws::{Event, Output, TimeControl};
use crate::position::{Komi, Position};
use crate::search::MctsSetting;
use crate::search::{self, MonteCarloTree};
use board_game_traits::{GameResult, Position as EvalPosition};
use lambda_runtime::LambdaEvent;
use pgn_traits::PgnPosition;
use std::convert::TryFrom;
use std::time::{Duration, Instant};

type Error = Box<dyn std::error::Error + Sync + Send>;

/// AWS serverside handler
pub async fn handle_aws_event(event: LambdaEvent<Event>) -> Result<Output, Error> {
    match event.payload.size {
        4 => handle_aws_event_generic::<4>(event.payload),
        5 => handle_aws_event_generic::<5>(event.payload),
        6 => handle_aws_event_generic::<6>(event.payload),
        s => panic!("Unsupported board size {}", s),
    }
}

pub fn handle_aws_event_generic<const S: usize>(e: Event) -> Result<Output, Error> {
    let komi = Komi::try_from(e.komi)?;
    let eval_komi = match e.eval_komi {
        Some(komi_f64) => Komi::try_from(komi_f64)?,
        None => komi,
    };
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
                score: 0.5,
                ..Default::default()
            })
        }
        Some(result) if result == GameResult::win_by(position.side_to_move()) => {
            return Ok(Output {
                score: 0.0,
                ..Default::default()
            })
        }
        Some(_) => {
            return Ok(Output {
                score: 1.0,
                ..Default::default()
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
    .add_value_params(<Position<S>>::value_params(eval_komi))
    .add_policy_params(<Position<S>>::policy_params(eval_komi));

    let start_time = Instant::now();

    match e.time_control {
        TimeControl::Time(time_left, increment) => {
            let max_time = if position.half_moves_played() < 4 {
                Duration::min(time_left / 80 + increment / 6, Duration::from_secs(40))
            } else {
                Duration::min(time_left / 40 + increment / 3, Duration::from_secs(40))
            };

            let mut tree = MonteCarloTree::new(position, settings);
            tree.search_for_time(max_time, |_| {});

            let score = 1.0 - tree.best_move().unwrap().1;
            let pv = tree.pv().map(|mv| mv.to_string()).collect();
            Ok(Output {
                pv,
                score,
                nodes: tree.visits(),
                time_taken: start_time.elapsed(),
            })
        }
        TimeControl::FixedNodes(nodes) => {
            let mut tree = search::MonteCarloTree::new(position, settings);
            for _ in 0..nodes {
                if let Err(err) = tree.select() {
                    eprintln!("Warning: {err}");
                    break;
                }
            }
            let score = 1.0 - tree.best_move().unwrap().1;
            let pv = tree.pv().map(|mv| mv.to_string()).collect();
            Ok(Output {
                pv,
                score,
                nodes: tree.visits(),
                time_taken: start_time.elapsed(),
            })
        }
    }
}

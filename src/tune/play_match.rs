use std::time::Duration;
use std::time::Instant;

use board_game_traits::{Color, Position as PositionTrait};
use chrono::Datelike;
use half::f16;
use pgn_traits::PgnPosition;
use rand::seq::SliceRandom;

use crate::position::ExpMove;
use crate::position::Komi;
use crate::position::Move;
use crate::position::Position;
use crate::position::Role;
use crate::ptn::{Game, PtnMove};
use crate::search;
use crate::search::MctsSetting;
use crate::search::TimeControl;

/// Play a single training game between two parameter sets
pub fn play_game<const S: usize>(
    white_settings: &MctsSetting<S>,
    black_settings: &MctsSetting<S>,
    komi: Komi,
    opening: &[Move<S>],
    temperature: f64,
    time_control: &TimeControl,
) -> (Game<Position<S>>, Vec<Vec<(Move<S>, f16)>>) {
    let mut position = Position::start_position_with_komi(komi);
    let mut game_moves = opening.to_vec();
    let mut move_scores = vec![vec![]; opening.len()];
    for mv in opening {
        position.do_move(*mv);
    }
    let mut rng = rand::thread_rng();

    let (mut white_time_left, mut black_time_left, increment) = match time_control {
        TimeControl::FixedNodes(_) => (Duration::MAX, Duration::MAX, Duration::ZERO),
        TimeControl::Time(time, increment) => (*time, *time, *increment),
    };

    while position.game_result().is_none() {
        let num_plies = game_moves.len();
        if num_plies > 400 {
            break;
        }

        let start_time = Instant::now();

        let moves_scores = match (time_control, position.side_to_move()) {
            (TimeControl::FixedNodes(_), Color::White) => {
                search::mcts_training::<S>(position.clone(), time_control, white_settings.clone())
            }
            (TimeControl::FixedNodes(_), Color::Black) => {
                search::mcts_training::<S>(position.clone(), time_control, black_settings.clone())
            }
            (TimeControl::Time(_, _), Color::White) => search::mcts_training::<S>(
                position.clone(),
                &TimeControl::Time(white_time_left, increment),
                white_settings.clone(),
            ),
            (TimeControl::Time(_, _), Color::Black) => search::mcts_training::<S>(
                position.clone(),
                &TimeControl::Time(black_time_left, increment),
                white_settings.clone(),
            ),
        };

        match position.side_to_move() {
            Color::White => {
                white_time_left -= start_time.elapsed();
                white_time_left += increment;
            }
            Color::Black => {
                black_time_left -= start_time.elapsed();
                black_time_left += increment;
            }
        }

        // For white's first and second move, choose a random flatstone move
        // This reduces white's first move advantage, and prevents white from "cheesing"
        // the training games by always playing 1.c3 or 2.Cc3
        let best_move = if komi.half_komi() < 4
            && (position.half_moves_played() == 0 || position.half_moves_played() == 2)
        {
            let flat_moves = moves_scores
                .iter()
                .map(|(mv, _)| mv)
                .filter(|mv| matches!(mv.expand(), ExpMove::Place(Role::Flat, _)))
                .collect::<Vec<_>>();
            **flat_moves.choose(&mut rng).unwrap()
        } else {
            // Turn off temperature after the opening (after `2 * (S - 1)` ply), when all games are expected to be unique
            let temperature = (position.half_moves_played() < 2 * (S - 1)).then_some(temperature);
            search::best_move(&mut rand::thread_rng(), temperature, &moves_scores[..])
        };
        position.do_move(best_move);
        game_moves.push(best_move);
        move_scores.push(moves_scores);
    }

    let date = chrono::Local::now();

    let tags = vec![
        ("Event".to_string(), "Tiltak training".to_string()),
        ("Site".to_string(), "Tiltak".to_string()),
        ("Player1".to_string(), "Tiltak".to_string()),
        ("Player2".to_string(), "Tiltak".to_string()),
        ("Size".to_string(), S.to_string()),
        (
            "Date".to_string(),
            format!("{}.{:0>2}.{:0>2}", date.year(), date.month(), date.day()),
        ),
        ("Komi".to_string(), position.komi().to_string()),
    ];

    (
        Game {
            start_position: Position::start_position_with_komi(komi),
            moves: game_moves
                .into_iter()
                .map(|mv| PtnMove {
                    mv,
                    annotations: vec![],
                    comment: String::new(),
                })
                .collect::<Vec<_>>(),
            game_result_str: position.pgn_game_result(),
            tags,
        },
        move_scores,
    )
}

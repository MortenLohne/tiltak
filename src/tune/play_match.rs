use board_game_traits::{Color, Position as PositionTrait};
use rand::seq::SliceRandom;
use rand::Rng;

use crate::position::mv::Move;
use crate::position::utils::Role;
use crate::position::Position;
use crate::ptn::{Game, PtnMove};
use crate::search;
use crate::search::{MctsSetting, Score};

/// Play a single training game between two parameter sets
pub fn play_game<const S: usize>(
    white_settings: &MctsSetting<S>,
    black_settings: &MctsSetting<S>,
    opening: &[Move],
    temperature: f64,
) -> (Game<Position<S>>, Vec<Vec<(Move, Score)>>) {
    const MCTS_NODES: u64 = 100_000;

    let mut board = Position::start_position();
    let mut game_moves = opening.to_vec();
    let mut move_scores = vec![vec![]; opening.len()];
    for mv in opening {
        board.do_move(mv.clone());
    }
    let mut rng = rand::thread_rng();

    while board.game_result().is_none() {
        let num_plies = game_moves.len();
        if num_plies > 200 {
            break;
        }

        let moves_scores = match board.side_to_move() {
            Color::White => {
                search::mcts_training::<S>(board.clone(), MCTS_NODES, white_settings.clone())
            }
            Color::Black => {
                search::mcts_training::<S>(board.clone(), MCTS_NODES, black_settings.clone())
            }
        };

        // For the first regular move (White's move #2), choose a random flatstone move
        // This reduces white's first move advantage, and prevents white from always playing 2.Cc3
        let best_move = if board.half_moves_played() == 2 {
            let flat_moves = moves_scores
                .iter()
                .map(|(mv, _)| mv)
                .filter(|mv| matches!(*mv, Move::Place(Role::Flat, _)))
                .collect::<Vec<_>>();
            (*flat_moves.choose(&mut rng).unwrap()).clone()
        }
        // Turn off temperature in the middle-game, when all games are expected to be unique
        else if board.half_moves_played() < 20 {
            best_move(temperature, &moves_scores[..])
        } else {
            best_move(0.1, &moves_scores[..])
        };
        board.do_move(best_move.clone());
        game_moves.push(best_move);
        move_scores.push(moves_scores);
    }
    (
        Game {
            start_position: Position::default(),
            moves: game_moves
                .into_iter()
                .map(|mv| PtnMove {
                    mv,
                    annotations: vec![],
                    comment: String::new(),
                })
                .collect::<Vec<_>>(),
            game_result: board.game_result(),
            tags: vec![],
        },
        move_scores,
    )
}

pub fn best_move(temperature: f64, move_scores: &[(Move, Score)]) -> Move {
    let mut rng = rand::thread_rng();
    let mut move_probabilities = vec![];
    let mut cumulative_prob = 0.0;

    for (mv, individual_prob) in move_scores.iter() {
        cumulative_prob += (*individual_prob as f64).powf(1.0 / temperature);
        move_probabilities.push((mv, cumulative_prob));
    }

    let p = rng.gen_range(0.0, cumulative_prob);
    for (mv, cumulative_prob) in move_probabilities {
        if cumulative_prob > p {
            return mv.clone();
        }
    }
    unreachable!()
}

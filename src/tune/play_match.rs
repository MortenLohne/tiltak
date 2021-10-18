use board_game_traits::{Color, Position as PositionTrait};
use rand::seq::SliceRandom;

use crate::position::Move;
use crate::position::Position;
use crate::position::Role;
use crate::ptn::{Game, PtnMove};
use crate::search;
use crate::search::{MctsSetting, Score};

/// Play a single training game between two parameter sets
pub fn play_game<const S: usize>(
    white_settings: &MctsSetting<S>,
    black_settings: &MctsSetting<S>,
    opening: &[Move],
    temperature: f64,
    mcts_nodes: u64,
) -> (Game<Position<S>>, Vec<Vec<(Move, Score)>>) {
    let mut position = Position::start_position();
    let mut game_moves = opening.to_vec();
    let mut move_scores = vec![vec![]; opening.len()];
    for mv in opening {
        position.do_move(mv.clone());
    }
    let mut rng = rand::thread_rng();

    while position.game_result().is_none() {
        let num_plies = game_moves.len();
        if num_plies > 200 {
            break;
        }

        let moves_scores = match position.side_to_move() {
            Color::White => {
                search::mcts_training::<S>(position.clone(), mcts_nodes, white_settings.clone())
            }
            Color::Black => {
                search::mcts_training::<S>(position.clone(), mcts_nodes, black_settings.clone())
            }
        };

        // For the first regular move (White's move #2), choose a random flatstone move
        // This reduces white's first move advantage, and prevents white from always playing 2.Cc3
        let best_move = if position.half_moves_played() == 2 {
            let flat_moves = moves_scores
                .iter()
                .map(|(mv, _)| mv)
                .filter(|mv| matches!(*mv, Move::Place(Role::Flat, _)))
                .collect::<Vec<_>>();
            (*flat_moves.choose(&mut rng).unwrap()).clone()
        }
        // Turn off temperature in the middle-game, when all games are expected to be unique
        else if position.half_moves_played() < 20 {
            search::best_move(&mut rand::thread_rng(), temperature, &moves_scores[..])
        } else {
            search::best_move(&mut rand::thread_rng(), 0.1, &moves_scores[..])
        };
        position.do_move(best_move.clone());
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
            game_result: position.game_result(),
            tags: vec![],
        },
        move_scores,
    )
}

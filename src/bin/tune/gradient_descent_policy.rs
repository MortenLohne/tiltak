use board_game_traits::board::Board as BoardTrait;
use pgn_traits::pgn::PgnBoard;
use rayon::prelude::*;
use std::fmt::Debug;
use taik::board::TunableBoard;
use taik::mcts::Score;

pub fn gradient_descent_policy<B>(
    positions: &[B],
    move_scores: &[Vec<(<B as BoardTrait>::Move, Score)>],
    test_positions: &[B],
    test_move_scores: &[Vec<(<B as BoardTrait>::Move, Score)>],
    params: &[f32],
) -> Vec<f32>
where
    B: TunableBoard + BoardTrait + PgnBoard + Send + Debug + Sync + Clone,
    <B as BoardTrait>::Move: Send + Sync,
{
    assert_eq!(positions.len(), move_scores.len());
    assert_eq!(test_positions.len(), test_move_scores.len());

    let mut eta = 500.0;
    let beta = 0.8;

    // If error is not reduced this number of times, reduce eta, or abort if eta is already low
    const MAX_TRIES: usize = 20;

    let initial_error = average_error(test_positions, test_move_scores, params);
    println!(
        "Running gradient descent on {} positions and {} test positions",
        positions.len(),
        test_positions.len()
    );
    println!("Initial parameters: {:?}", params);
    println!("Initial test error: {}", initial_error);
    println!(
        "Initial training error: {}",
        average_error(positions, move_scores, params)
    );

    let mut error_sets = vec![initial_error];
    let mut best_iteration = 0;
    let mut lowest_error = initial_error;
    let mut parameter_sets: Vec<Vec<f32>> = vec![params.to_vec()];
    let mut best_parameter_set = params.to_vec();
    let mut gradients = vec![0.0; params.len()];

    for i in 0.. {
        let last_params = parameter_sets.last().unwrap().clone();
        let slopes = calc_slope(positions, move_scores, &last_params);
        gradients = gradients
            .iter()
            .zip(slopes)
            .map(|(gradient, slope)| beta * gradient + (1.0 - beta) * slope)
            .collect();
        println!("Gradients: {:?}", gradients);

        let new_params: Vec<f32> = last_params
            .iter()
            .zip(gradients.iter())
            .map(|(param, gradient)| param + gradient * eta)
            .collect();
        println!("New parameters: {:?}", new_params);

        let error = average_error(test_positions, test_move_scores, &new_params);
        println!("Error now {}\n", error);

        if error < lowest_error && i - best_iteration <= MAX_TRIES {
            if lowest_error / error > 1.00001 {
                best_iteration = i;
            }
            lowest_error = error;
            best_parameter_set = new_params.to_vec();
        } else if i - best_iteration > MAX_TRIES {
            if eta < 10.0 {
                println!(
                    "Finished gradient descent, error is {}. Parameters:\n{:?}",
                    lowest_error, best_parameter_set
                );
                return best_parameter_set;
            } else {
                eta /= 10.0;
                parameter_sets = vec![best_parameter_set.clone()];
                error_sets = vec![lowest_error];
                best_iteration = i;
                println!("Reduced eta to {}, best error was {}\n", eta, lowest_error);
                continue;
            }
        }
        error_sets.push(error);
        parameter_sets.push(new_params);
    }
    unreachable!()
}

/// For each parameter, calculate the slope for that dimension
fn calc_slope<B>(
    positions: &[B],
    mcts_move_scores: &[Vec<(<B as BoardTrait>::Move, Score)>],
    params: &[f32],
) -> Vec<f32>
where
    B: TunableBoard + BoardTrait + PgnBoard + Send + Sync + Debug + Clone,
    <B as BoardTrait>::Move: Send + Sync,
{
    const EPSILON: f32 = 0.001;

    params
        .par_iter()
        .enumerate()
        .map(|(i, p)| {
            let mut params_hat: Vec<f32> = params.to_vec();
            params_hat[i] = p + EPSILON;

            let error_old = average_error(positions, mcts_move_scores, params);
            let error_new = average_error(positions, mcts_move_scores, &params_hat);

            (error_old - error_new) / EPSILON
        })
        .collect()
}

/// Mean squared error of the parameter set, measured against given positions and move scores
fn average_error<B>(
    positions: &[B],
    move_scores: &[Vec<(<B as BoardTrait>::Move, Score)>],
    params: &[f32],
) -> f32
where
    B: TunableBoard + BoardTrait + PgnBoard + Send + Debug + Sync,
    <B as BoardTrait>::Move: Send + Sync,
{
    assert_eq!(positions.len(), move_scores.len());
    positions
        .into_par_iter()
        .zip(move_scores)
        .map(|(board, mcts_move_score)| error::<B>(&board, mcts_move_score, params))
        .sum::<f32>()
        / (positions.len() as f32)
}
/// MSE of a single move generation
fn error<B: TunableBoard + Debug>(
    board: &B,
    mcts_move_score: &[(B::Move, f32)],
    params: &[f32],
) -> f32 {
    let mut static_probs: Vec<f32> = mcts_move_score
        .iter()
        .map(|(mv, _)| board.probability_for_move(params, mv))
        .collect();

    let prob_sum: f32 = static_probs.iter().sum();

    for p in static_probs.iter_mut() {
        *p /= prob_sum
    }

    mcts_move_score
        .iter()
        .zip(static_probs)
        .map(|((_move, mcts_score), static_prob)| {
            let error = f32::powf(static_prob - *mcts_score, 2.0);
            assert!(
                error >= 0.0 && error <= 1.0,
                "Error was {} for static prob {}, mcts score {} for move {:?} on board\n{:?}",
                error,
                static_prob,
                mcts_score,
                _move,
                board
            );
            error
        })
        .sum::<f32>()
        / mcts_move_score.len() as f32
}

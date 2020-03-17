use board_game_traits::board::Board as BoardTrait;
use board_game_traits::board::GameResult;
use pgn_traits::pgn::PgnBoard;
use rayon::prelude::*;
use std::fmt::Debug;
use taik::board::TunableBoard;
use taik::mcts;

pub fn gradient_descent<B>(
    positions: &[B],
    results: &[GameResult],
    test_positions: &[B],
    test_results: &[GameResult],
    params: &[f32],
) -> Vec<f32>
where
    B: TunableBoard + BoardTrait + PgnBoard + Send + Debug + Sync + Clone,
{
    assert_eq!(positions.len(), results.len());
    assert_eq!(test_positions.len(), test_results.len());

    let mut eta = 5.0;
    let beta = 0.8;

    // If error is not reduced this number of times, reduce eta, or abort if eta is already low
    const MAX_TRIES: usize = 12;

    let initial_error = average_error(test_positions, test_results, params);
    println!(
        "Running gradient descent on {} positions and {} test positions",
        positions.len(),
        test_positions.len()
    );
    println!("Initial parameters: {:?}", params);
    println!("Initial test error: {}", initial_error);
    println!(
        "Initial training error: {}",
        average_error(positions, results, params)
    );

    let mut error_sets = vec![initial_error];
    let mut best_iteration = 0;
    let mut lowest_error = initial_error;
    let mut parameter_sets: Vec<Vec<f32>> = vec![params.to_vec()];
    let mut best_parameter_set = params.to_vec();
    let mut gradients = vec![0.0; params.len()];

    for i in 0.. {
        let last_params = parameter_sets.last().unwrap().clone();
        let slopes = calc_slope(positions, results, &last_params);
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

        let error = average_error(test_positions, test_results, &new_params);
        println!("Error now {}\n", error);

        if error < lowest_error && i - best_iteration <= MAX_TRIES {
            if lowest_error / error > 1.00001 {
                best_iteration = i;
            }
            lowest_error = error;
            best_parameter_set = new_params.to_vec();
        } else if i - best_iteration > MAX_TRIES {
            if eta < 0.01 {
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
fn calc_slope<B>(positions: &[B], results: &[GameResult], params: &[f32]) -> Vec<f32>
where
    B: TunableBoard + BoardTrait + PgnBoard + Send + Sync + Clone + Debug,
{
    const EPSILON: f32 = 0.001;

    params
        .par_iter()
        .enumerate()
        .map(|(i, p)| {
            let mut params_hat: Vec<f32> = params.to_vec();
            params_hat[i] = p + EPSILON;

            let error_old = average_error(positions, results, params);
            let error_new = average_error(positions, results, &params_hat);

            (error_old - error_new) / EPSILON
        })
        .collect()
}

/// Mean squared error of the parameter set, measured against given results and positions
fn average_error<B>(positions: &[B], results: &[GameResult], params: &[f32]) -> f32
where
    B: TunableBoard + BoardTrait + PgnBoard + Send + Debug + Sync,
{
    assert_eq!(positions.len(), results.len());
    positions
        .into_par_iter()
        .zip(results)
        .map(|(board, game_result)| {
            let eval = board.static_eval_with_params(params);
            error(eval, *game_result)
        })
        .sum::<f32>()
        / (positions.len() as f32)
}
/// Squared error of a single centipawn evaluation
fn error(eval: f32, game_result: GameResult) -> f32 {
    let answer = match game_result {
        GameResult::WhiteWin => 1.0,
        GameResult::Draw => 0.5,
        GameResult::BlackWin => 0.0,
    };

    f32::powf(answer - sigmoid(eval), 2.0)
}

fn sigmoid(eval: f32) -> f32 {
    mcts::cp_to_win_percentage(eval)
}

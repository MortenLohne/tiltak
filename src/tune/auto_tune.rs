use crate::mcts;
use board_game_traits::board::Board as BoardTrait;
use board_game_traits::board::GameResult;
use pgn_traits::pgn::PgnBoard;
use rayon::prelude::*;
use std::fmt::Debug;

pub trait TunableBoard {
    const PARAMS: &'static [f32];

    fn static_eval_with_params(&self, params: &[f32]) -> f32;
}

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

    let mut eta = 0.1;
    let beta = 0.8;

    const MAX_TRIES: usize = 8;

    let initial_error = average_error(test_positions, test_results, params);
    println!(
        "Running gradient descent on {} positions and {} test positions",
        positions.len(),
        test_positions.len()
    );
    println!("Initial error: {}", initial_error);

    let mut errors = vec![initial_error];
    let mut lowest_error = initial_error;
    let mut paramss: Vec<Vec<f32>> = vec![params.to_vec()];
    let mut best_params = params.to_vec();
    let mut gradients = vec![0.0; params.len()];

    loop {
        let last_params = paramss.last().unwrap().clone();
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

        if error < lowest_error {
            lowest_error = error;
            best_params = new_params.to_vec();
        } else if errors.len() >= MAX_TRIES
            && (1..=MAX_TRIES).all(|i| errors[errors.len() - i] > lowest_error)
        {
            if eta < 0.005 {
                return best_params;
            } else {
                eta /= 10.0;
                paramss = vec![best_params.clone()];
                errors = vec![lowest_error];
                println!("Reduced eta to {}\n", eta);
                continue;
            }
        }
        errors.push(error);
        paramss.push(new_params);
    }
}

pub fn calc_slope<B>(positions: &[B], results: &[GameResult], params: &[f32]) -> Vec<f32>
where
    B: TunableBoard + BoardTrait + PgnBoard + Send + Sync + Clone,
{
    const EPSILON: f32 = 0.001;

    params
        .par_iter()
        .enumerate()
        .map(|(i, p)| {
            let mut params_hat: Vec<f32> = params.to_vec();
            params_hat[i] = p + EPSILON;
            positions
                .iter()
                .zip(results)
                .map(|(board, &game_result)| {
                    let score1 = board.static_eval_with_params(params);
                    let score2 = board.static_eval_with_params(&params_hat);
                    error(score1, game_result) - error(score2, game_result)
                })
                .sum()
        })
        .collect()
}

pub fn average_error<B>(positions: &[B], results: &[GameResult], params: &[f32]) -> f32
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

pub fn error(eval: f32, game_result: GameResult) -> f32 {
    let answer = match game_result {
        GameResult::WhiteWin => 1.0,
        GameResult::Draw => 0.5,
        GameResult::BlackWin => 0.0,
    };

    f32::powf(answer - sigmoid(eval), 2.0)
}

pub fn sigmoid(eval: f32) -> f32 {
    mcts::cp_to_win_percentage(eval)
}

use rayon::prelude::*;

pub fn gradient_descent<B, R, F>(
    positions: &[B],
    results: &[R],
    test_positions: &[B],
    test_results: &[R],
    params: &[f32],
    error_function: F,
) -> Vec<f32>
where
    B: Send + Sync,
    R: Send + Sync,
    F: Fn(&B, &R, &[f32]) -> f32 + Send + Sync,
{
    assert_eq!(positions.len(), results.len());
    assert_eq!(test_positions.len(), test_results.len());

    let etas = &[5.0, 0.5, 0.05];
    let beta = 0.8;

    // If error is not reduced this number of times, reduce eta, or abort if eta is already low
    const MAX_TRIES: usize = 12;

    let initial_error = average_error(test_positions, test_results, params, &error_function);
    println!(
        "Running gradient descent on {} positions and {} test positions",
        positions.len(),
        test_positions.len()
    );
    println!("Initial parameters: {:?}", params);
    println!("Initial test error: {}", initial_error);
    println!(
        "Initial training error: {}",
        average_error(positions, results, params, &error_function)
    );

    let mut lowest_error = initial_error;
    let mut best_parameter_set = params.to_vec();

    for eta in etas {
        let mut best_iteration = 0;

        let mut parameter_set = best_parameter_set.clone();
        let mut gradients = vec![0.0; params.len()];

        for i in 0.. {
            let slopes = calc_slope(positions, results, &parameter_set, &error_function);
            gradients = gradients
                .iter()
                .zip(slopes)
                .map(|(gradient, slope)| beta * gradient + (1.0 - beta) * slope)
                .collect();
            println!("Gradients: {:?}", gradients);

            parameter_set = parameter_set
                .iter()
                .zip(gradients.iter())
                .map(|(param, gradient)| param + gradient * eta)
                .collect();
            println!("New parameters: {:?}", parameter_set);

            let error = average_error(
                test_positions,
                test_results,
                &parameter_set,
                &error_function,
            );
            println!("Error now {}\n", error);

            if error < lowest_error && i - best_iteration <= MAX_TRIES {
                if lowest_error / error > 1.00001 {
                    best_iteration = i;
                }
                lowest_error = error;
                best_parameter_set = parameter_set.clone();
            } else if i - best_iteration > MAX_TRIES {
                break;
            }
        }
        println!("Reduced eta to {}, best error was {}\n", eta, lowest_error);
        continue;
    }
    println!(
        "Finished gradient descent, error is {}. Parameters:\n{:?}",
        lowest_error, best_parameter_set
    );
    return best_parameter_set;
}

/// For each parameter, calculate the slope for that dimension
fn calc_slope<B, R, F>(positions: &[B], results: &[R], params: &[f32], error: &F) -> Vec<f32>
where
    B: Send + Sync,
    R: Send + Sync,
    F: Fn(&B, &R, &[f32]) -> f32 + Send + Sync,
{
    const EPSILON: f32 = 0.001;

    params
        .par_iter()
        .enumerate()
        .map(|(i, p)| {
            let mut params_hat: Vec<f32> = params.to_vec();
            params_hat[i] = p + EPSILON;

            let error_old = average_error(positions, results, params, error);
            let error_new = average_error(positions, results, &params_hat, error);

            (error_old - error_new) / EPSILON
        })
        .collect()
}

/// Mean squared error of the parameter set, measured against given results and positions
fn average_error<B, R, F>(positions: &[B], results: &[R], params: &[f32], error: &F) -> f32
where
    B: Send + Sync,
    R: Send + Sync,
    F: Fn(&B, &R, &[f32]) -> f32 + Send + Sync,
{
    assert_eq!(positions.len(), results.len());
    positions
        .into_par_iter()
        .zip(results)
        .map(|(board, game_result)| error(board, game_result, params))
        .sum::<f32>()
        / (positions.len() as f32)
}

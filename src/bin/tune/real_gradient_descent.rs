use rayon::prelude::*;

pub fn gradient_descent(
    coefficient_sets: &[Vec<f64>],
    results: &[f64],
    test_coefficient_sets: &[Vec<f64>],
    test_results: &[f64],
    params: &[f64],
) -> Vec<f64> {
    assert_eq!(coefficient_sets.len(), results.len());
    assert_eq!(test_coefficient_sets.len(), test_results.len());

    let beta = 0.9;

    // If error is not reduced this number of times, reduce eta, or abort if eta is already low
    const MAX_TRIES: usize = 8;

    let initial_error = average_error(test_coefficient_sets, test_results, params);
    println!(
        "Running gradient descent on {} positions and {} test positions",
        coefficient_sets.len(),
        test_coefficient_sets.len()
    );
    println!("Initial parameters: {:?}", params);
    println!("Initial test error: {}", initial_error);
    println!(
        "Initial training error: {}",
        average_error(coefficient_sets, results, params)
    );

    let mut lowest_error = initial_error;
    let mut best_parameter_set = params.to_vec();

    let mut iterations_since_improvement = 0;
    let mut iterations_since_large_improvement = 0;

    let mut parameter_set = best_parameter_set.clone();
    let mut gradients = vec![0.0; params.len()];

    let eta = 1.0;

    println!("First n sets:");
    for (coefficients, result) in coefficient_sets.iter().zip(results).take(5) {
        println!("{:?}", coefficients);
        println!("Result {}", result)
    }

    loop {
        let slopes = calc_slope(coefficient_sets, results, &parameter_set);
        println!("Slopes: {:?}", slopes);
        gradients = gradients
            .iter()
            .zip(slopes)
            .map(|(gradient, slope)| beta * gradient + (1.0 - beta) * slope)
            .collect();
        println!("Gradients: {:?}", gradients);

        parameter_set = parameter_set
            .iter()
            .zip(gradients.iter())
            .map(|(param, gradient)| param - gradient * eta)
            .collect();
        println!("New parameters: {:?}", parameter_set);

        let error = average_error(test_coefficient_sets, test_results, &parameter_set);
        println!("Error now {}\n", error);

        if error < lowest_error {
            iterations_since_improvement = 0;
            if lowest_error / error > 1.000_001 {
                iterations_since_large_improvement = 0;
            } else {
                iterations_since_large_improvement += 1;
                if iterations_since_large_improvement >= MAX_TRIES * 2 {
                    break;
                }
            }
            lowest_error = error;
            best_parameter_set = parameter_set.clone();
        } else {
            iterations_since_improvement += 1;
            iterations_since_large_improvement += 1;
            if iterations_since_improvement >= MAX_TRIES {
                break;
            }
        }
    }

    println!(
        "Finished gradient descent, error is {}. Parameters:\n{:?}",
        lowest_error, best_parameter_set
    );
    best_parameter_set
}

/// For each parameter, calculate the slope for that dimension
fn calc_slope(coefficient_sets: &[Vec<f64>], results: &[f64], params: &[f64]) -> Vec<f64> {
    let mut slopes = coefficient_sets
        .par_iter()
        .zip(results)
        .map(|(coefficients, result)| {
            let estimated_result = eval_from_params(coefficients, params);
            let estimated_sigmoid = sigmoid(estimated_result);
            let derived_sigmoid_result = sigmoid_derived(estimated_result);

            let gradients_for_this_training_sample = coefficients.iter().map(|coefficient| {
                (estimated_sigmoid - result) * derived_sigmoid_result * *coefficient
            });

            gradients_for_this_training_sample.collect::<Vec<f64>>()
        })
        .fold(
            || vec![0.0; params.len()],
            |mut a, b| {
                assert_eq!(a.len(), b.len());
                for (c, d) in a.iter_mut().zip(b) {
                    *c += d;
                }
                a
            },
        )
        .reduce(
            || vec![0.0; params.len()],
            |mut a, b| {
                assert_eq!(a.len(), b.len());
                for (c, d) in a.iter_mut().zip(b) {
                    *c += d;
                }
                a
            },
        );

    for slope in slopes.iter_mut() {
        *slope /= coefficient_sets.len() as f64;
    }
    slopes
}

/// Mean squared error of the parameter set, measured against given results and positions
fn average_error(coefficient_sets: &[Vec<f64>], results: &[f64], params: &[f64]) -> f64 {
    assert_eq!(coefficient_sets.len(), results.len());
    coefficient_sets
        .into_par_iter()
        .zip(results)
        .map(|(coefficients, game_result)| {
            (sigmoid(eval_from_params(coefficients, params)) - game_result).powf(2.0)
        })
        .sum::<f64>()
        / (coefficient_sets.len() as f64)
}

pub fn eval_from_params(coefficients: &[f64], params: &[f64]) -> f64 {
    assert_eq!(coefficients.len(), params.len());
    coefficients.iter().zip(params).map(|(c, p)| c * p).sum()
}

pub fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + f64::exp(-x as f64))
}

pub fn sigmoid_derived(x: f64) -> f64 {
    f64::exp(x as f64) / f64::powi(1.0 + f64::exp(x as f64), 2)
}

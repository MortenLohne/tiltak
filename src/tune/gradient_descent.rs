use half::f16;
use log::trace;
use rayon::prelude::*;
use std::{array, time::Instant};

pub struct TrainingSample<const N: usize> {
    pub features: [f16; N],
    pub offset: f32,
    pub result: f16,
}

pub fn gradient_descent<R: rand::Rng, const N: usize>(
    samples: &[TrainingSample<N>],
    params: &[f32; N],
    initial_learning_rate: f32,
    rng: &mut R,
) -> [f32; N] {
    let start_time = Instant::now();
    let beta = 0.98;

    // If error is not reduced this number of times, reduce eta, or abort if eta is already low
    const MAX_TRIES: usize = 50;
    const ERROR_THRESHOLD: f64 = 1.000_000_1;
    const MINIBATCH_SIZE: usize = 10000;

    let initial_error = average_error(samples, params);
    let num_minibatches = samples.len().div_ceil(MINIBATCH_SIZE);
    println!("Running gradient descent on {} positions", samples.len(),);
    trace!("Initial parameters: {:?}", params);
    println!("Initial error: {}", initial_error);

    let mut lowest_error = initial_error;
    let mut best_parameter_set = *params;
    let mut i = 0;

    'eta_loop: for eta in [
        initial_learning_rate,
        initial_learning_rate / 10.0,
        initial_learning_rate / 100.0,
        initial_learning_rate / 1000.0,
    ]
    .iter()
    {
        trace!("\nTuning with eta = {}\n", eta);
        let mut parameter_set = best_parameter_set;
        let mut gradients = [0.0; N];

        let mut iterations_since_improvement = 0;
        let mut iterations_since_large_improvement = 0;
        'minibatch_loop: loop {
            for _ in 0..num_minibatches {
                let minibatch_samples = if samples.len() <= MINIBATCH_SIZE {
                    samples
                } else {
                    let start_index = rng.gen_range(0..(samples.len() - MINIBATCH_SIZE));
                    &samples[start_index..(start_index + MINIBATCH_SIZE)]
                };
                let slopes = calc_slope(minibatch_samples, &parameter_set);
                trace!("Slopes: {:?}", slopes);
                gradients
                    .iter_mut()
                    .zip(slopes.iter())
                    .for_each(|(gradient, slope)| {
                        *gradient = beta * *gradient + (1.0 - beta) * slope
                    });
                trace!("Gradients: {:?}", gradients);

                parameter_set
                    .iter_mut()
                    .zip(gradients.iter())
                    .for_each(|(param, gradient)| *param -= gradient * eta);
            }
            trace!("New parameters: {:?}", parameter_set);

            let error = average_error(samples, &parameter_set);

            if i % 100 == 0 {
                println!(
                    "\n{:04} iterations in {:.1}s: Error {:.8}, eta={:.4}, {} ({}) iterations since (large) improvement, {:.8} error ratio, {} minibatches\n",
                    i,
                    start_time.elapsed().as_secs_f32(),
                    error,
                    eta,
                    iterations_since_improvement,
                    iterations_since_large_improvement,
                    lowest_error / error,
                    num_minibatches
                );
                if i % 1000 == 0 {
                    trace!("New parameters: {:?}", parameter_set);
                }
            } else {
                trace!(
                    "\n{:04} iterations in {:.1}s: Error {:.8}, eta={:.4}, {} ({}) iterations since (large) improvement, {:.8} error ratio, {} minibatches\n",
                    i,
                    start_time.elapsed().as_secs_f32(),
                    error,
                    eta,
                    iterations_since_improvement,
                    iterations_since_large_improvement,
                    lowest_error / error,
                    num_minibatches
                );
                trace!("New parameters: {:?}", parameter_set);
            }
            i += 1;

            if error < lowest_error {
                iterations_since_improvement = 0;
                if lowest_error / error > ERROR_THRESHOLD {
                    iterations_since_large_improvement = 0;
                } else {
                    iterations_since_large_improvement += 1;
                    if iterations_since_large_improvement >= MAX_TRIES {
                        println!(
                            "\n{:04} iterations in {:.1}s, lowest error {:.8}. Reducing eta because improvements have been insignificant for {} iterations\n",
                            i,
                            start_time.elapsed().as_secs_f32(),
                            lowest_error,
                            iterations_since_large_improvement
                        );
                        // If we can only get minute improvements with this eta,
                        // going to smaller etas will be no good
                        break 'eta_loop;
                    }
                }
                lowest_error = error;
                best_parameter_set = parameter_set;
            } else {
                iterations_since_improvement += 1;
                iterations_since_large_improvement += 1;
                if iterations_since_improvement >= MAX_TRIES {
                    println!(
                        "\n{:04} iterations in {:.1}s, lowest error {:.8}. Reducing eta because no improvements were seen for {} iterations\n",
                        i,
                        start_time.elapsed().as_secs_f32(),
                        lowest_error,
                        iterations_since_improvement
                    );
                    break 'minibatch_loop;
                }
            }
        }
    }

    let elapsed = start_time.elapsed();

    println!(
        "Finished gradient descent in {:.1}s, error is {:.8}. Parameters:\n{:?}",
        elapsed.as_secs_f64(),
        lowest_error,
        best_parameter_set.to_vec()
    );
    best_parameter_set
}

/// For each parameter, calculate the slope for that dimension
fn calc_slope<const N: usize>(samples: &[TrainingSample<N>], params: &[f32; N]) -> [f32; N] {
    let mut slopes = samples
        .par_iter()
        .map(
            |TrainingSample {
                 features,
                 result,
                 offset,
             }| {
                let estimated_result = eval_from_params(features, params, *offset);
                let estimated_sigmoid = sigmoid(estimated_result);
                let derived_sigmoid_result = sigmoid_derived(estimated_result);

                features.map(|feature| {
                    (estimated_sigmoid - result.to_f32())
                        * derived_sigmoid_result
                        * feature.to_f32()
                })
            },
        )
        .reduce(
            || [0.0; N],
            |mut a, b| {
                for i in 0..N {
                    a[i] += b[i];
                }
                a
            },
        );

    for slope in slopes.iter_mut() {
        *slope /= samples.len() as f32;
    }
    slopes
}

/// Mean squared error of the parameter set, measured against given results and positions
fn average_error<const N: usize>(samples: &[TrainingSample<N>], params: &[f32; N]) -> f64 {
    samples
        .par_iter()
        .map(
            |TrainingSample {
                 features,
                 result,
                 offset,
             }| {
                (sigmoid(eval_from_params(features, params, *offset)) - result.to_f32()).powi(2)
            },
        )
        .map(|f| f as f64)
        .sum::<f64>()
        / (samples.len() as f64)
}

pub fn eval_from_params<const N: usize>(
    features: &[f16; N],
    params: &[f32; N],
    offset: f32,
) -> f32 {
    const SIMD_WIDTH: usize = 8;
    assert_eq!(N % SIMD_WIDTH, 0);

    let partial_sums: [f32; SIMD_WIDTH] = features
        .chunks_exact(SIMD_WIDTH)
        .zip(params.chunks_exact(SIMD_WIDTH))
        .fold([0.0; SIMD_WIDTH], |acc, (c, p)| {
            array::from_fn(|i| acc[i] + c[i].to_f32() * p[i])
        });

    partial_sums.iter().sum::<f32>() + offset
}

pub fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + f32::exp(-x))
}

pub fn sigmoid_derived(x: f32) -> f32 {
    f32::exp(x) / f32::powi(1.0 + f32::exp(x), 2)
}

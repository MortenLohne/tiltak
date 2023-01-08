use dfdx::{
    data::SubsetIterator,
    optim::{Momentum, Sgd, SgdConfig},
    prelude::*,
    tensor::Cpu,
};
use log::trace;
use rand::{rngs::StdRng, SeedableRng};
use rayon::prelude::*;
use std::time::Instant;

pub struct TrainingSample<const N: usize> {
    pub features: [f32; N],
    pub offset: f32,
    pub result: f32,
}

pub fn get_batch<const N: usize, const B: usize>(
    samples: &[TrainingSample<N>],
    dev: &Cpu,
    idxs: [usize; B],
) -> (Tensor<Rank2<B, N>, f32, Cpu>, Tensor<Rank2<B, 1>, f32, Cpu>) {
    let mut input_data: Vec<f32> = Vec::with_capacity(B * N);
    let mut output_data: Vec<f32> = Vec::with_capacity(B * 1);
    for (_batch_i, &data_index) in idxs.iter().enumerate() {
        input_data.extend(samples[data_index].features);
        output_data.push(samples[data_index].result);
    }
    let mut input_tensor = dev.zeros();
    input_tensor.copy_from(&input_data);
    let mut output_tensor = dev.zeros();
    output_tensor.copy_from(&output_data);
    (input_tensor, output_tensor)
}

pub type Model<const N: usize> = (
    (Linear<N, 256>, ReLU),
    (Linear<256, 256>, ReLU),
    (Linear<256, 256>, ReLU),
    (Linear<256, 1>, Tanh),
);

pub fn gradient_descent_dfdx<const N: usize>(
    samples: &[TrainingSample<N>],
    dev: &mut Cpu,
    model: &mut Model<N>,
    learning_rate: f32,
) {
    let mut rng = StdRng::seed_from_u64(0);

    let mut sgd: Sgd<Model<N>> = Sgd::new(SgdConfig {
        lr: learning_rate,
        momentum: Some(Momentum::Nesterov(0.95)),
        weight_decay: None,
    });

    const BATCH_SIZE: usize = 1000;

    let first_start = Instant::now();
    for i_epoch in 0.. {
        let mut total_epoch_loss = 0.0;
        let mut num_batches = 0;
        let start = Instant::now();

        for (test_samples, test_samples_output) in
            SubsetIterator::<BATCH_SIZE>::shuffled(samples.len(), &mut rng)
                .map(|i| get_batch(samples, &dev, i))
        {
            let prediction = model.forward_mut(test_samples.trace());
            let loss = mse_loss(prediction, test_samples_output.clone());

            total_epoch_loss += loss.array();
            num_batches += 1;

            let gradients = loss.backward();
            sgd.update(model, gradients)
                .expect("Oops, there were some unused params");
        }
        let dur = Instant::now() - start;
        println!(
            "Epoch {} in {:.3}s ({:.1} batches/s): avg sample loss {:.5}",
            i_epoch + 1,
            dur.as_secs_f32(),
            num_batches as f32 / dur.as_secs_f32(),
            total_epoch_loss / num_batches as f32,
        );
        if i_epoch % 10 == 0 {
            model.save(format!("model_B{}_256_v{}.zip", BATCH_SIZE, i_epoch / 10)).unwrap();
        }
    }

    println!("Finished in {:.1}s", first_start.elapsed().as_secs_f32());

    model.save("model.model").unwrap();
}

pub fn gradient_descent<const N: usize>(
    samples: &[TrainingSample<N>],
    params: &[f32; N],
    initial_learning_rate: f32,
) -> [f32; N] {
    let start_time = Instant::now();
    let beta = 0.95;

    // If error is not reduced this number of times, reduce eta, or abort if eta is already low
    const MAX_TRIES: usize = 50;
    const ERROR_THRESHOLD: f32 = 1.000_000_5;

    let initial_error = average_error(samples, params);
    println!("Running gradient descent on {} positions", samples.len(),);
    trace!("Initial parameters: {:?}", params);
    println!("Initial error: {}", initial_error);

    let mut lowest_error = initial_error;
    let mut best_parameter_set = *params;

    'eta_loop: for eta in [
        initial_learning_rate,
        initial_learning_rate / 3.0,
        initial_learning_rate / 10.0,
        initial_learning_rate / 30.0,
    ]
    .iter()
    {
        trace!("\nTuning with eta = {}\n", eta);
        let mut parameter_set = best_parameter_set;
        let mut gradients = [0.0; N];

        let mut iterations_since_improvement = 0;
        let mut iterations_since_large_improvement = 0;
        loop {
            let slopes = calc_slope(samples, &parameter_set);
            trace!("Slopes: {:?}", slopes);
            gradients
                .iter_mut()
                .zip(slopes.iter())
                .for_each(|(gradient, slope)| *gradient = beta * *gradient + (1.0 - beta) * slope);
            trace!("Gradients: {:?}", gradients);

            parameter_set
                .iter_mut()
                .zip(gradients.iter())
                .for_each(|(param, gradient)| *param -= gradient * eta);
            trace!("New parameters: {:?}", parameter_set);

            let error = average_error(samples, &parameter_set);
            trace!("Error now {}, eta={}\n", error, eta);

            if error < lowest_error {
                iterations_since_improvement = 0;
                if lowest_error / error > ERROR_THRESHOLD {
                    iterations_since_large_improvement = 0;
                } else {
                    iterations_since_large_improvement += 1;
                    if iterations_since_large_improvement >= MAX_TRIES {
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
                    break;
                }
            }
        }
    }

    let elapsed = start_time.elapsed();

    println!(
        "Finished gradient descent in {:.1}s, error is {:.7}. Parameters:\n{:?}",
        elapsed.as_secs_f64(),
        lowest_error,
        best_parameter_set
            .iter()
            .map(|f| *f as f32)
            .collect::<Vec<f32>>()
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

                let mut gradients_for_this_training_sample = [0.0; N];
                gradients_for_this_training_sample
                    .iter_mut()
                    .zip(features)
                    .for_each(|(gradient, feature)| {
                        *gradient = (estimated_sigmoid - result) * derived_sigmoid_result * *feature
                    });
                gradients_for_this_training_sample
            },
        )
        // Sum each individual chunk as f32
        // Then sum those chunks as f64, to avoid rounding errors
        .chunks(256)
        .map(|chunks: Vec<[f32; N]>| {
            chunks.into_iter().fold([0.0; N], |mut a, b| {
                for (c, d) in a.iter_mut().zip(b.iter()) {
                    *c += *d;
                }
                a
            })
        })
        .fold(
            || [0.0; N],
            |mut a, b| {
                for (c, d) in a.iter_mut().zip(b.iter()) {
                    *c += *d as f64;
                }
                a
            },
        )
        .reduce(
            || [0.0; N],
            |mut a, b| {
                for (c, d) in a.iter_mut().zip(b.iter()) {
                    *c += *d;
                }
                a
            },
        );

    for slope in slopes.iter_mut() {
        *slope /= samples.len() as f64;
    }
    let mut f32_slopes = [0.0; N];
    for (f64_slope, slope) in f32_slopes.iter_mut().zip(&slopes) {
        *f64_slope = *slope as f32;
    }
    f32_slopes
}

/// Mean squared error of the parameter set, measured against given results and positions
fn average_error<const N: usize>(samples: &[TrainingSample<N>], params: &[f32; N]) -> f32 {
    samples
        .into_par_iter()
        .map(
            |TrainingSample {
                 features,
                 result,
                 offset,
             }| {
                (sigmoid(eval_from_params(features, params, *offset)) - result).powf(2.0)
            },
        )
        .map(|f| f as f64)
        .sum::<f64>() as f32
        / (samples.len() as f32)
}

pub fn eval_from_params<const N: usize>(
    features: &[f32; N],
    params: &[f32; N],
    offset: f32,
) -> f32 {
    features.iter().zip(params).map(|(c, p)| c * p).sum::<f32>() + offset
}

pub fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + f32::exp(-x as f32))
}

pub fn sigmoid_derived(x: f32) -> f32 {
    f32::exp(x) / f32::powi(1.0 + f32::exp(x), 2)
}

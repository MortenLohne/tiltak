use dfdx::{
    data::SubsetIterator,
    optim::{Momentum, Sgd, SgdConfig},
    prelude::*,
    tensor::Cpu,
};
use rand::{rngs::StdRng, SeedableRng};
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
    let mut output_data: Vec<f32> = Vec::with_capacity(B);
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

pub fn gradient_descent_dfdx<const B: usize, const N: usize, E: Dtype, M>(
    samples: &[TrainingSample<N>],
    dev: &mut Cpu,
    model: &mut M,
    learning_rate: f32,
) where
    M: GradientUpdate<Cpu, E>
        + ModuleMut<
            Tensor<(Const<B>, Const<N>), f32, Cpu, OwnedTape<Cpu>>,
            Output = Tensor<(Const<B>, Const<1>), f32, Cpu, OwnedTape<Cpu>>,
        > + GradientUpdate<Cpu, f32>
        + SaveToNpz,
{
    let mut rng = StdRng::seed_from_u64(0);

    let mut sgd: Sgd<M> = Sgd::new(SgdConfig {
        lr: learning_rate,
        momentum: Some(Momentum::Nesterov(0.95)),
        weight_decay: None,
    });

    let first_start = Instant::now();
    let mut best_epoch = 0;
    let mut best_epoch_loss = f32::MAX;
    let mut escaping = false;

    for i_epoch in 0.. {
        let mut total_epoch_loss = 0.0;
        let mut num_batches = 0;
        let start = Instant::now();

        for (test_samples, test_samples_output) in
            SubsetIterator::<B>::shuffled(samples.len(), &mut rng)
                .map(|i| get_batch(samples, dev, i))
        {
            let prediction: Tensor<(Const<B>, Const<1>), f32, Cpu, OwnedTape<Cpu>> =
                model.forward_mut(test_samples.trace());
            let loss = mse_loss(prediction, test_samples_output.clone());

            total_epoch_loss += loss.array();
            num_batches += 1;

            let gradients = loss.backward();
            sgd.update(model, gradients)
                .expect("Oops, there were some unused params");
        }
        let dur = Instant::now() - start;
        println!(
            "Epoch {} in {:.3}s ({:.1} batches/s): avg sample loss {:.7}",
            i_epoch + 1,
            dur.as_secs_f32(),
            num_batches as f32 / dur.as_secs_f32(),
            total_epoch_loss / num_batches as f32,
        );
        if i_epoch % 20 == 0 {}
        if total_epoch_loss < best_epoch_loss {
            best_epoch_loss = total_epoch_loss;
            best_epoch = i_epoch;
            if escaping {
                break;
            }
        }
        if best_epoch + 40 < i_epoch {
            break;
        }
        if best_epoch + 20 < i_epoch {
            escaping = true;
            println!("Escaping at the next opportunity")
        }
    }

    model
        .save(format!(
            "policy_model_2x2_B{}_16_lr{:05}.zip",
            B,
            (learning_rate * 10000.0) as i32
        ))
        .unwrap();
    println!("Finished in {:.1}s", first_start.elapsed().as_secs_f32());

    model.save("model.model").unwrap();
}

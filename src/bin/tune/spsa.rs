/// Tune search variable using a version of SPSA (Simultaneous perturbation stochastic approximation),
/// similar to [Stockfish's tuning method](https://www.chessprogramming.org/Stockfish%27s_Tuning_Method)
use crate::tune::play_match::play_game;
use board_game_traits::board::GameResult;
use rand::SeedableRng;
use rayon::prelude::*;
use std::sync::Mutex;
use taik::mcts::MctsSetting;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Variable {
    pub value: f32,
    pub delta: f32,
    pub apply_factor: f32,
}

/// In each iteration of SPSA, each variable can be increased, decreased or left unchanged.
#[derive(Clone, Copy, Debug, PartialEq)]
enum SPSADirection {
    Increase,
    Decrease,
    NoChange,
}

/// Tune the variables indefinitely
pub fn tune(variables: &mut [Variable]) {
    let mutex_variables = Mutex::new(variables);

    (1..u64::max_value()).into_par_iter().for_each(|i| {
        let cloned_variables = (*mutex_variables.lock().unwrap()).to_vec();
        let mut rng = rand::rngs::StdRng::from_entropy();

        let result = tuning_iteration(&cloned_variables, &mut rng);
        {
            let mut mut_variables = mutex_variables.lock().unwrap();
            for (variable, result) in (*mut_variables).iter_mut().zip(&result) {
                match result {
                    SPSADirection::Increase => {
                        variable.value += variable.value * variable.apply_factor
                    }
                    SPSADirection::Decrease => {
                        variable.value -= variable.value * variable.apply_factor
                    }
                    SPSADirection::NoChange => (),
                }
            }
        }

        if i % 100 == 0 {
            println!(
                "{}: Variables: {:?}",
                i,
                mutex_variables
                    .lock()
                    .unwrap()
                    .iter()
                    .map(|variable| variable.value)
                    .collect::<Vec<_>>()
            );
        }
    })
}

/// Run one iteration of the SPSA algorithm
fn tuning_iteration<R: rand::Rng>(variables: &[Variable], rng: &mut R) -> Vec<SPSADirection> {
    let (player1_variables, player2_variables): (
        Vec<(SPSADirection, f32)>,
        Vec<(SPSADirection, f32)>,
    ) = variables
        .iter()
        .map(|variable| {
            (
                (SPSADirection::Increase, variable.value + variable.delta),
                (SPSADirection::Decrease, variable.value - variable.delta),
            )
        })
        .map(|(a, b)| if rng.gen() { (a, b) } else { (b, a) })
        .unzip();

    let player1_settings =
        MctsSetting::with_search_params(player1_variables.iter().map(|(_, a)| *a).collect());
    let player2_settings =
        MctsSetting::with_search_params(player2_variables.iter().map(|(_, a)| *a).collect());

    let (game, _) = play_game(&player1_settings, &player2_settings);
    match game.game_result {
        Some(GameResult::WhiteWin) => player1_variables.iter().map(|(a, _)| *a).collect(),
        Some(GameResult::BlackWin) => player2_variables.iter().map(|(a, _)| *a).collect(),
        None | Some(GameResult::Draw) => vec![SPSADirection::NoChange; variables.len()],
    }
}
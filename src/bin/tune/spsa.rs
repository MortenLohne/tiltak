use crate::tune::play_match::play_game;
use board_game_traits::board::GameResult;
use rand::SeedableRng;
use rayon::prelude::*;
use std::sync::Mutex;
use taik::mcts::MctsSetting;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Variable {
    pub init: f32,
    pub delta: f32,
    pub apply_factor: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum SPSAGameResult {
    Increase,
    Decrease,
    NoChange,
}

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
                    SPSAGameResult::Increase => {
                        variable.init += variable.init * variable.apply_factor
                    }
                    SPSAGameResult::Decrease => {
                        variable.init -= variable.init * variable.apply_factor
                    }
                    SPSAGameResult::NoChange => (),
                }
            }
        }

        if i % 10 == 0 {
            println!(
                "Variables: {:?}",
                mutex_variables
                    .lock()
                    .unwrap()
                    .iter()
                    .map(|variable| variable.init)
                    .collect::<Vec<_>>()
            );
        }
    })
}

fn tuning_iteration<R: rand::Rng>(variables: &[Variable], rng: &mut R) -> Vec<SPSAGameResult> {
    let (player1_variables, player2_variables): (
        Vec<(SPSAGameResult, f32)>,
        Vec<(SPSAGameResult, f32)>,
    ) = variables
        .iter()
        .map(|variable| {
            (
                (SPSAGameResult::Increase, variable.init + variable.delta),
                (SPSAGameResult::Decrease, variable.init - variable.delta),
            )
        })
        .map(|(a, b)| if rng.gen() { (a, b) } else { (b, a) })
        .unzip();

    let player1_settings =
        MctsSetting::with_search_params(player1_variables.iter().map(|(_, a)| *a).collect());
    let player2_settings =
        MctsSetting::with_search_params(player2_variables.iter().map(|(_, a)| *a).collect());

    if rng.gen() {
        let (game, _) = play_game(&player1_settings, &player2_settings);
        match game.game_result {
            None => vec![SPSAGameResult::NoChange; variables.len()],
            Some(GameResult::WhiteWin) => player1_variables.iter().map(|(a, _)| *a).collect(),
            Some(GameResult::BlackWin) => player2_variables.iter().map(|(a, _)| *a).collect(),
            Some(GameResult::Draw) => vec![SPSAGameResult::NoChange; variables.len()],
        }
    } else {
        let (game, _) = play_game(&player2_settings, &player1_settings);
        match game.game_result {
            None => vec![SPSAGameResult::NoChange; variables.len()],
            Some(GameResult::WhiteWin) => player2_variables.iter().map(|(a, _)| *a).collect(),
            Some(GameResult::BlackWin) => player1_variables.iter().map(|(a, _)| *a).collect(),
            Some(GameResult::Draw) => vec![SPSAGameResult::NoChange; variables.len()],
        }
    }
}

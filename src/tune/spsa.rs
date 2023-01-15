use crate::position::Move;
use crate::search::{MctsSetting, TimeControl};
use crate::tune::openings::openings_from_file;
/// Tune search variable using a version of SPSA (Simultaneous perturbation stochastic approximation),
/// similar to [Stockfish's tuning method](https://www.chessprogramming.org/Stockfish%27s_Tuning_Method)
use crate::tune::play_match::play_game;
use board_game_traits::GameResult;
use rand::SeedableRng;
use rayon::prelude::*;
use std::sync::Mutex;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Variable {
    pub value: f32,
    pub delta: f32,
    pub apply_factor: f32,
}

/// In each iteration of SPSA, each variable can be increased, decreased or left unchanged.
#[derive(Clone, Copy, Debug, PartialEq)]
enum SpsaDirection {
    Increase,
    Decrease,
    NoChange,
}

/// Tune the variables indefinitely
pub fn tune<const S: usize, const N: usize, const M: usize>(
    variables: &mut [Variable],
    book_path: Option<&str>,
) {
    let openings = if let Some(path) = book_path {
        openings_from_file::<S>(path).unwrap()
    } else {
        vec![vec![]]
    };
    let mutex_variables = Mutex::new(variables);

    (1..usize::max_value()).into_par_iter().for_each(|i| {
        let cloned_variables = (*mutex_variables.lock().unwrap()).to_vec();
        let mut rng = rand::rngs::StdRng::from_entropy();

        let result = tuning_iteration::<_, S, N, M>(
            &cloned_variables,
            &mut rng,
            &openings[i % openings.len()],
        );
        {
            let mut mut_variables = mutex_variables.lock().unwrap();
            for (variable, result) in (*mut_variables).iter_mut().zip(&result) {
                match result {
                    SpsaDirection::Increase => {
                        variable.value += variable.delta * variable.apply_factor
                    }
                    SpsaDirection::Decrease => {
                        variable.value -= variable.delta * variable.apply_factor
                    }
                    SpsaDirection::NoChange => (),
                }
            }
        }

        if i % 29 == 0 {
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
fn tuning_iteration<R: rand::Rng, const S: usize, const N: usize, const M: usize>(
    variables: &[Variable],
    rng: &mut R,
    opening: &[Move],
) -> Vec<SpsaDirection> {
    #[allow(clippy::type_complexity)]
    let (player1_variables, player2_variables): (
        Vec<(SpsaDirection, f32)>,
        Vec<(SpsaDirection, f32)>,
    ) = variables
        .iter()
        .map(|variable| {
            (
                (SpsaDirection::Increase, variable.value + variable.delta),
                (SpsaDirection::Decrease, variable.value - variable.delta),
            )
        })
        .map(|(a, b)| if rng.gen() { (a, b) } else { (b, a) })
        .unzip();

    let player1_settings = <MctsSetting<S, N, M>>::default()
        .add_search_params(player1_variables.iter().map(|(_, a)| *a).collect());
    let player2_settings = <MctsSetting<S, N, M>>::default()
        .add_search_params(player2_variables.iter().map(|(_, a)| *a).collect());

    let (game, _) = play_game::<S, N, M>(
        &player1_settings,
        &player2_settings,
        opening,
        0.2,
        &TimeControl::Time(Duration::from_secs(20), Duration::from_millis(200)),
    );
    match game.game_result() {
        Some(GameResult::WhiteWin) => player1_variables.iter().map(|(a, _)| *a).collect(),
        Some(GameResult::BlackWin) => player2_variables.iter().map(|(a, _)| *a).collect(),
        None | Some(GameResult::Draw) => vec![SpsaDirection::NoChange; variables.len()],
    }
}

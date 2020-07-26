use crate::tune::play_match::play_game;
use board_game_traits::board::{Board as EvalBoard, GameResult};
use rand::prelude::SliceRandom;
use taik::board::{Board, Move};
use taik::mcts::MctsSetting;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Variable {
    init: f32,
    delta: f32,
    apply_factor: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum SPSAGameResult {
    Increase,
    Decrease,
    NoChange,
}

fn tuning_iteration<R: rand::Rng>(variables: &[Variable], rng: &mut R) -> SPSAGameResult {
    let (player1_variables, player2_variables): (Vec<f32>, Vec<f32>) = variables
        .iter()
        .map(|variable| {
            (
                variable.init + variable.delta,
                variable.init - variable.delta,
            )
        })
        .unzip();

    let player1_settings = MctsSetting::with_search_params(player1_variables);
    let player2_settings = MctsSetting::with_search_params(player2_variables);

    if rng.gen() {
        let (game, _) = play_game(&player1_settings, &player2_settings);
        match game.game_result {
            None => SPSAGameResult::NoChange,
            Some(GameResult::WhiteWin) => SPSAGameResult::Increase,
            Some(GameResult::BlackWin) => SPSAGameResult::Decrease,
            Some(GameResult::Draw) => SPSAGameResult::NoChange,
        }
    } else {
        let (game, _) = play_game(&player2_settings, &player1_settings);
        match game.game_result {
            None => SPSAGameResult::NoChange,
            Some(GameResult::WhiteWin) => SPSAGameResult::Decrease,
            Some(GameResult::BlackWin) => SPSAGameResult::Increase,
            Some(GameResult::Draw) => SPSAGameResult::NoChange,
        }
    }
}

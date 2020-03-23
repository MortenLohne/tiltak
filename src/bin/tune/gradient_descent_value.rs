use crate::tune::gradient_descent;
use board_game_traits::board::Board as BoardTrait;
use board_game_traits::board::GameResult;
use pgn_traits::pgn::PgnBoard;
use std::fmt::Debug;
use taik::board::TunableBoard;
use taik::mcts;

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
    gradient_descent::gradient_descent(
        positions,
        results,
        test_positions,
        test_results,
        params,
        &[5.0, 0.5, 0.05],
        |a, b, c| error(a, *b, c),
    )
}
/// Squared error of a single centipawn evaluation
fn error<B: TunableBoard>(board: &B, game_result: GameResult, params: &[f32]) -> f32 {
    let answer = match game_result {
        GameResult::WhiteWin => 1.0,
        GameResult::Draw => 0.5,
        GameResult::BlackWin => 0.0,
    };
    let eval = board.static_eval_with_params(params);

    f32::powf(answer - sigmoid(eval), 2.0)
}

fn sigmoid(eval: f32) -> f32 {
    mcts::cp_to_win_percentage(eval)
}

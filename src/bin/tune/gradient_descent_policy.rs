use crate::tune::gradient_descent;
use board_game_traits::board::Board as BoardTrait;
use pgn_traits::pgn::PgnBoard;
use std::fmt::Debug;
use taik::board::TunableBoard;
use taik::mcts::Score;

pub fn gradient_descent_policy<B>(
    positions: &[B],
    move_scores: &[Vec<(<B as BoardTrait>::Move, Score)>],
    test_positions: &[B],
    test_move_scores: &[Vec<(<B as BoardTrait>::Move, Score)>],
    params: &[f32],
) -> Vec<f32>
where
    B: TunableBoard + BoardTrait + PgnBoard + Send + Debug + Sync + Clone,
    <B as BoardTrait>::Move: Send + Sync,
{
    gradient_descent::gradient_descent(
        positions,
        move_scores,
        test_positions,
        test_move_scores,
        params,
        &[10000.0, 1000.0, 100.0],
        |a, b, c| error(a, b, c),
    )
}
/// MSE of a single move generation
fn error<B: TunableBoard + Debug>(
    board: &B,
    mcts_move_score: &[(B::Move, f32)],
    params: &[f32],
) -> f32 {
    let static_probs: Vec<f32> = mcts_move_score
        .iter()
        .map(|(mv, _)| board.probability_for_move(params, mv, mcts_move_score.len()))
        .collect();

    mcts_move_score
        .iter()
        .zip(static_probs)
        .map(|((_move, mcts_score), static_prob)| {
            let error = f32::powf(static_prob - *mcts_score, 2.0);
            assert!(
                error >= 0.0 && error <= 1.0,
                "Error was {} for static prob {}, mcts score {} for move {:?} on board\n{:?}",
                error,
                static_prob,
                mcts_score,
                _move,
                board
            );
            error
        })
        .sum::<f32>()
        / mcts_move_score.len() as f32
}

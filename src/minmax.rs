//! A very simple implementation of the minmax search algorithm.
//! This is not used in the core engine at all, it is just here for fun/testing.

use arrayvec::ArrayVec;
use board_game_traits::{Color, GameResult};
use board_game_traits::{EvalPosition, Position as _};

use crate::position::{ExpMove, Move, Position, Role};

/// A very simple implementation of the minmax search algorithm. Returns the best move and a centipawn evaluation, calculating up to `depth` plies deep.
pub fn minmax<B: EvalPosition>(position: &mut B, depth: u16) -> (Option<B::Move>, f32) {
    match position.game_result() {
        Some(GameResult::WhiteWin) => return (None, 100.0),
        Some(GameResult::BlackWin) => return (None, -100.0),
        Some(GameResult::Draw) => return (None, 0.0),
        None => (),
    }
    if depth == 0 {
        (None, position.static_eval())
    } else {
        let side_to_move = position.side_to_move();
        let mut moves = vec![];
        position.generate_moves(&mut moves);
        let child_evaluations = moves.into_iter().map(|mv| {
            let reverse_move = position.do_move(mv.clone());
            let (_, eval) = minmax(position, depth - 1);
            position.reverse_move(reverse_move);
            (Some(mv), eval)
        });
        match side_to_move {
            Color::White => child_evaluations
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .unwrap(),
            Color::Black => child_evaluations
                .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .unwrap(),
        }
    }
}

pub fn minmax_noeval<const S: usize>(
    position: &mut Position<S>,
    depth: u16,
) -> (ArrayVec<Move<S>, 16>, Option<GameResult>) {
    let group_data = position.group_data();
    match position.game_result_with_group_data(&group_data) {
        Some(result) => return (ArrayVec::new(), Some(result)),
        None => (),
    }
    if depth == 0 {
        return (ArrayVec::new(), None);
    }
    let side_to_move = position.side_to_move();
    let mut moves = vec![];
    position.generate_moves(&mut moves);
    let mut child_results: Vec<(ArrayVec<Move<S>, 16>, Option<GameResult>)> =
        Vec::with_capacity(moves.len());
    moves.sort_by_cached_key(|mv| match (mv.expand(), position.fcd_for_move(*mv)) {
        (ExpMove::Place(Role::Flat, sq), _)
            if group_data.is_critical_square(sq, position.side_to_move()) =>
        {
            -2
        }
        (ExpMove::Place(Role::Flat, _), _) => 0,
        (ExpMove::Move(_, _, _), fcd) => 3 - (fcd * 2),
        (ExpMove::Place(Role::Wall, _), 0) => 2,
        (ExpMove::Place(_, _), _) => unreachable!(),
    });
    for mv in moves {
        let reverse_move = position.do_move(mv.clone());
        let (mut child_moves, result) = minmax_noeval(position, depth - 1);
        position.reverse_move(reverse_move);
        child_moves.push(mv);
        child_results.push((child_moves, result));
        if side_to_move == Color::White && result == Some(GameResult::WhiteWin)
            || side_to_move == Color::Black && result == Some(GameResult::BlackWin)
        {
            break;
        }
    }

    match side_to_move {
        Color::White => child_results
            .iter()
            .rev()
            .max_by_key(|(moves, result)| match result {
                Some(GameResult::WhiteWin) => 300,
                None => 200,
                Some(GameResult::Draw) => 100,
                Some(GameResult::BlackWin) => 0 + moves.len(),
            })
            .unwrap()
            .clone(),
        Color::Black => child_results
            .iter()
            .rev()
            .max_by_key(|(moves, result)| match result {
                Some(GameResult::BlackWin) => 300,
                None => 200,
                Some(GameResult::Draw) => 100,
                Some(GameResult::WhiteWin) => 0 + moves.len(),
            })
            .unwrap()
            .clone(),
    }
}

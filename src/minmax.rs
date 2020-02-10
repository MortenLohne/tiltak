use board_game_traits::board::EvalBoard;
use board_game_traits::board::{Board, Color, GameResult};

pub fn minmax<B: EvalBoard>(board: &mut B, depth: u16) -> (Option<B::Move>, f32) {
    match board.game_result() {
        Some(GameResult::WhiteWin) => return (None, 100.0),
        Some(GameResult::BlackWin) => return (None, -100.0),
        Some(GameResult::Draw) => return (None, 0.0),
        None => (),
    }
    if depth == 0 {
        (None, board.static_eval())
    } else {
        let side_to_move = board.side_to_move();
        let mut moves = vec![];
        board.generate_moves(&mut moves);
        let child_evaluations = moves.into_iter().map(|mv| {
            let reverse_move = board.do_move(mv.clone());
            let (_, eval) = minmax(board, depth - 1);
            board.reverse_move(reverse_move);
            (Some(mv), eval)
        }); //.collect::<Vec<(Option<B::Move>, f32)>>();
        match side_to_move {
            Color::White => child_evaluations
                .max_by(|(_, a), (_, b)| a.partial_cmp(&b).unwrap())
                .unwrap(),
            Color::Black => child_evaluations
                .min_by(|(_, a), (_, b)| a.partial_cmp(&b).unwrap())
                .unwrap(),
        }
    }
}

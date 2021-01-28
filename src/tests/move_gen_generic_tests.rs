use crate::board::Board;
use board_game_traits::board::Board as EvalBoard;

#[test]
fn start_board_move_gen_test() {
    start_board_move_gen_prop::<4>();
    start_board_move_gen_prop::<5>();
    start_board_move_gen_prop::<6>();
    start_board_move_gen_prop::<7>();
    start_board_move_gen_prop::<8>();
}

pub fn perft<const S: usize>(board: &mut Board<S>, depth: u16) -> u64 {
    if depth == 0 || board.game_result().is_some() {
        1
    } else {
        let mut moves = vec![];
        board.generate_moves(&mut moves);
        moves
            .into_iter()
            .map(|mv| {
                let old_board = board.clone();
                let reverse_move = board.do_move(mv.clone());
                let num_moves = perft(board, depth - 1);
                board.reverse_move(reverse_move);
                debug_assert_eq!(
                    *board, old_board,
                    "Failed to restore old board after {:?} on\n{:?}",
                    mv, old_board
                );
                num_moves
            })
            .sum()
    }
}

/// Verifies the perft result of a position against a known answer
pub fn perft_check_answers<const S: usize>(board: &mut Board<S>, answers: &[u64]) {
    for (depth, &answer) in answers.iter().enumerate() {
        assert_eq!(
            perft(board, depth as u16),
            answer,
            "Wrong perft result on\n{:?}",
            board
        );
        assert_eq!(
            perft(&mut board.flip_board_x(), depth as u16),
            answer,
            "Wrong perft result on\n{:?}",
            board
        );
        assert_eq!(
            perft(&mut board.flip_board_y(), depth as u16),
            answer,
            "Wrong perft result on\n{:?}",
            board
        );
        assert_eq!(
            perft(&mut board.flip_colors(), depth as u16),
            answer,
            "Wrong perft result on\n{:?}",
            board
        );
        assert_eq!(
            perft(&mut board.rotate_board(), depth as u16),
            answer,
            "Wrong perft result on\n{:?}",
            board
        );
        assert_eq!(
            perft(&mut board.rotate_board().rotate_board(), depth as u16),
            answer,
            "Wrong perft result on\n{:?}",
            board
        );
        assert_eq!(
            perft(
                &mut board.rotate_board().rotate_board().rotate_board(),
                depth as u16
            ),
            answer,
            "Wrong perft result on\n{:?}",
            board
        );
    }
}

fn start_board_move_gen_prop<const S: usize>() {
    let mut board = <Board<S>>::default();
    let mut moves = vec![];
    board.generate_moves(&mut moves);
    assert_eq!(moves.len(), S * S);
    for mv in moves {
        let reverse_move = board.do_move(mv);
        let mut moves = vec![];
        board.generate_moves(&mut moves);
        assert_eq!(moves.len(), S * S - 1);
        board.reverse_move(reverse_move);
    }
}

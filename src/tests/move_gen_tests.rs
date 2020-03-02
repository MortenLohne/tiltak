use crate::board::Board;
use crate::board::Move;
use crate::tests::do_moves_and_check_validity;
use board_game_traits::board::Board as BoardTrait;
use pgn_traits::pgn::PgnBoard;

#[test]
fn start_board_move_gen_test() {
    let mut board = Board::default();
    let mut moves = vec![];
    board.generate_moves(&mut moves);
    assert_eq!(moves.len(), 75);
    for mv in moves {
        let reverse_move = board.do_move(mv);
        let mut moves = vec![];
        board.generate_moves(&mut moves);
        assert_eq!(moves.len(), 72);
        board.reverse_move(reverse_move);
    }
}

#[test]
fn move_stack_test() {
    let mut board = Board::default();
    let mut moves = vec![];

    do_moves_and_check_validity(&mut board, &["c3", "d3", "c4", "1d3<", "1c4-", "Sc4"]);

    board.generate_moves(&mut moves);
    assert_eq!(
        moves.len(),
        69 + 18,
        "Generated wrong moves on board:\n{:?}\nExpected moves: {:?}\nExpected move moves:{:?}",
        board,
        moves,
        moves
            .iter()
            .filter(|mv| match mv {
                Move::Move(_, _, _) => true,
                _ => false,
            })
            .collect::<Vec<_>>()
    );
}

#[test]
fn respect_carry_limit_test() {
    let mut board = Board::default();
    let mut moves = vec![];

    do_moves_and_check_validity(
        &mut board,
        &[
            "c3", "c2", "d3", "b3", "c4", "1c2+", "1d3<", "1b3>", "1c4+", "Cc2", "a1", "1c2+", "a2",
        ],
    );
    board.generate_moves(&mut moves);
    assert!(
        moves.contains(&board.move_from_san("5c3>").unwrap()),
        "5c3> was not a legal move among {:?} on board\n{:?}",
        moves,
        board
    );

    assert!(
        !moves.contains(&board.move_from_san("6c3>").unwrap()),
        "6c3> was a legal move among {:?} on board\n{:?}",
        moves,
        board
    );
}

#[test]
fn start_pos_perf_test() {
    let mut board = Board::default();
    perft_check_answers(&mut board, &[1, 75, 5400, 348_080, 21_536_636]);
}

#[test]
fn perf_test2() {
    let mut board = Board::default();

    do_moves_and_check_validity(&mut board, &["c3", "d3", "c4", "1d3<", "1c4-", "Sc4"]);

    perft_check_answers(&mut board, &[1, 87, 6155, 461_800]);
}

#[test]
fn perf_test3() {
    let mut board = Board::default();

    do_moves_and_check_validity(
        &mut board,
        &[
            "c3", "c2", "d3", "b3", "c4", "1c2+", "1d3<", "1b3>", "1c4-", "Cc2", "a1", "1c2+", "a2",
        ],
    );

    perft_check_answers(&mut board, &[1, 104, 7743, 592_645]);
}

#[test]
fn suicide_perf_test() {
    let move_strings = [
        "c2", "c4", "d2", "c3", "b2", "d3", "1d2+", "b3", "d2", "b4", "1c2+", "1b3>", "2d3<",
        "1c4-", "d4", "5c3<3", "c2", "c4", "1d4<", "d3", "1d2+", "1c3+", "Cc3", "2c4>", "1c3<",
        "d2", "c3", "1d2+", "1c3+", "1b4>", "2b3>1", "3c4-2", "d2", "c4", "b4", "c5", "1b3>",
        "1c4<", "3c3-", "e5", "e2",
    ];

    let mut board = Board::default();
    do_moves_and_check_validity(&mut board, &move_strings);
    perft_check_answers(&mut board, &[1, 83, 11_204]);
    perft_check_answers(&mut board, &[1, 83, 11_204, 942_217]);
}

pub fn perft(board: &mut Board, depth: u16) -> u64 {
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

#[cfg(test)]
/// Verifies the perft result of a position against a known answer
pub fn perft_check_answers(board: &mut Board, answers: &[u64]) {
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

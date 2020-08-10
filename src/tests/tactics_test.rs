use crate::board::Board;
use crate::mcts;
use crate::tests::do_moves_and_check_validity;
use board_game_traits::board::Board as BoardTrait;
use pgn_traits::pgn::PgnBoard;

#[test]
fn avoid_loss_in_two() {
    let move_strings = [
        "b5", "e2", "Cc3", "b3", "b2", "Cc2", "b4", "c4", "d3", "c5", "e3",
    ];

    let mut board = Board::start_board();

    do_moves_and_check_validity(&mut board, &move_strings);

    plays_correct_hard_move_property(&move_strings, &["a3", "d2", "d4", "a2", "c2<"]);
}

#[test]
// c3< wins if not stopped
fn avoid_loss_in_three2() {
    let move_strings = [
        "b5", "e3", "Cc3", "Cb3", "b2", "b4", "b1", "c2", "d3", "c4", "d2", "c1", "c3-", "b3-",
        "b3", "c3",
    ];

    let mut board = Board::start_board();

    do_moves_and_check_validity(&mut board, &move_strings);

    plays_correct_hard_move_property(&move_strings, &["a3", "2c2+"]);
}

#[test]
fn find_win_in_two() {
    let move_strings = [
        "a5", "e4", "Cc3", "c4", "b3", "Cd3", "b4", "b5", "d4", "d5", "a4", "c4>", "e4<", "d3+",
        "e3", "d3", "d2", "4d4<22", "a3", "3b4-", "c5", "2c4+", "a4+", "b2", "b4", "c4", "b1",
        "c2", "c1", "d1", "d4", "a2", "a4", "e2", "d2<", "c4<", "a4>", "d2", "c4", "b2>", "c1+",
        "b5-", "4c2>22", "4b4>22", "c3+", "d3-", "3c4>", "3d2>", "4d4-22", "5e2+122", "d4>",
        "2e3+", "d4>", "2e5-", "d4", "3e4<", "e3", "c2", "a4", "e1", "e3+", "4d4>", "a1", "a2+",
        "a2",
    ];

    let mut board = Board::start_board();

    do_moves_and_check_validity(&mut board, &move_strings);

    plays_correct_hard_move_property(&move_strings, &["5e4+"]);
}

#[test]
fn tactic_test1() {
    let move_strings = [
        "b4", "e1", "Cc3", "Cc4", "d4", "b3", "b2", "d3", "c2", "a3", "c3>", "e4", "c3",
    ];

    let mut board = Board::start_board();

    do_moves_and_check_validity(&mut board, &move_strings);

    plays_correct_hard_move_property(&move_strings, &["d5"]);
}

#[test]
fn simple_move_move_to_win() {
    // a5 e2 Cc3 a4 b3 a3 a2 b2 e3 b2< a1 Cb2 b1 b2< Se1 a2-
    let move_strings = [
        "a5", "e2", "Cc3", "a4", "b3", "a3", "a2", "b2", "e3", "b2<", "a1", "Cb2", "b1",
    ];

    let mut board = Board::start_board();

    do_moves_and_check_validity(&mut board, &move_strings);

    plays_correct_hard_move_property(&move_strings, &["b2<"]);
}

#[cfg(test)]
fn plays_correct_hard_move_property(move_strings: &[&str], correct_moves: &[&str]) {
    let mut board = Board::default();
    let mut moves = vec![];

    do_moves_and_check_validity(&mut board, move_strings);

    board.generate_moves(&mut moves);

    for move_string in correct_moves {
        assert_eq!(
            *move_string,
            board.move_to_san(&board.move_from_san(move_string).unwrap())
        );
        assert!(
            moves.contains(&board.move_from_san(move_string).unwrap()),
            "Candidate move {} was not among legal moves {:?} on board\n{:?}",
            move_string,
            moves,
            board
        );
    }
    let (best_move, score) = mcts::mcts(board.clone(), 50_000);

    assert!(
        correct_moves
            .iter()
            .any(|move_string| move_string == &board.move_to_san(&best_move)),
        "{} didn't play one of the correct moves {:?}, {} {:.1}% played instead on board:\n{:?}",
        board.side_to_move(),
        correct_moves,
        board.move_to_san(&best_move),
        score * 100.0,
        board
    );
}

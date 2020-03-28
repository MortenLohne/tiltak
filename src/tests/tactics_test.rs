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
    let (best_move, _score) = mcts::mcts(board.clone(), 100_000);

    assert!(correct_moves
        .iter()
        .any(|move_string| move_string == &board.move_to_san(&best_move)));
}

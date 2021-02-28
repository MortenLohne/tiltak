#[cfg(test)]
mod board_generic_tests;
#[cfg(test)]
mod board_tests;
#[cfg(test)]
mod mcts_tests;
#[cfg(test)]
mod move_gen_5s_tests;
#[cfg(test)]
mod move_gen_generic_tests;
#[cfg(test)]
mod tactics_tests_5s;
#[cfg(test)]
mod tactics_tests_6s;

#[cfg(test)]
use crate::board::Board;
#[cfg(test)]
use crate::search;
#[cfg(test)]
use board_game_traits::Position as PositionTrait;
#[cfg(test)]
use pgn_traits::PgnPosition;

#[cfg(test)]
fn do_moves_and_check_validity<const S: usize>(board: &mut Board<S>, move_strings: &[&str]) {
    let mut moves = vec![];
    for mv_san in move_strings.iter() {
        let mv = board.move_from_san(&mv_san).unwrap();
        board.generate_moves(&mut moves);
        assert!(
            moves.contains(&mv),
            "Move {} was not among legal moves: {:?}\n{:?}",
            board.move_to_san(&mv),
            moves,
            board
        );
        board.do_move(mv);
        moves.clear();
    }
}

#[cfg(test)]
fn plays_correct_hard_move_property<const S: usize>(move_strings: &[&str], correct_moves: &[&str]) {
    let mut board = <Board<S>>::default();
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
    let (best_move, score) = search::mcts(board.clone(), 50_000);

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

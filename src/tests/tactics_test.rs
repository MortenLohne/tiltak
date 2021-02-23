use crate::board::Board;
use crate::search;
use crate::tests::do_moves_and_check_validity;
use board_game_traits::Position as PositionTrait;
use pgn_traits::PgnPosition;

#[test]
fn avoid_loss_in_two() {
    let move_strings = [
        "b5", "e2", "Cc3", "b3", "b2", "Cc2", "b4", "c4", "d3", "c5", "e3",
    ];

    let mut board = <Board<5>>::start_position();

    do_moves_and_check_validity(&mut board, &move_strings);

    plays_correct_hard_move_property::<5>(&move_strings, &["a3", "d2", "d4", "a2", "c2<"]);
}

#[test]
// c3< wins if not stopped
fn avoid_loss_in_three2() {
    let move_strings = [
        "b5", "e3", "Cc3", "Cb3", "b2", "b4", "b1", "c2", "d3", "c4", "d2", "c1", "c3-", "b3-",
        "b3", "c3",
    ];

    let mut board = <Board<5>>::start_position();

    do_moves_and_check_validity(&mut board, &move_strings);

    plays_correct_hard_move_property::<5>(&move_strings, &["a3", "2c2+"]);
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

    let mut board = <Board<5>>::start_position();

    do_moves_and_check_validity(&mut board, &move_strings);

    plays_correct_hard_move_property::<5>(&move_strings, &["5e4+"]);
}

#[test]
fn find_win_in_two2() {
    // a1 a5 b5 Cc3 c5 d5 Cd4 c4 e5 1c4+1 1d4+1 c4 1d5<1 1d5>1 d5 e4 2c5>11 1d5<1 2e5<11 2d5>2
    let move_strings = [
        "a1", "a5", "b5", "Cc3", "c5", "d5", "Cd4", "c4", "e5", "1c4+1", "1d4+1", "c4", "1d5<1",
        "1d5>1", "d5", "e4", "2c5>11", "1d5<1", "2e5<11", "2d5>2",
    ];

    let mut board = <Board<5>>::start_position();

    do_moves_and_check_validity(&mut board, &move_strings);

    plays_correct_hard_move_property::<5>(&move_strings, &["2c5>11"]);
}

#[test]
fn find_win_in_two3() {
    // a5 e5 e4 Cc3 e3 e2 Cd3 d2 e1 c4 1e1+1 e1 1d3-1 Sd1
    let move_strings = [
        "a5", "e5", "e4", "Cc3", "e3", "e2", "Cd3", "d2", "e1", "c4", "1e1+1", "e1", "1d3-1", "Sd1",
    ];

    let mut board = <Board<5>>::start_position();

    do_moves_and_check_validity(&mut board, &move_strings);

    plays_correct_hard_move_property::<5>(&move_strings, &["d2>"]);
}

#[test]
fn find_capstone_spread_win_in_two() {
    // b4 a5 e5 b5 b3 Cc3 Cc5 d5 d4 d3 b3+ a4 2b4+ a4+ a4 b4 d4+ b4< b4 a3 b3 a2 3b5< 2a4+ Sa4 b2 e3 e2 a4+ d2 5a5-122 3a5-21 3a2+ c2 5a3- c3< 5a2>113 2a4- a5 Sb5 d4 c3 e3< c3> 4d2< e3 Sc3 3d3+12 c5> e4 5d5> c4 5e5-212 2d4> e3+ e1
    let move_strings = [
        "b4", "a5", "e5", "b5", "b3", "Cc3", "Cc5", "d5", "d4", "d3", "b3+", "a4", "2b4+", "a4+",
        "a4", "b4", "d4+", "b4<", "b4", "a3", "b3", "a2", "3b5<", "2a4+", "Sa4", "b2", "e3", "e2",
        "a4+", "d2", "5a5-122", "3a5-21", "3a2+", "c2", "5a3-", "c3<", "5a2>113", "2a4-", "a5",
        "Sb5", "d4", "c3", "e3<", "c3>", "4d2<", "e3", "Sc3", "3d3+12", "c5>", "e4", "5d5>", "c4",
        "5e5-212", "2d4>", "e3+", "e1",
    ];

    let mut board = <Board<5>>::start_position();

    do_moves_and_check_validity(&mut board, &move_strings);

    plays_correct_hard_move_property::<5>(&move_strings, &["2e2+11"]);
}

#[test]
fn capture_stack_in_strong_file() {
    // b5 a5 e1 b3 Cc3 b4 b2 c5 a4 d5 c4 e5 a3 b3< a5>
    let move_strings = [
        "b5", "a5", "e1", "b3", "Cc3", "b4", "b2", "c5", "a4", "d5", "c4", "e5", "a3", "b3<", "a5>",
    ];

    let mut board = <Board<5>>::start_position();

    do_moves_and_check_validity(&mut board, &move_strings);

    plays_correct_hard_move_property::<5>(&move_strings, &["b4+"]);
}

#[test]
fn spread_stack_for_tinue() {
    // c3 a5 e1 b3 Cc2 d4 a4 a3 b4 d3 c2+ d2 c4 d5 2c3> Cc3 c2 e4 b2 c5 3d3+ d3 4d4- d4 5d3+ d3 b4- c3< e3 c3 e3< b4 b5 e3 2d3> d3 3e3< d2+ Se3 d2 e3< e3 2d3< e3< 3c3> c3 a2 2b3- a4> b3+ c4< b3 4d3< b3+ b5- d2+ 5c3> 3b2+ c1 a3- d1 3b3>21
    let move_strings = [
        "c3", "a5", "e1", "b3", "Cc2", "d4", "a4", "a3", "b4", "d3", "c2+", "d2", "c4", "d5",
        "2c3>", "Cc3", "c2", "e4", "b2", "c5", "3d3+", "d3", "4d4-", "d4", "5d3+", "d3", "b4-",
        "c3<", "e3", "c3", "e3<", "b4", "b5", "e3", "2d3>", "d3", "3e3<", "d2+", "Se3", "d2",
        "e3<", "e3", "2d3<", "e3<", "3c3>", "c3", "a2", "2b3-", "a4>", "b3+", "c4<", "b3", "4d3<",
        "b3+", "b5-", "d2+", "5c3>", "3b2+", "c1", "a3-", "d1", "3b3>21",
    ];

    let mut board = <Board<5>>::start_position();

    do_moves_and_check_validity(&mut board, &move_strings);

    plays_correct_hard_move_property::<5>(&move_strings, &["4b4-211"]);
}

#[test]
fn find_win_in_three() {
    // e1 e5 Cc3 c1 d1 d2 a3 b1 b3 d2- a1 a2 a1> Cb2 Sc2 a1 2b1> b2+ b5 b1 c4 d2 c5
    let move_strings = [
        "e1", "e5", "Cc3", "c1", "d1", "d2", "a3", "b1", "b3", "d2-", "a1", "a2", "a1>", "Cb2",
        "Sc2", "a1", "2b1>", "b2+", "b5", "b1", "c4", "d2", "c5",
    ];

    let mut board = <Board<5>>::start_position();

    do_moves_and_check_validity(&mut board, &move_strings);

    plays_correct_hard_move_property::<5>(&move_strings, &["2b3-11"]);
}

#[test]
fn find_win_in_three2() {
    // c4 a5 e1 c3 d1 c2 c1 b1 Cb2 c5 b2- a1 a2 c2- c2 2c1> d2 Cb2 c1 b2> d2- 2c2- c2 3c1> b2 d3 Sd2 c1 a3 a1+ a3-
    let move_strings = [
        "c4", "a5", "e1", "c3", "d1", "c2", "c1", "b1", "Cb2", "c5", "b2-", "a1", "a2", "c2-",
        "c2", "2c1>", "d2", "Cb2", "c1", "b2>", "d2-", "2c2-", "c2", "3c1>", "b2", "d3", "Sd2",
        "c1", "a3", "a1+", "a3-",
    ];

    let mut board = <Board<5>>::start_position();

    do_moves_and_check_validity(&mut board, &move_strings);

    plays_correct_hard_move_property::<5>(&move_strings, &["d1<"]);
}

#[test]
fn tactic_test1() {
    let move_strings = [
        "b4", "e1", "Cc3", "Cc4", "d4", "b3", "b2", "d3", "c2", "a3", "c3>", "e4", "c3",
    ];

    let mut board = <Board<5>>::start_position();

    do_moves_and_check_validity(&mut board, &move_strings);

    plays_correct_hard_move_property::<5>(&move_strings, &["d5"]);
}

#[test]
fn simple_move_move_to_win() {
    // a5 e2 Cc3 a4 b3 a3 a2 b2 e3 b2< a1 Cb2 b1 b2< Se1 a2-
    let move_strings = [
        "a5", "e2", "Cc3", "a4", "b3", "a3", "a2", "b2", "e3", "b2<", "a1", "Cb2", "b1",
    ];

    let mut board = <Board<5>>::start_position();

    do_moves_and_check_validity(&mut board, &move_strings);

    plays_correct_hard_move_property::<5>(&move_strings, &["b2<"]);
}

#[test]
fn flatten_our_stone_to_win() {
    // c4 c5 Cc3 Cd3 c2 b4 d4 d3+ d3 b3 c1 b2 b1 b5 a1 e3 c3+ Sc3 d1 Se1 e2 Sd2 a2 a3 a4 2d4- a5 d3< d1< 2c3-11
    let move_strings = [
        "c4", "c5", "Cc3", "Cd3", "c2", "b4", "d4", "d3+", "d3", "b3", "c1", "b2", "b1", "b5",
        "a1", "e3", "c3+", "Sc3", "d1", "Se1", "e2", "Sd2", "a2", "a3", "a4", "2d4-", "a5",
    ];

    let mut board = <Board<5>>::start_position();

    do_moves_and_check_validity(&mut board, &move_strings);

    plays_correct_hard_move_property::<5>(&move_strings, &["d3<"]);
}

#[test]
fn winning_movement_test() {
    // e1 e5 Cc3 d1 c1 Cc2 d2 b1 c1> a1 c1 d3 b2 e1< c1> b3 d4 e2 b4 d3- 4d1<22 c2- Sd1 c2 d1+ 2c1< c4
    let move_strings = [
        "e1", "e5", "Cc3", "d1", "c1", "Cc2", "d2", "b1", "c1>", "a1", "c1", "d3", "b2", "e1<",
        "c1>", "b3", "d4", "e2", "b4", "d3-", "4d1<22", "c2-", "Sd1", "c2", "d1+", "2c1<", "c4",
    ];

    let mut board = <Board<5>>::start_position();

    do_moves_and_check_validity(&mut board, &move_strings);

    plays_correct_hard_move_property::<5>(&move_strings, &["4b1>13"]);
}

#[test]
fn winning_movement_test2() {
    let move_strings = [
        "a1", "a5", "b5", "Cc3", "c5", "d5", "Cd4", "c4", "e5", "c4+", "c4", "b4", "c4+", "d5<",
        "d5", "c4", "d4<", "4c5<22", "c5", "b3", "2c4+", "3b5-", "2c5<", "a4",
    ];

    let mut board = <Board<5>>::start_position();

    do_moves_and_check_validity(&mut board, &move_strings);

    plays_correct_hard_move_property::<5>(&move_strings, &["b5<"]);
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

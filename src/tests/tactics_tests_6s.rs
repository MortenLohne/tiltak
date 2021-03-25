use crate::position::Board;
use crate::tests::{do_moves_and_check_validity, plays_correct_hard_move_property};
use board_game_traits::Position as PositionTrait;

#[test]
fn tinue_5ply_test() {
    // a6 f1 d3 b6 c3 c6 b3 d6 Se6 d5 e5 d4 Ce4 e3 f3 Cc4 f6 f2 f5 1c4-1 e2 d2 e1 1e3-1 1e1+1 1d2>1 Sd2 c4 1d2>1 c1 Sc2 f4 4e2>4 Se3 e1 d2 1e4>1 1e3-1
    let move_strings = [
        "a6", "f1", "d3", "b6", "c3", "c6", "b3", "d6", "Se6", "d5", "e5", "d4", "Ce4", "e3", "f3",
        "Cc4", "f6", "f2", "f5", "1c4-1", "e2", "d2", "e1", "1e3-1", "1e1+1", "1d2>1", "Sd2", "c4",
        "1d2>1", "c1", "Sc2", "f4", "4e2>4", "Se3", "e1", "d2", "1e4>1", "1e3-1",
    ];

    let mut board = <Board<6>>::start_position();

    do_moves_and_check_validity(&mut board, &move_strings);

    plays_correct_hard_move_property::<6>(&move_strings, &["2f4-11"]);
}

#[test]
fn tinue_7ply_test() {
    // a1 f6 d3 a2 d4 a3 d5 a4 Cb3 Cc3 c5 b5 b6 a6 b3< b4 a5 b3 e5 b2 b6- b4+ a5> b6 2a3+11 b6- a5> b4
    let move_strings = [
        "a1", "f6", "d3", "a2", "d4", "a3", "d5", "a4", "Cb3", "Cc3", "c5", "b5", "b6", "a6",
        "b3<", "b4", "a5", "b3", "e5", "b2", "b6-", "b4+", "a5>", "b6", "2a3+11", "b6-", "a5>",
        "b4",
    ];

    let mut board = <Board<6>>::start_position();

    do_moves_and_check_validity(&mut board, &move_strings);

    plays_correct_hard_move_property::<6>(&move_strings, &["4b5<"]);
}

#[test]
fn tinue_3ply_test() {
    // b6 a6 a5 b3 b5 c3 c5 d3 e5 d5 f5 d4 d6 d5> e6 Cd5 c6 b6> Cc4 d2 c5+ d1 c4> a3 f6 d5+ d5 Sc5 c2 e1 f1 f2 2d4- e2 f3 b1 f4 c1 f3- 2e5> f4+ Sf4 b2 e3 f3 f4+ d4 5f5-122 3d3>12 3f2- 3f3- e3> f4- e3> e3 5f3<32 Sf3 2d6> f5 a1 f4
    // Requires spread that gives us a hard cap next to our critical square
    let move_strings = [
        "b6", "a6", "a5", "b3", "b5", "c3", "c5", "d3", "e5", "d5", "f5", "d4", "d6", "d5>", "e6",
        "Cd5", "c6", "b6>", "Cc4", "d2", "c5+", "d1", "c4>", "a3", "f6", "d5+", "d5", "Sc5", "c2",
        "e1", "f1", "f2", "2d4-", "e2", "f3", "b1", "f4", "c1", "f3-", "2e5>", "f4+", "Sf4", "b2",
        "e3", "f3", "f4+", "d4", "5f5-122", "3d3>12", "3f2-", "3f3-", "e3>", "f4-", "e3>", "e3",
        "5f3<32", "Sf3", "2d6>", "f5", "a1", "f4",
    ];

    let mut board = <Board<6>>::start_position();

    do_moves_and_check_validity(&mut board, &move_strings);

    plays_correct_hard_move_property::<6>(&move_strings, &["3e6-111"]);
}

#[test]
fn tinue_3ply_test2() {
    // a6 f1 d3 b6 d4 c6 e6 d6 Cd5 d2 e5 e2 c3 Cc2 f2 f3 e3 c2+ e1 f3< e4 f3 d3> e2+ e2 d3 d5> 4e3-22 Se3 c4 e3- c5 d4< Se3 3e2- b3 2e5-11* d6> 2e4+11 Se4
    // Requires spreading our cap, flattening our wall, which creates two orthogonal road threats
    let move_strings = [
        "a6", "f1", "d3", "b6", "d4", "c6", "e6", "d6", "Cd5", "d2", "e5", "e2", "c3", "Cc2", "f2",
        "f3", "e3", "c2+", "e1", "f3<", "e4", "f3", "d3>", "e2+", "e2", "d3", "d5>", "4e3-22",
        "Se3", "c4", "e3-", "c5", "d4<", "Se3", "3e2-", "b3", "2e5-11", "d6>", "2e4+11", "Se4",
    ];

    let mut board = <Board<6>>::start_position();

    do_moves_and_check_validity(&mut board, &move_strings);

    plays_correct_hard_move_property::<6>(&move_strings, &["2e3-11"]);
}

#[test]
fn tinue_nply_test() {
    // e6 f6 f5 e5 f4 e4 f3 Ce3 f2 e3> e2 e3 d2 c2 d3 d4 Cc3 d1 b3 c2> d3- d1+ e2< e1 e2 2f3- e2+ 3f2<12 d1 d5 b2
    // Requires centralizing our cap, making the vertical threats unstoppable
    // White can delay for 5+ moves
    let move_strings = [
        "e6", "f6", "f5", "e5", "f4", "e4", "f3", "Ce3", "f2", "e3>", "e2", "e3", "d2", "c2", "d3",
        "d4", "Cc3", "d1", "b3", "c2>", "d3-", "d1+", "e2<", "e1", "e2", "2f3-", "e2+", "3f2<12",
        "d1", "d5", "b2",
    ];

    let mut board = <Board<6>>::start_position();

    do_moves_and_check_validity(&mut board, &move_strings);

    plays_correct_hard_move_property::<6>(&move_strings, &["5d2>"]);
}

use std::convert::TryFrom;

use crate::position::Komi;

use super::TestPosition;

#[test]
fn avoid_loss_in_two() {
    let test_position = TestPosition::from_move_strings(&[
        "b5", "e2", "Cc3", "b3", "b2", "Cc2", "b4", "c4", "d3", "c5", "e3",
    ]);

    test_position.plays_correct_move_long_prop::<5>(&["a3", "d2", "d4", "a2", "c2<"]);
}

#[test]
// c3< wins if not stopped
fn avoid_loss_in_three2() {
    // x,2,x3/x,2,2,x2/x,1,2,1,1/x,12C,21C,1,x/x,1,2,x2 1 9
    let test_position = TestPosition::from_move_strings(&[
        "b5", "e3", "Cc3", "Cb3", "b2", "b4", "b1", "c2", "d3", "c4", "d2", "c1", "c3-", "b3-",
        "b3", "c3",
    ]);

    test_position.plays_correct_move_long_prop::<5>(&[
        "a1", "Sa1", "a2", "Sa2", "a3", "Sa3", "Sa4", "Sa5", "Sc5", "2c2+", "2c2-", "b3+",
    ]);
}

#[test]
fn find_win_in_two() {
    let test_position = TestPosition::from_move_strings(&[
        "a5", "e4", "Cc3", "c4", "b3", "Cd3", "b4", "b5", "d4", "d5", "a4", "c4>", "e4<", "d3+",
        "e3", "d3", "d2", "4d4<22", "a3", "3b4-", "c5", "2c4+", "a4+", "b2", "b4", "c4", "b1",
        "c2", "c1", "d1", "d4", "a2", "a4", "e2", "d2<", "c4<", "a4>", "d2", "c4", "b2>", "c1+",
        "b5-", "4c2>22", "4b4>22", "c3+", "d3-", "3c4>", "3d2>", "4d4-22", "5e2+122", "d4>",
        "2e3+", "d4>", "2e5-", "d4", "3e4<", "e3", "c2", "a4", "e1", "e3+", "4d4>", "a1", "a2+",
        "a2",
    ]);

    test_position.plays_correct_move_long_prop::<5>(&["5e4+"]);
}

#[test]
fn find_win_in_two2() {
    // a1 a5 b5 Cc3 c5 d5 Cd4 c4 e5 1c4+1 1d4+1 c4 1d5<1 1d5>1 d5 e4 2c5>11 1d5<1 2e5<11 2d5>2
    let test_position = TestPosition::from_move_strings(&[
        "a1", "a5", "b5", "Cc3", "c5", "d5", "Cd4", "c4", "e5", "1c4+1", "1d4+1", "c4", "1d5<1",
        "1d5>1", "d5", "e4", "2c5>11", "1d5<1", "2e5<11", "2d5>2",
    ]);

    test_position.plays_correct_move_long_prop::<5>(&["2c5>11"]);
}

#[test]
fn find_win_in_two3() {
    // a5 e5 e4 Cc3 e3 e2 Cd3 d2 e1 c4 1e1+1 e1 1d3-1 Sd1
    let test_position = TestPosition::from_move_strings(&[
        "a5", "e5", "e4", "Cc3", "e3", "e2", "Cd3", "d2", "e1", "c4", "1e1+1", "e1", "1d3-1", "Sd1",
    ]);

    test_position.plays_correct_move_long_prop::<5>(&["d2>"]);
}

#[test]
fn find_capstone_spread_win_in_two() {
    // b4 a5 e5 b5 b3 Cc3 Cc5 d5 d4 d3 b3+ a4 2b4+ a4+ a4 b4 d4+ b4< b4 a3 b3 a2 3b5< 2a4+ Sa4 b2 e3 e2 a4+ d2 5a5-122 3a5-21 3a2+ c2 5a3- c3< 5a2>113 2a4- a5 Sb5 d4 c3 e3< c3> 4d2< e3 Sc3 3d3+12 c5> e4 5d5> c4 5e5-212 2d4> e3+ e1
    let test_position = TestPosition::from_move_strings(&[
        "b4", "a5", "e5", "b5", "b3", "Cc3", "Cc5", "d5", "d4", "d3", "b3+", "a4", "2b4+", "a4+",
        "a4", "b4", "d4+", "b4<", "b4", "a3", "b3", "a2", "3b5<", "2a4+", "Sa4", "b2", "e3", "e2",
        "a4+", "d2", "5a5-122", "3a5-21", "3a2+", "c2", "5a3-", "c3<", "5a2>113", "2a4-", "a5",
        "Sb5", "d4", "c3", "e3<", "c3>", "4d2<", "e3", "Sc3", "3d3+12", "c5>", "e4", "5d5>", "c4",
        "5e5-212", "2d4>", "e3+", "e1",
    ]);

    test_position.plays_correct_move_long_prop::<5>(&["2e2+11"]);
}

#[test]
fn capture_stack_in_strong_file() {
    // b5 a5 e1 b3 Cc3 b4 b2 c5 a4 d5 c4 e5 a3 b3< a5>
    let test_position = TestPosition::from_move_strings(&[
        "b5", "a5", "e1", "b3", "Cc3", "b4", "b2", "c5", "a4", "d5", "c4", "e5", "a3", "b3<", "a5>",
    ]);

    test_position.plays_correct_move_long_prop::<5>(&["b4+"]);
}

#[test]
fn spread_stack_for_tinue() {
    // c3 a5 e1 b3 Cc2 d4 a4 a3 b4 d3 c2+ d2 c4 d5 2c3> Cc3 c2 e4 b2 c5 3d3+ d3 4d4- d4 5d3+ d3 b4- c3< e3 c3 e3< b4 b5 e3 2d3> d3 3e3< d2+ Se3 d2 e3< e3 2d3< e3< 3c3> c3 a2 2b3- a4> b3+ c4< b3 4d3< b3+ b5- d2+ 5c3> 3b2+ c1 a3- d1 3b3>21
    let test_position = TestPosition::from_move_strings(&[
        "c3", "a5", "e1", "b3", "Cc2", "d4", "a4", "a3", "b4", "d3", "c2+", "d2", "c4", "d5",
        "2c3>", "Cc3", "c2", "e4", "b2", "c5", "3d3+", "d3", "4d4-", "d4", "5d3+", "d3", "b4-",
        "c3<", "e3", "c3", "e3<", "b4", "b5", "e3", "2d3>", "d3", "3e3<", "d2+", "Se3", "d2",
        "e3<", "e3", "2d3<", "e3<", "3c3>", "c3", "a2", "2b3-", "a4>", "b3+", "c4<", "b3", "4d3<",
        "b3+", "b5-", "d2+", "5c3>", "3b2+", "c1", "a3-", "d1", "3b3>21",
    ]);

    test_position.plays_correct_move_long_prop::<5>(&["4b4-211"]);
}

#[test]
fn find_win_in_three() {
    // e1 e5 Cc3 c1 d1 d2 a3 b1 b3 d2- a1 a2 a1> Cb2 Sc2 a1 2b1> b2+ b5 b1 c4 d2 c5
    let test_position = TestPosition::from_move_strings(&[
        "e1", "e5", "Cc3", "c1", "d1", "d2", "a3", "b1", "b3", "d2-", "a1", "a2", "a1>", "Cb2",
        "Sc2", "a1", "2b1>", "b2+", "b5", "b1", "c4", "d2", "c5",
    ]);

    test_position.plays_correct_move_long_prop::<5>(&["2b3-11"]);
}

#[test]
fn find_win_in_three2() {
    // c4 a5 e1 c3 d1 c2 c1 b1 Cb2 c5 b2- a1 a2 c2- c2 2c1> d2 Cb2 c1 b2> d2- 2c2- c2 3c1> b2 d3 Sd2 c1 a3 a1+ a3-
    let test_position = TestPosition::from_move_strings(&[
        "c4", "a5", "e1", "c3", "d1", "c2", "c1", "b1", "Cb2", "c5", "b2-", "a1", "a2", "c2-",
        "c2", "2c1>", "d2", "Cb2", "c1", "b2>", "d2-", "2c2-", "c2", "3c1>", "b2", "d3", "Sd2",
        "c1", "a3", "a1+", "a3-",
    ]);

    test_position.plays_correct_move_long_prop::<5>(&["d1<"]);
}

#[test]
fn tactic_test1() {
    let test_position = TestPosition::from_move_strings(&[
        "b4", "e1", "Cc3", "Cc4", "d4", "b3", "b2", "d3", "c2", "a3", "c3>", "e4", "c3",
    ]);

    test_position.plays_correct_move_long_prop::<5>(&["d5"]);
}

#[test]
fn simple_move_move_to_win() {
    // a5 e2 Cc3 a4 b3 a3 a2 b2 e3 b2< a1 Cb2 b1 b2< Se1 a2-
    let test_position = TestPosition::from_move_strings(&[
        "a5", "e2", "Cc3", "a4", "b3", "a3", "a2", "b2", "e3", "b2<", "a1", "Cb2", "b1",
    ]);

    test_position.plays_correct_move_long_prop::<5>(&["b2<"]);
}

#[test]
fn flatten_our_stone_to_win() {
    // c4 c5 Cc3 Cd3 c2 b4 d4 d3+ d3 b3 c1 b2 b1 b5 a1 e3 c3+ Sc3 d1 Se1 e2 Sd2 a2 a3 a4 2d4- a5 d3< d1< 2c3-11
    let test_position = TestPosition::from_move_strings(&[
        "c4", "c5", "Cc3", "Cd3", "c2", "b4", "d4", "d3+", "d3", "b3", "c1", "b2", "b1", "b5",
        "a1", "e3", "c3+", "Sc3", "d1", "Se1", "e2", "Sd2", "a2", "a3", "a4", "2d4-", "a5",
    ]);

    test_position.plays_correct_move_long_prop::<5>(&["d3<"]);
}

#[test]
fn winning_movement_test() {
    // e1 e5 Cc3 d1 c1 Cc2 d2 b1 c1> a1 c1 d3 b2 e1< c1> b3 d4 e2 b4 d3- 4d1<22 c2- Sd1 c2 d1+ 2c1< c4
    let test_position = TestPosition::from_move_strings(&[
        "e1", "e5", "Cc3", "d1", "c1", "Cc2", "d2", "b1", "c1>", "a1", "c1", "d3", "b2", "e1<",
        "c1>", "b3", "d4", "e2", "b4", "d3-", "4d1<22", "c2-", "Sd1", "c2", "d1+", "2c1<", "c4",
    ]);

    test_position.plays_correct_move_long_prop::<5>(&["4b1>13"]);
}

#[test]
fn winning_movement_test2() {
    // a1 a5 b5 Cc3 c5 d5 Cd4 c4 e5 c4+ c4 b4 c4+ d5< d5 c4 d4< 4c5<22 c5 b3 2c4+ 3b5- 2c5< a4
    let test_position = TestPosition::from_move_strings(&[
        "a1", "a5", "b5", "Cc3", "c5", "d5", "Cd4", "c4", "e5", "c4+", "c4", "b4", "c4+", "d5<",
        "d5", "c4", "d4<", "4c5<22", "c5", "b3", "2c4+", "3b5-", "2c5<", "a4",
    ]);

    test_position.plays_correct_move_long_prop::<5>(&["b5<"]);
}

#[test]
fn double_tak_threat_from_citadel_test() {
    let test_position = TestPosition {
        tps_string: Some(
            "1,x3,2/1,1,112112C,x2/2,x2,212,11212/1,x2,2211112S,12221C/2,2S,2,2221,1 1 37",
        ),
        move_strings: &[],
        komi: Komi::try_from(2.0).unwrap(),
    };
    test_position.plays_correct_move_long_prop::<5>(&["e2<"]);
}

#[test]
fn cap_movement_creating_tak_threat() {
    let test_position = TestPosition {
        tps_string: Some("x2,2,1,x/x,2,2,1,1/x,12,12112C,111112S,1/2,x,2,1,12S/1S,1C,1,1,1 2 21"),
        move_strings: &[],
        komi: Komi::default(),
    };
    test_position.plays_correct_move_long_prop::<5>(&["3c3-"]);
}

/// This is the continuation of the above tactics test
#[test]
fn place_anchor_flat_to_tinue() {
    let test_position = TestPosition {
        tps_string: Some("x2,2,1,x/x2,212,x,1/x,12,12,111112S,1/2,x,2112C,1,12S/1S,x,11C,1,1 2 23"),
        move_strings: &[],
        komi: Komi::default(),
    };
    test_position.plays_correct_move_long_prop::<5>(&["b1"]);
}

#[test]
fn place_cap_in_strong_line_for_tinue() {
    let test_position = TestPosition {
        tps_string: Some("2,x2,1,x/2,x2,1,x/2,12,x,1C,x/x2,2121,x2/x,1,112,x2 2 13"),
        move_strings: &[],
        komi: Komi::default(),
    };
    test_position.plays_correct_move_long_prop::<5>(&["Ca1", "Ca2"]);
}

#[test]
fn delay_cap_placement_for_tinue() {
    let test_position = TestPosition {
        tps_string: Some("1,1,1C,x,21/1,x,2,x,12/2S,x,112,12,x/1,x,2,x2/1,x,2,x2 2 14"),
        move_strings: &[],
        komi: Komi::default(),
    };
    test_position.plays_correct_move_long_prop::<5>(&["d5"]);
}

/// Tinue goes 26... a4> 27. a3> c3<*!, which Tiltak struggles to see from afar
#[test]
fn pure_spread_avoiding_draw() {
    let test_position = TestPosition {
        tps_string: Some(
            "21112,x,1C,1,1/22,1,x2,1/21S,2121122,21212C,x,1/x2,1,1,2S/1,x2,12,x 2 26",
        ),
        move_strings: &[],
        komi: Komi::default(),
    };
    test_position.plays_correct_move_long_prop::<5>(&["a4>"]);
}

#[test]
fn cap_throw_with_tinue() {
    let test_position = TestPosition {
        tps_string: Some("x2,22221C,x2/x2,22,x2/x3,2,x/x2,121112112,x2/x2,1221,x2 1 25"),
        move_strings: &[],
        komi: Komi::default(),
    };
    test_position.plays_correct_move_long_prop::<5>(&["4c5-112", "3c5-111", "4c5-211", "4c5-121"]);
}

#[test]
fn simple_capture_to_tinue() {
    let test_position = TestPosition {
        tps_string: Some("1,x2,1,1/1,1C,2,2,1/x,1,12C,1,2/x3,2,x/2,x2,2,x 2 9"),
        move_strings: &[],
        komi: Komi::default(),
    };
    test_position.plays_correct_move_long_prop::<5>(&["d4-"]);
}

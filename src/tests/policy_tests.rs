use crate::evaluation::parameters::{NUM_POLICY_FEATURES_5S, NUM_POLICY_FEATURES_6S};

use super::TestPosition;

#[test]
fn pure_spread_into_road() {
    let test_position = TestPosition::from_tps(
        "2,2,2,1S,22,1112/x2,1,2,22,21112C/1,1,1,2,21,11121C/x,1,x,2,x2/x,1,x3,2/x6 1 30",
    );
    test_position.top_policy_move_prop::<6, NUM_POLICY_FEATURES_6S>(&[
        "4f4<112", "4f4<13", "4f4<22", "3f4<12",
    ]);
}

#[test]
fn two_pure_spreads_into_road() {
    let test_position = TestPosition::from_tps("x,2,2S,21,1,1/2,212,12,12111122C,x,1/21,x,1,211,x,1S/1,2,1C,x2,1/x2,1,2,11212,2/x2,1,2,x,212 2 36");
    test_position.top_policy_move_prop::<6, NUM_POLICY_FEATURES_6S>(&["6d5-51", "6d5>51"]);
}

#[test]
fn imperfect_cap_spread_onto_critical_square() {
    let test_position = TestPosition::from_tps(
        "2,x4/2,1,x,1,2S/1112,1,x,1S,x/2,121121C,x,1,x/1,x,112C,x,111212S 2 24",
    );
    test_position
        .top_policy_move_prop::<5, NUM_POLICY_FEATURES_5S>(&["3c1<12", "3c1<21", "2c1<11"]);
}

#[test]
fn simple_flat_movement_onto_critical_square() {
    let test_position =
        TestPosition::from_tps("1,1,2,x2/1,1,2,x2/x,1,2,x2/1,2,1,1,x/x,2,2,x,2 2 8");
    test_position.top_policy_move_prop::<5, NUM_POLICY_FEATURES_5S>(&["b2>", "c1+"]);
}

#[test]
fn pure_spread_onto_critical_square() {
    let test_position = TestPosition::from_tps(
        "x4,11212,221C/x2,2,2,1212,1S/2,2,2,1111,2122C,2S/21,x4,1/x,1112121S,x,1,1,1/2,x4,1 2 36",
    );
    test_position.top_policy_move_prop::<6, NUM_POLICY_FEATURES_6S>(&["e4>"]);
}

#[test]
fn pure_spread_without_critical_square() {
    let test_position = TestPosition::from_tps("2,1,1,1,1,2S/1,12,1,x,1C,11112121/x,2,2,212,2C,11/2,21122,x2,1,x/x3,1,1,x/x2,2,21,x,112S 2 34");
    test_position.top_policy_move_prop::<6, NUM_POLICY_FEATURES_6S>(&["4b3>1111"]);
}

#[test]
fn winning_wall_spread_from_critical_square() {
    let test_position = TestPosition::from_tps(
        "2,2,x,2121S,x,1/x,2,12,221,2,x/x,2,2,221C,12C,12/x,2,x,121,x,1/x,2,x,1,1,1/x3,1,x,1 1 25",
    );
    test_position.top_policy_move_prop::<6, NUM_POLICY_FEATURES_6S>(&["2d6<", "2d6>", "2d6<11"]);
}

#[test]
fn false_positive_win_test() {
    let test_position = TestPosition::from_tps(
        "21,21C,122S,1,1/x,221,1,2C,1S/1S,x,21,1222,1/1S,x3,2S/1212,x,11S,1S,2 1 42",
    );
    assert!(!test_position.sets_winning_flag::<5>());
}

#[test]
fn false_positive_win_test2() {
    let test_position = TestPosition::from_tps("2,x,2S,1/2,x,1,1S/2,1,22S,1/1S,2,x,1S 2 12");
    assert!(!test_position.sets_winning_flag::<4>());
}

#[test]
fn false_positive_win_test3() {
    let test_position = TestPosition::from_tps("1S,2S,21C,2S,12,x/22C,1S,212S,2S,1,x/11121,x2,21,x,1/122S,22S,2S,1,1,1S/2S,x,2S,2S,x2/1S,1,2S,2S,1121S,2 1 52");
    assert!(!test_position.sets_winning_flag::<6>());
}

#[test]
fn false_positive_win_test4() {
    let test_position = TestPosition::from_tps("2,2,1,22S,21C,2S/11,212,2,12212S,2,112C/2S,2,x,1S,12,x/1S,1S,2S,22S,21,1S/2S,1,x,1,1,1S/121S,x,112S,2S,12S,2 2 64");
    assert!(!test_position.sets_winning_flag::<6>());
}

#[test]
fn impure_spread_inside_group_win_test() {
    let test_position = TestPosition::from_tps("x,1,x,12112S,x,2/1,1,2211,212222212,12121121C,2/x,1,x,212,x2/2S,1,x,2,x2/21,12S,x,222,21212C,x/x,2,21,x,1,x 1 56");
    test_position.top_policy_move_prop::<6, NUM_POLICY_FEATURES_6S>(&["3c5-111", "4c5-121"]);
}

#[test]
fn create_hard_cap_next_to_critical_square_test() {
    let test_position = TestPosition::from_tps(
        "2,x,2,2,1S,x/x,22,2,221S,2,2/x2,1S,2,2,x/1,2S,x4/2S,1,1,1,1,1/1,212C,111C,1,1,x 1 20",
    );
    test_position.top_five_policy_move_prop::<6, NUM_POLICY_FEATURES_6S>(&["3c1+12"]);
}

#[test]
fn create_double_tak_threat_test() {
    // Taken from Alion's Puzzle #3
    let test_position = TestPosition::from_tps("x2,1,21,x,2/1,x,212,1,212,2/1S,2,2,2C,21,2/21S,1,121C,x2,12/2,2,121,1,1,1/2,2,1,x2,22S 1 28");
    test_position.top_five_policy_move_prop::<6, NUM_POLICY_FEATURES_6S>(&["3c3-"]);
}

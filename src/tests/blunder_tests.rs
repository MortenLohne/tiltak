//! Test that we don't make one-move blunders on very low node counts
use crate::evaluation::parameters::{NUM_POLICY_FEATURES_6S, NUM_VALUE_FEATURES_6S};

use super::TestPosition;

#[test]
fn do_not_blunder_road_win() {
    let test_position = TestPosition::from_tps("x2,2,1,x2/x,2,22,21S,2,2/x,22221C,2,221111211112C,1221S,x/2,2,2S,1S,1,1/x,2,1,2,1,12/1,2,1,21,221,2 2 44");

    test_position.plays_correct_move_short_prop::<6, NUM_VALUE_FEATURES_6S, NUM_POLICY_FEATURES_6S>(
        &["f1<", "d2>", "2f2<", "b1<", "2f2+", "d2-", "b1>", "f2<"],
    )
}

#[test]
fn do_not_blunder_road_win2() {
    let test_position = TestPosition::from_tps("1,1,1,2,2,1/1,x,2,12C,x2/1S,121212S,x2,21,x/112112,x,2,21C,2,x/2,1,1,x,2,2/x,1,x2,1,2 1 25");

    test_position.plays_correct_move_short_prop::<6, NUM_VALUE_FEATURES_6S, NUM_POLICY_FEATURES_6S>(
        &["2e4-", "e1+", "Sd4", "Sc4", "e1>", "2d3>"],
    )
}

#[test]
fn do_not_blunder_road_win3() {
    let test_position = TestPosition::from_tps(
        "1,2,1,1,1,1/x2,212112S,221,1,x/x,2,x,12C,21,2/x2,2,2,2,x/x,221122221C,x4/x6 2 29",
    );

    test_position.plays_correct_move_short_prop::<6, NUM_VALUE_FEATURES_6S, NUM_POLICY_FEATURES_6S>(
        &["b6>", "5c5+", "3c5+", "Sb5", "b6<", "6c5+", "3c5<"],
    )
}

#[test]
fn do_not_blunder_road_win4() {
    let test_position = TestPosition::from_tps("1,x5/1,2112C,1,x,12S,x/x,22212121,x3,1/1211212S,12222,22,2,12,2112211/11,x3,1,1/x2,22122212,12,x,21C 2 71");

    test_position.plays_correct_move_short_prop::<6, NUM_VALUE_FEATURES_6S, NUM_POLICY_FEATURES_6S>(
        &[
            "5b3>1112", "5b3>2111", "2e3>", "5b3>1121", "5b3>1211", "e3>",
        ],
    )
}

#[test]
fn do_not_blunder_road_win5() {
    let test_position = TestPosition::from_tps("x3,2,1,x/2,2,2,2,1,1/2,1,2,21C,1,2/2,2,1S,1,2,2/2,1,2S,2,1121S,1/1,1,x,1,111211212C,1 1 30");

    test_position.plays_correct_move_short_prop::<6, NUM_VALUE_FEATURES_6S, NUM_POLICY_FEATURES_6S>(
        &[
            "a1+", "e6<", "e5<", "Sc1", "b4+", "b4<", "b2<", "2d4+", "4e2+22",
        ],
    )
}

#[test]
fn do_not_blunder_road_win6() {
    let test_position = TestPosition::from_tps(
        "1,121,1,121,x2/x2,2,x3/2,2,21,2,2,1/x2,2S,12C,122122121C,x/x,212S,21,2,12S,1/x6 2 29",
    );

    test_position.plays_correct_move_short_prop::<6, NUM_VALUE_FEATURES_6S, NUM_POLICY_FEATURES_6S>(
        &["Se5", "Se6", "c5+", "e4>"],
    )
}

#[test]
fn do_not_blunder_road_win7() {
    let test_position = TestPosition::from_tps("1,1,2,1,1,1/11112221,x,2S,1,1,x/121,1,21C,12C,112,112/221,x,2,1S,1,1/2,2,2,2,12,1/x3,2,x,12221 1 40");

    test_position.plays_correct_move_short_prop::<6, NUM_VALUE_FEATURES_6S, NUM_POLICY_FEATURES_6S>(
        &["3a4-12", "6a5-114", "3a3-", "f3+", "e3-", "f2<", "2c4-11"],
    )
}

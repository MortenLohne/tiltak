use crate::position::Komi;

use super::TestPosition;

#[test]
fn tinue_5ply_test() {
    // a6 f1 d3 b6 c3 c6 b3 d6 Se6 d5 e5 d4 Ce4 e3 f3 Cc4 f6 f2 f5 1c4-1 e2 d2 e1 1e3-1 1e1+1 1d2>1 Sd2 c4 1d2>1 c1 Sc2 f4 4e2>4 Se3 e1 d2 1e4>1 1e3-1
    let test_position = TestPosition::from_move_strings(&[
        "a6", "f1", "d3", "b6", "c3", "c6", "b3", "d6", "Se6", "d5", "e5", "d4", "Ce4", "e3", "f3",
        "Cc4", "f6", "f2", "f5", "1c4-1", "e2", "d2", "e1", "1e3-1", "1e1+1", "1d2>1", "Sd2", "c4",
        "1d2>1", "c1", "Sc2", "f4", "4e2>4", "Se3", "e1", "d2", "1e4>1", "1e3-1",
    ]);

    test_position.plays_correct_move_long_prop::<6>(&["2f4-11"]);
}

#[test]
fn tinue_7ply_test() {
    // a1 f6 d3 a2 d4 a3 d5 a4 Cb3 Cc3 c5 b5 b6 a6 b3< b4 a5 b3 e5 b2 b6- b4+ a5> b6 2a3+11 b6- a5> b4
    let test_position = TestPosition::from_move_strings(&[
        "a1", "f6", "d3", "a2", "d4", "a3", "d5", "a4", "Cb3", "Cc3", "c5", "b5", "b6", "a6",
        "b3<", "b4", "a5", "b3", "e5", "b2", "b6-", "b4+", "a5>", "b6", "2a3+11", "b6-", "a5>",
        "b4",
    ]);

    test_position.plays_correct_move_long_prop::<6>(&["4b5<"]);
}

#[test]
fn tinue_3ply_test() {
    // b6 a6 a5 b3 b5 c3 c5 d3 e5 d5 f5 d4 d6 d5> e6 Cd5 c6 b6> Cc4 d2 c5+ d1 c4> a3 f6 d5+ d5 Sc5 c2 e1 f1 f2 2d4- e2 f3 b1 f4 c1 f3- 2e5> f4+ Sf4 b2 e3 f3 f4+ d4 5f5-122 3d3>12 3f2- 3f3- e3> f4- e3> e3 5f3<32 Sf3 2d6> f5 a1 f4
    // Requires spread that gives us a hard cap next to our critical square
    let test_position = TestPosition::from_move_strings(&[
        "b6", "a6", "a5", "b3", "b5", "c3", "c5", "d3", "e5", "d5", "f5", "d4", "d6", "d5>", "e6",
        "Cd5", "c6", "b6>", "Cc4", "d2", "c5+", "d1", "c4>", "a3", "f6", "d5+", "d5", "Sc5", "c2",
        "e1", "f1", "f2", "2d4-", "e2", "f3", "b1", "f4", "c1", "f3-", "2e5>", "f4+", "Sf4", "b2",
        "e3", "f3", "f4+", "d4", "5f5-122", "3d3>12", "3f2-", "3f3-", "e3>", "f4-", "e3>", "e3",
        "5f3<32", "Sf3", "2d6>", "f5", "a1", "f4",
    ]);

    test_position.plays_correct_move_long_prop::<6>(&["3e6-111"]);
}

#[test]
fn tinue_3ply_test2() {
    // a6 f1 d3 b6 d4 c6 e6 d6 Cd5 d2 e5 e2 c3 Cc2 f2 f3 e3 c2+ e1 f3< e4 f3 d3> e2+ e2 d3 d5> 4e3-22 Se3 c4 e3- c5 d4< Se3 3e2- b3 2e5-11* d6> 2e4+11 Se4
    // Requires spreading our cap, flattening our wall, which creates two orthogonal road threats
    let test_position = TestPosition::from_move_strings(&[
        "a6", "f1", "d3", "b6", "d4", "c6", "e6", "d6", "Cd5", "d2", "e5", "e2", "c3", "Cc2", "f2",
        "f3", "e3", "c2+", "e1", "f3<", "e4", "f3", "d3>", "e2+", "e2", "d3", "d5>", "4e3-22",
        "Se3", "c4", "e3-", "c5", "d4<", "Se3", "3e2-", "b3", "2e5-11", "d6>", "2e4+11", "Se4",
    ]);

    test_position.plays_correct_move_long_prop::<6>(&["2e3-11"]);
}

#[test]
fn endgame_tinue_test() {
    // f1 a1 a2 b4 b2 c3 c2 d2 e2 d3 f2 d1 b3 d4 Cc4 c5 c4> d2< d2 c1 c4 c6 c4- 2c2< c2 b4- b1 c1< a1> 3b2< b2 f1+ c4 Ca3 d5 2b3> c2+ d3< c4- Sc4 5c3>122 c4- b3 3c3>12 b4 a3- b5 5a2> b6 c2 3b1>12 Se1 c4 e1< c3 3d1<12 e4 6b2+51 c4+ Sc4 e5 c4+ c3< 2b4- c4 b4 c3 b2 c3- c3 a6 2c5< b6> b6 e6 4e3- e3 b6> 2c2- 2b1> c4+ 2c6- Sc4 4c5>13 2d4- 4c1<13 4d3+13 a3 4d5<31 e1 d2- d2 4b5>112
    let test_position = TestPosition::from_move_strings(&[
        "f1", "a1", "a2", "b4", "b2", "c3", "c2", "d2", "e2", "d3", "f2", "d1", "b3", "d4", "Cc4",
        "c5", "c4>", "d2<", "d2", "c1", "c4", "c6", "c4-", "2c2<", "c2", "b4-", "b1", "c1<", "a1>",
        "3b2<", "b2", "f1+", "c4", "Ca3", "d5", "2b3>", "c2+", "d3<", "c4-", "Sc4", "5c3>122",
        "c4-", "b3", "3c3>12", "b4", "a3-", "b5", "5a2>", "b6", "c2", "3b1>12", "Se1", "c4", "e1<",
        "c3", "3d1<12", "e4", "6b2+51", "c4+", "Sc4", "e5", "c4+", "c3<", "2b4-", "c4", "b4", "c3",
        "b2", "c3-", "c3", "a6", "2c5<", "b6>", "b6", "e6", "4e3-", "e3", "b6>", "2c2-", "2b1>",
        "c4+", "2c6-", "Sc4", "4c5>13", "2d4-", "4c1<13", "4d3+13", "a3", "4d5<31", "e1", "d2-",
        "d2", "4b5>112",
    ]);

    test_position.plays_correct_move_long_prop::<6>(&["3e2+"]);
}

#[test]
fn tinue_nply_test() {
    // e6 f6 f5 e5 f4 e4 f3 Ce3 f2 e3> e2 e3 d2 c2 d3 d4 Cc3 d1 b3 c2> d3- d1+ e2< e1 e2 2f3- e2+ 3f2<12 d1 d5 b2
    // Requires centralizing our cap, making the vertical threats unstoppable
    // White can delay for 5+ moves
    let test_position = TestPosition::from_move_strings(&[
        "e6", "f6", "f5", "e5", "f4", "e4", "f3", "Ce3", "f2", "e3>", "e2", "e3", "d2", "c2", "d3",
        "d4", "Cc3", "d1", "b3", "c2>", "d3-", "d1+", "e2<", "e1", "e2", "2f3-", "e2+", "3f2<12",
        "d1", "d5", "b2",
    ]);

    test_position.plays_correct_move_long_prop::<6>(&["5d2>"]);
}

#[test]
fn flatten_our_wall_to_win_test() {
    // b5 b6 f6 b4 c6 b3 a6 Cc5 b2 d5 d6 c5+ c5 c3 Cc4 d3 d4 e3 c4- c4 e4 a3 e5 a2 a1 f3 e5< e5 c2 2c6> e4- 3d6- b2+ a4 e2 f5 f6- d2 d1 f4 Se4 4d5< 2f5< d6 e6 f2 b1 f1 e1 f6 e4> d3> 2c3>11 f6< 3e5+ e5 4e6- Se4 b2 c3 c1 a2- a2 a3- a3 a4- 2b3< 2a1> b2- d3+ 4b1+112 d2< 4a3-13 c4< b3+ b5- Sc4 4b4+22 b1 2c2- b1> Sc2 d2 a5 a6> 2b4+11 3c1< c2- c2 c3- f5 f6 Sc6 f6< 5e5+ 2b6< c6< e5 3b6>12 f6 b6- 2c1<11 3b5< 4a1+1111 6e6>
    let test_position = TestPosition::from_move_strings(&[
        "b5", "b6", "f6", "b4", "c6", "b3", "a6", "Cc5", "b2", "d5", "d6", "c5+", "c5", "c3",
        "Cc4", "d3", "d4", "e3", "c4-", "c4", "e4", "a3", "e5", "a2", "a1", "f3", "e5<", "e5",
        "c2", "2c6>", "e4-", "3d6-", "b2+", "a4", "e2", "f5", "f6-", "d2", "d1", "f4", "Se4",
        "4d5<", "2f5<", "d6", "e6", "f2", "b1", "f1", "e1", "f6", "e4>", "d3>", "2c3>11", "f6<",
        "3e5+", "e5", "4e6-", "Se4", "b2", "c3", "c1", "a2-", "a2", "a3-", "a3", "a4-", "2b3<",
        "2a1>", "b2-", "d3+", "4b1+112", "d2<", "4a3-13", "c4<", "b3+", "b5-", "Sc4", "4b4+22",
        "b1", "2c2-", "b1>", "Sc2", "d2", "a5", "a6>", "2b4+11", "3c1<", "c2-", "c2", "c3-", "f5",
        "f6", "Sc6", "f6<", "5e5+", "2b6<", "c6<", "e5", "3b6>12", "f6", "b6-", "2c1<11", "3b5<",
        "4a1+1111", "6e6>",
    ]);

    test_position.plays_correct_move_long_prop::<6>(&["2c5<11", "5c5<41"])
}

#[test]
fn plus_two_fcd_move_in_endgame() {
    // c4 f5 f2 c3 f3 f4 e4 c2 f6 Ce3 e2 b2 e4> e3> e3 c5 Cd4 d3 f1 2f3+ e5 4f4-13 d2 4f2< f2 a2 d4- e4 f4 5e2+ d4 c1 2d3< b4 b3 a4 a3 d5 f2+ e4> d3 e4 2f3+ 6e3< f3 f2 e2 f2- e3 e4> f3+ Se4 5f4-32 e4> e4 2f4< f4 3e4-12 e1 2f1+ e4 4f2+112 e3> 3f5< e4+ d5> Se4 d1 e4+ Sf2 e4 f2+ e6 6f3+222 f3 Sd5 f3+ 3f6-12 f6 c6 d6 4f4<13 e3+ d1+ 3c3+21 e3 5e5-14 4d4> 2c5- 6e4+ 5e3+ e3 6e4- a5 b3- a1
    let test_position = TestPosition::from_move_strings(&[
        "c4", "f5", "f2", "c3", "f3", "f4", "e4", "c2", "f6", "Ce3", "e2", "b2", "e4>", "e3>",
        "e3", "c5", "Cd4", "d3", "f1", "2f3+", "e5", "4f4-13", "d2", "4f2<", "f2", "a2", "d4-",
        "e4", "f4", "5e2+", "d4", "c1", "2d3<", "b4", "b3", "a4", "a3", "d5", "f2+", "e4>", "d3",
        "e4", "2f3+", "6e3<", "f3", "f2", "e2", "f2-", "e3", "e4>", "f3+", "Se4", "5f4-32", "e4>",
        "e4", "2f4<", "f4", "3e4-12", "e1", "2f1+", "e4", "4f2+112", "e3>", "3f5<", "e4+", "d5>",
        "Se4", "d1", "e4+", "Sf2", "e4", "f2+", "e6", "6f3+222", "f3", "Sd5", "f3+", "3f6-12",
        "f6", "c6", "d6", "4f4<13", "e3+", "d1+", "3c3+21", "e3", "5e5-14", "4d4>", "2c5-", "6e4+",
        "5e3+", "e3", "6e4-", "a5", "b3-", "a1",
    ]);

    test_position.plays_correct_move_long_prop::<6>(&["2f5-"])
}

#[test]
fn double_tak_threat_to_win() {
    // c4 f5 f2 c3 Cc2 b3 d2 b2 e2 b1 b4 c5 c2+ b5 d3 b6 b4- Ca3 Sb4 a3> f4 a5 a4 f6 d5 e5 d4 d6 e6 e5+ f5+ f5 e5 f5+ Sc6 e4 f4< a3 d1 f5 e3 c2 e1 a2 b4+ 2b3+ a1 a6 a4+ c1 2c3+ f5< 2e4+ 2e6- d5> Sd5
    let test_position = TestPosition::from_move_strings(&[
        "c4", "f5", "f2", "c3", "Cc2", "b3", "d2", "b2", "e2", "b1", "b4", "c5", "c2+", "b5", "d3",
        "b6", "b4-", "Ca3", "Sb4", "a3>", "f4", "a5", "a4", "f6", "d5", "e5", "d4", "d6", "e6",
        "e5+", "f5+", "f5", "e5", "f5+", "Sc6", "e4", "f4<", "a3", "d1", "f5", "e3", "c2", "e1",
        "a2", "b4+", "2b3+", "a1", "a6", "a4+", "c1", "2c3+", "f5<", "2e4+", "2e6-", "d5>", "Sd5",
    ]);
    test_position.plays_correct_move_long_prop::<6>(&["6e5-15"])
}

#[test]
fn play_instant_road_win_test() {
    let test_position = TestPosition::from_tps("1,2S,x,2,2,2/1,2,21,112,11121112,12S/12C,2,1121C,x,2,1/x,212211112112,x2,1,1/2,2S,x3,1/x,21S,x,2,21,2221S 2 53");
    test_position.plays_correct_move_short_prop::<6>(&["6e5<24"]);
}

#[test]
fn play_instant_road_win_test2() {
    let test_position = TestPosition::from_tps("1,1,2,1,1,1/x2,2,2,1,x/x,2,2,1,2221S,x/1,x,2222221C,11112C,2,x/x,2121,2,1,21,2/21,2,2,1,12,x 1 36");
    test_position.plays_correct_move_short_prop::<6>(&["6c3+114"]);
}

#[test]
fn smash_to_create_two_vulnerable_critical_squares() {
    let test_position = TestPosition::from_tps("x,221,221,x,2S,2/x,2,2,2S,1,x/x2,2C,1,1,1/1,12S,x,1222221C,2,2/x,12,21,1,1,1/x,2,2,x2,1 1 26");
    test_position.plays_correct_move_short_prop::<6>(&["6d3<51"]);
}

#[test]
fn setup_smash_in_strong_line_tinue() {
    // x,1,1,112C,x,1/1,1,2,2,1,1/x,2,21S,2,2,2/2,2,2,1C,2,2/x,2,1,2,1,1/1,2,1,1,1,x 2 19
    let test_position = TestPosition::from_move_strings(&[
        "c5", "b6", "f5", "c4", "c6", "d5", "d6", "c3", "e6", "Ce5", "f6", "e5+", "e5", "d4", "c2",
        "b3", "Cd3", "b2", "Sb4", "2e6<", "b4>", "e4", "b5", "e3", "e2", "b4", "a5", "f3", "f2",
        "d2", "e1", "f4", "d1", "b1", "c1", "a3", "a1",
    ]);
    test_position.plays_correct_move_long_prop::<6>(&["3d6<"]);
}

#[test]
fn smash_followed_by_negative_fcd_throw_tinue() {
    // 1S,2,2,2,12,2/2,21S,111112C,12,112,x/1,21C,x,2S,1,1/x2,12,11221S,x2/x2,1111212,1S,112,112/x6 1 47
    let test_position = TestPosition::from_move_strings(&[
        "b5", "f6", "f2", "b4", "f3", "b3", "f4", "Cf5", "Cc4", "e5", "d5", "e4", "d4", "e3", "d3",
        "e2", "d2", "b2", "d1", "d6", "c6", "e3<", "c5", "b1", "c4<", "e3", "c3", "b3>", "f3<",
        "e2+", "Sb3", "2e3<", "d4-", "2c3>", "d2+", "e3<", "Sd4", "5d3-", "d4-", "5d2>32", "d2",
        "e6", "d4", "b5>", "c6>", "e6<", "d5+", "e6", "d5", "e6<", "d5+", "f5+", "d5", "2f6<11",
        "4d3+22", "f6", "c4", "e5+", "Sa6", "e4<", "c2", "c6", "c3", "a5", "3d5<12", "4d4-22",
        "c1", "b1>", "d1<", "6d6<", "Sd1", "6c6-2112", "d1+", "c2>", "c2<", "5c5>23", "b3>", "b6",
        "2c3>", "Sd4", "c5", "5d2<14", "e4", "5b2>32", "2c1+", "2d2<", "a4", "4c2+13", "c1+",
        "b2>", "Sd2", "5c4+", "a4+",
    ]);
    test_position.plays_correct_move_long_prop::<6>(&["c5<"]);
}

#[test]
fn impure_stack_throw_onto_strong_line_tinue() {
    // x6/2,2,2,2,1,1/x3,2,1,x/x3,1C,1,11212C/x3,1,x2/x5,1 2 11
    let test_position = TestPosition::from_move_strings(&[
        "b5", "f4", "f1", "c5", "f3", "f2", "e2", "d5", "e5", "Ce4", "f5", "d4", "e3", "a5", "e2>",
        "e4>", "e4", "2f4-11", "Cd3", "3f2+", "d2",
    ]);
    test_position.plays_correct_move_long_prop::<6>(&["5f3+14"]);
}

#[test]
fn flat_capture_to_create_double_critical_square_tinue() {
    // 1,1,x,2,2,1/1,2,2,2,1,1/112C,2,2,x,1C,1/x2,21,212,x2/1,1,1S,21,12,2/x3,2,x2 2 20
    let test_position = TestPosition::from_move_strings(&[
        "b5", "b6", "a4", "c5", "a5", "d5", "a6", "b4", "a3", "Cb3", "a2", "b3<", "b3", "c4", "e5",
        "d4", "Ce4", "c3", "Sc2", "d6", "e3", "e6", "f6", "d2", "e2", "d1", "e2<", "d3", "e2",
        "e1", "b2", "2a3+", "f5", "f2", "f4", "e1+", "e3<", "d4-", "b3>",
    ]);
    test_position.plays_correct_move_long_prop::<6>(&["d1+"]);
}

#[test]
fn create_hard_cap_next_to_critical_square_tinue() {
    // 21S,12C,x4/21,2,2,2,2,2/1,x,2,x3/x,1C,2,x3/1,x,12,1,1,1/x2,1,1,2,1 2 16
    let test_position = TestPosition::from_move_strings(&[
        "c5", "a5", "f1", "c4", "c2", "c3", "d2", "b2", "Cb3", "b4", "d1", "e1", "e2", "d5", "a4",
        "e5", "a2", "f5", "f2", "b2>", "c1", "b4<", "a5-", "Cb5", "Sa5", "a6", "b6", "b5+", "a5+",
        "b5", "a5",
    ]);

    test_position.plays_correct_move_long_prop::<6>(&["b6-"]);
}

#[test]
fn create_hard_cap_next_to_critical_square_tinue_2() {
    let test_position = TestPosition::from_tps(
        "x2,1,2,1,1/2,2,1,2,2S,2/x,1,12C,221C,12,1/2,2,2,1,1,1/x,1,x,2,2,1/2,x4,121 1 20",
    );

    test_position.plays_correct_move_long_prop::<6>(&["3d4>21"]);
}

#[test]
fn pure_cap_spread_onto_strong_line_tinue() {
    // 1,21,21,x3/2,2,12,2,x2/x,2,x,1,x2/x,2,2,1C,1,x/x,1,1,12C,1,1/x,2,x4 2 14
    let test_position = TestPosition::from_move_strings(&[
        "b5", "a6", "f2", "b4", "c2", "b3", "b2", "c3", "d2", "Cd3", "e2", "d3-", "Cd3", "b6",
        "c5", "c4", "d4", "b1", "a6>", "a5", "a6", "c6", "d6", "d5", "e3", "c4+", "d6<",
    ]);
    test_position.plays_correct_move_long_prop::<6>(&["2d2<11"]);
}

#[test]
fn pure_stack_spread_setting_up_fortress_smash_tinue() {
    // 2,2,x3,1/x,2,2,1,2,1/1S,2,2,x,1C,x/2,21S,2,2,1S,x/2,21,1,1,1,121112C/112,x,1S,1,1,1 2 25
    let test_position = TestPosition::from_move_strings(&[
        "b5", "f6", "f3", "c5", "f5", "Cf4", "Ce4", "e5", "d5", "c4", "e3", "c3", "e1", "f4-",
        "f4", "2f3<", "f3", "f2", "e2", "c2", "d2", "b6", "d2<", "b3", "f1", "f2+", "b1", "d3",
        "f4-", "d2", "d1", "d2<", "Sc1", "3c2<12", "a1", "3e3>", "Se3", "2a2-", "b1+", "a2", "Sa3",
        "a6", "c2", "b4", "d2", "6f3-", "a3>", "a3", "Sa4",
    ]);
    test_position.plays_correct_move_long_prop::<6>(&["6f2<1113", "6f2<2211"]);
}

#[test]
fn setup_double_tak_threat_test() {
    // Alion's Puzzle #8
    let test_position = TestPosition::from_tps("2,2,21,2,12S,2/2,2,2,211112C,22221C,21/12,2,x,1,21,221S/2,1,1,1,1,2/2,1,1,1,1,2/2S,1,1,x2,12 1 35");
    test_position.plays_correct_move_long_prop::<6>(&["b3>"]);
}

#[test]
fn double_tak_threat_tinue() {
    // Alion's Puzzle #6
    let test_position = TestPosition::from_tps("x,1,x4/x,12S,x,2,x2/x,12,212S,x,2C,x/12,2111211C,1S,112,11112,x/2221,x,2,1,2,1S/2222221,x,2,2,2,2 1 44");
    test_position.plays_correct_move_long_prop::<6>(&["6b3-"]);
}

#[test]
fn setup_cap_transposition() {
    // Not actually tinue, but the threat of 63... 4f5- is devastating, and wins shortly
    let test_position = TestPosition::from_tps_with_komi("x3,1S,x2/x,2,11,x2,11122212C/x,211222221C,1,211,1212,2/2,112,x,1121111112S,x,121/x,21S,x,112,x2/2,x5 2 62", Komi::from_half_komi(4).unwrap());
    test_position.plays_correct_move_long_prop::<6>(&["f6"]);
}

#[test]
fn capture_stack_for_gaelet() {
    // Other moves allow white to draw immediately
    let test_position = TestPosition::from_tps_with_komi("2,2,1,1,x,2/2,2,x2,1,2/1,2S,x,2,1,x/x,11121S,x,112S,21112221,2/22221S,11,1112S,1,2S,2112221C/x,1212121S,1,2C,x2 2 57", Komi::from_half_komi(4).unwrap());
    test_position.plays_correct_move_long_prop::<6>(&["f3<", "e2+"]);
}

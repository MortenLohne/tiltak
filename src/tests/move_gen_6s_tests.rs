use pgn_traits::PgnPosition;

use crate::position::Position;

use super::move_gen_generic_tests::perft_check_answers;

#[test]
fn start_pos_perf_test() {
    let mut position = <Position<6>>::default();
    perft_check_answers(
        &mut position,
        &[1, 36, 1_260, 132_720, 13_586_048],
        // &[1, 36, 1_260, 132_720, 13_586_048, 1_253_506_520],
    );
}

// From game #636814 on playtak
#[test]
fn endgame_perf_test() {
    let mut position = <Position<6>>::from_fen("2,2,21S,2,2,2/2,x,222221,2,2,x/1,1,2221C,x,111112C,2S/x,1,2S,x2,121211212/1,1,1212S,1S,2,1S/x2,2,1,21,1 1 42").unwrap();
    perft_check_answers(
        &mut position,
        &[1, 140, 21_413, 2_774_947],
        // &[1, 140, 21_413, 2_774_947, 395_517_158, 48_999_979_678],
    );
}

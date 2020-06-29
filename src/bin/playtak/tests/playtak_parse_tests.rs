use crate::playtak::parse;
use taik::board::Move;

#[test]
fn parse_place_move_test() {
    let move_strings = [
        ("P a1", "a1"),
        ("P a1 W", "Sa1"),
        ("P a1 C", "Ca1"),
        ("P d3 W", "Sd3"),
    ];

    for (playtak_move_string, san_move_string) in move_strings.iter() {
        assert_eq!(
            parse::parse_move(playtak_move_string).to_string(),
            *san_move_string
        );
    }
}

#[test]
fn parse_move_move_test() {
    let move_strings = [("M a1 c1 1 2", "3a1>12"), ("M c2 c3 1", "c2+")];

    for (playtak_move_string, san_move_string) in move_strings.iter() {
        assert_eq!(
            parse::parse_move(playtak_move_string).to_string(),
            *san_move_string
        );
    }
}

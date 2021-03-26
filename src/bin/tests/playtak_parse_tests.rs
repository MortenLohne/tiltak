use crate::position::Position;
use board_game_traits::Position as PositionTrait;
use pgn_traits::PgnPosition;
use tiltak::position::mv::Move;

#[test]
fn parse_place_move_test() {
    let move_strings = [
        ("P A1", "a1"),
        ("P A1 W", "Sa1"),
        ("P A1 C", "Ca1"),
        ("P D3 W", "Sd3"),
    ];

    for (playtak_move_string, san_move_string) in move_strings.iter() {
        assert_eq!(
            Move::from_string_playtak::<5>(playtak_move_string).to_string::<5>(),
            *san_move_string
        );
    }
}

#[test]
fn parse_move_move_test() {
    let move_strings = [("M A1 C1 1 2", "3a1>12"), ("M C2 C3 1", "c2+")];

    for (playtak_move_string, san_move_string) in move_strings.iter() {
        assert_eq!(
            Move::from_string_playtak::<5>(playtak_move_string).to_string::<5>(),
            *san_move_string
        );
    }
}

#[test]
fn write_place_move_5s_test() {
    let move_strings = [
        ("P A1", "a1"),
        ("P A1 W", "Sa1"),
        ("P A1 C", "Ca1"),
        ("P D3 W", "Sd3"),
    ];

    for (playtak_move_string, san_move_string) in move_strings.iter() {
        let position = <Position<5>>::start_position();
        assert_eq!(
            Move::to_string_playtak::<5>(&position.move_from_san(san_move_string).unwrap()),
            *playtak_move_string
        );
    }
}

#[test]
fn write_move_move_5s_test() {
    let move_strings = [("M A1 C1 1 2", "3a1>12"), ("M C2 C3 1", "c2+")];

    for (playtak_move_string, san_move_string) in move_strings.iter() {
        let position = <Position<5>>::start_position();
        assert_eq!(
            Move::to_string_playtak::<5>(&position.move_from_san(san_move_string).unwrap()),
            *playtak_move_string
        );
    }
}

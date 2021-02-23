use crate::board::Board;
use crate::playtak;
use board_game_traits::Position as PositionTrait;
use pgn_traits::PgnPosition;

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
            playtak::parse_move::<5>(playtak_move_string).to_string::<5>(),
            *san_move_string
        );
    }
}

#[test]
fn parse_move_move_test() {
    let move_strings = [("M A1 C1 1 2", "3a1>12"), ("M C2 C3 1", "c2+")];

    for (playtak_move_string, san_move_string) in move_strings.iter() {
        assert_eq!(
            playtak::parse_move::<5>(playtak_move_string).to_string::<5>(),
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
        let board = <Board<5>>::start_position();
        let mut sink = String::new();
        playtak::write_move::<5>(board.move_from_san(san_move_string).unwrap(), &mut sink);
        assert_eq!(sink, *playtak_move_string);
    }
}

#[test]
fn write_move_move_5s_test() {
    let move_strings = [("M A1 C1 1 2", "3a1>12"), ("M C2 C3 1", "c2+")];

    for (playtak_move_string, san_move_string) in move_strings.iter() {
        let board = <Board<5>>::start_position();
        let mut sink = String::new();
        playtak::write_move::<5>(board.move_from_san(san_move_string).unwrap(), &mut sink);
        assert_eq!(sink, *playtak_move_string);
    }
}

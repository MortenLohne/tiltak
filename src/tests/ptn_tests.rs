use crate::position::Position;
use crate::ptn::{ptn_parser, Game, PtnMove};
use crate::tests::do_moves_and_check_validity;
use board_game_traits::{GameResult, Position as PositionTrait};
use std::io::Cursor;

#[test]
pub fn write_and_read_ptn_test() {
    let mut position = <Position<6>>::start_position();
    let move_strings = vec![
        "a1", "f6", "e6", "a2", "d6", "a3", "c6", "a4", "b6", "a5", "a6",
    ];
    do_moves_and_check_validity(&mut position, &move_strings);
    let moves = position
        .moves()
        .iter()
        .map(|mv| PtnMove {
            mv: mv.clone(),
            annotations: vec![],
            comment: "".to_string(),
        })
        .collect();

    let game: Game<Position<6>> = Game {
        start_position: Position::start_position(),
        moves,
        game_result: Some(GameResult::WhiteWin),
        tags: vec![
            ("Player1".to_string(), "tiltak".to_string()),
            ("Player2".to_string(), "tiltak".to_string()),
            ("Date".to_string(), "2021.03.06".to_string()),
            ("Size".to_string(), "6".to_string()),
            ("Result".to_string(), "1-0".to_string()),
            ("Round".to_string(), "1".to_string()),
        ],
    };

    let mut ptn_writer = Cursor::new(vec![]);
    game.game_to_ptn(&mut ptn_writer).unwrap();
    let ptn = String::from_utf8(ptn_writer.into_inner()).unwrap();

    let parsed_games: Vec<Game<Position<6>>> = ptn_parser::parse_ptn(&ptn).unwrap();

    assert_eq!(parsed_games, vec![game])
}

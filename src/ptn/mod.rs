use board_game_traits::{GameResult, Position};
use pgn_traits::PgnPosition;
use std::error;

pub mod ptn_parser;
pub mod ptn_writer;

type ParseError = Box<dyn error::Error + Send + Sync>;

#[derive(Debug, Clone, PartialEq)]
pub struct Game<B: Position> {
    pub start_position: B,
    pub moves: Vec<PtnMove<B::Move>>,
    pub game_result_str: Option<&'static str>,
    pub tags: Vec<(String, String)>,
}

impl<B: PgnPosition> Game<B> {
    pub fn game_result(&self) -> Option<GameResult> {
        self.game_result_str.and_then(|game_result_str| {
            B::POSSIBLE_GAME_RESULTS
                .iter()
                .find(|(result_str, _result)| *result_str == game_result_str)
                .unwrap()
                .1
        })
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct PtnMove<Move> {
    pub mv: Move,
    pub annotations: Vec<&'static str>,
    pub comment: String,
}

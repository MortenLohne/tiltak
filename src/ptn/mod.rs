use board_game_traits::GameResult;
use board_game_traits::Position;
use std::error;

pub mod ptn_parser;
pub mod ptn_writer;

type ParseError = Box<dyn error::Error + Send + Sync>;

#[derive(Debug, Clone, PartialEq)]
pub struct Game<B: Position> {
    pub start_position: B,
    pub moves: Vec<PtnMove<B::Move>>,
    pub game_result: Option<GameResult>,
    pub tags: Vec<(String, String)>,
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct PtnMove<Move> {
    pub mv: Move,
    pub annotations: Vec<&'static str>,
    pub comment: String,
}

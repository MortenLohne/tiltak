use board_game_traits::GameResult;
use board_game_traits::Position;

pub mod ptn_writer;
pub mod ptn_parser;

#[derive(Debug, Clone, PartialEq)]
pub struct Game<B: Position> {
    pub start_position: B,
    pub moves: Vec<(B::Move, Vec<&'static str>, String)>,
    pub game_result: Option<GameResult>,
    pub tags: Vec<(String, String)>,
}

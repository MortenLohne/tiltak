use board_game_traits::Position as PositionTrait;
use board_game_traits::{Color, GameResult};
use pgn_traits::PgnPosition;
use std::io;
use std::io::Write;

#[derive(Debug, Clone, PartialEq)]
pub struct Game<B: PositionTrait> {
    pub start_position: B,
    pub moves: Vec<(B::Move, Vec<&'static str>, String)>,
    pub game_result: Option<GameResult>,
    pub tags: Vec<(String, String)>,
}

impl<B: PgnPosition + Clone> Game<B> {
    pub fn game_to_ptn<W: Write>(&self, f: &mut W) -> Result<(), io::Error> {
        // Write the required tags first, in the correct order
        // Fill in default value if they are not available
        // We must ensure that all required tags are included, and written in the correct order
        let mut tags = self.tags.clone();

        for (required_tag, default_value) in B::REQUIRED_TAGS.iter() {
            let position = tags
                .iter()
                .position(|(tag, _value)| tag.eq_ignore_ascii_case(required_tag));
            if let Some(position) = position {
                let (_tag, value) = tags.remove(position);
                // Write the tag with correct capitalization
                writeln!(f, "[{} \"{}\"]", required_tag, value)?;
            } else {
                writeln!(f, "[{} \"{}\"]", required_tag, default_value)?;
            }
        }

        writeln!(
            f,
            "[Result \"{}\"]",
            match self.game_result {
                None => "*",
                Some(GameResult::WhiteWin) => "1-0",
                Some(GameResult::BlackWin) => "0-1",
                Some(GameResult::Draw) => "1/2-1/2",
            }
        )?;

        if self.start_position != B::start_position()
            && tags
                .iter()
                .find(|(tag, _)| tag.eq_ignore_ascii_case("FEN"))
                .is_none()
        {
            writeln!(f, "[FEN \"{}\"", self.start_position.to_fen())?;
        }

        // Write any remaining tags
        for (tag, value) in tags.iter() {
            writeln!(f, "[{} \"{}\"]", tag, value)?;
        }

        let mut board = self.start_position.clone();

        for (i, (mv, _annotation, comment)) in self.moves.iter().enumerate() {
            if i % 12 == 0 {
                writeln!(f)?;
            }
            if i == 0 && board.side_to_move() == Color::Black {
                write!(f, "1... {} {{{}}} ", board.move_to_san(&mv), comment)?;
            } else if board.side_to_move() == Color::White {
                write!(
                    f,
                    "{}. {} {}",
                    (i + 1) / 2 + 1,
                    board.move_to_san(&mv),
                    if comment.is_empty() {
                        "".to_string()
                    } else {
                        "{".to_string() + comment + "} "
                    }
                )?;
            } else {
                write!(
                    f,
                    "{} {}",
                    board.move_to_san(&mv),
                    if comment.is_empty() {
                        "".to_string()
                    } else {
                        "{".to_string() + comment + "} "
                    }
                )?;
            }
            board.do_move(mv.clone());
        }

        write!(
            f,
            "{}",
            match self.game_result {
                None => "*",
                Some(GameResult::WhiteWin) => "1-0",
                Some(GameResult::BlackWin) => "0-1",
                Some(GameResult::Draw) => "1/2-1/2",
            }
        )?;
        writeln!(f)?;
        writeln!(f)?;
        Ok(())
    }
}

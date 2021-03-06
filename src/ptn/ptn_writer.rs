use crate::ptn::{Game, PtnMove};
use board_game_traits::{Color, GameResult};
use pgn_traits::PgnPosition;
use std::io;
use std::io::Write;

const LINE_WIDTH: usize = 80;

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
                // If the result tag is required, but not provided, manually write it
                if required_tag.eq_ignore_ascii_case("Result") {
                    let result_string = match self.game_result {
                        Some(GameResult::WhiteWin) => "1-0",
                        Some(GameResult::BlackWin) => "0-1",
                        Some(GameResult::Draw) => "1/2-1/2",
                        None => "*",
                    }
                    .to_string();

                    writeln!(f, "[{} \"{}\"]", required_tag, result_string)?;
                } else {
                    writeln!(f, "[{} \"{}\"]", required_tag, default_value)?;
                }
            }
        }

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

        writeln!(f)?;

        let mut board = self.start_position.clone();
        let mut column_position = 0;
        let mut buffer = String::new();

        for (i, PtnMove { mv, comment, .. }) in self.moves.iter().enumerate() {
            if i == 0 && board.side_to_move() == Color::Black {
                buffer.push_str(&format!("1... {}", board.move_to_san(&mv)));
            } else if board.side_to_move() == Color::White {
                buffer.push_str(&format!("{}. {}", (i + 1) / 2 + 1, board.move_to_san(&mv),));
            } else {
                buffer.push_str(&board.move_to_san(&mv));
            }

            if !comment.is_empty() {
                buffer.push_str(" {");
                buffer.push_str(&comment);
                buffer.push('}');
            }
            if board.side_to_move() == Color::Black {
                if column_position == 0 {
                    write!(f, "{}", buffer)?;
                    column_position = buffer.len();
                } else if column_position + buffer.len() < LINE_WIDTH {
                    write!(f, " {}", buffer)?;
                    column_position += buffer.len() + 1;
                } else {
                    write!(f, "\n{}", buffer)?;
                    column_position = buffer.len();
                }
                buffer.clear();
            } else {
                buffer.push(' ');
            }

            board.do_move(mv.clone());
        }

        write!(
            f,
            " {}",
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

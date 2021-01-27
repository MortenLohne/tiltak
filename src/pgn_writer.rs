use board_game_traits::board::Board as BoardTrait;
use board_game_traits::board::{Color, GameResult};
use pgn_traits::pgn::PgnBoard;
use std::io;
use std::io::Write;

#[derive(Debug, Clone, PartialEq)]
pub struct Game<B: BoardTrait> {
    pub start_board: B,
    pub moves: Vec<(B::Move, String)>,
    pub game_result: Option<GameResult>,
    pub tags: Vec<(String, String)>,
}

pub fn game_to_pgn<W: Write, B: PgnBoard>(
    board: &mut B,
    moves: &[(B::Move, String)],
    event: &str,
    site: &str,
    date: &str,
    round: &str,
    white: &str,
    black: &str,
    result: Option<GameResult>,
    tags_pairs: &[(&str, &str)],
    f: &mut W,
) -> Result<(), io::Error> {
    writeln!(f, "[Event \"{}\"]", event)?;
    writeln!(f, "[Site \"{}\"]", site)?;
    writeln!(f, "[Date \"{}\"]", date)?;
    writeln!(f, "[Round \"{}\"]", round)?;
    writeln!(f, "[White \"{}\"]", white)?;
    writeln!(f, "[Black \"{}\"]", black)?;
    writeln!(f, "[Size \"{}\"]", 5)?; // TODO: Support other sizes
    writeln!(
        f,
        "[Result \"{}\"]",
        match result {
            None => "*",
            Some(GameResult::WhiteWin) => "1-0",
            Some(GameResult::BlackWin) => "0-1",
            Some(GameResult::Draw) => "1/2-1/2",
        }
    )?;

    if tags_pairs.iter().find(|(tag, _)| *tag == "FEN").is_none() && *board != B::start_board() {
        writeln!(f, "[FEN \"{}\"", board.to_fen())?;
    }

    for (i, (mv, comment)) in moves.iter().enumerate() {
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
        match result {
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

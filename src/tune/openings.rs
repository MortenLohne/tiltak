use std::fs;
use std::io;
use std::io::BufRead;

use board_game_traits::Position as PositionTrait;
use pgn_traits::PgnPosition;

use crate::position::Komi;
use crate::position::Move;
use crate::position::Position;

pub fn openings_from_file<const S: usize>(path: &str, komi: Komi) -> io::Result<Vec<Vec<Move<S>>>> {
    let reader = io::BufReader::new(fs::File::open(path)?);
    let mut openings = vec![];

    for line in reader.lines() {
        let mut position = <Position<S>>::start_position_with_komi(komi);
        let mut moves = vec![];
        for mv_string in line?.split_whitespace() {
            let mv = position
                .move_from_san(mv_string)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
            let mut legal_moves = vec![];
            position.generate_moves(&mut legal_moves);
            if !legal_moves.contains(&mv) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Illegal move {}", mv_string),
                ));
            }
            position.do_move(mv);
            moves.push(mv);
        }
        openings.push(moves);
    }
    Ok(openings)
}

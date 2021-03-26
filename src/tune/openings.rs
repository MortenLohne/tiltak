use std::fs;
use std::io;
use std::io::BufRead;

use board_game_traits::Position as PositionTrait;
use pgn_traits::PgnPosition;

use crate::position::mv::Move;
use crate::position::Board;

pub fn openings_from_file<const S: usize>(path: &str) -> io::Result<Vec<Vec<Move>>> {
    let reader = io::BufReader::new(fs::File::open(path)?);
    let mut openings = vec![];

    for line in reader.lines() {
        let mut board = <Board<S>>::start_position();
        let mut moves = vec![];
        for mv_string in line?.split_whitespace() {
            let mv = board
                .move_from_san(&mv_string)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
            let mut legal_moves = vec![];
            board.generate_moves(&mut legal_moves);
            if !legal_moves.contains(&mv) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Illegal move {}", mv_string),
                ));
            }
            board.do_move(mv.clone());
            moves.push(mv);
        }
        openings.push(moves);
    }
    Ok(openings)
}

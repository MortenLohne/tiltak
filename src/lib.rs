extern crate board_game_traits;
extern crate pgn_traits;
extern crate rand;
#[macro_use]
extern crate smallvec;
extern crate arrayvec;

mod bitboard;
pub mod board;
pub mod mcts;
pub mod minmax;
mod move_gen;
mod tests;

use crate::board::Board;
use board_game_traits::board::Board as BoardTrait;
use pgn_traits::pgn::PgnBoard;

pub fn do_moves_and_check_validity(board: &mut Board, move_strings: &[&str]) {
    let mut moves = vec![];
    for mv_san in move_strings.iter() {
        let mv = board.move_from_san(&mv_san).unwrap();
        board.generate_moves(&mut moves);
        assert!(
            moves.contains(&mv),
            "Move {} was not among legal moves: {:?}\n{:?}",
            board.move_to_san(&mv),
            moves,
            board
        );
        board.do_move(mv);
        moves.clear();
    }
}

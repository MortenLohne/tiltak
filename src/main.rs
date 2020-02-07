extern crate board_game_traits;
extern crate smallvec;

mod board;
mod tests;

use board as board_mod;
use board_game_traits::board::Board;

fn main() {
    let mut board = board_mod::Board::default();
    let mut moves = vec![];
    board.generate_moves(&mut moves);
    for mv in moves {
        println!("Move: {:?}", mv);
        let reverse_move = board.do_move(mv);
        board.reverse_move(reverse_move);
    }
}

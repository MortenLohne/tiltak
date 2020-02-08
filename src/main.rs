extern crate board_game_traits;
extern crate smallvec;
extern crate rand;

mod board;
mod tests;

use board as board_mod;
use board_game_traits::board::Board;
use rand::seq::SliceRandom;

fn main() {
    let mut board = board_mod::Board::default();
    let mut moves = vec![];
    board.generate_moves(&mut moves);
    for mv in moves.clone() {
        println!("Move: {:?}", mv);
        let reverse_move = board.do_move(mv);
        board.reverse_move(reverse_move);
    }

    let mut rng = rand::thread_rng();

    for i in 0.. {
        moves.clear();
        board.generate_moves(&mut moves);
        println!("Moves: {:?}", moves);
        println!("Board: {:?}", board);
        let mv = moves.choose(&mut rng).expect("No legal moves available").clone();
        println!("Doing move {:?}", mv);
        board.do_move(mv);
        if board.game_result().is_some() {
            println!("Game ended with {:?} after {} moves", board.game_result(), i);
            break;
        }
    }
}

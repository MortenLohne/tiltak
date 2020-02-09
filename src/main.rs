extern crate board_game_traits;
extern crate rand;
#[macro_use]
extern crate smallvec;

mod board;
mod tests;

use board as board_mod;
use board_game_traits::board::Board;
use rand::seq::SliceRandom;

fn main() {
    let mut rng = rand::thread_rng();
    for j in 0..100 {
        let mut board = board_mod::Board::default();
        let mut moves = vec![];
        for i in 0.. {
            moves.clear();
            board.generate_moves(&mut moves);
            println!("Moves: {:?}", moves);
            println!("Board:\n{:?}", board);
            let mv = moves
                .choose(&mut rng)
                .expect("No legal moves available")
                .clone();
            println!("Doing move {:?}", mv);
            board.do_move(mv);
            if board.game_result().is_some() {
                println!("Board:\n{:?}", board);
                println!(
                    "Game ended with {:?} after {} moves",
                    board.game_result(),
                    i
                );
                break;
            }
        }
    }
}

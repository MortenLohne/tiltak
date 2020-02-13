use crate::{board_mod, mcts};
use board_game_traits::board::Board;
use pgn_traits::pgn::PgnBoard;

#[test]
fn win_in_two_moves_test() {
    let mut board = board_mod::Board::default();
    let mut moves = vec![];

    for mv_san in ["c3", "e5", "c2", "d5", "c1", "c5", "d3", "a4", "e3"].iter() {
        let mv = board.move_from_san(&mv_san).unwrap();
        board.generate_moves(&mut moves);
        assert!(moves.contains(&mv));
        board.do_move(mv);
        moves.clear();
    }
    let mut tree = mcts::Tree::new_root();
    for _ in 0..10_000 {
        tree.select(&mut board.clone());
    }
    let (_score, mv) = tree.best_move();
    assert!(
        mv == board.move_from_san("b4").unwrap()
            || mv == board.move_from_san("b5").unwrap()
            || mv == board.move_from_san("Cb4").unwrap()
            || mv == board.move_from_san("Cb5").unwrap()
    )
}

use crate::board::{Board, Square, Piece};
use smallvec::SmallVec;

#[test]
fn default_board_test() {
    let board = Board::default();
    for square in (0..25).into_iter().map(|i| Square(i)) {
        assert!(board[square].is_empty());
    }
}
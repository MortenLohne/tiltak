use crate::board::{board_iterator, Board, Piece, Square, BOARD_SIZE};
use smallvec::SmallVec;

#[test]
fn default_board_test() {
    let board = Board::default();
    for square in board_iterator() {
        assert!(board[square].is_empty());
    }
}

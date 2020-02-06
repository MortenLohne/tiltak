use crate::board::{Board, Square, Piece};

#[test]
fn default_board_test() {
    let board = Board::default();
    for square in (0..25).into_iter().map(|i| Square(i)) {
        assert_eq!(board[square], [Piece::Empty; 10]);
    }
}
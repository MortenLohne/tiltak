//* Traits for white/black colors. Useful for writing functions that are generic over color,
//* like how the move generator is structured.

use board_game_traits::{Color, GameResult};

use crate::position::bitboard::BitBoard;
use crate::position::utils::Piece;
use crate::position::utils::Piece::{
    BlackCap, BlackFlat, BlackWall, WhiteCap, WhiteFlat, WhiteWall,
};
use crate::position::{GroupData, Position};

use super::Square;

pub(crate) trait ColorTr {
    fn color() -> Color;

    fn win() -> GameResult;

    fn stones_left<const S: usize>(position: &Position<S>) -> u8;

    fn caps_left<const S: usize>(position: &Position<S>) -> u8;

    fn road_stones<const S: usize>(group_data: &GroupData<S>) -> BitBoard;

    fn blocking_stones<const S: usize>(group_data: &GroupData<S>) -> BitBoard;

    fn flats<const S: usize>(group_data: &GroupData<S>) -> BitBoard;

    fn walls<const S: usize>(group_data: &GroupData<S>) -> BitBoard;

    fn caps<const S: usize>(group_data: &GroupData<S>) -> BitBoard;

    fn flat_piece() -> Piece;

    fn wall_piece() -> Piece;

    fn cap_piece() -> Piece;

    fn is_road_stone(piece: Piece) -> bool;

    fn piece_is_ours(piece: Piece) -> bool;

    fn is_critical_square<const S: usize>(group_data: &GroupData<S>, square: Square<S>) -> bool;

    fn critical_squares<const S: usize>(group_data: &GroupData<S>) -> BitBoard;
}

pub(crate) struct WhiteTr {}

impl ColorTr for WhiteTr {
    fn color() -> Color {
        Color::White
    }

    fn win() -> GameResult {
        GameResult::WhiteWin
    }

    fn stones_left<const S: usize>(position: &Position<S>) -> u8 {
        position.white_stones_left
    }

    fn caps_left<const S: usize>(position: &Position<S>) -> u8 {
        position.white_caps_left
    }

    fn road_stones<const S: usize>(group_data: &GroupData<S>) -> BitBoard {
        group_data.white_road_pieces()
    }

    fn blocking_stones<const S: usize>(group_data: &GroupData<S>) -> BitBoard {
        group_data.white_blocking_pieces()
    }

    fn flats<const S: usize>(group_data: &GroupData<S>) -> BitBoard {
        group_data.white_flat_stones
    }

    fn walls<const S: usize>(group_data: &GroupData<S>) -> BitBoard {
        group_data.white_walls
    }

    fn caps<const S: usize>(group_data: &GroupData<S>) -> BitBoard {
        group_data.white_caps
    }

    fn flat_piece() -> Piece {
        Piece::WhiteFlat
    }

    fn wall_piece() -> Piece {
        Piece::WhiteWall
    }

    fn cap_piece() -> Piece {
        Piece::WhiteCap
    }

    fn is_road_stone(piece: Piece) -> bool {
        piece == WhiteFlat || piece == WhiteCap
    }

    fn piece_is_ours(piece: Piece) -> bool {
        piece == WhiteFlat || piece == WhiteWall || piece == WhiteCap
    }

    fn is_critical_square<const S: usize>(group_data: &GroupData<S>, square: Square<S>) -> bool {
        group_data.white_critical_squares.get_square(square)
    }

    fn critical_squares<const S: usize>(group_data: &GroupData<S>) -> BitBoard {
        group_data.white_critical_squares
    }
}

pub(crate) struct BlackTr {}

impl ColorTr for BlackTr {
    fn color() -> Color {
        Color::Black
    }

    fn win() -> GameResult {
        GameResult::BlackWin
    }

    fn stones_left<const S: usize>(position: &Position<S>) -> u8 {
        position.black_stones_left
    }

    fn caps_left<const S: usize>(position: &Position<S>) -> u8 {
        position.black_caps_left
    }

    fn road_stones<const S: usize>(group_data: &GroupData<S>) -> BitBoard {
        group_data.black_road_pieces()
    }

    fn blocking_stones<const S: usize>(group_data: &GroupData<S>) -> BitBoard {
        group_data.black_blocking_pieces()
    }

    fn flats<const S: usize>(group_data: &GroupData<S>) -> BitBoard {
        group_data.black_flat_stones
    }

    fn walls<const S: usize>(group_data: &GroupData<S>) -> BitBoard {
        group_data.black_walls
    }

    fn caps<const S: usize>(group_data: &GroupData<S>) -> BitBoard {
        group_data.black_caps
    }

    fn flat_piece() -> Piece {
        Piece::BlackFlat
    }

    fn wall_piece() -> Piece {
        Piece::BlackWall
    }

    fn cap_piece() -> Piece {
        Piece::BlackCap
    }

    fn is_road_stone(piece: Piece) -> bool {
        piece == BlackFlat || piece == BlackCap
    }

    fn piece_is_ours(piece: Piece) -> bool {
        piece == BlackFlat || piece == BlackCap || piece == BlackWall
    }

    fn is_critical_square<const S: usize>(group_data: &GroupData<S>, square: Square<S>) -> bool {
        group_data.black_critical_squares.get_square(square)
    }

    fn critical_squares<const S: usize>(group_data: &GroupData<S>) -> BitBoard {
        group_data.black_critical_squares
    }
}

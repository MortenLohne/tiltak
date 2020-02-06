const BOARD_SIZE: usize = 5;

use board_game_traits::board;
use board_game_traits::board::{Color, GameResult};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Piece {
    Empty,
    WhiteFlat,
    BlackFlat,
    WhiteStanding,
    BlackStanding,
    WhiteCap,
    BlackCap,
}

impl Default for Piece {
    fn default() -> Self {
        Piece::Empty
    }
}

type Cell = [Piece; 10];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Board {
    cells: [[Cell; BOARD_SIZE]; BOARD_SIZE],
    to_move: Color,
    white_stones_left: u8,
    black_stones_left: u8,
    white_capstones_left: u8,
    black_capstones_left: u8,
}

impl Default for Board {
    fn default() -> Self {
        Board {
            cells: Default::default(),
            to_move: Color::White,
            white_stones_left: 21,
            black_stones_left: 21,
            white_capstones_left: 1,
            black_capstones_left: 1,
        }
    }
}

impl board::Board for Board {
    type Move = ();
    type ReverseMove = Self;

    fn start_board() -> Self {
        Self::default()
    }

    fn side_to_move(&self) -> Color {
        self.to_move
    }

    fn generate_moves(&self, moves: &mut Vec<Self::Move>) {
        unimplemented!()
    }

    fn do_move(&mut self, mv: Self::Move) -> Self::ReverseMove {
        unimplemented!()
    }

    fn reverse_move(&mut self, reverse_move: Self::ReverseMove) {
        *self = reverse_move
    }

    fn game_result(&self) -> Option<GameResult> {
        unimplemented!()
    }
}

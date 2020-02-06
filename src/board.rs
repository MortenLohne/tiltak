const BOARD_SIZE: usize = 5;

use board_game_traits::board;
use board_game_traits::board::{Color, GameResult};
use std::ops::Index;
use smallvec::SmallVec;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Square(pub u8);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Piece {
    WhiteFlat,
    BlackFlat,
    WhiteStanding,
    BlackStanding,
    WhiteCap,
    BlackCap,
}

type Cell = SmallVec<[Piece; 4]>;

pub enum Move {
    Place(Piece, Square),
    Move([Movement;5]),
}

pub struct Movement {
    pub pieces_to_leave: u8,
    pub dest_square: Square,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Board {
    cells: [[Cell; BOARD_SIZE]; BOARD_SIZE],
    to_move: Color,
    white_stones_left: u8,
    black_stones_left: u8,
    white_capstones_left: u8,
    black_capstones_left: u8,
}

impl Index<Square> for Board {
    type Output = Cell;

    fn index(&self, square: Square) -> &Self::Output {
        &self.cells[square.0 as usize % BOARD_SIZE][square.0 as usize / BOARD_SIZE]
    }
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

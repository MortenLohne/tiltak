pub const BOARD_SIZE: usize = 5;

use board_game_traits::board;
use board_game_traits::board::{Color, GameResult};
use smallvec::SmallVec;
use std::ops::{Index, IndexMut};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Square(pub u8);

pub fn board_iterator() -> impl Iterator<Item = Square> {
    (0..(BOARD_SIZE * BOARD_SIZE))
        .map(|i| Square(i as u8))
}

pub fn neighbours(square: Square) -> impl Iterator<Item = Square> {
    [-(BOARD_SIZE as i8), -1, 1, BOARD_SIZE as i8]
        .iter()
        .map(move |sq| sq + square.0 as i8)
        .filter(|&sq| sq >= 0 && sq < BOARD_SIZE as i8 * BOARD_SIZE as i8)
        .map(|sq| Square(sq as u8))
}

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

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Move {
    Place(Piece, Square),
    Move(Square, SmallVec<[Movement; 5]>),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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

impl IndexMut<Square> for Board {
    fn index_mut(&mut self, square: Square) -> &mut Self::Output {
        &mut self.cells[square.0 as usize % BOARD_SIZE][square.0 as usize / BOARD_SIZE]
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
    type Move = Move;
    type ReverseMove = Self;

    fn start_board() -> Self {
        Self::default()
    }

    fn side_to_move(&self) -> Color {
        self.to_move
    }

    fn generate_moves(&self, moves: &mut Vec<Self::Move>) {
        for square in board_iterator() {
            match self[square].last() {
                None => match self.to_move {
                    Color::White => {
                        if self.white_stones_left > 0 {
                            moves.push(Move::Place(Piece::WhiteFlat, square));
                            moves.push(Move::Place(Piece::WhiteStanding, square));
                        }
                        if self.white_capstones_left > 0 {
                            moves.push(Move::Place(Piece::WhiteCap, square));
                        }
                    }
                    Color::Black => {
                        if self.black_stones_left > 0 {
                            moves.push(Move::Place(Piece::BlackFlat, square));
                            moves.push(Move::Place(Piece::BlackStanding, square));
                        }
                        if self.black_capstones_left > 0 {
                            moves.push(Move::Place(Piece::BlackCap, square));
                        }
                    }
                },
                Some(&piece) => {
                    if self.to_move == Color::White && piece == Piece::WhiteCap {
                        for neighbour in neighbours(square) {
                            let mut vec = SmallVec::new();
                            vec.push(Movement {
                                pieces_to_leave: 0,
                                dest_square: neighbour,
                            });
                            moves.push(Move::Move(square, vec));
                        }
                    }
                }
            }
        }
    }

    fn do_move(&mut self, mv: Self::Move) -> Self::ReverseMove {
        let reverse_move = self.clone();
        match mv {
            Move::Place(piece, to) => self[to].push(piece),
            Move::Move(mut from, movements) => {
                self[from].truncate(movements[0].pieces_to_leave as usize);
                for Movement { pieces_to_leave, dest_square } in movements {
                    self[dest_square] = self[from].clone();
                    for _ in 0..pieces_to_leave {
                        let piece = self[dest_square].remove(0);
                        self[from].push(piece);
                    }
                    from = dest_square;
                }
            },
        }
        self.to_move = !self.to_move;
        reverse_move
    }

    fn reverse_move(&mut self, reverse_move: Self::ReverseMove) {
        *self = reverse_move
    }

    fn game_result(&self) -> Option<GameResult> {
        unimplemented!()
    }
}

use std::convert::TryFrom;
use std::fmt::{self, Write};
use std::iter::FromIterator;
use std::ops;
use std::ops::{Index, IndexMut};
use std::str::FromStr;

use board_game_traits::{Color, GameResult};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::position::bitboard::BitBoard;
use crate::position::color_trait::{BlackTr, ColorTr, WhiteTr};
use crate::position::utils::Direction::*;
use crate::position::Piece::{BlackCap, BlackFlat, BlackWall, WhiteCap, WhiteFlat, WhiteWall};
use crate::position::Role::{Cap, Flat, Wall};

/// A location on the board. Can be used to index a `Board`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Square(pub u8);

impl Square {
    pub fn from_rank_file<const S: usize>(rank: u8, file: u8) -> Self {
        debug_assert!(rank < S as u8 && file < S as u8);
        Square(rank * S as u8 + file as u8)
    }

    pub fn rank<const S: usize>(self) -> u8 {
        self.0 / S as u8
    }

    pub fn file<const S: usize>(self) -> u8 {
        self.0 % S as u8
    }

    pub fn neighbours<const S: usize>(self) -> impl Iterator<Item = Square> {
        (if self.0 as usize == 0 {
            [1, S as i8].iter()
        } else if self.0 as usize == S - 1 {
            [-1, S as i8].iter()
        } else if self.0 as usize == S * S - S {
            [1, -(S as i8)].iter()
        } else if self.0 as usize == S * S - 1 {
            [-1, -(S as i8)].iter()
        } else if self.rank::<S>() == 0 {
            [-1, 1, S as i8].iter()
        } else if self.rank::<S>() == S as u8 - 1 {
            [-(S as i8), -1, 1].iter()
        } else if self.file::<S>() == 0 {
            [-(S as i8), 1, S as i8].iter()
        } else if self.file::<S>() == S as u8 - 1 {
            [-(S as i8), -1, S as i8].iter()
        } else {
            [-(S as i8), -1, 1, S as i8].iter()
        })
        .cloned()
        .map(move |sq| sq + self.0 as i8)
        .map(|sq| Square(sq as u8))
    }

    pub fn directions<const S: usize>(self) -> impl Iterator<Item = Direction> {
        (if self.0 as usize == 0 {
            [East, South].iter()
        } else if self.0 as usize == S - 1 {
            [West, South].iter()
        } else if self.0 as usize == S * S - S {
            [East, North].iter()
        } else if self.0 as usize == S * S - 1 {
            [West, North].iter()
        } else if self.rank::<S>() == 0 {
            [West, East, South].iter()
        } else if self.rank::<S>() == S as u8 - 1 {
            [North, West, East].iter()
        } else if self.file::<S>() == 0 {
            [North, East, South].iter()
        } else if self.file::<S>() == S as u8 - 1 {
            [North, West, South].iter()
        } else {
            [North, West, East, South].iter()
        })
        .cloned()
    }

    pub fn go_direction<const S: usize>(self, direction: Direction) -> Option<Self> {
        self.jump_direction::<S>(direction, 1)
    }

    pub fn jump_direction<const S: usize>(self, direction: Direction, len: u8) -> Option<Self> {
        match direction {
            North => self.0.checked_sub((S as u8) * len).map(Square),
            West => {
                if self.file::<S>() < len {
                    None
                } else {
                    Some(Square(self.0 - len))
                }
            }
            East => {
                if self.file::<S>() >= S as u8 - len {
                    None
                } else {
                    Some(Square(self.0 + len))
                }
            }
            South => {
                if self.0 + (S as u8) * len >= (S * S) as u8 {
                    None
                } else {
                    Some(Square(self.0 + len * S as u8))
                }
            }
        }
    }

    pub fn parse_square<const S: usize>(input: &str) -> Result<Square, pgn_traits::Error> {
        if input.len() != 2 {
            return Err(pgn_traits::Error::new_parse_error(format!(
                "Couldn't parse square \"{}\"",
                input
            )));
        }
        let mut chars = input.chars();
        let file = (chars.next().unwrap() as u8).overflowing_sub(b'a').0;
        let rank = (S as u8 + b'0')
            .overflowing_sub(chars.next().unwrap() as u8)
            .0;
        if file >= S as u8 || rank >= S as u8 {
            Err(pgn_traits::Error::new_parse_error(format!(
                "Couldn't parse square \"{}\" at size {}",
                input, S
            )))
        } else {
            Ok(Square(file + rank * S as u8))
        }
    }

    pub fn to_string<const S: usize>(self) -> String {
        let mut string = String::new();
        write!(string, "{}", (self.file::<S>() + b'a') as char).unwrap();
        write!(string, "{}", S as u8 - self.rank::<S>()).unwrap();
        string
    }
}

/// Iterates over all board squares.
pub fn squares_iterator<const S: usize>() -> impl Iterator<Item = Square> {
    (0..(S * S)).map(|i| Square(i as u8))
}

#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Komi {
    half_komi: i8,
}

impl Komi {
    pub fn game_result_with_flatcounts(self, white_flats: i8, black_flats: i8) -> GameResult {
        match (2 * (white_flats - black_flats) - self.half_komi).signum() {
            -1 => GameResult::BlackWin,
            0 => GameResult::Draw,
            1 => GameResult::WhiteWin,
            _ => unreachable!(),
        }
    }
}

impl TryFrom<f64> for Komi {
    type Error = String;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        // Match against a list of floats literals to convert, 
        // to avoid any float math shenanigans
        if let Some((_, half_komi)) = [
            -5.0, -4.5, -4.0, -3.5, -3.0, -2.5, -2.0, -1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0,
            2.5, 3.0, 3.5, 4.0, 4.5, 5.0,
        ]
        .iter()
        .zip(-10..=10)
        .find(|(komi, _)| **komi == value)
        {
            Ok(Komi { half_komi })
        } else {
            Err(format!("Invalid komi {}", value))
        }
    }
}

impl TryFrom<f32> for Komi {
    type Error = String;

    fn try_from(value: f32) -> Result<Self, Self::Error> {
        Self::try_from(value as f64)
    }
}

impl TryFrom<&str> for Komi {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        f64::from_str(value)
            .map_err(|err| err.to_string())
            .and_then(Self::try_from)
    }
}

impl fmt::Display for Komi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        ((self.half_komi as f64) / 2.0).fmt(f)
    }
}

/// One of the 3 piece roles in Tak. The same as piece, but without different variants for each color.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Role {
    Flat,
    Wall,
    Cap,
}

impl Role {
    pub fn disc(self) -> usize {
        self as u16 as usize
    }
}

/// One of the 6 game pieces in Tak. Each piece has one variant for each color.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Piece {
    WhiteFlat = 0,
    BlackFlat = 1,
    WhiteWall = 2,
    BlackWall = 3,
    WhiteCap = 4,
    BlackCap = 5,
}

impl Piece {
    pub fn from_role_color(role: Role, color: Color) -> Self {
        match (role, color) {
            (Flat, Color::White) => WhiteFlat,
            (Wall, Color::White) => WhiteWall,
            (Cap, Color::White) => WhiteCap,
            (Flat, Color::Black) => BlackFlat,
            (Wall, Color::Black) => BlackWall,
            (Cap, Color::Black) => BlackCap,
        }
    }

    pub fn role(self) -> Role {
        match self {
            WhiteFlat | BlackFlat => Flat,
            WhiteWall | BlackWall => Wall,
            WhiteCap | BlackCap => Cap,
        }
    }

    pub fn color(self) -> Color {
        match self {
            WhiteFlat | WhiteWall | WhiteCap => Color::White,
            BlackFlat | BlackWall | BlackCap => Color::Black,
        }
    }

    pub fn is_road_piece(self) -> bool {
        WhiteTr::is_road_stone(self) || BlackTr::is_road_stone(self)
    }

    pub fn flip_color(self) -> Self {
        match self {
            WhiteFlat => BlackFlat,
            BlackFlat => WhiteFlat,
            WhiteWall => BlackWall,
            BlackWall => WhiteWall,
            WhiteCap => BlackCap,
            BlackCap => WhiteCap,
        }
    }
}

impl ops::Not for Piece {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            WhiteFlat => BlackFlat,
            BlackFlat => WhiteFlat,
            WhiteWall => BlackWall,
            BlackWall => WhiteWall,
            WhiteCap => BlackCap,
            BlackCap => WhiteCap,
        }
    }
}

/// The contents of a square on the board, consisting of zero or more pieces
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Stack {
    pub(crate) top_stone: Option<Piece>,
    pub(crate) bitboard: BitBoard,
    pub(crate) height: u8,
}

impl Stack {
    /// Get a piece by index. 0 is the bottom of the stack
    pub fn get(&self, i: u8) -> Option<Piece> {
        if i >= self.height {
            None
        } else if i == self.height - 1 {
            self.top_stone
        } else if self.bitboard.get(i) {
            Some(WhiteFlat)
        } else {
            Some(BlackFlat)
        }
    }

    pub fn top_stone(&self) -> Option<Piece> {
        self.top_stone
    }

    /// Push a new piece to the top of the stack
    ///
    /// Any piece already on the stack will be flattened, including capstones
    pub fn push(&mut self, piece: Piece) {
        if self.height > 0 && self.top_stone.unwrap().color() == Color::White {
            self.bitboard = self.bitboard.set(self.height - 1);
        }
        self.top_stone = Some(piece);
        self.height += 1;
    }

    /// Remove the top piece from the stack, a
    ///
    /// Will not un-flatten a previously flattened stone
    pub fn pop(&mut self) -> Option<Piece> {
        debug_assert_ne!(self.height, 0);
        let old_piece = self.top_stone;
        if self.height > 1 {
            let piece = if self.bitboard.get(self.height - 2) {
                Piece::WhiteFlat
            } else {
                Piece::BlackFlat
            };
            self.bitboard = self.bitboard.clear(self.height - 2);
            self.top_stone = Some(piece);
        } else {
            self.top_stone = None;
        }
        self.height -= 1;
        old_piece
    }

    pub fn replace_top(&mut self, piece: Piece) -> Option<Piece> {
        self.top_stone.replace(piece)
    }

    pub fn remove(&mut self, i: u8) -> Piece {
        if i == self.height - 1 {
            self.pop().expect("Tried to remove from empty stack")
        } else {
            let piece = if self.bitboard.get(i) {
                Piece::WhiteFlat
            } else {
                Piece::BlackFlat
            };
            let pieces_below = self.bitboard & BitBoard::lower_n_bits(i);
            let pieces_above = self.bitboard & !BitBoard::lower_n_bits(i + 1);
            self.bitboard = pieces_below
                | BitBoard {
                    board: pieces_above.board >> 1,
                };
            self.height -= 1;
            piece
        }
    }

    pub fn is_empty(&self) -> bool {
        self.height == 0
    }

    pub fn len(&self) -> u8 {
        self.height
    }
}

/// An iterator over the pieces in a stack, from the bottom up
pub struct StackIterator {
    stack: Stack,
}

impl Iterator for StackIterator {
    type Item = Piece;

    fn next(&mut self) -> Option<Self::Item> {
        if self.stack.is_empty() {
            None
        } else {
            Some(self.stack.remove(0))
        }
    }
}

impl IntoIterator for Stack {
    type Item = Piece;
    type IntoIter = StackIterator;

    fn into_iter(self) -> Self::IntoIter {
        StackIterator { stack: self }
    }
}

/// One of the four cardinal directions on the board
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Direction {
    North,
    West,
    East,
    South,
}

impl Direction {
    pub(crate) fn reverse(self) -> Direction {
        match self {
            North => South,
            West => East,
            East => West,
            South => North,
        }
    }

    pub(crate) fn parse(ch: char) -> Option<Self> {
        match ch {
            '+' => Some(North),
            '<' => Some(West),
            '>' => Some(East),
            '-' => Some(South),
            _ => None,
        }
    }
}

/// One or more `Movement`s, storing how many pieces are dropped off at each step
#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct StackMovement {
    // The first 4 bits is the number of squares moved
    // The remaining 28 bits are the number of pieces taken, 4 bits per number
    data: u32,
}

impl StackMovement {
    pub fn new() -> Self {
        StackMovement { data: 0 }
    }

    pub fn get(self, index: u8) -> Movement {
        assert!(index < self.len() as u8);
        let movement_in_place = self.data & 0b1111 << (index * 4);
        Movement {
            pieces_to_take: (movement_in_place >> (index * 4)) as u8,
        }
    }

    pub fn push(&mut self, movement: Movement) {
        let length = self.len() as u32;
        debug_assert!(
            length < 7,
            "Stack movement cannot grow any more: {:#b}",
            self.data
        );
        debug_assert!(movement.pieces_to_take < 8);
        self.data |= (movement.pieces_to_take as u32) << (length * 4);
        self.data &= (1_u32 << 28).overflowing_sub(1).0;
        self.data |= (length + 1) << 28;
    }

    pub fn len(self) -> usize {
        (self.data >> (28_u32)) as usize
    }

    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
}

impl FromIterator<Movement> for StackMovement {
    fn from_iter<T: IntoIterator<Item = Movement>>(iter: T) -> Self {
        let mut result = StackMovement::new();
        for movement in iter {
            result.push(movement)
        }
        result
    }
}

impl IntoIterator for StackMovement {
    type Item = Movement;
    type IntoIter = StackMovementIterator;

    fn into_iter(self) -> Self::IntoIter {
        StackMovementIterator {
            num_left: self.len() as u8,
            _movements: self.data,
        }
    }
}

pub struct StackMovementIterator {
    num_left: u8,
    _movements: u32,
}

impl Iterator for StackMovementIterator {
    type Item = Movement;

    fn next(&mut self) -> Option<Self::Item> {
        if self.num_left == 0 {
            None
        } else {
            self.num_left -= 1;
            let result = self._movements & 0b1111;
            self._movements >>= 4;
            Some(Movement {
                pieces_to_take: result as u8,
            })
        }
    }
}

/// Moving a stack of pieces consists of one or more `Movement`s
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Movement {
    pub pieces_to_take: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct AbstractBoard<T, const S: usize> {
    pub(crate) raw: [[T; S]; S],
}

impl<T: Default + Copy, const S: usize> Default for AbstractBoard<T, S> {
    fn default() -> Self {
        AbstractBoard {
            raw: [[T::default(); S]; S],
        }
    }
}

impl<T, const S: usize> Index<Square> for AbstractBoard<T, S> {
    type Output = T;

    fn index(&self, square: Square) -> &Self::Output {
        &self.raw[square.0 as usize % S][square.0 as usize / S]
    }
}

impl<T, const S: usize> IndexMut<Square> for AbstractBoard<T, S> {
    fn index_mut(&mut self, square: Square) -> &mut Self::Output {
        &mut self.raw[square.0 as usize % S][square.0 as usize / S]
    }
}

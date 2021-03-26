use std::fmt::Write;
use std::ops;

use board_game_traits::Color;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::position::color_trait::{BlackTr, ColorTr, WhiteTr};
use crate::position::Direction;
use crate::position::Direction::{East, North, South, West};
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
        match direction {
            North => self.0.checked_sub(S as u8).map(Square),
            West => {
                if self.file::<S>() == 0 {
                    None
                } else {
                    Some(Square(self.0 - 1))
                }
            }
            East => {
                if self.file::<S>() == S as u8 - 1 {
                    None
                } else {
                    Some(Square(self.0 + 1))
                }
            }
            South => {
                if self.0 as usize + S >= S * S {
                    None
                } else {
                    Some(Square(self.0 + S as u8))
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

    pub fn to_string<const S: usize>(&self) -> String {
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

/// One of the 3 piece roles in Tak. The same as piece, but without different variants for each color.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Role {
    Flat,
    Wall,
    Cap,
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

use std::fmt;
use std::mem;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use super::Direction::{self, *};

/// A location on the board. Can be used to index a `Board`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Square<const S: usize> {
    inner: u8,
}

impl<const S: usize> Square<S> {
    pub const fn from_u8(inner: u8) -> Self {
        assert!((inner as usize) < S * S);
        Square { inner }
    }

    /// # Safety `inner` must be a valid square for the board size, i.e. less than S * S
    pub const unsafe fn from_u8_unchecked(inner: u8) -> Self {
        debug_assert!((inner as usize) < S * S);
        Square { inner }
    }

    pub const fn into_inner(self) -> u8 {
        self.inner
    }

    pub const fn corners() -> [Self; 4] {
        [
            Self::from_u8(0),
            Self::from_u8(S as u8 - 1),
            Self::from_u8((S * (S - 1)) as u8),
            Self::from_u8((S * S - 1) as u8),
        ]
    }

    pub const fn from_rank_file(rank: u8, file: u8) -> Self {
        assert!(rank < S as u8 && file < S as u8);
        Square::from_u8(file * S as u8 + rank)
    }

    pub const fn rank(self) -> u8 {
        self.inner % S as u8
    }

    pub const fn file(self) -> u8 {
        self.inner / S as u8
    }

    pub fn downcast_size<const N: usize>(self) -> Square<N> {
        if S == N {
            unsafe { mem::transmute(self) }
        } else {
            panic!("Tried to use {}s square as {}s square", S, N)
        }
    }

    pub const fn cache_data(self) -> SquareCacheEntry<S> {
        let mut neighbors = SquareCacheEntry {
            neighbor_squares: [Square::from_u8(0); 4],
            directions: [None; 4],
            go_direction: [self; 4],
        };

        let mut i = 0;
        if let Some(neighbor) = self.go_direction_const(North) {
            neighbors.neighbor_squares[i] = neighbor;
            neighbors.directions[i] = Some(North);
            neighbors.go_direction[North as u8 as usize] = neighbor;
            i += 1;
        }
        if let Some(neighbor) = self.go_direction_const(West) {
            neighbors.neighbor_squares[i] = neighbor;
            neighbors.directions[i] = Some(West);
            neighbors.go_direction[West as u8 as usize] = neighbor;
            i += 1;
        }
        if let Some(neighbor) = self.go_direction_const(East) {
            neighbors.neighbor_squares[i] = neighbor;
            neighbors.directions[i] = Some(East);
            neighbors.go_direction[East as u8 as usize] = neighbor;
            i += 1;
        }
        if let Some(neighbor) = self.go_direction_const(South) {
            neighbors.neighbor_squares[i] = neighbor;
            neighbors.directions[i] = Some(South);
            neighbors.go_direction[South as u8 as usize] = neighbor;
        }

        neighbors
    }

    pub const fn go_direction_const(self, direction: Direction) -> Option<Self> {
        self.jump_direction(direction, 1)
    }

    pub const fn jump_direction(self, direction: Direction, len: u8) -> Option<Self> {
        let rank = self.rank();
        let file = self.file();
        match direction {
            North => {
                if let Some(new_rank) = rank.checked_sub(len) {
                    Some(Square::from_rank_file(new_rank, file))
                } else {
                    None
                }
            }
            West => {
                if file < len {
                    None
                } else {
                    Some(Square::from_rank_file(rank, file - len))
                }
            }
            East => {
                if file >= S as u8 - len {
                    None
                } else {
                    Some(Square::from_rank_file(rank, file + len))
                }
            }
            South => {
                if rank + len >= S as u8 {
                    None
                } else {
                    Some(Square::from_rank_file(rank + len, file))
                }
            }
        }
    }

    /// Do a known valid jump. If the jump is not valid, the function either returns an arbitrary square, or panics
    pub const fn jump_valid_direction(self, direction: Direction, len: u8) -> Self {
        match direction {
            North => Square::from_u8(self.inner - len),
            West => Square::from_u8(self.inner - len * S as u8),
            East => Square::from_u8(self.inner + len * S as u8),
            South => Square::from_u8(self.inner + len),
        }
    }

    pub fn parse_square(input: &str) -> Result<Square<S>, pgn_traits::Error> {
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
            Ok(Square::from_rank_file(rank, file))
        }
    }
}

impl<const S: usize> fmt::Display for Square<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", (self.file() + b'a') as char)?;
        write!(f, "{}", S as u8 - self.rank())
    }
}

/// Iterates over all board squares.
pub fn squares_iterator<const S: usize>() -> impl Iterator<Item = Square<S>> {
    // Safety: `i` must be smaller than `S * S`, which is trivially true here
    (0..(S * S)).map(|i| unsafe { Square::from_u8_unchecked(i as u8) })
}

#[derive(Clone, Copy, Debug)]
#[repr(align(4))]
pub struct SquareCacheEntry<const S: usize> {
    neighbor_squares: [Square<S>; 4],
    directions: [Option<Direction>; 4], // Safety: The first two elements are never `None`
    go_direction: [Square<S>; 4],
}

impl<const S: usize> SquareCacheEntry<S> {
    pub const fn empty() -> Self {
        SquareCacheEntry {
            neighbor_squares: [Square::from_u8(0); 4],
            directions: [Some(North), Some(North), None, None],
            go_direction: [Square::from_u8(0); 4],
        }
    }

    pub fn go_direction(
        &self,
        origin_square: Square<S>,
        direction: Direction,
    ) -> Option<Square<S>> {
        let square = self.go_direction[direction as u8 as usize];
        if square == origin_square {
            None
        } else {
            Some(square)
        }
    }

    pub fn downcast_size<const N: usize>(self) -> SquareCacheEntry<N> {
        if S == N {
            unsafe { mem::transmute(self) }
        } else {
            panic!(
                "Tried to use {}s neighbor array as {}s neighbor array",
                S, N
            )
        }
    }
}

impl<const S: usize> IntoIterator for SquareCacheEntry<S> {
    type Item = (Direction, Square<S>);

    type IntoIter = SquareCacheEntryIter<S>;

    fn into_iter(self) -> Self::IntoIter {
        SquareCacheEntryIter {
            neighbor_squares: self.neighbor_squares,
            directions: self.directions,
            position: 0,
            len: if self.directions[2].is_none() {
                2
            } else if self.directions[3].is_none() {
                3
            } else {
                4
            },
        }
    }
}

pub struct SquareCacheEntryIter<const S: usize> {
    directions: [Option<Direction>; 4],
    neighbor_squares: [Square<S>; 4],
    position: usize,
    len: usize,
}

impl<const S: usize> Iterator for SquareCacheEntryIter<S> {
    type Item = (Direction, Square<S>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.position < self.len {
            let output = Some((
                // Safety: Directions are only `None` outside the length
                unsafe { self.directions[self.position].unwrap_unchecked() },
                self.neighbor_squares[self.position],
            ));
            self.position += 1;
            output
        } else {
            None
        }
    }
}

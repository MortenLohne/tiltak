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

    pub const fn neighbors_array(self) -> ConstNeighborArray<S> {
        let mut neighbors = ConstNeighborArray::empty();

        if let Some(north) = self.go_direction(North) {
            neighbors = neighbors.push(north);
        }
        if let Some(west) = self.go_direction(West) {
            neighbors = neighbors.push(west);
        }
        if let Some(east) = self.go_direction(East) {
            neighbors = neighbors.push(east);
        }
        if let Some(south) = self.go_direction(South) {
            neighbors = neighbors.push(south);
        }

        neighbors
    }

    pub fn directions(self) -> impl Iterator<Item = Direction> {
        (if self.rank() == 0 && self.file() == 0 {
            [East, South].iter()
        } else if self.rank() == 0 && self.file() == S as u8 - 1 {
            [West, South].iter()
        } else if self.rank() == S as u8 - 1 && self.file() == 0 {
            [East, North].iter()
        } else if self.rank() == S as u8 - 1 && self.file() == S as u8 - 1 {
            [West, North].iter()
        } else if self.rank() == 0 {
            [West, East, South].iter()
        } else if self.rank() == S as u8 - 1 {
            [North, West, East].iter()
        } else if self.file() == 0 {
            [North, East, South].iter()
        } else if self.file() == S as u8 - 1 {
            [North, West, South].iter()
        } else {
            [North, West, East, South].iter()
        })
        .cloned()
    }

    pub const fn go_direction(self, direction: Direction) -> Option<Self> {
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
pub struct ConstNeighborArray<const S: usize> {
    len: u8,
    array: [Square<S>; 4],
}

impl<const S: usize> ConstNeighborArray<S> {
    pub const fn empty() -> Self {
        Self {
            len: 0,
            array: [Square::from_u8(0); 4],
        }
    }

    #[must_use]
    pub const fn push(mut self, square: Square<S>) -> Self {
        self.array[self.len as usize] = square;
        self.len += 1;
        self
    }

    pub fn downcast_size<const N: usize>(self) -> ConstNeighborArray<N> {
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

impl<const S: usize> IntoIterator for ConstNeighborArray<S> {
    type Item = Square<S>;

    type IntoIter = ConstNeighborArrayIter<S>;

    fn into_iter(self) -> Self::IntoIter {
        ConstNeighborArrayIter {
            len: self.len,
            array: self.array,
            position: 0,
        }
    }
}

pub struct ConstNeighborArrayIter<const S: usize> {
    len: u8,
    array: [Square<S>; 4],
    position: u8,
}

impl<const S: usize> Iterator for ConstNeighborArrayIter<S> {
    type Item = Square<S>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.position == self.len {
            None
        } else {
            self.position += 1;
            Some(self.array[self.position as usize - 1])
        }
    }
}

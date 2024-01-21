#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::{fmt, ops};

use super::{Direction, Square};

#[derive(PartialEq, Eq, Clone, Copy, Hash, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub(crate) struct BitBoard {
    pub board: u64,
}

impl ops::BitOr for BitBoard {
    type Output = BitBoard;
    #[inline]
    fn bitor(self, rhs: BitBoard) -> BitBoard {
        BitBoard::from_u64(self.board | rhs.board)
    }
}

impl ops::BitOrAssign for BitBoard {
    #[inline]
    fn bitor_assign(&mut self, rhs: BitBoard) {
        self.board |= rhs.board
    }
}

impl ops::BitAnd for BitBoard {
    type Output = BitBoard;
    #[inline]
    fn bitand(self, rhs: BitBoard) -> BitBoard {
        BitBoard::from_u64(self.board & rhs.board)
    }
}

impl ops::BitAndAssign for BitBoard {
    #[inline]
    fn bitand_assign(&mut self, rhs: BitBoard) {
        self.board &= rhs.board
    }
}

impl ops::Not for BitBoard {
    type Output = BitBoard;
    #[inline]
    fn not(self) -> BitBoard {
        BitBoard::from_u64(!self.board)
    }
}

impl BitBoard {
    #[inline]
    pub const fn empty() -> Self {
        BitBoard { board: 0 }
    }
    #[inline]
    pub const fn full() -> Self {
        BitBoard {
            board: u64::max_value(),
        }
    }

    pub fn lines_for_square<const S: usize>(square: Square<S>) -> [Self; 2] {
        [
            Self::full().rank::<S>(square.rank()),
            Self::full().file::<S>(square.file()),
        ]
    }

    #[inline]
    pub fn lower_n_bits(n: u8) -> Self {
        if n >= 64 {
            Self::full()
        } else {
            BitBoard {
                board: (1 << n as u64) - 1,
            }
        }
    }

    #[inline]
    pub const fn from_u64(n: u64) -> Self {
        BitBoard { board: n }
    }

    #[inline]
    pub fn get(self, i: u8) -> bool {
        debug_assert!(i < 64);
        self.board & (1 << i) != 0
    }

    #[inline]
    pub fn get_square<const S: usize>(self, square: Square<S>) -> bool {
        debug_assert!(square.into_inner() < 64);
        self.board & (1 << square.into_inner()) != 0
    }

    // Sets the square to true
    #[inline]
    #[must_use]
    pub const fn set(self, i: u8) -> Self {
        debug_assert!(i < 64);
        BitBoard::from_u64(self.board | 1 << i)
    }

    // Sets the square to true
    #[inline]
    #[must_use]
    pub const fn set_square<const S: usize>(self, square: Square<S>) -> Self {
        debug_assert!(square.into_inner() < 64);
        BitBoard::from_u64(self.board | 1 << square.into_inner())
    }

    // Sets the square to false
    #[inline]
    #[must_use]
    pub fn clear(self, i: u8) -> Self {
        debug_assert!(i < 64);
        BitBoard::from_u64(self.board & !(1 << i))
    }

    // Sets the square to false
    #[inline]
    #[must_use]
    pub fn clear_square<const S: usize>(self, square: Square<S>) -> Self {
        debug_assert!(square.into_inner() < 64);
        BitBoard::from_u64(self.board & !(1 << square.into_inner()))
    }

    #[inline]
    pub fn file<const S: usize>(self, i: u8) -> Self {
        debug_assert!(i < S as u8);
        let mask = (1 << S) - 1;
        BitBoard::from_u64(self.board & (mask << (i as u64 * S as u64)))
    }

    #[inline]
    pub fn rank<const S: usize>(self, i: u8) -> Self {
        debug_assert!(i < S as u8);
        #[allow(clippy::unusual_byte_groupings)]
        let mask = match S {
            1 => 0b1,
            2 => 0b0101,
            3 => 0b1_001_001,
            4 => 0b1_0001_0001_0001,
            5 => 0b1_00001_00001_00001_00001,
            6 => 0b1_000001_000001_000001_000001_000001,
            7 => 0b1_0000001_0000001_0000001_0000001_0000001_0000001,
            8 => 0b1_00000001_00000001_00000001_00000001_00000001_00000001_00000001,
            _ => 0,
        };
        BitBoard::from_u64(self.board & (mask << i as u64))
    }

    #[inline]
    pub fn is_empty(self) -> bool {
        self.board == 0
    }

    #[inline]
    pub fn count(self) -> u8 {
        self.board.count_ones() as u8
    }

    // Get a bitboard with only the square's neighbors set
    // Note: The implementation is somewhat convoluted in order to work in const contexts.
    pub const fn neighbors<const S: usize>(square: Square<S>) -> BitBoard {
        let mut board = BitBoard::empty();
        if let Some(north) = square.go_direction_const(Direction::North) {
            board = board.set_square(north);
        }
        if let Some(west) = square.go_direction_const(Direction::West) {
            board = board.set_square(west);
        }
        if let Some(east) = square.go_direction_const(Direction::East) {
            board = board.set_square(east);
        }
        if let Some(south) = square.go_direction_const(Direction::South) {
            board = board.set_square(south);
        }
        board
    }

    #[inline]
    pub fn occupied_square<const S: usize>(self) -> Option<Square<S>> {
        if self.is_empty() {
            None
        } else {
            Some(Square::from_u8(self.board.trailing_zeros() as u8))
        }
    }

    pub fn into_iter<const S: usize>(self) -> BitBoardIter<S> {
        BitBoardIter::new(self)
    }
}

pub struct BitBoardIter<const S: usize> {
    board: BitBoard,
}

impl<const S: usize> BitBoardIter<S> {
    fn new(board: BitBoard) -> Self {
        BitBoardIter { board }
    }
}

impl<const S: usize> Iterator for BitBoardIter<S> {
    type Item = Square<S>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.board.is_empty() {
            None
        } else {
            let i = self.board.board.trailing_zeros() as u8;
            self.board = self.board.clear(i);
            Some(Square::from_u8(i))
        }
    }
}

impl fmt::Debug for BitBoard {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for n in 0..8 {
            writeln!(f, "{:08b}", (self.board >> (n * 8)) as u8).unwrap();
        }
        Ok(())
    }
}

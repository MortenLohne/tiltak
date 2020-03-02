use std::{fmt, ops};
use crate::board::BOARD_SIZE;

#[derive(PartialEq, Eq, Clone, Copy, Hash, Default)]
pub struct BitBoard {
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

    #[inline]
    pub fn lower_n_bits(n: u8) -> Self {
        if n >= 32 {
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
        debug_assert!(i < BOARD_SIZE as u8 * BOARD_SIZE as u8);
        self.board & (1 << i) != 0
    }
    // Sets the square to true
    #[inline]
    pub fn set(self, i: u8) -> Self {
        debug_assert!(i < BOARD_SIZE as u8 * BOARD_SIZE as u8);
        BitBoard::from_u64(self.board | 1 << i)
    }

    // Sets the square to false
    #[inline]
    pub fn clear(self, i: u8) -> Self {
        debug_assert!(i < BOARD_SIZE as u8 * BOARD_SIZE as u8);
        BitBoard::from_u64(self.board & !(1 << i))
    }

    #[inline]
    pub const fn is_empty(self) -> bool {
        self.board == 0
    }

    #[inline]
    pub const fn popcount(self) -> u32 {
        self.board.count_ones()
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

//! Tak move generation, along with all required data types.

use std::cmp::Ordering;
use std::fmt::Write;
use std::hash::{Hash, Hasher};
use std::iter::FromIterator;
use std::mem;
use std::ops::{Index, IndexMut};
use std::{fmt, ops};

use board_game_traits::GameResult::{BlackWin, Draw, WhiteWin};
use board_game_traits::{Color, GameResult};
use board_game_traits::{EvalPosition as EvalPositionTrait, Position as PositionTrait};
use lazy_static::lazy_static;
use rand::{Rng, SeedableRng};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use bitboard::BitBoard;
use color_trait::{BlackTr, WhiteTr};
use utils::Piece::*;
use utils::Role::Flat;
use utils::Role::*;
use utils::{Piece, Role, Square};

use crate::evaluation::parameters::{
    POLICY_PARAMS_4S, POLICY_PARAMS_5S, POLICY_PARAMS_6S, VALUE_PARAMS_4S, VALUE_PARAMS_5S,
    VALUE_PARAMS_6S,
};
use crate::evaluation::{policy_eval, value_eval};
use crate::position::color_trait::ColorTr;
use crate::position::Direction::*;
use crate::search;

pub(crate) mod bitboard;
pub(crate) mod color_trait;
pub mod utils;

lazy_static! {
    pub(crate) static ref ZOBRIST_KEYS_4S: Box<ZobristKeys<4>> = ZobristKeys::new();
    pub(crate) static ref ZOBRIST_KEYS_5S: Box<ZobristKeys<5>> = ZobristKeys::new();
    pub(crate) static ref ZOBRIST_KEYS_6S: Box<ZobristKeys<6>> = ZobristKeys::new();
    pub(crate) static ref ZOBRIST_KEYS_7S: Box<ZobristKeys<7>> = ZobristKeys::new();
    pub(crate) static ref ZOBRIST_KEYS_8S: Box<ZobristKeys<8>> = ZobristKeys::new();
}

pub const MAX_BOARD_SIZE: usize = 8;

pub const fn starting_stones<const S: usize>() -> u8 {
    match S {
        3 => 10,
        4 => 16,
        5 => 21,
        6 => 30,
        7 => 40,
        8 => 50,
        _ => 0,
    }
}

pub const fn starting_capstones<const S: usize>() -> u8 {
    match S {
        3 => 0,
        4 => 0,
        5 => 1,
        6 => 1,
        7 => 2,
        8 => 2,
        _ => 0,
    }
}

pub(crate) const fn num_square_symmetries<const S: usize>() -> usize {
    match S {
        4 => 3,
        5 => 6,
        6 => 6,
        _ => 0,
    }
}

pub(crate) const fn square_symmetries<const S: usize>() -> &'static [usize] {
    match S {
        4 => &[0, 1, 1, 0, 1, 2, 2, 1, 1, 2, 2, 1, 0, 1, 1, 0],
        5 => &[
            0, 1, 2, 1, 0, 1, 3, 4, 3, 1, 2, 4, 5, 4, 2, 1, 3, 4, 3, 1, 0, 1, 2, 1, 0,
        ],
        6 => &[
            0, 1, 2, 2, 1, 0, 1, 3, 4, 4, 3, 1, 2, 4, 5, 5, 4, 2, 2, 4, 5, 5, 4, 2, 1, 3, 4, 4, 3,
            1, 0, 1, 2, 2, 1, 0,
        ],
        _ => &[],
    }
}

/// Extra items for tuning evaluation constants.
pub trait TunableBoard: PositionTrait {
    fn value_params() -> &'static [f32];
    fn policy_params() -> &'static [f32];
    type ExtraData;

    fn static_eval_coefficients(&self, coefficients: &mut [f32]);

    fn static_eval_with_params(&self, params: &[f32]) -> f32 {
        let mut coefficients: Vec<f32> = vec![0.0; Self::value_params().len()];
        self.static_eval_coefficients(&mut coefficients);
        coefficients.iter().zip(params).map(|(a, b)| a * b).sum()
    }

    fn generate_moves_with_params(
        &self,
        params: &[f32],
        data: &Self::ExtraData,
        simple_moves: &mut Vec<<Self as PositionTrait>::Move>,
        moves: &mut Vec<(<Self as PositionTrait>::Move, search::Score)>,
    );

    fn coefficients_for_move(
        &self,
        coefficients: &mut [f32],
        mv: &Move,
        data: &Self::ExtraData,
        num_legal_moves: usize,
    );

    /// Move generation that includes a heuristic probability of each move being played.
    ///
    /// # Arguments
    ///
    /// * `simple_moves` - An empty vector to temporarily store moves without probabilities. The vector will be emptied before the function returns, and only serves to re-use allocated memory.
    /// * `moves` A vector to place the moves and associated probabilities.
    fn generate_moves_with_probabilities(
        &self,
        group_data: &Self::ExtraData,
        simple_moves: &mut Vec<Move>,
        moves: &mut Vec<(Move, search::Score)>,
    );
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

/// A legal move for a position.
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Move {
    Place(Role, Square),
    Move(Square, Direction, StackMovement), // Number of stones to take
}

impl Move {
    pub fn to_string<const S: usize>(&self) -> String {
        let mut string = String::new();
        match self {
            Move::Place(role, square) => match role {
                Cap => write!(string, "C{}", square.to_string::<S>()).unwrap(),
                Flat => write!(string, "{}", square.to_string::<S>()).unwrap(),
                Wall => write!(string, "S{}", square.to_string::<S>()).unwrap(),
            },
            Move::Move(square, direction, stack_movements) => {
                let mut pieces_held = stack_movements.get(0).pieces_to_take;
                if pieces_held == 1 {
                    write!(string, "{}", square.to_string::<S>()).unwrap();
                } else {
                    write!(string, "{}{}", pieces_held, square.to_string::<S>()).unwrap();
                }
                match direction {
                    North => string.push('+'),
                    West => string.push('<'),
                    East => string.push('>'),
                    South => string.push('-'),
                }
                // Omit number of pieces dropped, if all stones are dropped immediately
                if stack_movements.len() > 1 {
                    for movement in stack_movements.into_iter().skip(1) {
                        let pieces_to_drop = pieces_held - movement.pieces_to_take;
                        write!(string, "{}", pieces_to_drop).unwrap();
                        pieces_held -= pieces_to_drop;
                    }
                    write!(string, "{}", pieces_held).unwrap();
                }
            }
        }
        string
    }

    fn from_string<const S: usize>(input: &str) -> Result<Self, pgn_traits::Error> {
        if input.len() < 2 {
            return Err(pgn_traits::Error::new(
                pgn_traits::ErrorKind::ParseError,
                "Input move too short.",
            ));
        }
        if !input.is_ascii() {
            return Err(pgn_traits::Error::new(
                pgn_traits::ErrorKind::ParseError,
                "Input move contained non-ascii characters.",
            ));
        }
        let first_char = input.chars().next().unwrap();
        match first_char {
            'a'..='h' if input.len() == 2 => {
                let square = Square::parse_square::<S>(input)?;
                Ok(Move::Place(Flat, square))
            }
            'a'..='h' if input.len() == 3 => {
                let square = Square::parse_square::<S>(&input[0..2])?;
                let direction = Direction::parse(input.chars().nth(2).unwrap());
                // Moves in the simplified move notation always move one piece
                let mut movement = StackMovement::new();
                movement.push(Movement { pieces_to_take: 1 });
                Ok(Move::Move(square, direction, movement))
            }
            'C' if input.len() == 3 => {
                Ok(Move::Place(Cap, Square::parse_square::<S>(&input[1..])?))
            }
            'S' if input.len() == 3 => {
                Ok(Move::Place(Wall, Square::parse_square::<S>(&input[1..])?))
            }
            '1'..='8' if input.len() > 3 => {
                let square = Square::parse_square::<S>(&input[1..3])?;
                let direction = Direction::parse(input.chars().nth(3).unwrap());
                let pieces_taken = first_char.to_digit(10).unwrap() as u8;
                let mut pieces_held = pieces_taken;

                let mut amounts_to_drop: Vec<u8> = input
                    .chars()
                    .skip(4)
                    .map(|ch| ch.to_digit(10).map(|i| i as u8))
                    .collect::<Option<Vec<u8>>>()
                    .ok_or_else(|| {
                        pgn_traits::Error::new_parse_error(
                            format!("Couldn't parse move \"{}\": found non-integer when expecting number of pieces to drop", input
                        ))
                    })?;
                amounts_to_drop.pop(); //

                let mut movements = StackMovement::new();
                movements.push(Movement {
                    pieces_to_take: pieces_taken,
                });

                for amount_to_drop in amounts_to_drop {
                    movements.push(Movement {
                        pieces_to_take: pieces_held - amount_to_drop,
                    });
                    pieces_held -= amount_to_drop;
                }
                Ok(Move::Move(square, direction, movements))
            }
            _ => Err(pgn_traits::Error::new(
                pgn_traits::ErrorKind::ParseError,
                format!(
                    "Couldn't parse move \"{}\". Moves cannot start with {} and have length {}.",
                    input,
                    first_char,
                    input.len()
                ),
            )),
        }
    }
}

/// The counterpart of `Move`. When applied to a `Board`, it fully reverses the accompanying `Move`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ReverseMove {
    Place(Square),
    Move(Square, Direction, StackMovement, bool),
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
    fn reverse(self) -> Direction {
        match self {
            North => South,
            West => East,
            East => West,
            South => North,
        }
    }

    fn parse(ch: char) -> Self {
        match ch {
            '+' => North,
            '<' => West,
            '>' => East,
            '-' => South,
            _ => panic!("Couldn't parse \"{}\" as direction.", ch),
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

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(remote = "Color"))]
pub enum ColorDef {
    White,
    Black,
}

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct GroupEdgeConnection {
    data: u8,
}

impl GroupEdgeConnection {
    pub fn connect_square<const S: usize>(self, square: Square) -> Self {
        let mut edge_connection = self;
        if square.rank::<S>() == S as u8 - 1 {
            edge_connection = edge_connection.connect_north();
        }
        if square.rank::<S>() == 0 {
            edge_connection = edge_connection.connect_south();
        }
        if square.file::<S>() == 0 {
            edge_connection = edge_connection.connect_west();
        }
        if square.file::<S>() == S as u8 - 1 {
            edge_connection = edge_connection.connect_east();
        }
        edge_connection
    }

    pub fn is_winning(self) -> bool {
        self.is_connected_north() && self.is_connected_south()
            || self.is_connected_east() && self.is_connected_west()
    }

    pub fn is_connected_north(self) -> bool {
        self.data & 0b1000 != 0
    }

    pub fn connect_north(self) -> Self {
        GroupEdgeConnection {
            data: self.data | 0b1000,
        }
    }

    pub fn is_connected_west(self) -> bool {
        self.data & 0b100 != 0
    }

    pub fn connect_west(self) -> Self {
        GroupEdgeConnection {
            data: self.data | 0b100,
        }
    }

    pub fn is_connected_east(self) -> bool {
        self.data & 0b10 != 0
    }

    pub fn connect_east(self) -> Self {
        GroupEdgeConnection {
            data: self.data | 0b10,
        }
    }

    pub fn is_connected_south(self) -> bool {
        self.data & 1 != 0
    }

    pub fn connect_south(self) -> Self {
        GroupEdgeConnection {
            data: self.data | 1,
        }
    }
}

impl ops::BitOr for GroupEdgeConnection {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        GroupEdgeConnection {
            data: self.data | rhs.data,
        }
    }
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GroupData<const S: usize> {
    pub(crate) groups: AbstractBoard<u8, S>,
    pub(crate) amount_in_group: Box<[(u8, GroupEdgeConnection)]>,
    pub(crate) white_critical_squares: BitBoard,
    pub(crate) black_critical_squares: BitBoard,
    white_flat_stones: BitBoard,
    black_flat_stones: BitBoard,
    white_caps: BitBoard,
    black_caps: BitBoard,
    white_walls: BitBoard,
    black_walls: BitBoard,
}

impl<const S: usize> Default for GroupData<S> {
    fn default() -> Self {
        GroupData {
            groups: Default::default(),
            amount_in_group: vec![(0, GroupEdgeConnection::default()); S * S + 1]
                .into_boxed_slice(),
            white_critical_squares: Default::default(),
            black_critical_squares: Default::default(),
            white_flat_stones: Default::default(),
            black_flat_stones: Default::default(),
            white_caps: Default::default(),
            black_caps: Default::default(),
            white_walls: Default::default(),
            black_walls: Default::default(),
        }
    }
}

impl<const S: usize> GroupData<S> {
    pub(crate) fn white_road_pieces(&self) -> BitBoard {
        self.white_flat_stones | self.white_caps
    }

    pub(crate) fn black_road_pieces(&self) -> BitBoard {
        self.black_flat_stones | self.black_caps
    }

    pub(crate) fn white_blocking_pieces(&self) -> BitBoard {
        self.white_walls | self.white_caps
    }

    pub(crate) fn black_blocking_pieces(&self) -> BitBoard {
        self.black_walls | self.black_caps
    }

    pub(crate) fn all_pieces(&self) -> BitBoard {
        self.white_flat_stones
            | self.white_blocking_pieces()
            | self.black_flat_stones
            | self.black_blocking_pieces()
    }

    pub fn is_critical_square(&self, square: Square, color: Color) -> bool {
        match color {
            Color::White => WhiteTr::is_critical_square(self, square),
            Color::Black => BlackTr::is_critical_square(self, square),
        }
    }

    pub fn critical_squares(&self, color: Color) -> impl Iterator<Item = Square> {
        match color {
            Color::White => self.white_critical_squares.into_iter(),
            Color::Black => self.black_critical_squares.into_iter(),
        }
    }
}
#[derive(PartialEq, Eq, Debug)]
pub struct ZobristKeys<const S: usize> {
    top_stones: AbstractBoard<[u64; 6], S>,
    stones_in_stack: [AbstractBoard<[u64; 256], S>; 8],
    to_move: [u64; 2],
}

pub fn zobrist_top_stones<const S: usize>(square: Square, piece: Piece) -> u64 {
    match S {
        4 => ZOBRIST_KEYS_4S.top_stones[square][piece as u16 as usize],
        5 => ZOBRIST_KEYS_5S.top_stones[square][piece as u16 as usize],
        6 => ZOBRIST_KEYS_6S.top_stones[square][piece as u16 as usize],
        7 => ZOBRIST_KEYS_7S.top_stones[square][piece as u16 as usize],
        8 => ZOBRIST_KEYS_8S.top_stones[square][piece as u16 as usize],
        _ => panic!("No zobrist keys for size {}. Size not supported.", S),
    }
}

pub fn zobrist_stones_in_stack<const S: usize>(
    square: Square,
    place_in_stack: usize,
    stack_slice: usize,
) -> u64 {
    match S {
        4 => ZOBRIST_KEYS_4S.stones_in_stack[place_in_stack][square][stack_slice],
        5 => ZOBRIST_KEYS_5S.stones_in_stack[place_in_stack][square][stack_slice],
        6 => ZOBRIST_KEYS_6S.stones_in_stack[place_in_stack][square][stack_slice],
        7 => ZOBRIST_KEYS_7S.stones_in_stack[place_in_stack][square][stack_slice],
        8 => ZOBRIST_KEYS_8S.stones_in_stack[place_in_stack][square][stack_slice],
        _ => panic!("No zobrist keys for size {}. Size not supported.", S),
    }
}

pub fn zobrist_to_move<const S: usize>(color: Color) -> u64 {
    match S {
        4 => ZOBRIST_KEYS_4S.to_move[color.disc()],
        5 => ZOBRIST_KEYS_5S.to_move[color.disc()],
        6 => ZOBRIST_KEYS_6S.to_move[color.disc()],
        7 => ZOBRIST_KEYS_7S.to_move[color.disc()],
        8 => ZOBRIST_KEYS_8S.to_move[color.disc()],
        _ => panic!("No zobrist keys for size {}. Size not supported.", S),
    }
}

impl<const S: usize> ZobristKeys<S> {
    pub(crate) fn new() -> Box<Self> {
        let mut rng = rand::rngs::StdRng::from_seed([0; 32]);
        let mut random_vec: Vec<u64> = vec![0; mem::size_of::<ZobristKeys<S>>() / 8];
        for word in random_vec.iter_mut() {
            *word = rng.gen();
        }
        let zobrist = unsafe { mem::transmute(Box::from_raw(random_vec.as_mut_ptr())) };

        mem::forget(random_vec);
        zobrist
    }
}

/// Complete representation of a Tak position
#[derive(Clone)]
pub struct Board<const S: usize> {
    cells: AbstractBoard<Stack, S>,
    to_move: Color,
    white_stones_left: u8,
    black_stones_left: u8,
    white_caps_left: u8,
    black_caps_left: u8,
    half_moves_played: usize,
    moves: Vec<Move>,
    hash: u64,              // Zobrist hash of current position
    hash_history: Vec<u64>, // Zobrist hashes of previous board states, up to the last irreversible move. Does not include the corrent position
}

impl<const S: usize> PartialEq for Board<S> {
    fn eq(&self, other: &Self) -> bool {
        self.cells == other.cells
            && self.to_move == other.to_move
            && self.white_stones_left == other.white_stones_left
            && self.black_stones_left == other.black_stones_left
            && self.white_caps_left == other.white_caps_left
            && self.black_caps_left == other.black_caps_left
            && self.half_moves_played == other.half_moves_played
    }
}

impl<const S: usize> Eq for Board<S> {}

impl<const S: usize> Hash for Board<S> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.cells.hash(state);
        self.to_move.hash(state);
        self.white_stones_left.hash(state);
        self.black_stones_left.hash(state);
        self.white_caps_left.hash(state);
        self.black_caps_left.hash(state);
        self.half_moves_played.hash(state);
    }
}

impl<const S: usize> Index<Square> for Board<S> {
    type Output = Stack;

    fn index(&self, square: Square) -> &Self::Output {
        &self.cells[square]
    }
}

impl<const S: usize> IndexMut<Square> for Board<S> {
    fn index_mut(&mut self, square: Square) -> &mut Self::Output {
        &mut self.cells[square]
    }
}

impl<const S: usize> Default for Board<S> {
    fn default() -> Self {
        Board {
            cells: Default::default(),
            to_move: Color::White,
            white_stones_left: starting_stones::<S>(),
            black_stones_left: starting_stones::<S>(),
            white_caps_left: starting_capstones::<S>(),
            black_caps_left: starting_capstones::<S>(),
            half_moves_played: 0,
            moves: vec![],
            hash: zobrist_to_move::<S>(Color::White),
            hash_history: vec![],
        }
    }
}

impl<const S: usize> fmt::Debug for Board<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        for y in 0..S {
            for print_row in 0..3 {
                for x in 0..S {
                    for print_column in 0..3 {
                        match self.cells.raw[x][y].get(print_column * 3 + print_row) {
                            None => write!(f, "[.]")?,
                            Some(WhiteFlat) => write!(f, "[w]")?,
                            Some(WhiteWall) => write!(f, "[W]")?,
                            Some(WhiteCap) => write!(f, "[C]")?,
                            Some(BlackFlat) => write!(f, "[b]")?,
                            Some(BlackWall) => write!(f, "[B]")?,
                            Some(BlackCap) => write!(f, "[c]")?,
                        }
                    }
                    write!(f, " ")?;
                }
                writeln!(f)?;
            }
        }
        writeln!(
            f,
            "Stones left: {}/{}.",
            self.white_stones_left, self.black_stones_left
        )?;
        writeln!(
            f,
            "Capstones left: {}/{}.",
            self.white_caps_left, self.black_caps_left
        )?;
        writeln!(f, "{} to move.", self.side_to_move())?;
        writeln!(
            f,
            "Hash: {}, hash history: {:?}",
            self.hash, self.hash_history
        )?;
        Ok(())
    }
}

impl<const S: usize> Board<S> {
    pub fn white_reserves_left(&self) -> u8 {
        self.white_stones_left
    }

    pub fn black_reserves_left(&self) -> u8 {
        self.black_stones_left
    }

    pub fn white_caps_left(&self) -> u8 {
        self.white_caps_left
    }

    pub fn black_caps_left(&self) -> u8 {
        self.black_caps_left
    }

    #[cfg(test)]
    pub fn zobrist_hash(&self) -> u64 {
        self.hash
    }

    /// Number of moves/plies played in the game
    pub fn half_moves_played(&self) -> usize {
        self.half_moves_played
    }

    /// All the moves played in the game
    pub fn moves(&self) -> &Vec<Move> {
        &self.moves
    }

    pub fn null_move(&mut self) {
        self.to_move = !self.to_move;
    }

    pub(crate) fn zobrist_hash_from_scratch(&self) -> u64 {
        let mut hash = 0;
        hash ^= zobrist_to_move::<S>(self.to_move);

        for square in utils::squares_iterator::<S>() {
            hash ^= self.zobrist_hash_for_square(square);
        }
        hash
    }

    pub(crate) fn zobrist_hash_for_square(&self, square: Square) -> u64 {
        let mut hash = 0;
        let stack = &self[square];
        if let Some(top_stone) = stack.top_stone {
            hash ^= zobrist_top_stones::<S>(square, top_stone);
            for i in 0..(stack.len() as usize - 1) / 8 {
                hash ^= zobrist_stones_in_stack::<S>(
                    square,
                    i as usize,
                    stack.bitboard.board as usize >> (i * 8) & 255,
                )
            }
        }
        hash
    }

    fn is_critical_square_from_scratch(
        &self,
        groups: &AbstractBoard<u8, S>,
        amount_in_group: &[(u8, GroupEdgeConnection)],
        square: Square,
        color: Color,
    ) -> bool {
        let sum_of_connections = square
            .neighbours::<S>()
            .filter(|neighbour| self[*neighbour].top_stone().map(Piece::color) == Some(color))
            .map(|neighbour| amount_in_group[groups[neighbour] as usize].1)
            .fold(
                GroupEdgeConnection::default().connect_square::<S>(square),
                |acc, connection| acc | connection,
            );

        sum_of_connections.is_winning()
    }

    pub fn flip_board_y(&self) -> Board<S> {
        let mut new_board = self.clone();
        for x in 0..S as u8 {
            for y in 0..S as u8 {
                new_board[Square(y * S as u8 + x)] = self[Square((S as u8 - y - 1) * S as u8 + x)];
            }
        }
        new_board
    }

    pub fn flip_board_x(&self) -> Board<S> {
        let mut new_board = self.clone();
        for x in 0..S as u8 {
            for y in 0..S as u8 {
                new_board[Square(y * S as u8 + x)] = self[Square(y * S as u8 + (S as u8 - x - 1))];
            }
        }
        new_board
    }

    pub fn rotate_board(&self) -> Board<S> {
        let mut new_board = self.clone();
        for x in 0..S as u8 {
            for y in 0..S as u8 {
                let new_x = y;
                let new_y = S as u8 - x - 1;
                new_board[Square(y * S as u8 + x)] = self[Square(new_y * S as u8 + new_x)];
            }
        }
        new_board
    }

    pub fn flip_colors(&self) -> Board<S> {
        let mut new_board = self.clone();
        for square in utils::squares_iterator::<S>() {
            new_board[square] = Stack::default();
            for piece in self[square] {
                new_board[square].push(piece.flip_color());
            }
        }
        mem::swap(
            &mut new_board.white_stones_left,
            &mut new_board.black_stones_left,
        );
        mem::swap(
            &mut new_board.white_caps_left,
            &mut new_board.black_caps_left,
        );
        new_board.to_move = !new_board.to_move;
        new_board
    }

    /// Returns all 8 symmetries of the board
    pub fn symmetries(&self) -> Vec<Board<S>> {
        vec![
            self.clone(),
            self.flip_board_x(),
            self.flip_board_y(),
            self.rotate_board(),
            self.rotate_board().rotate_board(),
            self.rotate_board().rotate_board().rotate_board(),
            self.rotate_board().flip_board_x(),
            self.rotate_board().flip_board_y(),
        ]
    }

    /// Returns all 16 symmetries of the board, where swapping the colors is also a symmetry
    pub fn symmetries_with_swapped_colors(&self) -> Vec<Board<S>> {
        self.symmetries()
            .into_iter()
            .flat_map(|board| vec![board.clone(), board.flip_colors()])
            .collect()
    }

    fn count_all_pieces(&self) -> u8 {
        self.cells
            .raw
            .iter()
            .flatten()
            .map(|stack: &Stack| stack.len())
            .sum()
    }

    #[inline(never)]
    pub fn group_data(&self) -> GroupData<S> {
        let mut group_data = GroupData::default();

        for square in utils::squares_iterator::<S>() {
            match self[square].top_stone() {
                Some(WhiteFlat) => {
                    group_data.white_flat_stones = group_data.white_flat_stones.set(square.0)
                }
                Some(BlackFlat) => {
                    group_data.black_flat_stones = group_data.black_flat_stones.set(square.0)
                }
                Some(WhiteWall) => group_data.white_walls = group_data.white_walls.set(square.0),
                Some(BlackWall) => group_data.black_walls = group_data.black_walls.set(square.0),
                Some(WhiteCap) => group_data.white_caps = group_data.white_caps.set(square.0),
                Some(BlackCap) => group_data.black_caps = group_data.black_caps.set(square.0),
                None => (),
            }
        }

        let mut highest_component_id = 1;

        connected_components_graph(
            group_data.white_road_pieces(),
            &mut group_data.groups,
            &mut highest_component_id,
        );
        connected_components_graph(
            group_data.black_road_pieces(),
            &mut group_data.groups,
            &mut highest_component_id,
        );

        for square in utils::squares_iterator::<S>() {
            group_data.amount_in_group[group_data.groups[square] as usize].0 += 1;
            if self[square].top_stone().map(Piece::is_road_piece) == Some(true) {
                group_data.amount_in_group[group_data.groups[square] as usize].1 = group_data
                    .amount_in_group[group_data.groups[square] as usize]
                    .1
                    .connect_square::<S>(square);
            }
        }

        for square in utils::squares_iterator::<S>() {
            if self.is_critical_square_from_scratch(
                &group_data.groups,
                &group_data.amount_in_group,
                square,
                Color::White,
            ) {
                group_data.white_critical_squares = group_data.white_critical_squares.set(square.0);
            }
            if self.is_critical_square_from_scratch(
                &group_data.groups,
                &group_data.amount_in_group,
                square,
                Color::Black,
            ) {
                group_data.black_critical_squares = group_data.black_critical_squares.set(square.0);
            }
        }
        group_data
    }

    /// An iterator over the top stones left behind after a stack movement
    pub fn top_stones_left_behind_by_move<'a>(
        &'a self,
        square: Square,
        stack_movement: &'a StackMovement,
    ) -> impl Iterator<Item = Option<Piece>> + 'a {
        stack_movement
            .into_iter()
            .map(move |Movement { pieces_to_take }| {
                let piece_index = self[square].len() - pieces_to_take;
                if piece_index == 0 {
                    None
                } else {
                    Some(self[square].get(piece_index - 1).unwrap())
                }
            })
            .chain(std::iter::once(self[square].top_stone()))
    }

    pub(crate) fn game_result_with_group_data(
        &self,
        group_data: &GroupData<S>,
    ) -> Option<GameResult> {
        let repetitions = self
            .hash_history
            .iter()
            .filter(|hash| **hash == self.hash)
            .count();

        if repetitions >= 2 {
            return Some(GameResult::Draw);
        }

        if group_data
            .amount_in_group
            .iter()
            .any(|(_, group_connection)| group_connection.is_winning())
        {
            let highest_component_id = group_data
                .amount_in_group
                .iter()
                .enumerate()
                .skip(1)
                .find(|(_i, v)| (**v).0 == 0)
                .map(|(i, _v)| i)
                .unwrap_or(S * S + 1) as u8;

            if let Some(square) = self.is_win_by_road(&group_data.groups, highest_component_id) {
                debug_assert!(self[square].top_stone().unwrap().is_road_piece());
                return if self[square].top_stone().unwrap().color() == Color::White {
                    Some(GameResult::WhiteWin)
                } else {
                    Some(GameResult::BlackWin)
                };
            };
            unreachable!(
                "Board has winning connection, but isn't winning\n{:?}",
                self
            )
        }

        if (self.white_stones_left == 0 && self.white_caps_left == 0)
            || (self.black_stones_left == 0 && self.black_caps_left == 0)
            || utils::squares_iterator::<S>().all(|square| !self[square].is_empty())
        {
            // Count points
            let mut white_points = 0;
            let mut black_points = 0;
            for square in utils::squares_iterator::<S>() {
                match self[square].top_stone() {
                    Some(WhiteFlat) => white_points += 1,
                    Some(BlackFlat) => black_points += 1,
                    _ => (),
                }
            }
            match white_points.cmp(&black_points) {
                Ordering::Greater => Some(WhiteWin),
                Ordering::Less => Some(BlackWin),
                Ordering::Equal => Some(Draw),
            }
        } else {
            None
        }
    }

    /// Check if either side has completed a road
    /// Returns one of the winning squares in the road
    pub(crate) fn is_win_by_road(
        &self,
        components: &AbstractBoard<u8, S>,
        highest_component_id: u8,
    ) -> Option<Square> {
        // If the side to move is already winning,
        // the last move was either a suicide, or a double win
        let mut suicide_win_square = None;

        // TODO: Include highest id?
        for id in 1..highest_component_id {
            if (components.raw[0].iter().any(|&cell| cell == id)
                && components.raw[S - 1].iter().any(|&cell| cell == id))
                || ((0..S).any(|y| components.raw[y][0] == id)
                    && (0..S).any(|y| components.raw[y][S - 1] == id))
            {
                let square = utils::squares_iterator::<S>()
                    .find(|&sq| components[sq] == id)
                    .unwrap();
                if self[square].top_stone.unwrap().color() == self.side_to_move() {
                    suicide_win_square = Some(square)
                } else {
                    return Some(square);
                }
            }
        }
        suicide_win_square
    }

    pub(crate) fn static_eval_with_params_and_data(
        &self,
        group_data: &GroupData<S>,
        params: &[f32],
    ) -> f32 {
        let mut coefficients = vec![0.0; Self::value_params().len()];
        value_eval::static_eval_game_phase(&self, group_data, &mut coefficients);
        coefficients.iter().zip(params).map(|(a, b)| a * b).sum()
    }
}

impl<const S: usize> PositionTrait for Board<S> {
    type Move = Move;
    type ReverseMove = ReverseMove;

    fn start_position() -> Self {
        Self::default()
    }

    fn side_to_move(&self) -> Color {
        self.to_move
    }

    /// Adds all legal moves to the provided vector. Some notes on the interpretation of the rules:
    /// * Suicide moves are considered legal, and are generated like any other move.
    /// This includes moves that complete a road for the opponent without creating an own road,
    /// and moves that fill the board when that would result in an immediate loss.
    ///
    /// * Capstones are not counted towards a flat win, but all capstones must also be placed to trigger a flat win.
    ///
    /// * A game is considered a draw after a three-fold repetition of the same position.
    fn generate_moves(&self, moves: &mut Vec<Self::Move>) {
        match self.half_moves_played() {
            0 | 1 => {
                for square in utils::squares_iterator::<S>() {
                    if self[square].is_empty() {
                        moves.push(Move::Place(Flat, square));
                    }
                }
            }
            _ => match self.side_to_move() {
                Color::White => self.generate_moves_colortr::<WhiteTr, BlackTr>(moves),
                Color::Black => self.generate_moves_colortr::<BlackTr, WhiteTr>(moves),
            },
        }
    }

    fn do_move(&mut self, mv: Self::Move) -> Self::ReverseMove {
        self.hash_history.push(self.hash);
        let reverse_move = match mv {
            Move::Place(role, to) => {
                debug_assert!(self[to].is_empty());
                // On the first move, the players place the opponent's color
                let color_to_place = if self.half_moves_played() > 1 {
                    self.side_to_move()
                } else {
                    !self.side_to_move()
                };
                let piece = Piece::from_role_color(role, color_to_place);
                self[to].push(piece);

                match (color_to_place, role) {
                    (Color::White, Flat) => self.white_stones_left -= 1,
                    (Color::White, Wall) => self.white_stones_left -= 1,
                    (Color::White, Cap) => self.white_caps_left -= 1,
                    (Color::Black, Flat) => self.black_stones_left -= 1,
                    (Color::Black, Wall) => self.black_stones_left -= 1,
                    (Color::Black, Cap) => self.black_caps_left -= 1,
                }

                self.hash ^= zobrist_top_stones::<S>(to, piece);
                self.hash_history.clear(); // This move is irreversible, so previous position are never repeated from here

                ReverseMove::Place(to)
            }
            Move::Move(square, direction, stack_movement) => {
                let mut from = square;

                let mut pieces_left_behind = StackMovement::new();
                let mut flattens_stone = false;

                for sq in <MoveIterator<S>>::new(square, direction, stack_movement) {
                    self.hash ^= self.zobrist_hash_for_square(sq);
                }

                for Movement { pieces_to_take } in stack_movement.into_iter() {
                    let to = from.go_direction::<S>(direction).unwrap();

                    if self[to].top_stone.map(Piece::role) == Some(Wall) {
                        flattens_stone = true;
                        debug_assert!(self[from].top_stone().unwrap().role() == Cap);
                    }

                    let pieces_to_leave = self[from].len() - pieces_to_take;
                    pieces_left_behind.push(Movement { pieces_to_take });

                    for _ in pieces_to_leave..self[from].len() {
                        let piece = self[from].get(pieces_to_leave).unwrap();
                        self[to].push(piece);
                        self[from].remove(pieces_to_leave);
                    }

                    from = to;
                }

                for sq in <MoveIterator<S>>::new(square, direction, stack_movement) {
                    self.hash ^= self.zobrist_hash_for_square(sq);
                }

                let mut movements = StackMovement::new();
                for left_behind in pieces_left_behind {
                    movements.push(left_behind)
                }

                let mut movement_vec: Vec<Movement> = pieces_left_behind.into_iter().collect();
                movement_vec.reverse();

                pieces_left_behind = StackMovement::new();
                for movement in movement_vec {
                    pieces_left_behind.push(movement);
                }

                ReverseMove::Move(
                    from,
                    direction.reverse(),
                    pieces_left_behind,
                    flattens_stone,
                )
            }
        };

        debug_assert_eq!(
            2 * (starting_stones::<S>() + starting_capstones::<S>())
                - self.white_stones_left
                - self.black_stones_left
                - self.white_caps_left
                - self.black_caps_left,
            self.count_all_pieces(),
            "Wrong number of stones on board:\n{:?}",
            self
        );

        self.moves.push(mv);
        self.half_moves_played += 1;

        self.hash ^= zobrist_to_move::<S>(self.to_move);
        self.to_move = !self.to_move;
        self.hash ^= zobrist_to_move::<S>(self.to_move);

        reverse_move
    }

    fn reverse_move(&mut self, reverse_move: Self::ReverseMove) {
        match reverse_move {
            ReverseMove::Place(square) => {
                let piece = self[square].pop().unwrap();

                self.hash ^= zobrist_top_stones::<S>(square, piece);

                debug_assert!(piece.color() != self.side_to_move() || self.half_moves_played() < 3);

                match piece {
                    WhiteFlat | WhiteWall => self.white_stones_left += 1,
                    WhiteCap => self.white_caps_left += 1,
                    BlackFlat | BlackWall => self.black_stones_left += 1,
                    BlackCap => self.black_caps_left += 1,
                };
            }

            ReverseMove::Move(from, direction, stack_movement, flattens_wall) => {
                let mut square = from;

                for square in <MoveIterator<S>>::new(from, direction, stack_movement) {
                    self.hash ^= self.zobrist_hash_for_square(square);
                }

                for Movement { pieces_to_take } in stack_movement.into_iter() {
                    let to = square.go_direction::<S>(direction).unwrap();

                    let pieces_to_leave = self[square].len() - pieces_to_take;

                    for _ in pieces_to_leave..self[square].len() {
                        let piece = self[square].get(pieces_to_leave).unwrap();
                        self[to].push(piece);
                        self[square].remove(pieces_to_leave);
                    }
                    square = to;
                }

                if flattens_wall {
                    match self[from].top_stone().unwrap().color() {
                        Color::White => self[from].replace_top(WhiteWall),
                        Color::Black => self[from].replace_top(BlackWall),
                    };
                };

                for square in <MoveIterator<S>>::new(from, direction, stack_movement) {
                    self.hash ^= self.zobrist_hash_for_square(square);
                }
            }
        }

        self.moves.pop();
        self.hash_history.pop();
        self.half_moves_played -= 1;

        self.hash ^= zobrist_to_move::<S>(self.to_move);
        self.to_move = !self.to_move;
        self.hash ^= zobrist_to_move::<S>(self.to_move);
    }

    fn game_result(&self) -> Option<GameResult> {
        self.game_result_with_group_data(&self.group_data())
    }
}

pub(crate) struct MoveIterator<const S: usize> {
    square: Square,
    direction: Direction,
    squares_left: usize,
    _size: [(); S],
}

impl<const S: usize> MoveIterator<S> {
    pub fn new(square: Square, direction: Direction, stack_movement: StackMovement) -> Self {
        MoveIterator {
            square,
            direction,
            squares_left: stack_movement.len() + 1,
            _size: [(); S],
        }
    }
}

impl<const S: usize> Iterator for MoveIterator<S> {
    type Item = Square;

    fn next(&mut self) -> Option<Self::Item> {
        if self.squares_left == 0 {
            None
        } else {
            let next_square = self.square;
            self.square = self
                .square
                .go_direction::<S>(self.direction)
                .unwrap_or(Square(0));
            self.squares_left -= 1;
            Some(next_square)
        }
    }
}

impl<const S: usize> EvalPositionTrait for Board<S> {
    fn static_eval(&self) -> f32 {
        self.static_eval_with_params(&Self::value_params())
    }
}

impl<const S: usize> TunableBoard for Board<S> {
    type ExtraData = GroupData<S>;

    fn value_params() -> &'static [f32] {
        match S {
            4 => &VALUE_PARAMS_4S,
            5 => &VALUE_PARAMS_5S,
            6 => &VALUE_PARAMS_6S,
            _ => &[],
        }
    }

    fn policy_params() -> &'static [f32] {
        match S {
            4 => &POLICY_PARAMS_4S,
            5 => &POLICY_PARAMS_5S,
            6 => &POLICY_PARAMS_6S,
            _ => &[],
        }
    }

    fn static_eval_coefficients(&self, coefficients: &mut [f32]) {
        debug_assert!(self.game_result().is_none());

        let group_data = self.group_data();
        value_eval::static_eval_game_phase(&self, &group_data, coefficients)
    }

    fn generate_moves_with_params(
        &self,
        params: &[f32],
        group_data: &GroupData<S>,
        simple_moves: &mut Vec<Self::Move>,
        moves: &mut Vec<(Self::Move, f32)>,
    ) {
        debug_assert!(simple_moves.is_empty());
        self.generate_moves(simple_moves);
        match self.side_to_move() {
            Color::White => self.generate_moves_with_probabilities_colortr::<WhiteTr, BlackTr>(
                params,
                group_data,
                simple_moves,
                moves,
            ),
            Color::Black => self.generate_moves_with_probabilities_colortr::<BlackTr, WhiteTr>(
                params,
                group_data,
                simple_moves,
                moves,
            ),
        }
    }

    fn coefficients_for_move(
        &self,
        coefficients: &mut [f32],
        mv: &Move,
        group_data: &GroupData<S>,
        num_legal_moves: usize,
    ) {
        match self.side_to_move() {
            Color::White => policy_eval::coefficients_for_move_colortr::<WhiteTr, BlackTr, S>(
                &self,
                coefficients,
                mv,
                group_data,
                num_legal_moves,
            ),
            Color::Black => policy_eval::coefficients_for_move_colortr::<BlackTr, WhiteTr, S>(
                &self,
                coefficients,
                mv,
                group_data,
                num_legal_moves,
            ),
        }
    }
    /// Move generation that includes a heuristic probability of each move being played.
    ///
    /// # Arguments
    ///
    /// * `simple_moves` - An empty vector to temporarily store moves without probabilities. The vector will be emptied before the function returns, and only serves to re-use allocated memory.
    /// * `moves` A vector to place the moves and associated probabilities.
    fn generate_moves_with_probabilities(
        &self,
        group_data: &GroupData<S>,
        simple_moves: &mut Vec<Move>,
        moves: &mut Vec<(Move, search::Score)>,
    ) {
        self.generate_moves_with_params(Self::policy_params(), group_data, simple_moves, moves)
    }
}

impl<const S: usize> pgn_traits::PgnPosition for Board<S> {
    const REQUIRED_TAGS: &'static [(&'static str, &'static str)] = &[
        ("Player1", "?"),
        ("Player2", "?"),
        ("Date", "????.??.??"),
        ("Size", "5"),
        ("Result", "*"),
    ];

    const POSSIBLE_GAME_RESULTS: &'static [(&'static str, Option<GameResult>)] = &[
        ("*", None),
        ("1-0", Some(GameResult::WhiteWin)),
        ("R-0", Some(GameResult::WhiteWin)),
        ("F-0", Some(GameResult::WhiteWin)),
        ("0-1", Some(GameResult::BlackWin)),
        ("0-R", Some(GameResult::BlackWin)),
        ("0-F", Some(GameResult::BlackWin)),
        ("1/2-1/2", Some(GameResult::Draw)),
    ];

    const POSSIBLE_MOVE_ANNOTATIONS: &'static [&'static str] = &["''", "'", "*", "!", "?"];

    fn from_fen(fen: &str) -> Result<Self, pgn_traits::Error> {
        let fen_words: Vec<&str> = fen.split_whitespace().collect();

        if fen_words.len() < 3 {
            return Err(pgn_traits::Error::new_parse_error(format!(
                "Couldn't parse TPS string \"{}\", missing move counter.",
                fen
            )));
        }
        if fen_words.len() > 3 {
            return Err(pgn_traits::Error::new_parse_error(format!(
                "Couldn't parse TPS string \"{}\", unexpected \"{}\"",
                fen, fen_words[3]
            )));
        }

        let fen_rows: Vec<&str> = fen_words[0].split('/').collect();
        if fen_rows.len() != S {
            return Err(pgn_traits::Error::new_parse_error(format!(
                "Couldn't parse TPS string \"{}\", had {} rows instead of {}.",
                fen,
                fen_rows.len(),
                S
            )));
        }

        let rows: Vec<[Stack; S]> = fen_rows
            .into_iter()
            .map(parse_row)
            .collect::<Result<_, _>>()
            .map_err(|e| {
                pgn_traits::Error::new_caused_by(
                    pgn_traits::ErrorKind::ParseError,
                    format!("Couldn't parse TPS string \"{}\"", fen),
                    e,
                )
            })?;
        let mut board = Board::default();
        for square in utils::squares_iterator::<S>() {
            let (file, rank) = (square.file::<S>(), square.rank::<S>());
            let stack = rows[rank as usize][file as usize];
            for piece in stack.into_iter() {
                match piece {
                    WhiteFlat | WhiteWall => board.white_stones_left -= 1,
                    WhiteCap => board.white_caps_left -= 1,
                    BlackFlat | BlackWall => board.black_stones_left -= 1,
                    BlackCap => board.black_caps_left -= 1,
                }
            }
            board[square] = stack;
        }

        match fen_words[1] {
            "1" => board.to_move = Color::White,
            "2" => board.to_move = Color::Black,
            s => {
                return Err(pgn_traits::Error::new_parse_error(format!(
                    "Error parsing TPS \"{}\": Got bad side to move \"{}\"",
                    fen, s
                )))
            }
        }

        match fen_words[2].parse::<usize>() {
            Ok(n) => match board.side_to_move() {
                Color::White => board.half_moves_played = (n - 1) * 2,
                Color::Black => board.half_moves_played = (n - 1) * 2 + 1,
            },
            Err(e) => {
                return Err(pgn_traits::Error::new_caused_by(
                    pgn_traits::ErrorKind::ParseError,
                    format!(
                        "Error parsing TPS \"{}\": Got bad move number \"{}\"",
                        fen, fen_words[2]
                    ),
                    e,
                ))
            }
        }

        board.hash = board.zobrist_hash_from_scratch();

        return Ok(board);

        fn parse_row<const S: usize>(row_str: &str) -> Result<[Stack; S], pgn_traits::Error> {
            let mut column_id = 0;
            let mut row = [Stack::default(); S];
            let mut row_str_iter = row_str.chars().peekable();
            while column_id < S as u8 {
                match row_str_iter.peek() {
                    None => {
                        return Err(pgn_traits::Error::new_parse_error(format!(
                            "Couldn't parse row \"{}\": not enough pieces",
                            row_str
                        )))
                    }
                    Some('x') => {
                        row_str_iter.next();
                        if let Some(n) = row_str_iter.peek().and_then(|ch| ch.to_digit(10)) {
                            row_str_iter.next();
                            column_id += n as u8;
                        } else {
                            column_id += 1;
                        }
                        if let Some(',') | None = row_str_iter.peek() {
                            row_str_iter.next();
                        } else {
                            return Err(pgn_traits::Error::new_parse_error(format!(
                                "Expected ',' on row \"{}\", found {:?}",
                                row_str,
                                row_str_iter.next()
                            )));
                        }
                    }
                    Some('1') | Some('2') => {
                        let stack = &mut row[column_id as usize];
                        loop {
                            match row_str_iter.next() {
                                Some('1') => stack.push(Piece::from_role_color(Flat, Color::White)),
                                Some('2') => stack.push(Piece::from_role_color(Flat, Color::Black)),
                                Some('S') => {
                                    let piece = stack.pop().unwrap();
                                    stack.push(Piece::from_role_color(Wall, piece.color()));
                                }
                                Some('C') => {
                                    let piece = stack.pop().unwrap();
                                    stack.push(Piece::from_role_color(Cap, piece.color()));
                                }
                                Some(',') | None => {
                                    column_id += 1;
                                    break;
                                }
                                Some(ch) => {
                                    return Err(pgn_traits::Error::new_parse_error(format!(
                                        "Expected '1', '2', 'S' or 'C' on row \"{}\", found {}",
                                        row_str, ch
                                    )))
                                }
                            }
                        }
                    }
                    Some(x) => {
                        return Err(pgn_traits::Error::new_parse_error(format!(
                            "Unexpected '{}' in row \"{}\".",
                            x, row_str
                        )))
                    }
                }
            }
            Ok(row)
        }
    }

    fn to_fen(&self) -> String {
        let mut f = String::new();
        utils::squares_iterator::<S>()
            .map(|square| self[square])
            .for_each(|stack: Stack| {
                (match stack.top_stone() {
                    None => write!(f, "-"),
                    Some(WhiteFlat) => write!(f, "w"),
                    Some(WhiteWall) => write!(f, "W"),
                    Some(WhiteCap) => write!(f, "C"),
                    Some(BlackFlat) => write!(f, "b"),
                    Some(BlackWall) => write!(f, "B"),
                    Some(BlackCap) => write!(f, "c"),
                })
                .unwrap()
            });
        f
    }

    fn move_from_san(&self, input: &str) -> Result<Self::Move, pgn_traits::Error> {
        Self::Move::from_string::<S>(input)
    }

    fn move_to_san(&self, mv: &Self::Move) -> String {
        mv.to_string::<S>()
    }

    fn move_from_lan(&self, input: &str) -> Result<Self::Move, pgn_traits::Error> {
        Self::Move::from_string::<S>(input)
    }

    fn move_to_lan(&self, mv: &Self::Move) -> String {
        self.move_to_san(mv)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct AbstractBoard<T, const S: usize> {
    raw: [[T; S]; S],
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

pub(crate) fn connected_components_graph<const S: usize>(
    road_pieces: BitBoard,
    components: &mut AbstractBoard<u8, S>,
    id: &mut u8,
) {
    for square in utils::squares_iterator::<S>() {
        if components[square] == 0 && road_pieces.get(square.0) {
            connect_component(road_pieces, components, square, *id);
            *id += 1;
        }
    }
}

fn connect_component<const S: usize>(
    road_pieces: BitBoard,
    components: &mut AbstractBoard<u8, S>,
    square: Square,
    id: u8,
) {
    components[square] = id;
    for neighbour in square.neighbours::<S>() {
        if road_pieces.get(neighbour.0) && components[neighbour] == 0 {
            connect_component(road_pieces, components, neighbour, id);
        }
    }
}

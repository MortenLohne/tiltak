//! Tak move generation, along with all required data types.

/// The size of the board. Only 5 works correctly for now.
pub const BOARD_SIZE: usize = 5;

pub const BOARD_AREA: usize = BOARD_SIZE * BOARD_SIZE;

pub const STARTING_CAPSTONES: u8 = 1;

use crate::bitboard::BitBoard;
use crate::board::Direction::*;
use crate::board::Piece::*;
use crate::board::Role::Flat;
use crate::board::Role::*;
use crate::{policy_eval, search};
use arrayvec::ArrayVec;
use board_game_traits::board;
use board_game_traits::board::GameResult::{BlackWin, Draw, WhiteWin};
use board_game_traits::board::{Board as BoardTrait, EvalBoard as EvalBoardTrait};
use board_game_traits::board::{Color, GameResult};
use pgn_traits::pgn;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt::Write;
use std::hash::{Hash, Hasher};
use std::iter::FromIterator;
use std::mem;
use std::ops::{Index, IndexMut};
use std::{fmt, iter, ops};

/// Extra items for tuning evaluation constants.
pub trait TunableBoard: BoardTrait {
    type ExtraData;
    const VALUE_PARAMS: &'static [f32];
    const POLICY_PARAMS: &'static [f32];

    fn static_eval_coefficients(&self, coefficients: &mut [f32]);

    fn static_eval_with_params(&self, params: &[f32]) -> f32 {
        // TODO: Using a vector here is inefficient, we would like to use an array
        let mut coefficients: Vec<f32> = vec![0.0; params.len()];
        self.static_eval_coefficients(&mut coefficients);
        coefficients.iter().zip(params).map(|(a, b)| a * b).sum()
    }

    fn generate_moves_with_params(
        &self,
        params: &[f32],
        data: &Self::ExtraData,
        simple_moves: &mut Vec<<Self as BoardTrait>::Move>,
        moves: &mut Vec<(<Self as BoardTrait>::Move, search::Score)>,
    );

    fn coefficients_for_move(
        &self,
        coefficients: &mut [f32],
        mv: &Move,
        data: &Self::ExtraData,
        num_legal_moves: usize,
    );
}

pub(crate) trait ColorTr {
    fn color() -> Color;

    fn stones_left(board: &Board) -> u8;

    fn caps_left(board: &Board) -> u8;

    fn road_stones(group_data: &GroupData) -> BitBoard;

    fn blocking_stones(group_data: &GroupData) -> BitBoard;

    fn flats(group_data: &GroupData) -> BitBoard;

    fn walls(group_data: &GroupData) -> BitBoard;

    fn caps(group_data: &GroupData) -> BitBoard;

    fn flat_piece() -> Piece;

    fn wall_piece() -> Piece;

    fn cap_piece() -> Piece;

    fn is_road_stone(piece: Piece) -> bool;

    fn piece_is_ours(piece: Piece) -> bool;

    fn is_critical_square(group_data: &GroupData, square: Square) -> bool;

    fn critical_squares(group_data: &GroupData) -> BitBoard;
}

struct WhiteTr {}

impl ColorTr for WhiteTr {
    fn color() -> Color {
        Color::White
    }

    fn stones_left(board: &Board) -> u8 {
        board.white_stones_left
    }

    fn caps_left(board: &Board) -> u8 {
        board.white_caps_left
    }

    fn road_stones(group_data: &GroupData) -> BitBoard {
        group_data.white_road_pieces()
    }

    fn blocking_stones(group_data: &GroupData) -> BitBoard {
        group_data.white_blocking_pieces()
    }

    fn flats(group_data: &GroupData) -> BitBoard {
        group_data.white_flat_stones
    }

    fn walls(group_data: &GroupData) -> BitBoard {
        group_data.white_walls
    }

    fn caps(group_data: &GroupData) -> BitBoard {
        group_data.white_caps
    }

    fn flat_piece() -> Piece {
        Piece::WhiteFlat
    }

    fn wall_piece() -> Piece {
        Piece::WhiteWall
    }

    fn cap_piece() -> Piece {
        Piece::WhiteCap
    }

    fn is_road_stone(piece: Piece) -> bool {
        piece == WhiteFlat || piece == WhiteCap
    }

    fn piece_is_ours(piece: Piece) -> bool {
        piece == WhiteFlat || piece == WhiteWall || piece == WhiteCap
    }

    fn is_critical_square(group_data: &GroupData, square: Square) -> bool {
        group_data.white_critical_squares.get(square.0)
    }

    fn critical_squares(group_data: &GroupData) -> BitBoard {
        group_data.white_critical_squares
    }
}

struct BlackTr {}

impl ColorTr for BlackTr {
    fn color() -> Color {
        Color::Black
    }

    fn stones_left(board: &Board) -> u8 {
        board.black_stones_left
    }

    fn caps_left(board: &Board) -> u8 {
        board.black_caps_left
    }

    fn road_stones(group_data: &GroupData) -> BitBoard {
        group_data.black_road_pieces()
    }

    fn blocking_stones(group_data: &GroupData) -> BitBoard {
        group_data.black_blocking_pieces()
    }

    fn flats(group_data: &GroupData) -> BitBoard {
        group_data.black_flat_stones
    }

    fn walls(group_data: &GroupData) -> BitBoard {
        group_data.black_walls
    }

    fn caps(group_data: &GroupData) -> BitBoard {
        group_data.black_caps
    }

    fn flat_piece() -> Piece {
        Piece::BlackFlat
    }

    fn wall_piece() -> Piece {
        Piece::BlackWall
    }

    fn cap_piece() -> Piece {
        Piece::BlackCap
    }

    fn is_road_stone(piece: Piece) -> bool {
        piece == BlackFlat || piece == BlackCap
    }

    fn piece_is_ours(piece: Piece) -> bool {
        piece == BlackFlat || piece == BlackCap || piece == BlackWall
    }

    fn is_critical_square(group_data: &GroupData, square: Square) -> bool {
        group_data.black_critical_squares.get(square.0)
    }

    fn critical_squares(group_data: &GroupData) -> BitBoard {
        group_data.black_critical_squares
    }
}

/// A location on the board. Can be used to index a `Board`.
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Square(pub u8);

impl Square {
    pub fn from_rank_file(rank: u8, file: u8) -> Self {
        debug_assert!(rank < BOARD_SIZE as u8 && file < BOARD_SIZE as u8);
        Square(rank * BOARD_SIZE as u8 + file as u8)
    }

    pub fn rank(self) -> u8 {
        self.0 / BOARD_SIZE as u8
    }

    pub fn file(self) -> u8 {
        self.0 % BOARD_SIZE as u8
    }

    pub fn neighbours(self) -> impl Iterator<Item = Square> {
        (if self.0 as usize == 0 {
            [1, BOARD_SIZE as i8].iter()
        } else if self.0 as usize == BOARD_SIZE - 1 {
            [-1, BOARD_SIZE as i8].iter()
        } else if self.0 as usize == BOARD_SIZE * BOARD_SIZE - BOARD_SIZE {
            [1, -(BOARD_SIZE as i8)].iter()
        } else if self.0 as usize == BOARD_SIZE * BOARD_SIZE - 1 {
            [-1, -(BOARD_SIZE as i8)].iter()
        } else if self.rank() == 0 {
            [-1, 1, BOARD_SIZE as i8].iter()
        } else if self.rank() == BOARD_SIZE as u8 - 1 {
            [-(BOARD_SIZE as i8), -1, 1].iter()
        } else if self.file() == 0 {
            [-(BOARD_SIZE as i8), 1, BOARD_SIZE as i8].iter()
        } else if self.file() == BOARD_SIZE as u8 - 1 {
            [-(BOARD_SIZE as i8), -1, BOARD_SIZE as i8].iter()
        } else {
            [-(BOARD_SIZE as i8), -1, 1, BOARD_SIZE as i8].iter()
        })
        .cloned()
        .map(move |sq| sq + self.0 as i8)
        .map(|sq| Square(sq as u8))
    }

    pub fn directions(self) -> impl Iterator<Item = Direction> {
        (if self.0 as usize == 0 {
            [East, South].iter()
        } else if self.0 as usize == BOARD_SIZE - 1 {
            [West, South].iter()
        } else if self.0 as usize == BOARD_SIZE * BOARD_SIZE - BOARD_SIZE {
            [East, North].iter()
        } else if self.0 as usize == BOARD_SIZE * BOARD_SIZE - 1 {
            [West, North].iter()
        } else if self.rank() == 0 {
            [West, East, South].iter()
        } else if self.rank() == BOARD_SIZE as u8 - 1 {
            [North, West, East].iter()
        } else if self.file() == 0 {
            [North, East, South].iter()
        } else if self.file() == BOARD_SIZE as u8 - 1 {
            [North, West, South].iter()
        } else {
            [North, West, East, South].iter()
        })
        .cloned()
    }

    pub fn go_direction(self, direction: Direction) -> Option<Self> {
        match direction {
            North => self.0.checked_sub(BOARD_SIZE as u8).map(Square),
            West => {
                if self.file() == 0 {
                    None
                } else {
                    Some(Square(self.0 - 1))
                }
            }
            East => {
                if self.file() == BOARD_SIZE as u8 - 1 {
                    None
                } else {
                    Some(Square(self.0 + 1))
                }
            }
            South => {
                if self.0 as usize + BOARD_SIZE >= BOARD_SIZE * BOARD_SIZE {
                    None
                } else {
                    Some(Square(self.0 + BOARD_SIZE as u8))
                }
            }
        }
    }

    pub fn parse_square(input: &str) -> Square {
        assert_eq!(input.len(), 2, "Couldn't parse square {}", input);
        Square(
            (input.chars().next().unwrap() as u8 - b'a')
                + (BOARD_SIZE as u8 + b'0' - input.chars().nth(1).unwrap() as u8)
                    * BOARD_SIZE as u8,
        )
    }
}

impl fmt::Display for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{}", (self.file() + b'a') as char)?;
        write!(f, "{}", BOARD_SIZE as u8 - self.rank())?;
        Ok(())
    }
}

impl fmt::Debug for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{}", self)
    }
}

/// Iterates over all board squares.
pub fn squares_iterator() -> impl Iterator<Item = Square> {
    (0..(BOARD_SIZE * BOARD_SIZE)).map(|i| Square(i as u8))
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
    WhiteFlat,
    BlackFlat,
    WhiteWall,
    BlackWall,
    WhiteCap,
    BlackCap,
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
#[derive(Clone, PartialEq, Eq, Debug, Default, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Stack {
    top_stone: Option<Piece>,
    bitboard: BitBoard,
    height: u8,
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
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Move {
    Place(Role, Square),
    Move(Square, Direction, StackMovement), // Number of stones to take
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            Move::Place(role, square) => match role {
                Cap => write!(f, "C{}", square)?,
                Flat => write!(f, "{}", square)?,
                Wall => write!(f, "S{}", square)?,
            },
            Move::Move(square, direction, stack_movements) => {
                let mut pieces_held = stack_movements.movements[0].pieces_to_take;
                if pieces_held == 1 {
                    write!(f, "{}", square)?;
                } else {
                    write!(f, "{}{}", pieces_held, square)?;
                }
                match direction {
                    North => f.write_char('+')?,
                    West => f.write_char('<')?,
                    East => f.write_char('>')?,
                    South => f.write_char('-')?,
                }
                // Omit number of pieces dropped, if all stones are dropped immediately
                if stack_movements.movements.len() > 1 {
                    for movement in stack_movements.movements.iter().skip(1) {
                        let pieces_to_drop = pieces_held - movement.pieces_to_take;
                        write!(f, "{}", pieces_to_drop)?;
                        pieces_held -= pieces_to_drop;
                    }
                    write!(f, "{}", pieces_held)?;
                }
            }
        }
        Ok(())
    }
}

impl fmt::Debug for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{}", self)
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
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct StackMovement {
    pub movements: ArrayVec<[Movement; BOARD_SIZE - 1]>,
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
    pub fn connect_square(self, square: Square) -> Self {
        let mut edge_connection = self;
        if square.rank() == BOARD_SIZE as u8 - 1 {
            edge_connection = edge_connection.connect_north();
        }
        if square.rank() == 0 {
            edge_connection = edge_connection.connect_south();
        }
        if square.file() == 0 {
            edge_connection = edge_connection.connect_west();
        }
        if square.file() == BOARD_SIZE as u8 - 1 {
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
#[derive(Clone, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct GroupData {
    pub(crate) groups: AbstractBoard<u8>,
    pub(crate) amount_in_group: [(u8, GroupEdgeConnection); BOARD_AREA + 1],
    pub(crate) white_critical_squares: BitBoard,
    pub(crate) black_critical_squares: BitBoard,
    white_flat_stones: BitBoard,
    black_flat_stones: BitBoard,
    white_caps: BitBoard,
    black_caps: BitBoard,
    white_walls: BitBoard,
    black_walls: BitBoard,
}

impl GroupData {
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

    pub fn critical_squares<'a>(&'a self, color: Color) -> impl Iterator<Item = Square> + 'a {
        match color {
            Color::White => self.white_critical_squares.into_iter(),
            Color::Black => self.black_critical_squares.into_iter(),
        }
    }
}

/// Complete representation of a Tak position
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Board {
    cells: AbstractBoard<Stack>,
    #[cfg_attr(feature = "serde", serde(with = "ColorDef"))]
    to_move: Color,
    white_stones_left: u8,
    black_stones_left: u8,
    white_caps_left: u8,
    black_caps_left: u8,
    moves_played: u16,
    moves: Vec<Move>,
}

impl PartialEq for Board {
    fn eq(&self, other: &Self) -> bool {
        self.cells == other.cells
            && self.to_move == other.to_move
            && self.white_stones_left == other.white_stones_left
            && self.black_stones_left == other.black_stones_left
            && self.white_caps_left == other.white_caps_left
            && self.black_caps_left == other.black_caps_left
            && self.moves_played == other.moves_played
    }
}

impl Eq for Board {}

impl Hash for Board {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.cells.hash(state);
        self.to_move.hash(state);
        self.white_stones_left.hash(state);
        self.black_stones_left.hash(state);
        self.white_caps_left.hash(state);
        self.black_caps_left.hash(state);
        self.moves_played.hash(state);
    }
}

impl Index<Square> for Board {
    type Output = Stack;

    fn index(&self, square: Square) -> &Self::Output {
        &self.cells[square]
    }
}

impl IndexMut<Square> for Board {
    fn index_mut(&mut self, square: Square) -> &mut Self::Output {
        &mut self.cells[square]
    }
}

impl Default for Board {
    fn default() -> Self {
        Board {
            cells: Default::default(),
            to_move: Color::White,
            white_stones_left: 21,
            black_stones_left: 21,
            white_caps_left: STARTING_CAPSTONES,
            black_caps_left: STARTING_CAPSTONES,
            moves_played: 0,
            moves: vec![],
        }
    }
}

impl fmt::Debug for Board {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        for y in 0..BOARD_SIZE {
            for print_row in 0..3 {
                for x in 0..BOARD_SIZE {
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
        Ok(())
    }
}

impl Board {
    /// Number of moves/plies played in the game
    pub fn half_moves_played(&self) -> u16 {
        self.moves_played
    }

    /// All the moves played in the game
    pub fn moves(&self) -> &Vec<Move> {
        &self.moves
    }

    pub fn null_move(&mut self) {
        self.to_move = !self.to_move;
    }

    fn is_critical_square_from_scratch(
        &self,
        groups: &AbstractBoard<u8>,
        amount_in_group: &[(u8, GroupEdgeConnection); BOARD_AREA + 1],
        square: Square,
        color: Color,
    ) -> bool {
        let sum_of_connections = square
            .neighbours()
            .filter(|neighbour| self[*neighbour].top_stone().map(Piece::color) == Some(color))
            .map(|neighbour| amount_in_group[groups[neighbour] as usize].1)
            .fold(
                GroupEdgeConnection::default().connect_square(square),
                |acc, connection| acc | connection,
            );

        sum_of_connections.is_winning()
    }

    pub fn flip_board_y(&self) -> Board {
        let mut new_board = self.clone();
        for x in 0..BOARD_SIZE as u8 {
            for y in 0..BOARD_SIZE as u8 {
                new_board[Square(y * BOARD_SIZE as u8 + x)] =
                    self[Square((BOARD_SIZE as u8 - y - 1) * BOARD_SIZE as u8 + x)].clone();
            }
        }
        new_board
    }

    pub fn flip_board_x(&self) -> Board {
        let mut new_board = self.clone();
        for x in 0..BOARD_SIZE as u8 {
            for y in 0..BOARD_SIZE as u8 {
                new_board[Square(y * BOARD_SIZE as u8 + x)] =
                    self[Square(y * BOARD_SIZE as u8 + (BOARD_SIZE as u8 - x - 1))].clone();
            }
        }
        new_board
    }

    pub fn rotate_board(&self) -> Board {
        let mut new_board = self.clone();
        for x in 0..BOARD_SIZE as u8 {
            for y in 0..BOARD_SIZE as u8 {
                let new_x = y;
                let new_y = BOARD_SIZE as u8 - x - 1;
                new_board[Square(y * BOARD_SIZE as u8 + x)] =
                    self[Square(new_y * BOARD_SIZE as u8 + new_x)].clone();
            }
        }
        new_board
    }

    pub fn flip_colors(&self) -> Board {
        let mut new_board = self.clone();
        for square in squares_iterator() {
            new_board[square] = Stack::default();
            for piece in self[square].clone() {
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
    pub fn symmetries(&self) -> Vec<Board> {
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
    pub fn symmetries_with_swapped_colors(&self) -> Vec<Board> {
        self.symmetries()
            .into_iter()
            .flat_map(|board| vec![board.clone(), board.flip_colors()])
            .collect()
    }

    /// Move generation that includes a heuristic probability of each move being played.
    ///
    /// # Arguments
    ///
    /// * `simple_moves` - An empty vector to temporarily store moves without probabilities. The vector will be emptied before the function returns, and only serves to re-use allocated memory.
    /// * `moves` A vector to place the moves and associated probabilities.
    pub fn generate_moves_with_probabilities(
        &self,
        group_data: &GroupData,
        simple_moves: &mut Vec<Move>,
        moves: &mut Vec<(Move, search::Score)>,
    ) {
        self.generate_moves_with_params(Board::POLICY_PARAMS, group_data, simple_moves, moves)
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
    pub fn group_data(&self) -> GroupData {
        let mut group_data = GroupData::default();

        group_data.white_flat_stones = BitBoard::empty();
        group_data.black_flat_stones = BitBoard::empty();
        group_data.white_walls = BitBoard::empty();
        group_data.black_walls = BitBoard::empty();
        group_data.white_caps = BitBoard::empty();
        group_data.black_caps = BitBoard::empty();

        for square in squares_iterator() {
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

        for square in squares_iterator() {
            group_data.amount_in_group[group_data.groups[square] as usize].0 += 1;
            if self[square].top_stone().map(Piece::is_road_piece) == Some(true) {
                group_data.amount_in_group[group_data.groups[square] as usize].1 = group_data
                    .amount_in_group[group_data.groups[square] as usize]
                    .1
                    .connect_square(square);
            }
        }

        for square in squares_iterator() {
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
            .movements
            .iter()
            .map(move |Movement { pieces_to_take }| {
                let piece_index = self[square].len() - *pieces_to_take;
                if piece_index == 0 {
                    None
                } else {
                    Some(self[square].get(piece_index - 1).unwrap())
                }
            })
            .chain(std::iter::once(self[square].top_stone()))
    }

    pub(crate) fn game_result_with_group_data(&self, group_data: &GroupData) -> Option<GameResult> {
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
                .unwrap_or(BOARD_AREA + 1) as u8;

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
            || squares_iterator().all(|square| !self[square].is_empty())
        {
            // Count points
            let mut white_points = 0;
            let mut black_points = 0;
            for square in squares_iterator() {
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

    fn static_eval_game_phase(&self, group_data: &GroupData, coefficients: &mut [f32]) {
        const FLAT_PSQT: usize = 0;
        const WALL_PSQT: usize = FLAT_PSQT + 6;
        const CAP_PSQT: usize = WALL_PSQT + 6;
        const OUR_STACK_PSQT: usize = CAP_PSQT + 6;
        const THEIR_STACK_PSQT: usize = OUR_STACK_PSQT + 6;

        let mut white_flat_count = 0;
        let mut black_flat_count = 0;

        for square in squares_iterator() {
            let stack = &self[square];
            if let Some(piece) = self[square].top_stone() {
                let i = square.0 as usize;
                match piece {
                    WhiteFlat => {
                        coefficients[FLAT_PSQT + SQUARE_SYMMETRIES[i]] += 1.0;
                        white_flat_count += 1;
                    }
                    BlackFlat => {
                        coefficients[FLAT_PSQT + SQUARE_SYMMETRIES[i]] -= 1.0;
                        black_flat_count += 1;
                    }
                    WhiteWall => coefficients[WALL_PSQT + SQUARE_SYMMETRIES[i]] += 1.0,
                    BlackWall => coefficients[WALL_PSQT + SQUARE_SYMMETRIES[i]] -= 1.0,
                    WhiteCap => coefficients[CAP_PSQT + SQUARE_SYMMETRIES[i]] += 1.0,
                    BlackCap => coefficients[CAP_PSQT + SQUARE_SYMMETRIES[i]] -= 1.0,
                }
                if stack.height > 1 {
                    let controlling_player = piece.color();
                    let color_factor = piece.color().multiplier() as f32;
                    for piece in stack.clone().into_iter().take(stack.height as usize - 1) {
                        if piece.color() == controlling_player {
                            coefficients[OUR_STACK_PSQT + SQUARE_SYMMETRIES[i]] += color_factor;
                        } else {
                            coefficients[THEIR_STACK_PSQT + SQUARE_SYMMETRIES[i]] -= color_factor;
                        }
                    }
                }
            }
        }

        const SIDE_TO_MOVE: usize = THEIR_STACK_PSQT + 6;
        const FLATSTONE_LEAD: usize = SIDE_TO_MOVE + 3;
        const NUMBER_OF_GROUPS: usize = FLATSTONE_LEAD + 3;

        // Give the side to move a bonus/malus depending on flatstone lead
        let white_flatstone_lead = white_flat_count - black_flat_count;

        // Bonus/malus depending on the number of groups each side has
        let mut seen_groups = [false; BOARD_AREA + 1];
        seen_groups[0] = true;

        let number_of_groups = squares_iterator()
            .map(|square| {
                let group_id = group_data.groups[square] as usize;
                if !seen_groups[group_id] {
                    seen_groups[group_id] = true;
                    self[square].top_stone.unwrap().color().multiplier()
                } else {
                    0
                }
            })
            .sum::<isize>() as f32;

        let opening_scale_factor = f32::min(
            f32::max((24.0 - self.half_moves_played() as f32) / 12.0, 0.0),
            1.0,
        );
        let endgame_scale_factor = f32::min(
            f32::max((self.half_moves_played() as f32 - 24.0) / 24.0, 0.0),
            1.0,
        );
        let middlegame_scale_factor = 1.0 - opening_scale_factor - endgame_scale_factor;

        debug_assert!(middlegame_scale_factor <= 1.0);
        debug_assert!(opening_scale_factor == 0.0 || endgame_scale_factor == 0.0);

        coefficients[SIDE_TO_MOVE] = self.side_to_move().multiplier() as f32 * opening_scale_factor;
        coefficients[FLATSTONE_LEAD] = white_flatstone_lead as f32 * opening_scale_factor;
        coefficients[NUMBER_OF_GROUPS] = number_of_groups * opening_scale_factor;

        coefficients[SIDE_TO_MOVE + 1] =
            self.side_to_move().multiplier() as f32 * middlegame_scale_factor;
        coefficients[FLATSTONE_LEAD + 1] = white_flatstone_lead as f32 * middlegame_scale_factor;
        coefficients[NUMBER_OF_GROUPS + 1] = number_of_groups * middlegame_scale_factor;

        coefficients[SIDE_TO_MOVE + 2] =
            self.side_to_move().multiplier() as f32 * endgame_scale_factor;
        coefficients[FLATSTONE_LEAD + 2] = white_flatstone_lead as f32 * endgame_scale_factor;
        coefficients[NUMBER_OF_GROUPS + 2] = number_of_groups * endgame_scale_factor;

        const CRITICAL_SQUARES: usize = NUMBER_OF_GROUPS + 3;

        for critical_square in group_data.critical_squares(Color::White) {
            self.critical_squares_eval::<WhiteTr, BlackTr>(
                critical_square,
                coefficients,
                CRITICAL_SQUARES,
            );
        }

        for critical_square in group_data.critical_squares(Color::Black) {
            self.critical_squares_eval::<BlackTr, WhiteTr>(
                critical_square,
                coefficients,
                CRITICAL_SQUARES,
            );
        }

        const CAPSTONE_OVER_OWN_PIECE: usize = CRITICAL_SQUARES + 6;
        const CAPSTONE_ON_STACK: usize = CAPSTONE_OVER_OWN_PIECE + 1;
        const STANDING_STONE_ON_STACK: usize = CAPSTONE_ON_STACK + 1;
        const FLAT_STONE_NEXT_TO_OUR_STACK: usize = STANDING_STONE_ON_STACK + 1;
        const STANDING_STONE_NEXT_TO_OUR_STACK: usize = FLAT_STONE_NEXT_TO_OUR_STACK + 1;
        const CAPSTONE_NEXT_TO_OUR_STACK: usize = STANDING_STONE_NEXT_TO_OUR_STACK + 1;

        squares_iterator()
            .map(|sq| (sq, &self[sq]))
            .filter(|(_, stack)| stack.len() > 1)
            .for_each(|(square, stack)| {
                let top_stone = stack.top_stone().unwrap();
                let controlling_player = top_stone.color();
                let color_factor = top_stone.color().multiplier() as f32;

                // Extra bonus for having your capstone over your own piece
                if top_stone.role() == Cap
                    && stack.get(stack.len() - 2).unwrap().color() == controlling_player
                {
                    coefficients[CAPSTONE_OVER_OWN_PIECE] += color_factor;
                }

                match top_stone.role() {
                    Cap => coefficients[CAPSTONE_ON_STACK] += color_factor,
                    Flat => (),
                    Wall => coefficients[STANDING_STONE_ON_STACK] += color_factor,
                }

                // Malus for them having stones next to our stack with flat stones on top
                for neighbour in square.neighbours() {
                    if let Some(neighbour_top_stone) = self[neighbour].top_stone() {
                        if top_stone.role() == Flat
                            && neighbour_top_stone.color() != controlling_player
                        {
                            match neighbour_top_stone.role() {
                                Flat => {
                                    coefficients[FLAT_STONE_NEXT_TO_OUR_STACK] +=
                                        color_factor * stack.len() as f32
                                }
                                Wall => {
                                    coefficients[STANDING_STONE_NEXT_TO_OUR_STACK] +=
                                        color_factor * stack.len() as f32
                                }
                                Cap => {
                                    coefficients[CAPSTONE_NEXT_TO_OUR_STACK] +=
                                        color_factor * stack.len() as f32
                                }
                            }
                        }
                    }
                }
            });

        // Number of pieces in each line
        const NUM_LINES_OCCUPIED: usize = CAPSTONE_NEXT_TO_OUR_STACK + 1;
        // Number of lines with at least one road stone
        const LINE_CONTROL: usize = NUM_LINES_OCCUPIED + 6;

        let mut num_ranks_occupied_white = 0;
        let mut num_files_occupied_white = 0;
        let mut num_ranks_occupied_black = 0;
        let mut num_files_occupied_black = 0;

        for line in BitBoard::all_lines().iter() {
            self.line_score::<WhiteTr, BlackTr>(&group_data, *line, coefficients, LINE_CONTROL);
            self.line_score::<BlackTr, WhiteTr>(&group_data, *line, coefficients, LINE_CONTROL);
        }

        for i in 0..BOARD_SIZE as u8 {
            if !WhiteTr::road_stones(&group_data).rank(i).is_empty() {
                num_ranks_occupied_white += 1;
            }
            if !BlackTr::road_stones(&group_data).rank(i).is_empty() {
                num_ranks_occupied_black += 1;
            }
        }

        for i in 0..BOARD_SIZE as u8 {
            if !WhiteTr::road_stones(&group_data).file(i).is_empty() {
                num_files_occupied_white += 1;
            }
            if !BlackTr::road_stones(&group_data).file(i).is_empty() {
                num_files_occupied_black += 1;
            }
        }

        coefficients[NUM_LINES_OCCUPIED + num_ranks_occupied_white] += 1.0;
        coefficients[NUM_LINES_OCCUPIED + num_files_occupied_white] += 1.0;
        coefficients[NUM_LINES_OCCUPIED + num_ranks_occupied_black] -= 1.0;
        coefficients[NUM_LINES_OCCUPIED + num_files_occupied_black] -= 1.0;

        const _NEXT_CONST: usize = LINE_CONTROL + 12;

        assert_eq!(_NEXT_CONST, coefficients.len());
    }

    /// Give bonus for our critical squares
    fn critical_squares_eval<Us: ColorTr, Them: ColorTr>(
        &self,
        critical_square: Square,
        coefficients: &mut [f32],
        critical_squares: usize,
    ) {
        let top_stone = self[critical_square].top_stone;
        if top_stone.is_none() {
            coefficients[critical_squares] += Us::color().multiplier() as f32;
        } else if top_stone == Some(Us::wall_piece()) {
            coefficients[critical_squares + 1] += Us::color().multiplier() as f32;
        } else if top_stone == Some(Them::flat_piece()) {
            coefficients[critical_squares + 2] += Us::color().multiplier() as f32;
        }
        // Their capstone or wall
        else {
            coefficients[critical_squares + 3] += Us::color().multiplier() as f32
        }

        // Bonus for having our cap next to our critical square
        for neighbour in critical_square.neighbours() {
            if self[neighbour].top_stone == Some(Us::cap_piece()) {
                coefficients[critical_squares + 4] += Us::color().multiplier() as f32;
                // Further bonus for a capped stack next to our critical square
                for piece in self[neighbour].clone().into_iter() {
                    if piece == Us::flat_piece() {
                        coefficients[critical_squares + 5] += Us::color().multiplier() as f32;
                    }
                }
            }
        }
    }

    fn line_score<Us: ColorTr, Them: ColorTr>(
        &self,
        group_data: &GroupData,
        line: BitBoard,
        coefficients: &mut [f32],
        line_control: usize,
    ) {
        let road_pieces_in_line = (Us::road_stones(group_data) & line).count();

        coefficients[line_control + road_pieces_in_line as usize] +=
            Us::color().multiplier() as f32;

        let block_their_line = line_control + 6;

        // Bonus for blocking their lines
        if road_pieces_in_line >= 3 {
            coefficients[block_their_line + road_pieces_in_line as usize - 3] +=
                ((Them::flats(group_data) & line).count() as isize * Them::color().multiplier())
                    as f32;
            coefficients[block_their_line + 2 + road_pieces_in_line as usize - 3] +=
                ((Them::walls(group_data) & line).count() as isize * Them::color().multiplier())
                    as f32;
            coefficients[block_their_line + 4 + road_pieces_in_line as usize - 3] +=
                ((Them::caps(group_data) & line).count() as isize * Them::color().multiplier())
                    as f32;
        }
    }

    /// Check if either side has completed a road
    /// Returns one of the winning squares in the road
    pub(crate) fn is_win_by_road(
        &self,
        components: &AbstractBoard<u8>,
        highest_component_id: u8,
    ) -> Option<Square> {
        // If the side to move is already winning,
        // the last move was either a suicide, or a double win
        let mut suicide_win_square = None;

        // TODO: Include highest id?
        for id in 1..highest_component_id {
            if (components.raw[0].iter().any(|&cell| cell == id)
                && components.raw[BOARD_SIZE - 1]
                    .iter()
                    .any(|&cell| cell == id))
                || ((0..BOARD_SIZE).any(|y| components.raw[y][0] == id)
                    && (0..BOARD_SIZE).any(|y| components.raw[y][BOARD_SIZE - 1] == id))
            {
                let square = squares_iterator().find(|&sq| components[sq] == id).unwrap();
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
        group_data: &GroupData,
        params: &[f32],
    ) -> f32 {
        // TODO: Using a vector here is inefficient, we would like to use an array
        let mut coefficients: Vec<f32> = vec![0.0; params.len()];
        self.static_eval_game_phase(group_data, &mut coefficients);
        coefficients.iter().zip(params).map(|(a, b)| a * b).sum()
    }
}

impl board::Board for Board {
    type Move = Move;
    type ReverseMove = ReverseMove;

    fn start_board() -> Self {
        Self::default()
    }

    fn side_to_move(&self) -> Color {
        self.to_move
    }

    /// Adds all legal moves to the provided vector.
    /// Suicide moves are considered illegal moves and are not generated.
    /// This includes moves that complete a road for the opponent without creating an own road,
    /// and moves that place your last piece on the board when that would result in an immediate loss.
    ///
    /// All pieces (including capstones) must be placed for the game to end.
    /// Capstones are not counted towards a flat win, if the game ended due to the board being filled.
    ///
    /// TODO: Suicide moves are allowed if it fills the board, both place and move moves
    fn generate_moves(&self, moves: &mut Vec<Self::Move>) {
        debug_assert!(
            self.game_result().is_none(),
            "Tried to generate moves on position with {:?} on\n{:?}",
            self.game_result(),
            self
        );

        match self.moves_played {
            0 | 1 => {
                for square in squares_iterator() {
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
        let reverse_move = match mv.clone() {
            Move::Place(role, to) => {
                debug_assert!(self[to].is_empty());
                // On the first move, the players place the opponent's color
                let color_to_place = if self.moves_played > 1 {
                    self.side_to_move()
                } else {
                    !self.side_to_move()
                };
                self[to].push(Piece::from_role_color(role, color_to_place));

                match (color_to_place, role) {
                    (Color::White, Flat) => self.white_stones_left -= 1,
                    (Color::White, Wall) => self.white_stones_left -= 1,
                    (Color::White, Cap) => self.white_caps_left -= 1,
                    (Color::Black, Flat) => self.black_stones_left -= 1,
                    (Color::Black, Wall) => self.black_stones_left -= 1,
                    (Color::Black, Cap) => self.black_caps_left -= 1,
                }
                ReverseMove::Place(to)
            }
            Move::Move(mut from, direction, stack_movement) => {
                let mut pieces_left_behind: ArrayVec<[u8; BOARD_SIZE - 1]> = ArrayVec::new();
                let mut flattens_stone = false;
                for Movement { pieces_to_take } in stack_movement.movements {
                    let to = from.go_direction(direction).unwrap();

                    if self[to].top_stone.map(Piece::role) == Some(Wall) {
                        flattens_stone = true;
                        debug_assert!(self[from].top_stone().unwrap().role() == Cap);
                    }

                    let pieces_to_leave = self[from].len() - pieces_to_take;
                    pieces_left_behind.push(pieces_to_take);

                    for _ in pieces_to_leave..self[from].len() {
                        let piece = self[from].get(pieces_to_leave).unwrap();
                        self[to].push(piece);
                        self[from].remove(pieces_to_leave);
                    }

                    from = to;
                }

                pieces_left_behind.reverse();
                ReverseMove::Move(
                    from,
                    direction.reverse(),
                    StackMovement {
                        movements: pieces_left_behind
                            .iter()
                            .map(|&pieces_to_take| Movement { pieces_to_take })
                            .collect(),
                    },
                    flattens_stone,
                )
            }
        };

        debug_assert_eq!(
            44 - self.white_stones_left
                - self.black_stones_left
                - self.white_caps_left
                - self.black_caps_left,
            self.count_all_pieces(),
            "Wrong number of stones on board:\n{:?}",
            self
        );

        self.moves.push(mv);
        self.to_move = !self.to_move;
        self.moves_played += 1;
        reverse_move
    }

    fn reverse_move(&mut self, reverse_move: Self::ReverseMove) {
        match reverse_move {
            ReverseMove::Place(square) => {
                let piece = self[square].pop().unwrap();
                debug_assert!(piece.color() != self.side_to_move() || self.moves_played < 3);

                match piece {
                    WhiteFlat | WhiteWall => self.white_stones_left += 1,
                    WhiteCap => self.white_caps_left += 1,
                    BlackFlat | BlackWall => self.black_stones_left += 1,
                    BlackCap => self.black_caps_left += 1,
                };
            }

            ReverseMove::Move(from, direction, stack_movement, flattens_wall) => {
                let mut square = from;
                for Movement { pieces_to_take } in stack_movement.movements {
                    let to = square.go_direction(direction).unwrap();

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
            }
        }

        self.moves.pop();
        self.moves_played -= 1;
        self.to_move = !self.to_move;
    }

    fn game_result(&self) -> Option<GameResult> {
        self.game_result_with_group_data(&self.group_data())
    }
}

impl EvalBoardTrait for Board {
    fn static_eval(&self) -> f32 {
        self.static_eval_with_params(Self::VALUE_PARAMS)
    }
}

pub(crate) const SQUARE_SYMMETRIES: [usize; 25] = [
    0, 1, 2, 1, 0, 1, 3, 4, 3, 1, 2, 4, 5, 4, 2, 1, 3, 4, 3, 1, 0, 1, 2, 1, 0,
];

impl TunableBoard for Board {
    type ExtraData = GroupData;
    #[allow(clippy::unreadable_literal)]
    const VALUE_PARAMS: &'static [f32] = &[
        -0.016845044,
        0.17022224,
        0.24124837,
        0.28603324,
        0.31441984,
        0.26619905,
        0.3240439,
        0.5464134,
        0.38624322,
        0.836523,
        0.8441856,
        0.50281864,
        -0.5319615,
        -0.08690647,
        -0.14608166,
        0.38866445,
        0.58775675,
        1.0267034,
        1.0977447,
        1.1716292,
        1.2097449,
        1.2128466,
        1.2514062,
        1.3240525,
        0.6959771,
        0.79473215,
        0.77087706,
        0.844586,
        0.83693826,
        0.90717065,
        0.86492157,
        0.8287618,
        1.0482398,
        0.40296316,
        0.25697726,
        0.60585046,
        -0.28701374,
        -0.12125129,
        -0.1178239,
        0.28548676,
        -0.11196409,
        0.15350688,
        -0.09609637,
        0.043966047,
        0.09228689,
        0.13676666,
        0.6389944,
        0.35806286,
        -0.03181746,
        -0.15553103,
        -0.13250662,
        0.60890996,
        -0.7032362,
        -0.36233038,
        -0.10586373,
        0.12679414,
        0.4258857,
        -0.8821736,
        -0.6005885,
        -0.119399525,
        0.50513303,
        1.0882376,
        0.009241403,
        0.031525154,
        0.037940275,
        0.21526852,
        0.05217122,
        0.024593933,
        -0.011519423,
    ];
    #[allow(clippy::unreadable_literal)]
    const POLICY_PARAMS: &'static [f32] = &[
        0.9305405,
        -0.34577864,
        -0.04961737,
        0.19245817,
        0.42355523,
        0.65872794,
        0.060075734,
        -0.1338366,
        -0.31138068,
        -0.48290452,
        -0.117499486,
        0.02988758,
        0.12378819,
        -1.9687757,
        -1.322118,
        -0.83446103,
        -1.3894782,
        -0.28321508,
        5.267339,
        -0.14667746,
        -0.24524969,
        0.2349205,
        0.85037255,
        1.2385551,
        -0.45662624,
        -0.50240517,
        -0.53431773,
        -0.23039874,
        -0.028741265,
        0.112411834,
        -0.4481093,
        -0.5031174,
        -0.020420931,
        -0.12086809,
        0.292516,
        0.04211807,
        0.27091402,
        0.5891712,
        0.7189378,
        -0.45219094,
        -0.5787303,
        -0.66003776,
        -0.20213033,
        0.15377757,
        0.03439601,
        -0.9556297,
        -0.46582177,
        -0.05709919,
        0.42028004,
        0.04214308,
        0.008917372,
        -0.217583,
        0.55765885,
        0.07484926,
        0.185138,
        0.20975994,
        1.1353594,
        0.22278507,
        2.435652,
        0.62582356,
        2.3848765,
        2.9783263,
        0.74396574,
        -3.6530883,
        -1.8598384,
        0.8874495,
        1.1030788,
        0.3378354,
        0.4636264,
        0.08576884,
        0.33934826,
        -0.6625324,
        0.2512112,
        -1.6344877,
        -1.9847263,
        -1.4961373,
        -1.7108375,
        0.5683625,
        0.25,
        0.5,
        1.5,
        2.0,
        1.1089591,
        3.224672,
        1.0086825,
        3.070864,
        0.8663666,
        2.125451,
    ];

    fn static_eval_coefficients(&self, coefficients: &mut [f32]) {
        debug_assert!(self.game_result().is_none());

        let group_data = self.group_data();
        self.static_eval_game_phase(&group_data, coefficients)
    }

    fn generate_moves_with_params(
        &self,
        params: &[f32],
        group_data: &GroupData,
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
        group_data: &GroupData,
        num_legal_moves: usize,
    ) {
        match self.side_to_move() {
            Color::White => policy_eval::coefficients_for_move_colortr::<WhiteTr, BlackTr>(
                &self,
                coefficients,
                mv,
                group_data,
                num_legal_moves,
            ),
            Color::Black => policy_eval::coefficients_for_move_colortr::<BlackTr, WhiteTr>(
                &self,
                coefficients,
                mv,
                group_data,
                num_legal_moves,
            ),
        }
    }
}

impl pgn_traits::pgn::PgnBoard for Board {
    fn from_fen(_fen: &str) -> Result<Self, pgn::Error> {
        unimplemented!()
    }

    fn to_fen(&self) -> String {
        let mut f = String::new();
        squares_iterator()
            .map(|square| self[square].clone())
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

    fn move_from_san(&self, input: &str) -> Result<Self::Move, pgn::Error> {
        if input.len() < 2 {
            return Err(pgn::Error::new(
                pgn::ErrorKind::ParseError,
                "Input move too short.",
            ));
        }
        if !input.is_ascii() {
            return Err(pgn::Error::new(
                pgn::ErrorKind::ParseError,
                "Input move contained non-ascii characters.",
            ));
        }
        let first_char = input.chars().next().unwrap();
        match first_char {
            'a'..='e' if input.len() == 2 => {
                let square = Square::parse_square(input);
                Ok(Move::Place(Flat, square))
            }
            'a'..='e' if input.len() == 3 => {
                let square = Square::parse_square(&input[0..2]);
                let direction = Direction::parse(input.chars().nth(2).unwrap());
                // Moves in the simplified move notation always move one piece
                let movements = ArrayVec::from_iter(iter::once(Movement { pieces_to_take: 1 }));
                Ok(Move::Move(square, direction, StackMovement { movements }))
            }
            'C' if input.len() == 3 => Ok(Move::Place(Cap, Square::parse_square(&input[1..]))),
            'S' if input.len() == 3 => Ok(Move::Place(Wall, Square::parse_square(&input[1..]))),
            '1'..='9' if input.len() > 3 => {
                let square = Square::parse_square(&input[1..3]);
                let direction = Direction::parse(input.chars().nth(3).unwrap());
                let pieces_taken = first_char.to_digit(10).unwrap() as u8;
                let mut pieces_held = pieces_taken;

                let mut amounts_to_drop = input
                    .chars()
                    .skip(4)
                    .map(|ch| ch.to_digit(10).unwrap() as u8)
                    .collect::<Vec<u8>>();
                amounts_to_drop.pop(); //

                let mut movements = ArrayVec::new();
                movements.push(Movement {
                    pieces_to_take: pieces_taken,
                });

                for amount_to_drop in amounts_to_drop {
                    movements.push(Movement {
                        pieces_to_take: pieces_held - amount_to_drop,
                    });
                    pieces_held -= amount_to_drop;
                }
                Ok(Move::Move(square, direction, StackMovement { movements }))
            }
            _ => Err(pgn::Error::new(
                pgn::ErrorKind::ParseError,
                format!(
                    "Couldn't parse move \"{}\". Moves cannot start with {} and have length {}.",
                    input,
                    first_char,
                    input.len()
                ),
            )),
        }
    }

    fn move_to_san(&self, mv: &Self::Move) -> String {
        let mut string = String::new();
        write!(string, "{}", mv).unwrap();
        string
    }

    fn move_from_lan(&self, input: &str) -> Result<Self::Move, pgn::Error> {
        self.move_from_san(input)
    }

    fn move_to_lan(&self, mv: &Self::Move) -> String {
        self.move_to_san(mv)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub(crate) struct AbstractBoard<T> {
    raw: [[T; BOARD_SIZE]; BOARD_SIZE],
}

impl<T> Index<Square> for AbstractBoard<T> {
    type Output = T;

    fn index(&self, square: Square) -> &Self::Output {
        &self.raw[square.0 as usize % BOARD_SIZE][square.0 as usize / BOARD_SIZE]
    }
}

impl<T> IndexMut<Square> for AbstractBoard<T> {
    fn index_mut(&mut self, square: Square) -> &mut Self::Output {
        &mut self.raw[square.0 as usize % BOARD_SIZE][square.0 as usize / BOARD_SIZE]
    }
}

pub(crate) fn connected_components_graph(
    road_pieces: BitBoard,
    components: &mut AbstractBoard<u8>,
    id: &mut u8,
) {
    for square in squares_iterator() {
        if components[square] == 0 && road_pieces.get(square.0) {
            connect_component(road_pieces, components, square, *id);
            *id += 1;
        }
    }
}

fn connect_component(
    road_pieces: BitBoard,
    components: &mut AbstractBoard<u8>,
    square: Square,
    id: u8,
) {
    components[square] = id;
    for neighbour in square.neighbours() {
        if road_pieces.get(neighbour.0) && components[neighbour] == 0 {
            connect_component(road_pieces, components, neighbour, id);
        }
    }
}

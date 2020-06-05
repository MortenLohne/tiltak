//! Tak move generation, along with all required data types.

/// The size of the board. Only 5 works correctly for now.
pub const BOARD_SIZE: usize = 5;

pub const STARTING_CAPSTONES: u8 = 1;

use crate::bitboard::BitBoard;
use crate::board::Direction::*;
use crate::board::Piece::*;
use crate::board::Role::Flat;
use crate::board::Role::*;
use crate::mcts;
use crate::move_gen::sigmoid;
use arrayvec::ArrayVec;
use board_game_traits::board;
use board_game_traits::board::GameResult::{BlackWin, Draw, WhiteWin};
use board_game_traits::board::{Board as BoardTrait, EvalBoard as EvalBoardTrait};
use board_game_traits::board::{Color, GameResult};
use pgn_traits::pgn;
use std::cmp::Ordering;
use std::fmt::Write;
use std::iter::FromIterator;
#[cfg(test)]
use std::mem;
use std::ops::{Index, IndexMut};
use std::{fmt, iter, ops};

/// Extra items for tuning evaluation constants.
pub trait TunableBoard: BoardTrait {
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
        simple_moves: &mut Vec<<Self as BoardTrait>::Move>,
        moves: &mut Vec<(<Self as BoardTrait>::Move, mcts::Score)>,
    );

    fn probability_for_move(&self, params: &[f32], mv: &Self::Move, num_moves: usize) -> f32;

    fn coefficients_for_move(&self, coefficients: &mut [f32], mv: &Move, num_legal_moves: usize);
}

pub(crate) trait ColorTr {
    fn color() -> Color;

    fn stones_left(board: &Board) -> u8;

    fn capstones_left(board: &Board) -> u8;

    fn road_stones(board: &Board) -> BitBoard;

    fn blocking_stones(board: &Board) -> BitBoard;

    fn flat_piece() -> Piece;

    fn standing_piece() -> Piece;

    fn cap_piece() -> Piece;

    fn is_road_stone(piece: Piece) -> bool;

    fn piece_is_ours(piece: Piece) -> bool;
}

struct WhiteTr {}

impl ColorTr for WhiteTr {
    fn color() -> Color {
        Color::White
    }

    fn stones_left(board: &Board) -> u8 {
        board.white_stones_left
    }

    fn capstones_left(board: &Board) -> u8 {
        board.white_capstones_left
    }

    fn road_stones(board: &Board) -> BitBoard {
        board.white_road_pieces()
    }

    fn blocking_stones(board: &Board) -> BitBoard {
        board.white_blocking_pieces()
    }

    fn flat_piece() -> Piece {
        Piece::WhiteFlat
    }

    fn standing_piece() -> Piece {
        Piece::WhiteStanding
    }

    fn cap_piece() -> Piece {
        Piece::WhiteCap
    }

    fn is_road_stone(piece: Piece) -> bool {
        piece == WhiteFlat || piece == WhiteCap
    }

    fn piece_is_ours(piece: Piece) -> bool {
        piece == WhiteFlat || piece == WhiteStanding || piece == WhiteCap
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

    fn capstones_left(board: &Board) -> u8 {
        board.black_capstones_left
    }

    fn road_stones(board: &Board) -> BitBoard {
        board.black_road_pieces()
    }

    fn blocking_stones(board: &Board) -> BitBoard {
        board.black_blocking_pieces()
    }

    fn flat_piece() -> Piece {
        Piece::BlackFlat
    }

    fn standing_piece() -> Piece {
        Piece::BlackStanding
    }

    fn cap_piece() -> Piece {
        Piece::BlackCap
    }

    fn is_road_stone(piece: Piece) -> bool {
        piece == BlackFlat || piece == BlackCap
    }

    fn piece_is_ours(piece: Piece) -> bool {
        piece == BlackFlat || piece == BlackCap || piece == BlackStanding
    }
}

/// A location on the board. Can be used to index a `Board`.
#[derive(Clone, Copy, PartialEq, Eq)]
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

    fn parse_square(input: &str) -> Square {
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
pub enum Role {
    Flat,
    Standing,
    Cap,
}

/// One of the 6 game pieces in Tak. Each piece has one variant for each color.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Piece {
    WhiteFlat,
    BlackFlat,
    WhiteStanding,
    BlackStanding,
    WhiteCap,
    BlackCap,
}

impl Piece {
    pub fn from_role_color(role: Role, color: Color) -> Self {
        match (role, color) {
            (Flat, Color::White) => WhiteFlat,
            (Standing, Color::White) => WhiteStanding,
            (Cap, Color::White) => WhiteCap,
            (Flat, Color::Black) => BlackFlat,
            (Standing, Color::Black) => BlackStanding,
            (Cap, Color::Black) => BlackCap,
        }
    }

    pub fn role(self) -> Role {
        match self {
            WhiteFlat | BlackFlat => Flat,
            WhiteStanding | BlackStanding => Standing,
            WhiteCap | BlackCap => Cap,
        }
    }

    pub fn color(self) -> Color {
        match self {
            WhiteFlat | WhiteStanding | WhiteCap => Color::White,
            BlackFlat | BlackStanding | BlackCap => Color::Black,
        }
    }

    pub fn is_road_piece(self) -> bool {
        WhiteTr::is_road_stone(self) || BlackTr::is_road_stone(self)
    }

    pub fn flip_color(self) -> Self {
        match self {
            WhiteFlat => BlackFlat,
            BlackFlat => WhiteFlat,
            WhiteStanding => BlackStanding,
            BlackStanding => WhiteStanding,
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
            WhiteStanding => BlackStanding,
            BlackStanding => WhiteStanding,
            WhiteCap => BlackCap,
            BlackCap => WhiteCap,
        }
    }
}

/// The contents of a square on the board, consisting of zero or more pieces
#[derive(Clone, PartialEq, Eq, Debug, Default)]
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

/// An iterator over the pieces in a stack.
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
                Standing => write!(f, "S{}", square)?,
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
pub struct StackMovement {
    pub movements: ArrayVec<[Movement; BOARD_SIZE - 1]>,
}

/// Moving a stack of pieces consists of one or more `Movement`s
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Movement {
    pub pieces_to_take: u8,
}

/// Complete representation of a Tak position
#[derive(Clone, PartialEq, Eq)]
pub struct Board {
    cells: AbstractBoard<Stack>,
    to_move: Color,
    white_flat_stones: BitBoard,
    black_flat_stones: BitBoard,
    white_capstones: BitBoard,
    black_capstones: BitBoard,
    white_standing_stones: BitBoard,
    black_standing_stones: BitBoard,
    white_stones_left: u8,
    black_stones_left: u8,
    white_capstones_left: u8,
    black_capstones_left: u8,
    moves_played: u16,
    moves: Vec<Move>,
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
            white_flat_stones: Default::default(),
            black_flat_stones: Default::default(),
            white_capstones: Default::default(),
            black_capstones: Default::default(),
            white_standing_stones: Default::default(),
            black_standing_stones: Default::default(),
            white_stones_left: 21,
            black_stones_left: 21,
            white_capstones_left: STARTING_CAPSTONES,
            black_capstones_left: STARTING_CAPSTONES,
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
                            Some(WhiteStanding) => write!(f, "[W]")?,
                            Some(WhiteCap) => write!(f, "[C]")?,
                            Some(BlackFlat) => write!(f, "[b]")?,
                            Some(BlackStanding) => write!(f, "[B]")?,
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
            self.white_capstones_left, self.black_capstones_left
        )?;
        writeln!(f, "{} to move.", self.side_to_move())?;
        writeln!(f, "White road stones: {:b}", self.white_road_pieces().board)?;
        writeln!(f, "Black road stones: {:b}", self.black_road_pieces().board)?;
        Ok(())
    }
}

impl Board {
    pub(crate) fn white_road_pieces(&self) -> BitBoard {
        self.white_flat_stones | self.white_capstones
    }

    pub(crate) fn black_road_pieces(&self) -> BitBoard {
        self.black_flat_stones | self.black_capstones
    }

    pub(crate) fn white_blocking_pieces(&self) -> BitBoard {
        self.white_standing_stones | self.white_capstones
    }

    pub(crate) fn black_blocking_pieces(&self) -> BitBoard {
        self.black_standing_stones | self.black_capstones
    }

    pub fn white_flat_tops_count(&self) -> u8 {
        self.white_road_pieces().count() - (STARTING_CAPSTONES - self.white_capstones_left)
    }

    pub fn black_flat_tops_count(&self) -> u8 {
        self.black_road_pieces().count() - (STARTING_CAPSTONES - self.black_capstones_left)
    }

    /// Number of moves/plies played in the game
    pub fn half_moves_played(&self) -> u16 {
        self.moves_played
    }

    /// All the moves played in the game
    pub fn moves(&self) -> &Vec<Move> {
        &self.moves
    }

    #[cfg(test)]
    pub fn flip_board_y(&self) -> Board {
        let mut new_board = self.clone();
        for x in 0..BOARD_SIZE as u8 {
            for y in 0..BOARD_SIZE as u8 {
                new_board[Square(y * BOARD_SIZE as u8 + x)] =
                    self[Square((BOARD_SIZE as u8 - y - 1) * BOARD_SIZE as u8 + x)].clone();
            }
        }
        new_board.bitboards_from_scratch();
        new_board
    }

    #[cfg(test)]
    pub fn flip_board_x(&self) -> Board {
        let mut new_board = self.clone();
        for x in 0..BOARD_SIZE as u8 {
            for y in 0..BOARD_SIZE as u8 {
                new_board[Square(y * BOARD_SIZE as u8 + x)] =
                    self[Square(y * BOARD_SIZE as u8 + (BOARD_SIZE as u8 - x - 1))].clone();
            }
        }
        new_board.bitboards_from_scratch();
        new_board
    }

    #[cfg(test)]
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
        new_board.bitboards_from_scratch();
        new_board
    }

    #[cfg(test)]
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
            &mut new_board.white_capstones_left,
            &mut new_board.black_capstones_left,
        );
        mem::swap(
            &mut new_board.white_flat_stones,
            &mut new_board.black_flat_stones,
        );
        mem::swap(
            &mut new_board.white_standing_stones,
            &mut new_board.black_standing_stones,
        );
        mem::swap(
            &mut new_board.white_capstones,
            &mut new_board.black_capstones,
        );
        new_board.to_move = !new_board.to_move;
        new_board
    }

    #[cfg(test)]
    pub fn rotations_and_symmetries(&self) -> Vec<Board> {
        vec![
            self.flip_board_x(),
            self.flip_board_y(),
            self.rotate_board(),
            self.rotate_board().rotate_board(),
            self.rotate_board().rotate_board().rotate_board(),
            self.rotate_board().flip_board_x(),
            self.rotate_board().flip_board_y(),
            self.flip_colors(),
        ]
    }

    /// Move generation that includes a heuristic probability of each move being played.
    ///
    /// # Arguments
    ///
    /// * `simple_moves` - An empty vector to temporarily store moves without probabilities. The vector will be emptied before the function returns, and only serves to re-use allocated memory.
    /// * `moves` A vector to place the moves and associated probabilities.
    pub fn generate_moves_with_probabilities(
        &self,
        simple_moves: &mut Vec<Move>,
        moves: &mut Vec<(Move, mcts::Score)>,
    ) {
        self.generate_moves_with_params(Board::POLICY_PARAMS, simple_moves, moves)
    }

    fn count_all_pieces(&self) -> u8 {
        self.cells
            .raw
            .iter()
            .flatten()
            .map(|stack: &Stack| stack.len())
            .sum()
    }

    fn bitboards_from_scratch(&mut self) {
        self.white_flat_stones = BitBoard::empty();
        self.black_flat_stones = BitBoard::empty();
        self.white_standing_stones = BitBoard::empty();
        self.black_standing_stones = BitBoard::empty();
        self.white_capstones = BitBoard::empty();
        self.black_capstones = BitBoard::empty();
        for square in squares_iterator() {
            match self[square].top_stone() {
                Some(WhiteFlat) => self.white_flat_stones = self.white_flat_stones.set(square.0),
                Some(BlackFlat) => self.black_flat_stones = self.black_flat_stones.set(square.0),
                Some(WhiteStanding) => {
                    self.white_standing_stones = self.white_standing_stones.set(square.0)
                }
                Some(BlackStanding) => {
                    self.black_standing_stones = self.black_standing_stones.set(square.0)
                }
                Some(WhiteCap) => self.white_capstones = self.white_capstones.set(square.0),
                Some(BlackCap) => self.black_capstones = self.black_capstones.set(square.0),
                None => (),
            }
        }
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

    fn static_eval_game_phase(&self, coefficients: &mut [f32]) {
        const FLAT_PSQT: usize = 0;
        const STAND_PSQT: usize = FLAT_PSQT + 6;
        const CAP_PSQT: usize = STAND_PSQT + 6;

        for square in squares_iterator() {
            if let Some(piece) = self[square].top_stone() {
                let i = square.0 as usize;
                match piece {
                    WhiteFlat => coefficients[FLAT_PSQT + SQUARE_SYMMETRIES[i]] += 1.0,
                    BlackFlat => coefficients[FLAT_PSQT + SQUARE_SYMMETRIES[i]] -= 1.0,
                    WhiteStanding => coefficients[STAND_PSQT + SQUARE_SYMMETRIES[i]] += 1.0,
                    BlackStanding => coefficients[STAND_PSQT + SQUARE_SYMMETRIES[i]] -= 1.0,
                    WhiteCap => coefficients[CAP_PSQT + SQUARE_SYMMETRIES[i]] += 1.0,
                    BlackCap => coefficients[CAP_PSQT + SQUARE_SYMMETRIES[i]] -= 1.0,
                }
            }
        }

        const SIDE_TO_MOVE: usize = CAP_PSQT + 6;
        const FLATSTONE_LEAD: usize = SIDE_TO_MOVE + 3;

        // Give the side to move a bonus/malus depending on flatstone lead
        let white_flatstone_lead =
            self.white_flat_tops_count() as i8 - self.black_flat_tops_count() as i8;

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

        coefficients[SIDE_TO_MOVE + 1] =
            self.side_to_move().multiplier() as f32 * middlegame_scale_factor;
        coefficients[FLATSTONE_LEAD + 1] = white_flatstone_lead as f32 * middlegame_scale_factor;

        coefficients[SIDE_TO_MOVE + 2] =
            self.side_to_move().multiplier() as f32 * endgame_scale_factor;
        coefficients[FLATSTONE_LEAD + 2] = white_flatstone_lead as f32 * endgame_scale_factor;

        const PIECES_IN_OUR_STACK: usize = FLATSTONE_LEAD + 3;
        const PIECES_IN_THEIR_STACK: usize = PIECES_IN_OUR_STACK + 1;
        const CAPSTONE_OVER_OWN_PIECE: usize = PIECES_IN_THEIR_STACK + 1;
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
                stack
                    .clone()
                    .into_iter()
                    .take(stack.len() as usize - 1)
                    .for_each(|piece| {
                        if piece.color() == controlling_player {
                            coefficients[PIECES_IN_OUR_STACK] += color_factor
                        } else {
                            coefficients[PIECES_IN_THEIR_STACK] -= color_factor
                        }
                    });

                // Extra bonus for having your capstone over your own piece
                if top_stone.role() == Cap
                    && stack.get(stack.len() - 2).unwrap().color() == controlling_player
                {
                    coefficients[CAPSTONE_OVER_OWN_PIECE] += color_factor;
                }

                match top_stone.role() {
                    Cap => coefficients[CAPSTONE_ON_STACK] += color_factor,
                    Flat => (),
                    Standing => coefficients[STANDING_STONE_ON_STACK] += color_factor,
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
                                Standing => {
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

        // Number of pieces in each rank/file
        const NUM_RANKS_FILES_OCCUPIED: usize = CAPSTONE_NEXT_TO_OUR_STACK + 1;
        // Number of ranks/files with at least one road stone
        const RANK_FILE_CONTROL: usize = NUM_RANKS_FILES_OCCUPIED + 6;

        let mut num_ranks_occupied_white = 0;
        let mut num_files_occupied_white = 0;
        let mut num_ranks_occupied_black = 0;
        let mut num_files_occupied_black = 0;

        for i in 0..BOARD_SIZE as u8 {
            num_ranks_occupied_white +=
                self.rank_score::<WhiteTr, BlackTr>(i, coefficients, RANK_FILE_CONTROL);
            num_ranks_occupied_black +=
                self.rank_score::<BlackTr, WhiteTr>(i, coefficients, RANK_FILE_CONTROL);
        }

        for i in 0..BOARD_SIZE as u8 {
            num_files_occupied_white +=
                self.file_score::<WhiteTr, BlackTr>(i, coefficients, RANK_FILE_CONTROL);
            num_files_occupied_black +=
                self.file_score::<BlackTr, WhiteTr>(i, coefficients, RANK_FILE_CONTROL);
        }

        coefficients[NUM_RANKS_FILES_OCCUPIED + num_ranks_occupied_white] += 1.0;
        coefficients[NUM_RANKS_FILES_OCCUPIED + num_files_occupied_white] += 1.0;
        coefficients[NUM_RANKS_FILES_OCCUPIED + num_ranks_occupied_black] -= 1.0;
        coefficients[NUM_RANKS_FILES_OCCUPIED + num_files_occupied_black] -= 1.0;

        const _NEXT_CONST: usize = RANK_FILE_CONTROL + 10;
        assert_eq!(_NEXT_CONST, coefficients.len());
    }

    fn rank_score<Us: ColorTr, Them: ColorTr>(
        &self,
        rank_id: u8,
        coefficients: &mut [f32],
        rank_file_control: usize,
    ) -> usize {
        let rank = Us::road_stones(self).rank(rank_id);
        let num_ranks_occupied = if rank.is_empty() { 0 } else { 1 };
        let road_pieces_in_rank = rank.count();
        coefficients[rank_file_control + road_pieces_in_rank as usize] +=
            Us::color().multiplier() as f32;

        let block_rank_file_with_capstone = rank_file_control + 6;
        let block_rank_file_with_standing_stone = block_rank_file_with_capstone + 2;

        // Give them a bonus for capstones/standing stones in our strong ranks
        if road_pieces_in_rank >= 3 {
            for file_id in 0..BOARD_SIZE as u8 {
                let square = Square::from_rank_file(rank_id, file_id);
                if self[square].top_stone() == Some(Them::cap_piece()) {
                    coefficients
                        [block_rank_file_with_capstone + road_pieces_in_rank as usize - 3] +=
                        Them::color().multiplier() as f32
                } else if self[square].top_stone() == Some(Them::standing_piece()) {
                    coefficients
                        [block_rank_file_with_standing_stone + road_pieces_in_rank as usize - 3] +=
                        Them::color().multiplier() as f32
                }
            }
        }
        num_ranks_occupied
    }

    fn file_score<Us: ColorTr, Them: ColorTr>(
        &self,
        file_id: u8,
        coefficients: &mut [f32],
        rank_file_control: usize,
    ) -> usize {
        let rank = Us::road_stones(self).file(file_id);
        let num_ranks_occupied = if rank.is_empty() { 0 } else { 1 };
        let road_pieces_in_rank = rank.count();
        coefficients[rank_file_control + road_pieces_in_rank as usize] +=
            Us::color().multiplier() as f32;

        let block_rank_file_with_capstone = rank_file_control + 6;
        let block_rank_file_with_standing_stone = block_rank_file_with_capstone + 2;

        // Give them a bonus for capstones/standing stones in our strong files
        if road_pieces_in_rank >= 3 {
            for rank_id in 0..BOARD_SIZE as u8 {
                let square = Square::from_rank_file(rank_id, file_id);
                if self[square].top_stone() == Some(Them::cap_piece()) {
                    coefficients
                        [block_rank_file_with_capstone + road_pieces_in_rank as usize - 3] +=
                        Them::color().multiplier() as f32
                } else if self[square].top_stone() == Some(Them::standing_piece()) {
                    coefficients
                        [block_rank_file_with_standing_stone + road_pieces_in_rank as usize - 3] +=
                        Them::color().multiplier() as f32
                }
            }
        }
        num_ranks_occupied
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
                match (role, color_to_place) {
                    (Flat, Color::White) => {
                        self.white_flat_stones = self.white_flat_stones.set(to.0)
                    }
                    (Flat, Color::Black) => {
                        self.black_flat_stones = self.black_flat_stones.set(to.0)
                    }
                    (Standing, Color::White) => {
                        self.white_standing_stones = self.white_standing_stones.set(to.0)
                    }
                    (Standing, Color::Black) => {
                        self.black_standing_stones = self.black_standing_stones.set(to.0)
                    }
                    (Cap, Color::White) => self.white_capstones = self.white_capstones.set(to.0),
                    (Cap, Color::Black) => self.black_capstones = self.black_capstones.set(to.0),
                }

                match (color_to_place, role) {
                    (Color::White, Flat) => self.white_stones_left -= 1,
                    (Color::White, Standing) => self.white_stones_left -= 1,
                    (Color::White, Cap) => self.white_capstones_left -= 1,
                    (Color::Black, Flat) => self.black_stones_left -= 1,
                    (Color::Black, Standing) => self.black_stones_left -= 1,
                    (Color::Black, Cap) => self.black_capstones_left -= 1,
                }
                ReverseMove::Place(to)
            }
            Move::Move(mut from, direction, stack_movement) => {
                let mut pieces_left_behind: ArrayVec<[u8; BOARD_SIZE - 1]> = ArrayVec::new();
                let mut flattens_stone = false;
                for Movement { pieces_to_take } in stack_movement.movements {
                    let to = from.go_direction(direction).unwrap();

                    if self[to].top_stone.map(Piece::role) == Some(Standing) {
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

                self.bitboards_from_scratch();

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
                - self.white_capstones_left
                - self.black_capstones_left,
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
                    WhiteFlat | WhiteStanding => {
                        self.white_flat_stones = self.white_flat_stones.clear(square.0);
                        self.white_standing_stones = self.white_standing_stones.clear(square.0);
                        self.white_stones_left += 1
                    }
                    WhiteCap => {
                        self.white_capstones = self.white_capstones.clear(square.0);
                        self.white_capstones_left += 1
                    }
                    BlackFlat | BlackStanding => {
                        self.black_flat_stones = self.black_flat_stones.clear(square.0);
                        self.black_standing_stones = self.black_standing_stones.clear(square.0);
                        self.black_stones_left += 1
                    }
                    BlackCap => {
                        self.black_capstones = self.black_capstones.clear(square.0);
                        self.black_capstones_left += 1
                    }
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
                        Color::White => self[from].replace_top(WhiteStanding),
                        Color::Black => self[from].replace_top(BlackStanding),
                    };
                };
                self.bitboards_from_scratch();
            }
        }
        self.moves.pop();
        self.moves_played -= 1;
        self.to_move = !self.to_move;
    }

    fn game_result(&self) -> Option<GameResult> {
        let (components, highest_component_id) = match self.side_to_move() {
            Color::White => connected_components_graph(self.black_road_pieces()),
            Color::Black => connected_components_graph(self.white_road_pieces()),
        };

        if let Some(square) = is_win_by_road(&components, highest_component_id) {
            debug_assert!(self[square].top_stone().unwrap().is_road_piece());
            debug_assert_ne!(
                self[square].top_stone().unwrap().color(),
                self.side_to_move()
            );
            return match self.side_to_move() {
                Color::White => Some(BlackWin),
                Color::Black => Some(WhiteWin),
            };
        };

        if (self.white_stones_left == 0 && self.white_capstones_left == 0)
            || (self.black_stones_left == 0 && self.black_capstones_left == 0)
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
    #[allow(clippy::unreadable_literal)]
    const VALUE_PARAMS: &'static [f32] = &[
        -0.25570056,
        -0.06306428,
        -0.050129674,
        0.04116685,
        0.09316665,
        0.12340328,
        0.75012887,
        0.65594965,
        0.7024855,
        0.75259584,
        0.84324247,
        0.79217625,
        0.050678793,
        0.47513855,
        0.58207846,
        0.82165027,
        0.94676894,
        1.2289112,
        1.0558432,
        0.64327145,
        0.93969756,
        0.4018749,
        -0.42575312,
        -0.01285331,
        1.0200886,
        0.6514834,
        0.23788722,
        0.4270889,
        0.38664544,
        -0.028407635,
        -0.13549738,
        -0.13895814,
        0.7804223,
        -0.82711303,
        -0.6252435,
        -0.3598254,
        0.17267509,
        0.85429937,
        -1.3655918,
        -1.0816205,
        -0.24295154,
        0.75123227,
        1.9697832,
        0.089783624,
        0.12940928,
        0.33033335,
        0.018162066,
        0.24058251,
    ];
    #[allow(clippy::unreadable_literal)]
    const POLICY_PARAMS: &'static [f32] = &[
        0.9850799,
        -0.3714848,
        0.10023011,
        0.6166247,
        0.79548424,
        1.4695351,
        1.1245415,
        -2.4836218,
        -2.8152757,
        -2.1377156,
        -2.2278273,
        -1.4414095,
        -0.29557583,
        -0.5848462,
        -0.989338,
        -0.7904228,
        -0.5100603,
        2.0650306,
        6.901775,
        0.65254486,
        0.64331657,
        0.2204491,
        0.32570517,
        0.014503168,
        -0.042288158,
        0.30570242,
        1.1221241,
        3.2166786,
        -0.002451834,
        0.0028005268,
        0.009542956,
        0.09157708,
        -0.13455543,
        1.7832398,
        -0.21330816,
        0.7393393,
        1.1862655,
        2.2540193,
        0.004225409,
        0.42277747,
    ];

    fn static_eval_coefficients(&self, coefficients: &mut [f32]) {
        debug_assert!(self.game_result().is_none());

        self.static_eval_game_phase(coefficients)
    }

    fn generate_moves_with_params(
        &self,
        params: &[f32],
        simple_moves: &mut Vec<Self::Move>,
        moves: &mut Vec<(Self::Move, f32)>,
    ) {
        debug_assert!(simple_moves.is_empty());
        self.generate_moves(simple_moves);
        match self.side_to_move() {
            Color::White => self.generate_moves_with_probabilities_colortr::<WhiteTr, BlackTr>(
                params,
                simple_moves,
                moves,
            ),
            Color::Black => self.generate_moves_with_probabilities_colortr::<BlackTr, WhiteTr>(
                params,
                simple_moves,
                moves,
            ),
        }
    }

    fn probability_for_move(&self, params: &[f32], mv: &Move, num_moves: usize) -> f32 {
        let mut coefficients = vec![0.0; Self::POLICY_PARAMS.len()];
        self.coefficients_for_move(&mut coefficients, mv, num_moves);
        let total_value: f32 = coefficients.iter().zip(params).map(|(c, p)| c * p).sum();

        sigmoid(total_value)
    }

    fn coefficients_for_move(&self, coefficients: &mut [f32], mv: &Move, num_legal_moves: usize) {
        match self.side_to_move() {
            Color::White => self.coefficients_for_move_colortr::<WhiteTr, BlackTr>(
                coefficients,
                mv,
                num_legal_moves,
            ),
            Color::Black => self.coefficients_for_move_colortr::<BlackTr, WhiteTr>(
                coefficients,
                mv,
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
                    Some(WhiteStanding) => write!(f, "W"),
                    Some(WhiteCap) => write!(f, "C"),
                    Some(BlackFlat) => write!(f, "b"),
                    Some(BlackStanding) => write!(f, "B"),
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
            'S' if input.len() == 3 => Ok(Move::Place(Standing, Square::parse_square(&input[1..]))),
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
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

pub(crate) fn connected_components_graph(road_pieces: BitBoard) -> (AbstractBoard<u8>, u8) {
    let mut components: AbstractBoard<u8> = Default::default();
    let mut id = 1;

    for square in squares_iterator() {
        if components[square] == 0 && road_pieces.get(square.0) {
            connect_component(road_pieces, &mut components, square, id);
            id += 1;
        }
    }
    (components, id)
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

/// Check if either side has completed a road
/// Returns one of the winning squares in the road
pub(crate) fn is_win_by_road(
    components: &AbstractBoard<u8>,
    highest_component_id: u8,
) -> Option<Square> {
    for id in 1..highest_component_id {
        if (components.raw[0].iter().any(|&cell| cell == id)
            && components.raw[BOARD_SIZE - 1]
                .iter()
                .any(|&cell| cell == id))
            || ((0..BOARD_SIZE).any(|y| components.raw[y][0] == id)
                && (0..BOARD_SIZE).any(|y| components.raw[y][BOARD_SIZE - 1] == id))
        {
            let square = squares_iterator().find(|&sq| components[sq] == id).unwrap();
            return Some(square);
        }
    }
    None
}

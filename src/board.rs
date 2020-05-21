//! Tak move generation, along with all required data types.

/// The size of the board. Only 5 works correctly for now.
pub const BOARD_SIZE: usize = 5;

use crate::bitboard::BitBoard;
use crate::board::Direction::*;
use crate::board::Piece::*;
use crate::board::Role::Flat;
use crate::board::Role::*;
use crate::mcts;
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
}

pub(crate) trait ColorTr {
    fn color() -> Color;

    fn stones_left(board: &Board) -> u8;

    fn capstones_left(board: &Board) -> u8;

    fn road_stones(board: &Board) -> BitBoard;

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
        board.white_road_pieces
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
        board.black_road_pieces
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
    white_road_pieces: BitBoard,
    black_road_pieces: BitBoard,
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
            white_road_pieces: BitBoard::default(),
            black_road_pieces: BitBoard::default(),
            white_stones_left: 21,
            black_stones_left: 21,
            white_capstones_left: 1,
            black_capstones_left: 1,
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
        writeln!(f, "White road stones: {:b}", self.white_road_pieces.board)?;
        writeln!(f, "Black road stones: {:b}", self.black_road_pieces.board)?;
        Ok(())
    }
}

impl Board {
    pub(crate) fn white_road_pieces(&self) -> BitBoard {
        self.white_road_pieces
    }

    pub(crate) fn black_road_pieces(&self) -> BitBoard {
        self.black_road_pieces
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
        new_board.black_road_pieces = new_board.black_road_pieces_from_scratch();
        new_board.white_road_pieces = new_board.white_road_pieces_from_scratch();
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
        new_board.black_road_pieces = new_board.black_road_pieces_from_scratch();
        new_board.white_road_pieces = new_board.white_road_pieces_from_scratch();
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
        new_board.black_road_pieces = new_board.black_road_pieces_from_scratch();
        new_board.white_road_pieces = new_board.white_road_pieces_from_scratch();
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
            &mut new_board.white_road_pieces,
            &mut new_board.black_road_pieces,
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

    fn white_road_pieces_from_scratch(&self) -> BitBoard {
        let mut bitboard = BitBoard::empty();
        for square in squares_iterator() {
            if self[square].top_stone.map(WhiteTr::is_road_stone) == Some(true) {
                bitboard = bitboard.set(square.0);
            }
        }
        bitboard
    }

    fn black_road_pieces_from_scratch(&self) -> BitBoard {
        let mut bitboard = BitBoard::empty();
        for square in squares_iterator() {
            if self[square].top_stone.map(BlackTr::is_road_stone) == Some(true) {
                bitboard = bitboard.set(square.0);
            }
        }
        bitboard
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

        const TO_MOVE: usize = CAP_PSQT + 6;

        match self.side_to_move() {
            Color::White => coefficients[TO_MOVE] = 1.0,
            Color::Black => coefficients[TO_MOVE] = -1.0,
        };

        const PIECES_IN_OUR_STACK: usize = TO_MOVE + 1;
        const PIECES_IN_THEIR_STACK: usize = PIECES_IN_OUR_STACK + 1;
        const CAPSTONE_OVER_OWN_PIECE: usize = PIECES_IN_THEIR_STACK + 1;
        const CAPSTONE_ON_STACK: usize = CAPSTONE_OVER_OWN_PIECE + 1;
        const STANDING_STONE_ON_STACK: usize = CAPSTONE_ON_STACK + 1;

        squares_iterator()
            .map(|sq| &self[sq])
            .filter(|stack| stack.len() > 1)
            .for_each(|stack| {
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
            });

        // Number of pieces in each rank/file
        const RANK_FILE_CONTROL: usize = STANDING_STONE_ON_STACK + 1;
        // Number of ranks/files with at least one road stone
        const NUM_RANKS_FILES_OCCUPIED: usize = RANK_FILE_CONTROL + 6;

        let mut num_ranks_occupied_white = 0;
        let mut num_files_occupied_white = 0;
        let mut num_ranks_occupied_black = 0;
        let mut num_files_occupied_black = 0;

        for rank in (0..BOARD_SIZE as u8).map(|i| self.white_road_pieces().rank(i)) {
            num_ranks_occupied_white += if rank.is_empty() { 0 } else { 1 };
            coefficients[RANK_FILE_CONTROL + rank.count() as usize] += 1.0;
        }

        for file in (0..BOARD_SIZE as u8).map(|i| self.white_road_pieces().file(i)) {
            num_files_occupied_white += if file.is_empty() { 0 } else { 1 };
            coefficients[RANK_FILE_CONTROL + file.count() as usize] += 1.0;
        }

        for rank in (0..BOARD_SIZE as u8).map(|i| self.black_road_pieces().rank(i)) {
            num_ranks_occupied_black += if rank.is_empty() { 0 } else { 1 };
            coefficients[RANK_FILE_CONTROL + rank.count() as usize] -= 1.0;
        }

        for file in (0..BOARD_SIZE as u8).map(|i| self.black_road_pieces().file(i)) {
            num_files_occupied_black += if file.is_empty() { 0 } else { 1 };
            coefficients[RANK_FILE_CONTROL + file.count() as usize] -= 1.0;
        }

        coefficients[NUM_RANKS_FILES_OCCUPIED + num_ranks_occupied_white] += 1.0;
        coefficients[NUM_RANKS_FILES_OCCUPIED + num_files_occupied_white] += 1.0;
        coefficients[NUM_RANKS_FILES_OCCUPIED + num_ranks_occupied_black] -= 1.0;
        coefficients[NUM_RANKS_FILES_OCCUPIED + num_files_occupied_black] -= 1.0;

        const _NEXT_CONST: usize = NUM_RANKS_FILES_OCCUPIED + 6;
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
                if role != Standing {
                    match color_to_place {
                        Color::White => self.white_road_pieces = self.white_road_pieces.set(to.0),
                        Color::Black => self.black_road_pieces = self.black_road_pieces.set(to.0),
                    };
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

                self.white_road_pieces = self.white_road_pieces_from_scratch();
                self.black_road_pieces = self.black_road_pieces_from_scratch();

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

        debug_assert_eq!(
            self.white_road_pieces,
            self.white_road_pieces_from_scratch()
        );
        debug_assert_eq!(
            self.black_road_pieces,
            self.black_road_pieces_from_scratch()
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

                self.white_road_pieces = self.white_road_pieces.clear(square.0);
                self.black_road_pieces = self.black_road_pieces.clear(square.0);

                match piece {
                    WhiteFlat | WhiteStanding => self.white_stones_left += 1,
                    WhiteCap => self.white_capstones_left += 1,
                    BlackFlat | BlackStanding => self.black_stones_left += 1,
                    BlackCap => self.black_capstones_left += 1,
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
                self.white_road_pieces = self.white_road_pieces_from_scratch();
                self.black_road_pieces = self.black_road_pieces_from_scratch();
            }
        }
        debug_assert_eq!(
            self.white_road_pieces,
            self.white_road_pieces_from_scratch()
        );
        debug_assert_eq!(
            self.black_road_pieces,
            self.black_road_pieces_from_scratch()
        );
        self.moves.pop();
        self.moves_played -= 1;
        self.to_move = !self.to_move;
    }

    fn game_result(&self) -> Option<GameResult> {
        let (components, highest_component_id) = match self.side_to_move() {
            Color::White => connected_components_graph(self.black_road_pieces),
            Color::Black => connected_components_graph(self.white_road_pieces),
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
        // If the only legal move is to suicidally place our capstone, we lose
        } else if (self.side_to_move() == Color::White
            && self.white_stones_left == 0
            && self.white_capstones_left == 1)
            || (self.side_to_move() == Color::Black
                && self.black_stones_left == 0
                && self.black_capstones_left == 1)
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
            match self.side_to_move() {
                Color::White => {
                    if black_points > white_points {
                        for square in squares_iterator() {
                            if let Some(piece) = self[square].top_stone() {
                                if piece.color() == Color::White
                                    && square
                                        .neighbours()
                                        .filter_map(|sq| self[sq].top_stone())
                                        .any(|piece| piece.role() == Flat)
                                {
                                    return None;
                                }
                            }
                        }
                        Some(BlackWin)
                    } else {
                        None
                    }
                }
                Color::Black => {
                    if white_points > black_points {
                        for square in squares_iterator() {
                            if let Some(piece) = self[square].top_stone() {
                                if piece.color() == Color::Black
                                    && square
                                        .neighbours()
                                        .filter_map(|sq| self[sq].top_stone())
                                        .any(|piece| piece.role() == Flat)
                                {
                                    return None;
                                }
                            }
                        }
                        Some(WhiteWin)
                    } else {
                        None
                    }
                }
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
        -0.0149931805,
        0.25723538,
        0.2940058,
        0.4238666,
        0.52233166,
        0.23532094,
        0.46646726,
        1.0888499,
        0.93079275,
        1.1300979,
        1.2846224,
        0.7606911,
        -0.32218972,
        -0.057136543,
        -0.04489895,
        0.29769522,
        0.6452908,
        1.1998287,
        1.1865233,
        1.470002,
        1.0057858,
        -0.11868016,
        0.91616416,
        0.93310755,
        -1.8471444,
        -1.4468176,
        -0.56441724,
        0.6747622,
        2.052937,
        -0.05114726,
        0.8312374,
        -0.99406725,
        -0.6882456,
        -0.5216054,
        -0.038465776,
        0.62074935,
    ];
    #[allow(clippy::unreadable_literal)]
    const POLICY_PARAMS: &'static [f32] = &[
        -0.22394775,
        0.34828594,
        0.5422076,
        1.1771424,
        1.4986687,
        0.97388643,
        -4.086922,
        -3.4035506,
        -3.1640594,
        -3.1151621,
        -1.7741977,
        -2.3559997,
        -3.1579914,
        -3.0281982,
        -1.5865799,
        -0.90471375,
        1.8511987,
        6.350537,
        0.9285992,
        0.004892543,
        1.0836914,
        0.10171267,
        1.6477052,
        -0.4607415,
        1.4072983,
        2.4922855,
        0.77424026,
        -2.9266522,
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
        match self.side_to_move() {
            Color::White => {
                self.probability_for_move_colortr::<WhiteTr, BlackTr>(params, mv, num_moves)
            }
            Color::Black => {
                self.probability_for_move_colortr::<BlackTr, WhiteTr>(params, mv, num_moves)
            }
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

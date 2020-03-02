pub const BOARD_SIZE: usize = 5;

use crate::bitboard::BitBoard;
use crate::board::Direction::*;
use crate::board::Piece::*;
use crate::board::Role::Flat;
use crate::board::Role::*;
use arrayvec::ArrayVec;
use board_game_traits::board;
use board_game_traits::board::GameResult::{BlackWin, Draw, WhiteWin};
use board_game_traits::board::{Board as BoardTrait, EvalBoard as EvalBoardTrait};
use board_game_traits::board::{Color, GameResult};
use pgn_traits::pgn;
use smallvec::alloc::fmt::{Error, Formatter};
use std::cmp::Ordering;
use std::fmt::Debug;
use std::fmt::Write;
use std::ops::{Index, IndexMut};
use std::{fmt, ops};

pub trait ColorTr {
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
            (input.chars().nth(0).unwrap() as u8 - b'a')
                + (BOARD_SIZE as u8 + b'0' - input.chars().nth(1).unwrap() as u8)
                    * BOARD_SIZE as u8,
        )
    }
}

impl fmt::Display for Square {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}", (self.file() + b'a') as char)?;
        write!(f, "{}", BOARD_SIZE as u8 - self.rank())?;
        Ok(())
    }
}

impl fmt::Debug for Square {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}", self)
    }
}

pub fn board_iterator() -> impl Iterator<Item = Square> {
    (0..(BOARD_SIZE * BOARD_SIZE)).map(|i| Square(i as u8))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Flat,
    Standing,
    Cap,
}

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
    /// Any piece already on the stack will be flattened, including capstones
    pub fn push(&mut self, piece: Piece) {
        if self.height > 0 && self.top_stone.unwrap().color() == Color::White {
            self.bitboard = self.bitboard.set(self.height - 1);
        }
        self.top_stone = Some(piece);
        self.height += 1;
    }

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

#[derive(Clone, PartialEq, Eq)]
pub enum Move {
    Place(Piece, Square),
    Move(Square, Direction, StackMovement), // Number of stones to take
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        match self {
            Move::Place(piece, square) => match piece {
                WhiteCap | BlackCap => write!(f, "C{}", square)?,
                WhiteFlat | BlackFlat => write!(f, "{}", square)?,
                WhiteStanding | BlackStanding => write!(f, "S{}", square)?,
            },
            Move::Move(square, direction, stack_movements) => {
                write!(
                    f,
                    "{}{}",
                    stack_movements.movements[0].pieces_to_take, square
                )
                .unwrap();
                match direction {
                    North => f.write_char('+')?,
                    West => f.write_char('<')?,
                    East => f.write_char('>')?,
                    South => f.write_char('-')?,
                }
                for movement in stack_movements.movements.iter().skip(1) {
                    write!(f, "{}", movement.pieces_to_take).unwrap();
                }
            }
        }
        Ok(())
    }
}

impl fmt::Debug for Move {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}", self)
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ReverseMove {
    Place(Square),
    Move(Square, Direction, StackMovement, bool),
}

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

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StackMovement {
    pub movements: ArrayVec<[Movement; BOARD_SIZE - 1]>,
}

/// Moving a stack of pieces consists of one or more `Movement`s
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Movement {
    pub pieces_to_take: u8,
}

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
        }
    }
}

impl Debug for Board {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        for y in 0..BOARD_SIZE {
            for print_row in 0..3 {
                for x in 0..BOARD_SIZE {
                    for print_column in 0..3 {
                        match self.cells.raw[y][x].get(print_column * 3 + print_row) {
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
        writeln!(f, "{} to move.", self.to_move)?;
        writeln!(f, "White road stones: {:b}", self.white_road_pieces.board)?;
        writeln!(f, "Black road stones: {:b}", self.black_road_pieces.board)?;
        Ok(())
    }
}

impl Board {
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
                new_board[Square(y * BOARD_SIZE as u8 + x)] =
                    self[Square(x * BOARD_SIZE as u8 + y)].clone();
            }
        }
        new_board.black_road_pieces = new_board.black_road_pieces_from_scratch();
        new_board.white_road_pieces = new_board.white_road_pieces_from_scratch();
        new_board
    }

    pub fn generate_moves_with_probabilities(
        &self,
        simple_moves: &mut Vec<Move>,
        moves: &mut Vec<(Move, f64)>,
    ) {
        debug_assert!(simple_moves.is_empty());
        self.generate_moves(simple_moves);
        let average = 1.0 / simple_moves.len() as f64;
        moves.extend(simple_moves.drain(..).map(|mv| (mv, average)));
    }

    pub fn count_all_stones(&self) -> u8 {
        self.cells
            .raw
            .iter()
            .flatten()
            .map(|stack: &Stack| stack.len())
            .sum()
    }

    pub fn white_road_pieces(&self) -> BitBoard {
        self.white_road_pieces
    }

    pub fn black_road_pieces(&self) -> BitBoard {
        self.black_road_pieces
    }

    fn white_road_pieces_from_scratch(&self) -> BitBoard {
        let mut bitboard = BitBoard::empty();
        for square in board_iterator() {
            if self[square].top_stone.map(WhiteTr::is_road_stone) == Some(true) {
                bitboard = bitboard.set(square.0);
            }
        }
        bitboard
    }

    fn black_road_pieces_from_scratch(&self) -> BitBoard {
        let mut bitboard = BitBoard::empty();
        for square in board_iterator() {
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

    fn generate_moves(&self, moves: &mut Vec<Self::Move>) {
        debug_assert!(
            self.game_result().is_none(),
            "Tried to generate moves on position with {:?} on\n{:?}",
            self.game_result(),
            self
        );
        match self.side_to_move() {
            Color::White => self.generate_moves_colortr::<WhiteTr>(moves),
            Color::Black => self.generate_moves_colortr::<BlackTr>(moves),
        }
    }

    fn do_move(&mut self, mv: Self::Move) -> Self::ReverseMove {
        let reverse_move = match mv.clone() {
            Move::Place(piece, to) => {
                self[to].push(piece);
                if piece.role() != Standing {
                    match self.side_to_move() {
                        Color::White => self.white_road_pieces = self.white_road_pieces.set(to.0),
                        Color::Black => self.black_road_pieces = self.black_road_pieces.set(to.0),
                    };
                }

                match (self.side_to_move(), piece) {
                    (Color::White, WhiteFlat) => self.white_stones_left -= 1,
                    (Color::White, WhiteStanding) => self.white_stones_left -= 1,
                    (Color::White, WhiteCap) => self.white_capstones_left -= 1,
                    (Color::Black, BlackFlat) => self.black_stones_left -= 1,
                    (Color::Black, BlackStanding) => self.black_stones_left -= 1,
                    (Color::Black, BlackCap) => self.black_capstones_left -= 1,
                    _ => unreachable!(
                        "Tried to place {} stone on {}'s move\n{:?}",
                        piece.color(),
                        self.side_to_move(),
                        self
                    ),
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
            self.count_all_stones(),
            "Wrong number of stones on board:\n{:?}",
            self
        );

        debug_assert_eq!(
            self.white_road_pieces,
            self.white_road_pieces_from_scratch(),
            "Wrong white road pieces after {:?} on\n{:?}",
            mv,
            self
        );
        debug_assert_eq!(
            self.black_road_pieces,
            self.black_road_pieces_from_scratch(),
            "Wrong black road pieces after {:?} on\n{:?}",
            mv,
            self
        );

        self.to_move = !self.to_move;
        reverse_move
    }

    fn reverse_move(&mut self, reverse_move: Self::ReverseMove) {
        match reverse_move.clone() {
            ReverseMove::Place(square) => {
                let piece = self[square].pop().unwrap();
                debug_assert_eq!(piece.color(), !self.side_to_move());

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
                    self.white_road_pieces = self.white_road_pieces.clear(square.0);
                    self.black_road_pieces = self.black_road_pieces.clear(square.0);

                    if self[square].top_stone().is_some() {
                        match self.side_to_move() {
                            Color::White => {
                                self.white_road_pieces = self.white_road_pieces.set(square.0)
                            }
                            Color::Black => {
                                self.black_road_pieces = self.black_road_pieces.set(square.0)
                            }
                        };
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
            self.white_road_pieces_from_scratch(),
            "Wrong white road pieces after undoing {:?} on\n{:?}",
            reverse_move,
            self
        );
        debug_assert_eq!(
            self.black_road_pieces,
            self.black_road_pieces_from_scratch(),
            "Wrong black road pieces after undoing {:?} on\n{:?}",
            reverse_move,
            self
        );
        self.to_move = !self.to_move;
    }

    fn game_result(&self) -> Option<GameResult> {
        let (components, highest_component_id) =
            connected_components_graph(WhiteTr::road_stones(self), BlackTr::road_stones(self));

        if let Some(square) = is_win_by_road(&components, highest_component_id) {
            debug_assert!(self[square].top_stone().unwrap().is_road_piece());
            return match self[square].top_stone().unwrap().color() {
                Color::White => Some(WhiteWin),
                Color::Black => Some(BlackWin),
            };
        };

        if (self.white_stones_left == 0 && self.white_capstones_left == 0)
            || (self.black_stones_left == 0 && self.black_capstones_left == 0)
        {
            // Count points
            let mut white_points = 0;
            let mut black_points = 0;
            for square in board_iterator() {
                match self[square].top_stone() {
                    Some(WhiteFlat) | Some(WhiteCap) => white_points += 1,
                    Some(BlackFlat) | Some(BlackCap) => black_points += 1,
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
        let material = (self.white_road_pieces.popcount() as i64
            - self.black_road_pieces.popcount() as i64
            + self.white_capstones_left as i64
            - self.black_capstones_left as i64) as f32;

        let to_move = match self.side_to_move() {
            Color::White => 0.5,
            Color::Black => -0.5,
        };

        let mut centre = 0.0;
        for x in 1..4 {
            for y in 1..4 {
                match self.cells.raw[y][x].top_stone().map(Piece::color) {
                    Some(Color::White) => centre += 0.2,
                    Some(Color::Black) => centre -= 0.2,
                    None => (),
                }
            }
        }
        match self.cells.raw[2][2].top_stone().map(Piece::color) {
            Some(Color::White) => centre += 0.1,
            Some(Color::Black) => centre -= 0.1,
            None => (),
        }

        let stacks: f32 = board_iterator()
            .map(|sq| &self[sq])
            .filter(|stack| stack.len() > 1)
            .map(|stack| {
                let controlling_player = stack.top_stone().unwrap().color();
                let val = stack
                    .clone()
                    .into_iter()
                    .take(stack.len() as usize - 1)
                    .map(|piece| {
                        if piece.color() == controlling_player {
                            0.8
                        } else {
                            -0.4
                        }
                    })
                    .sum::<f32>();
                match controlling_player {
                    Color::White => val,
                    Color::Black => val * -1.0,
                }
            })
            .sum();

        material + to_move + centre + stacks
    }
}

impl pgn_traits::pgn::PgnBoard for Board {
    fn from_fen(_fen: &str) -> Result<Self, pgn::Error> {
        unimplemented!()
    }

    fn to_fen(&self) -> String {
        unimplemented!()
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
            'a'..='e' if input.len() == 2 => match self.side_to_move() {
                Color::White => Ok(Move::Place(WhiteFlat, Square::parse_square(input))),
                Color::Black => Ok(Move::Place(BlackFlat, Square::parse_square(input))),
            },
            'C' if input.len() == 3 => match self.side_to_move() {
                Color::White => Ok(Move::Place(WhiteCap, Square::parse_square(&input[1..]))),
                Color::Black => Ok(Move::Place(BlackCap, Square::parse_square(&input[1..]))),
            },
            'S' if input.len() == 3 => match self.side_to_move() {
                Color::White => Ok(Move::Place(
                    WhiteStanding,
                    Square::parse_square(&input[1..]),
                )),
                Color::Black => Ok(Move::Place(
                    BlackStanding,
                    Square::parse_square(&input[1..]),
                )),
            },
            '1'..='9' if input.len() > 3 => {
                let square = Square::parse_square(&input[1..3]);
                let direction = Direction::parse(input.chars().nth(3).unwrap());
                let movements = StackMovement {
                    movements: input
                        .chars()
                        .take(1)
                        .chain(input.chars().skip(4))
                        .map(|ch| Movement {
                            pieces_to_take: ch as u8 - b'0',
                        })
                        .collect(),
                };
                Ok(Move::Move(square, direction, movements))
            }
            _ => Err(pgn::Error::new(
                pgn::ErrorKind::ParseError,
                format!("Couldn't parse {}", input),
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
pub struct AbstractBoard<T> {
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

pub fn connected_components_graph(
    white_road_pieces: BitBoard,
    black_road_pieces: BitBoard,
) -> (AbstractBoard<u8>, u8) {
    debug_assert!((white_road_pieces & black_road_pieces).is_empty());

    let mut components: AbstractBoard<u8> = Default::default();
    let mut id = 1;

    for square in board_iterator() {
        if components[square] == 0 {
            if white_road_pieces.get(square.0) {
                connect_component(white_road_pieces, &mut components, square, id);
                id += 1;
            } else if black_road_pieces.get(square.0) {
                connect_component(black_road_pieces, &mut components, square, id);
                id += 1;
            }
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
pub fn is_win_by_road(components: &AbstractBoard<u8>, highest_component_id: u8) -> Option<Square> {
    for id in 1..highest_component_id {
        if (components.raw[0].iter().any(|&cell| cell == id)
            && components.raw[BOARD_SIZE - 1]
                .iter()
                .any(|&cell| cell == id))
            || ((0..BOARD_SIZE).any(|y| components.raw[y][0] == id)
                && (0..BOARD_SIZE).any(|y| components.raw[y][BOARD_SIZE - 1] == id))
        {
            let square = board_iterator().find(|&sq| components[sq] == id).unwrap();
            return Some(square);
        }
    }
    None
}

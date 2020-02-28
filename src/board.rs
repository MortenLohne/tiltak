pub const BOARD_SIZE: usize = 5;

use crate::board::Direction::*;
use crate::board::Piece::*;
use crate::board::Role::Flat;
use crate::board::Role::*;
use board_game_traits::board;
use board_game_traits::board::GameResult::{BlackWin, Draw, WhiteWin};
use board_game_traits::board::{Board as BoardTrait, EvalBoard as EvalBoardTrait};
use board_game_traits::board::{Color, GameResult};
use pgn_traits::pgn;
use smallvec::alloc::fmt::{Error, Formatter};
use smallvec::SmallVec;
use std::cmp::Ordering;
use std::fmt::Debug;
use std::fmt::Write;
use std::ops::{Index, IndexMut};
use std::{fmt, ops};

pub trait ColorTr {
    fn color() -> Color;

    fn stones_left(board: &Board) -> u8;

    fn capstones_left(board: &Board) -> u8;

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

pub type Stack = SmallVec<[Piece; 4]>;

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
                    North => f.write_char('-')?,
                    West => f.write_char('<')?,
                    East => f.write_char('>')?,
                    South => f.write_char('+')?,
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    North,
    West,
    East,
    South,
}

impl Direction {
    fn parse(ch: char) -> Self {
        match ch {
            '-' => North,
            '<' => West,
            '>' => East,
            '+' => South,
            _ => panic!("Couldn't parse \"{}\" as direction.", ch),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StackMovement {
    pub movements: SmallVec<[Movement; 5]>,
}

/// Moving a stack of pieces consists of one or more `Movement`s
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Movement {
    pub pieces_to_take: u8,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Board {
    pub cells: AbstractBoard<Stack>,
    to_move: Color,
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
        Ok(())
    }
}

impl Board {
    pub fn generate_moves_with_probabilities(&self, moves: &mut Vec<(Move, f64)>) {
        let mut simple_moves = vec![];
        self.generate_moves(&mut simple_moves);
        let average = 1.0 / simple_moves.len() as f64;
        moves.extend(simple_moves.drain(..).map(|mv| (mv, average)));
    }

    pub fn count_all_stones(&self) -> u8 {
        self.cells.raw.iter().flatten().flatten().count() as u8
    }

    pub fn all_top_stones(&self) -> impl Iterator<Item = &Piece> {
        self.cells
            .raw
            .iter()
            .flatten()
            .filter_map(|cell| cell.last())
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
                let piece_index = self[square].len() - *pieces_to_take as usize;
                if piece_index == 0 {
                    None
                } else {
                    Some(self[square][piece_index - 1].clone())
                }
            })
            .chain(std::iter::once(self[square].last().cloned()))
    }

    pub fn connected_components_graph(&self) -> (AbstractBoard<u8>, u8) {
        let mut components: AbstractBoard<u8> = Default::default();
        let mut visited: AbstractBoard<bool> = Default::default();
        let mut id = 1;

        // Find white roads
        for square in board_iterator() {
            if !visited[square]
                && self[square]
                    .last()
                    .cloned()
                    .map(WhiteTr::is_road_stone)
                    .unwrap_or_default()
            {
                connect_component::<WhiteTr>(&self, &mut components, &mut visited, square, id);
                id += 1;
            }
        }

        // Find black roads
        for square in board_iterator() {
            if !visited[square]
                && self[square]
                    .last()
                    .cloned()
                    .map(BlackTr::is_road_stone)
                    .unwrap_or_default()
            {
                connect_component::<BlackTr>(&self, &mut components, &mut visited, square, id);
                id += 1;
            }
        }
        (components, id)
    }
}

impl board::Board for Board {
    type Move = Move;
    type ReverseMove = Self;

    fn start_board() -> Self {
        Self::default()
    }

    fn side_to_move(&self) -> Color {
        self.to_move
    }

    fn generate_moves(&self, moves: &mut Vec<Self::Move>) {
        match self.side_to_move() {
            Color::White => self.generate_moves_colortr::<WhiteTr>(moves),
            Color::Black => self.generate_moves_colortr::<BlackTr>(moves),
        }
    }

    fn do_move(&mut self, mv: Self::Move) -> Self::ReverseMove {
        let reverse_move = self.clone();
        match mv {
            Move::Place(piece, to) => {
                self[to].push(piece);
                match (self.side_to_move(), piece) {
                    (Color::White, WhiteFlat) => self.white_stones_left -= 1,
                    (Color::White, WhiteStanding) => self.white_stones_left -= 1,
                    (Color::White, WhiteCap) => self.white_capstones_left -= 1,
                    (Color::Black, BlackFlat) => self.black_stones_left -= 1,
                    (Color::Black, BlackStanding) => self.black_stones_left -= 1,
                    (Color::Black, BlackCap) => self.black_capstones_left -= 1,
                    _ => unreachable!(),
                }
            }
            Move::Move(mut from, direction, stack_movement) => {
                // self[from].truncate(movements[0].pieces_to_leave as usize);
                for Movement { pieces_to_take } in stack_movement.movements {
                    let to = from.go_direction(direction).unwrap();
                    if let Some(piece) = self[to].last_mut() {
                        match piece {
                            WhiteStanding => *piece = WhiteFlat,
                            BlackStanding => *piece = BlackFlat,
                            _ => (),
                        }
                        debug_assert!(
                            piece.role() != Standing || self[from].last().unwrap().role() == Cap
                        );
                    }
                    let pieces_to_leave = self[from].len() - pieces_to_take as usize;
                    let pieces_to_take: Vec<_> = self[from].drain(pieces_to_leave..).collect();
                    self[to].extend(pieces_to_take);

                    from = to;
                }
            }
        }

        debug_assert_eq!(
            44 - self.white_stones_left
                - self.black_stones_left
                - self.white_capstones_left
                - self.black_capstones_left,
            self.count_all_stones(),
            "Wrong number of stones on board:\n{:?}",
            self
        );
        self.to_move = !self.to_move;
        reverse_move
    }

    fn reverse_move(&mut self, reverse_move: Self::ReverseMove) {
        *self = reverse_move
    }

    fn game_result(&self) -> Option<GameResult> {
        let (components, highest_component_id) = self.connected_components_graph();

        // Check if any components cross the board
        for id in 1..highest_component_id {
            if (components.raw[0].iter().any(|&cell| cell == id)
                && components.raw[BOARD_SIZE - 1]
                    .iter()
                    .any(|&cell| cell == id))
                || ((0..BOARD_SIZE).any(|y| components.raw[y][0] == id)
                    && (0..BOARD_SIZE).any(|y| components.raw[y][BOARD_SIZE - 1] == id))
            {
                let square = board_iterator()
                    .find(|&square| components[square] == id)
                    .unwrap();
                let &piece = self[square].last().unwrap();
                if piece == Piece::WhiteCap || piece == Piece::WhiteFlat {
                    return Some(GameResult::WhiteWin);
                } else if piece == Piece::BlackCap || piece == Piece::BlackFlat {
                    return Some(GameResult::BlackWin);
                } else {
                    unreachable!();
                }
            }
        }

        if (self.white_stones_left == 0 && self.white_capstones_left == 0)
            || (self.black_stones_left == 0 && self.black_capstones_left == 0)
        {
            // Count points
            let mut white_points = 0;
            let mut black_points = 0;
            for square in board_iterator() {
                match self[square].last() {
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
        let material = self
            .all_top_stones()
            .map(|piece| match piece {
                WhiteFlat => 1.0,
                BlackFlat => -1.0,
                _ => 0.0,
            })
            .sum::<f32>();

        let to_move = match self.side_to_move() {
            Color::White => 0.5,
            Color::Black => -0.5,
        };

        let mut centre = 0.0;
        for x in 1..4 {
            for y in 1..4 {
                match self.cells.raw[y][x].last().cloned().map(Piece::color) {
                    Some(Color::White) => centre += 0.2,
                    Some(Color::Black) => centre -= 0.2,
                    None => (),
                }
            }
        }
        match self.cells.raw[2][2].last().cloned().map(Piece::color) {
            Some(Color::White) => centre += 0.1,
            Some(Color::Black) => centre -= 0.1,
            None => (),
        }

        let stacks: f32 = board_iterator()
            .map(|sq| &self[sq])
            .filter(|stack| stack.len() > 1)
            .map(|stack| {
                let controlling_player = stack.last().unwrap().color();
                let val = stack
                    .iter()
                    .take(stack.len() - 1)
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

impl<T> AbstractBoard<T> {
    fn map<F, U>(&self, f: F) -> AbstractBoard<U>
    where
        F: Fn(&T) -> U,
        U: Default,
    {
        let mut new_board = AbstractBoard::default();
        for square in board_iterator() {
            new_board[square] = f(&self[square]);
        }
        new_board
    }
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

fn connect_component<Color: ColorTr>(
    board: &Board,
    components: &mut AbstractBoard<u8>,
    visited: &mut AbstractBoard<bool>,
    square: Square,
    id: u8,
) {
    components[square] = id;
    visited[square] = true;
    for neighbour in square.neighbours() {
        if !board[neighbour].is_empty()
            && Color::is_road_stone(*board[neighbour].last().unwrap())
            && !visited[neighbour]
        {
            connect_component::<Color>(board, components, visited, neighbour, id);
        }
    }
}

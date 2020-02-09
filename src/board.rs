pub const BOARD_SIZE: usize = 5;

use crate::board::Direction::*;
use crate::board::Piece::*;
use crate::board::Role::Flat;
use crate::board::Role::*;
use board_game_traits::board;
use board_game_traits::board::GameResult::{BlackWin, Draw, WhiteWin};
use board_game_traits::board::{Color, GameResult};
use smallvec::alloc::fmt::{Error, Formatter};
use smallvec::SmallVec;
use std::cmp::Ordering;
use std::fmt::Debug;
use std::ops;
use std::ops::{Index, IndexMut};

trait ColorTr {
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
    fn role(self) -> Role {
        match self {
            WhiteFlat | BlackFlat => Flat,
            WhiteStanding | BlackStanding => Standing,
            WhiteCap | BlackCap => Cap,
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

type Cell = SmallVec<[Piece; 4]>;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Move {
    Place(Piece, Square),
    Move(Square, Direction, SmallVec<[Movement; 5]>), // Number of stones to take
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    North,
    West,
    East,
    South,
}

impl Direction {
    fn all() -> impl Iterator<Item = Direction> {
        [North, East, West, South].iter().cloned()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Movement {
    pub pieces_to_take: u8,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Board {
    cells: [[Cell; BOARD_SIZE]; BOARD_SIZE],
    to_move: Color,
    white_stones_left: u8,
    black_stones_left: u8,
    white_capstones_left: u8,
    black_capstones_left: u8,
}

impl Index<Square> for Board {
    type Output = Cell;

    fn index(&self, square: Square) -> &Self::Output {
        &self.cells[square.rank() as usize][square.file() as usize]
    }
}

impl IndexMut<Square> for Board {
    fn index_mut(&mut self, square: Square) -> &mut Self::Output {
        &mut self.cells[square.rank() as usize][square.file() as usize]
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
                        match self.cells[y][x].get(print_column * 3 + print_row) {
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
    fn generate_moves_colortr<Colorr: ColorTr>(
        &self,
        moves: &mut Vec<<Board as board_game_traits::board::Board>::Move>,
    ) {
        for square in board_iterator() {
            match self[square].last() {
                None => {
                    if Colorr::stones_left(&self) > 0 {
                        moves.push(Move::Place(Colorr::flat_piece(), square));
                        moves.push(Move::Place(Colorr::standing_piece(), square));
                    }
                    if Colorr::capstones_left(&self) > 0 {
                        moves.push(Move::Place(Colorr::cap_piece(), square));
                    }
                }
                Some(&piece) if Colorr::piece_is_ours(piece) => {
                    for direction in square.directions() {
                        let mut movements = vec![];
                        if piece == Colorr::cap_piece() {
                            self.generate_moving_moves_cap::<Colorr>(
                                direction,
                                square,
                                square,
                                self[square].len() as u8,
                                &smallvec![],
                                &mut movements,
                            );
                        } else if Colorr::piece_is_ours(piece) {
                            self.generate_moving_moves_non_cap::<Colorr>(
                                direction,
                                square,
                                square,
                                self[square].len() as u8,
                                &smallvec![],
                                &mut movements,
                            );
                        }
                        for movement in movements.into_iter().filter(|mv| mv.len() > 0) {
                            // TODO
                            moves.push(Move::Move(square, direction, movement));
                        }
                    }
                }
                Some(_) => (),
            }
        }
    }

    fn generate_moving_moves_cap<Colorr: ColorTr>(
        &self,
        direction: Direction,
        origin_square: Square,
        square: Square,
        pieces_carried: u8,
        partial_movement: &SmallVec<[Movement; 5]>,
        movements: &mut Vec<SmallVec<[Movement; 5]>>,
    ) {
        if let Some(neighbour) = square.go_direction(direction) {
            let max_pieces_to_take = if square == origin_square {
                pieces_carried
            } else {
                pieces_carried - 1
            };
            let neighbour_piece = self[neighbour].last().cloned();
            if neighbour_piece.map(Piece::role) == Some(Cap) {
                return;
            }
            if neighbour_piece.map(Piece::role) == Some(Standing) && max_pieces_to_take > 0 {
                let mut new_movement = partial_movement.clone();
                new_movement.push(Movement { pieces_to_take: 1 });
                movements.push(new_movement);
            } else {
                for pieces_to_take in 1..=max_pieces_to_take {
                    let mut new_movement = partial_movement.clone();
                    new_movement.push(Movement { pieces_to_take });

                    self.generate_moving_moves_cap::<Colorr>(
                        direction,
                        origin_square,
                        neighbour,
                        pieces_to_take,
                        &new_movement,
                        movements,
                    );
                    movements.push(new_movement);
                }
            }
        }
    }

    fn generate_moving_moves_non_cap<Colorr: ColorTr>(
        &self,
        direction: Direction,
        origin_square: Square,
        square: Square,
        pieces_carried: u8,
        partial_movement: &SmallVec<[Movement; 5]>,
        movements: &mut Vec<SmallVec<[Movement; 5]>>,
    ) {
        if let Some(neighbour) = square.go_direction(direction) {
            let neighbour_piece = self[neighbour].last().cloned();
            if neighbour_piece.is_some() && neighbour_piece.unwrap().role() != Flat {
                return;
            }

            let neighbour = square.go_direction(direction).unwrap();
            let max_pieces_to_take = if square == origin_square {
                pieces_carried
            } else {
                pieces_carried - 1
            };
            for pieces_to_take in 1..=max_pieces_to_take {
                let mut new_movement = partial_movement.clone();
                new_movement.push(Movement { pieces_to_take });

                self.generate_moving_moves_non_cap::<Colorr>(
                    direction,
                    origin_square,
                    neighbour,
                    pieces_to_take,
                    &new_movement,
                    movements,
                );
                movements.push(new_movement);
            }
        }
    }

    fn count_all_stones(&self) -> u8 {
        self.cells.iter().flatten().flatten().count() as u8
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
            Move::Move(mut from, direction, movements) => {
                // self[from].truncate(movements[0].pieces_to_leave as usize);
                for Movement { pieces_to_take } in movements {
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
        let mut components: AbstractBoard<u8> = Default::default();
        let mut visited: AbstractBoard<bool> = Default::default();
        let mut id = 1;
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

        // Check if any components cross the board
        for id in 1..id {
            if components.0[0].iter().any(|&cell| cell == id)
                && components.0[BOARD_SIZE - 1].iter().any(|&cell| cell == id)
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

#[derive(Debug, Default, PartialEq, Eq)]
struct AbstractBoard<T>([[T; BOARD_SIZE]; BOARD_SIZE]);

impl<T> Index<Square> for AbstractBoard<T> {
    type Output = T;

    fn index(&self, square: Square) -> &Self::Output {
        &self.0[square.0 as usize % BOARD_SIZE][square.0 as usize / BOARD_SIZE]
    }
}

impl<T> IndexMut<Square> for AbstractBoard<T> {
    fn index_mut(&mut self, square: Square) -> &mut Self::Output {
        &mut self.0[square.0 as usize % BOARD_SIZE][square.0 as usize / BOARD_SIZE]
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

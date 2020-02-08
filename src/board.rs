pub const BOARD_SIZE: usize = 5;

use crate::board::Piece::{BlackCap, BlackFlat, BlackStanding, WhiteCap, WhiteFlat, WhiteStanding};
use board_game_traits::board;
use board_game_traits::board::GameResult::{BlackWin, Draw, WhiteWin};
use board_game_traits::board::{Color, GameResult};
use smallvec::SmallVec;
use std::cmp::Ordering;
use std::ops::{Index, IndexMut};

trait ColorTr {
    fn stones_left(board: &Board) -> u8;

    fn capstones_left(board: &Board) -> u8;

    fn flat_piece() -> Piece;

    fn standing_piece() -> Piece;

    fn cap_piece() -> Piece;

    fn is_road_stone(piece: Piece) -> bool;
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
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Square(pub u8);

pub fn board_iterator() -> impl Iterator<Item = Square> {
    (0..(BOARD_SIZE * BOARD_SIZE)).map(|i| Square(i as u8))
}

pub fn neighbours(square: Square) -> impl Iterator<Item = Square> {
    [-(BOARD_SIZE as i8), -1, 1, BOARD_SIZE as i8]
        .iter()
        .map(move |sq| sq + square.0 as i8)
        .filter(|&sq| sq >= 0 && sq < BOARD_SIZE as i8 * BOARD_SIZE as i8)
        .map(|sq| Square(sq as u8))
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

type Cell = SmallVec<[Piece; 4]>;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Move {
    Place(Piece, Square),
    Move(Square, SmallVec<[Movement; 5]>),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Movement {
    pub pieces_to_leave: u8,
    pub dest_square: Square,
}

#[derive(Clone, PartialEq, Eq, Debug)]
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
        &self.cells[square.0 as usize % BOARD_SIZE][square.0 as usize / BOARD_SIZE]
    }
}

impl IndexMut<Square> for Board {
    fn index_mut(&mut self, square: Square) -> &mut Self::Output {
        &mut self.cells[square.0 as usize % BOARD_SIZE][square.0 as usize / BOARD_SIZE]
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
                Some(&piece) => {
                    if piece == Colorr::cap_piece() {
                        for neighbour in neighbours(square) {
                            let mut vec = SmallVec::new();
                            vec.push(Movement {
                                pieces_to_leave: 0,
                                dest_square: neighbour,
                            });
                            moves.push(Move::Move(square, vec));
                        }
                    }
                    else {
                        for neighbour in neighbours(square) {
                            if self[neighbour].last() == Some(&WhiteFlat) || self[neighbour].last() == Some(&BlackFlat) {
                                let mut vec = SmallVec::new();
                                vec.push(Movement {
                                    pieces_to_leave: 0,
                                    dest_square: neighbour,
                                });
                                moves.push(Move::Move(square, vec));
                            }
                        }
                    }
                }
            }
        }
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
            Move::Move(mut from, movements) => {
                self[from].truncate(movements[0].pieces_to_leave as usize);
                for Movement {
                    pieces_to_leave,
                    dest_square,
                } in movements
                {
                    self[dest_square] = self[from].clone();
                    for _ in 0..pieces_to_leave {
                        let piece = self[dest_square].remove(0);
                        self[from].push(piece);
                    }
                    from = dest_square;
                }
            }
        }
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
    for neighbour in neighbours(square) {
        if !board[neighbour].is_empty() && Color::is_road_stone(*board[neighbour].last().unwrap()) && !visited[neighbour] {
            connect_component::<Color>(board, components, visited, neighbour, id);
        }
    }
}

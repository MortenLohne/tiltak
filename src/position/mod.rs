//! Tak move generation, along with all required data types.

use std::fmt::Write;
use std::hash::{Hash, Hasher};
use std::ops::{Index, IndexMut};
use std::{fmt, ops};
use std::{iter, mem};

use arrayvec::ArrayVec;
use board_game_traits::{Color, GameResult};
use board_game_traits::{EvalPosition as EvalPositionTrait, Position as PositionTrait};
use dfdx::prelude::Module;
use dfdx::shapes::Const;
use dfdx::tensor::{AsArray, Cpu, Tensor, ZerosTensor};
use lazy_static::lazy_static;
use pgn_traits::PgnPosition;
use rand::{Rng, SeedableRng};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use bitboard::BitBoard;
use color_trait::{BlackTr, WhiteTr};

pub use utils::{
    squares_iterator, Direction, Komi, Movement, Piece, Piece::*, Role, Role::*, Square, Stack,
    StackMovement,
};

pub(crate) use utils::AbstractBoard;

pub use mv::{Move, ReverseMove};

use crate::evaluation::parameters::{
    ValueFeatures, ValueModel, NUM_VALUE_FEATURES_6S, POLICY_PARAMS_4S, POLICY_PARAMS_5S,
    POLICY_PARAMS_6S, VALUE_PARAMS_4S, VALUE_PARAMS_5S, VALUE_PARAMS_6S,
};
use crate::evaluation::value_eval;
use crate::position::color_trait::ColorTr;
use crate::search;

pub(crate) mod bitboard;
pub(crate) mod color_trait;
mod mv;
mod utils;

lazy_static! {
    pub(crate) static ref ZOBRIST_KEYS_3S: Box<ZobristKeys<3>> = ZobristKeys::new();
    pub(crate) static ref ZOBRIST_KEYS_4S: Box<ZobristKeys<4>> = ZobristKeys::new();
    pub(crate) static ref ZOBRIST_KEYS_5S: Box<ZobristKeys<5>> = ZobristKeys::new();
    pub(crate) static ref ZOBRIST_KEYS_6S: Box<ZobristKeys<6>> = ZobristKeys::new();
    pub(crate) static ref ZOBRIST_KEYS_7S: Box<ZobristKeys<7>> = ZobristKeys::new();
    pub(crate) static ref ZOBRIST_KEYS_8S: Box<ZobristKeys<8>> = ZobristKeys::new();
}

pub const MAX_BOARD_SIZE: usize = 8;

pub const fn starting_stones(size: usize) -> u8 {
    match size {
        3 => 10,
        4 => 15,
        5 => 21,
        6 => 30,
        7 => 40,
        8 => 50,
        _ => 0,
    }
}

pub const fn starting_capstones(size: usize) -> u8 {
    match size {
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

pub(crate) const fn num_line_symmetries<const S: usize>() -> usize {
    match S {
        4 => 2,
        5 => 3,
        6 => 3,
        _ => 0,
    }
}

pub(crate) const fn line_symmetries<const S: usize>() -> &'static [usize] {
    match S {
        4 => &[0, 1, 1, 0],
        5 => &[0, 1, 2, 1, 0],
        6 => &[0, 1, 2, 2, 1, 0],
        _ => &[],
    }
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
    pub(crate) white_flat_stones: BitBoard,
    pub(crate) black_flat_stones: BitBoard,
    pub(crate) white_caps: BitBoard,
    pub(crate) black_caps: BitBoard,
    pub(crate) white_walls: BitBoard,
    pub(crate) black_walls: BitBoard,
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
        3 => ZOBRIST_KEYS_3S.top_stones[square][piece as u16 as usize],
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
        3 => ZOBRIST_KEYS_3S.stones_in_stack[place_in_stack][square][stack_slice],
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
        3 => ZOBRIST_KEYS_3S.to_move[color.disc()],
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

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Settings {
    pub komi: Komi,
}

enum DetailedGameResult {
    WhiteRoadWin,
    BlackRoadWin,
    WhiteFlatWin,
    BlackFlatWin,
    Draw,
}

impl DetailedGameResult {
    fn game_result(&self) -> GameResult {
        match self {
            DetailedGameResult::WhiteRoadWin => GameResult::WhiteWin,
            DetailedGameResult::BlackRoadWin => GameResult::BlackWin,
            DetailedGameResult::WhiteFlatWin => GameResult::WhiteWin,
            DetailedGameResult::BlackFlatWin => GameResult::BlackWin,
            DetailedGameResult::Draw => GameResult::Draw,
        }
    }

    fn result_str(&self) -> &'static str {
        match self {
            DetailedGameResult::WhiteRoadWin => "R-0",
            DetailedGameResult::BlackRoadWin => "0-R",
            DetailedGameResult::WhiteFlatWin => "F-0",
            DetailedGameResult::BlackFlatWin => "0-F",
            DetailedGameResult::Draw => "1/2-1/2",
        }
    }
}

/// Complete representation of a Tak position
#[derive(Clone)]
pub struct Position<const S: usize> {
    cells: AbstractBoard<Stack, S>,
    to_move: Color,
    white_stones_left: u8,
    black_stones_left: u8,
    white_caps_left: u8,
    black_caps_left: u8,
    half_moves_played: usize,
    moves: Vec<Move>,
    komi: Komi,
    hash: u64,              // Zobrist hash of current position
    hash_history: Vec<u64>, // Zobrist hashes of previous board states, up to the last irreversible move. Does not include the corrent position
}

impl<const S: usize> PartialEq for Position<S> {
    fn eq(&self, other: &Self) -> bool {
        self.cells == other.cells
            && self.to_move == other.to_move
            && self.white_stones_left == other.white_stones_left
            && self.black_stones_left == other.black_stones_left
            && self.white_caps_left == other.white_caps_left
            && self.black_caps_left == other.black_caps_left
            && self.half_moves_played == other.half_moves_played
            && self.komi == other.komi
    }
}

impl<const S: usize> Eq for Position<S> {}

impl<const S: usize> Hash for Position<S> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.cells.hash(state);
        self.to_move.hash(state);
        self.white_stones_left.hash(state);
        self.black_stones_left.hash(state);
        self.white_caps_left.hash(state);
        self.black_caps_left.hash(state);
        self.half_moves_played.hash(state);
        self.komi.hash(state);
    }
}

impl<const S: usize> Index<Square> for Position<S> {
    type Output = Stack;

    fn index(&self, square: Square) -> &Self::Output {
        &self.cells[square]
    }
}

impl<const S: usize> IndexMut<Square> for Position<S> {
    fn index_mut(&mut self, square: Square) -> &mut Self::Output {
        &mut self.cells[square]
    }
}

impl<const S: usize> Default for Position<S> {
    fn default() -> Self {
        Self::start_position_with_komi(Komi::default())
    }
}

impl<const S: usize> fmt::Debug for Position<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        for y in 0..S {
            for print_row in 0..3 {
                for x in 0..S {
                    for print_column in 0..3 {
                        match self.cells[Square::from_rank_file::<S>(y as u8, x as u8)]
                            .get(print_column * 3 + print_row)
                        {
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

impl<const S: usize> Position<S> {
    pub fn start_position_with_komi(komi: Komi) -> Self {
        Position {
            cells: Default::default(),
            to_move: Color::White,
            white_stones_left: starting_stones(S),
            black_stones_left: starting_stones(S),
            white_caps_left: starting_capstones(S),
            black_caps_left: starting_capstones(S),
            half_moves_played: 0,
            moves: vec![],
            komi,
            hash: zobrist_to_move::<S>(Color::White),
            hash_history: vec![],
        }
    }

    pub fn from_fen_with_komi(fen: &str, komi: Komi) -> Result<Self, pgn_traits::Error> {
        let mut position = Self::from_fen(fen)?;
        position.komi = komi;
        Ok(position)
    }

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

    pub fn komi(&self) -> Komi {
        self.komi
    }

    pub fn set_komi(&mut self, komi: Komi) {
        self.komi = komi
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
            // Only enter this loop if stack.len() is 2 or more
            for i in 0..(stack.len() as usize + 6) / 8 {
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

    pub fn flip_board_y(&self) -> Position<S> {
        let mut new_board = self.clone();
        for x in 0..S as u8 {
            for y in 0..S as u8 {
                new_board[Square(y * S as u8 + x)] = self[Square((S as u8 - y - 1) * S as u8 + x)];
            }
        }
        new_board
    }

    pub fn flip_board_x(&self) -> Position<S> {
        let mut new_board = self.clone();
        for x in 0..S as u8 {
            for y in 0..S as u8 {
                new_board[Square(y * S as u8 + x)] = self[Square(y * S as u8 + (S as u8 - x - 1))];
            }
        }
        new_board
    }

    pub fn rotate_board(&self) -> Position<S> {
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

    pub fn flip_colors(&self) -> Position<S> {
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
    pub fn symmetries(&self) -> Vec<Position<S>> {
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
    pub fn symmetries_with_swapped_colors(&self) -> Vec<Position<S>> {
        self.symmetries()
            .into_iter()
            .flat_map(|board| vec![board.clone(), board.flip_colors()])
            .collect()
    }

    fn count_all_pieces(&self) -> u8 {
        squares_iterator::<S>()
            .map(|square| self[square].len())
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
            .into_iter::<S>()
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

    pub(crate) fn fcd_for_move(&self, mv: Move) -> i8 {
        match mv {
            Move::Place(Role::Flat, _) if self.half_moves_played() > 1 => 1,
            Move::Place(Role::Flat, _) => -1,
            Move::Place(_, _) => 0,
            Move::Move(square, direction, stack_movement) => {
                let mut destination_square =
                    if stack_movement.get_first::<S>().pieces_to_take == self[square].len() {
                        square.go_direction::<S>(direction).unwrap()
                    } else {
                        square
                    };

                let mut fcd = 0;

                if self[square].len() == stack_movement.get_first::<S>().pieces_to_take {
                    let top_stone = self[square].top_stone.unwrap();
                    if top_stone.role() == Flat {
                        fcd -= 1;
                    }
                }

                // This iterator skips the first square if we move the whole stack
                for piece in self
                    .top_stones_left_behind_by_move(square, &stack_movement)
                    .flatten()
                {
                    let destination_stack = &self[destination_square];

                    if let Some(captured_piece) = destination_stack.top_stone() {
                        if captured_piece.role() == Flat {
                            if captured_piece.color() == self.side_to_move() {
                                fcd -= 1;
                            } else {
                                fcd += 1;
                            }
                        }
                    }

                    if piece.role() == Flat {
                        if piece.color() == self.side_to_move() {
                            fcd += 1;
                        } else {
                            fcd -= 1;
                        }
                    }

                    destination_square = destination_square
                        .go_direction::<S>(direction)
                        .unwrap_or(destination_square);
                }

                fcd
            }
        }
    }

    pub(crate) fn game_result_with_group_data(
        &self,
        group_data: &GroupData<S>,
    ) -> Option<GameResult> {
        self.detailed_game_result(group_data)
            .map(|result| result.game_result())
    }

    fn detailed_game_result(&self, group_data: &GroupData<S>) -> Option<DetailedGameResult> {
        let repetitions = self
            .hash_history
            .iter()
            .filter(|hash| **hash == self.hash)
            .count();

        if repetitions >= 2 {
            return Some(DetailedGameResult::Draw);
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
                    Some(DetailedGameResult::WhiteRoadWin)
                } else {
                    Some(DetailedGameResult::BlackRoadWin)
                };
            };
            unreachable!(
                "Board has winning connection, but isn't winning\n{:?}",
                self
            )
        }

        if (self.white_stones_left == 0 && self.white_caps_left == 0)
            || (self.black_stones_left == 0 && self.black_caps_left == 0)
            || group_data.all_pieces().count() as usize == S * S
        {
            // Count points
            let white_points = group_data.white_road_pieces().count() as i8;
            let black_points = group_data.black_road_pieces().count() as i8;

            let result = self
                .komi
                .game_result_with_flatcounts(white_points, black_points);
            Some(match result {
                GameResult::WhiteWin => DetailedGameResult::WhiteFlatWin,
                GameResult::BlackWin => DetailedGameResult::BlackFlatWin,
                GameResult::Draw => DetailedGameResult::Draw,
            })
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
        cpu: &Cpu,
        model: &ValueModel<NUM_VALUE_FEATURES_6S>,
        features: &mut [f32],
    ) -> f32 {
        let mut value_features = ValueFeatures::new::<S>(features);
        value_eval::static_eval_game_phase(self, group_data, &mut value_features);

        let mut input_tensor: Tensor<(Const<1>, Const<NUM_VALUE_FEATURES_6S>)> = cpu.zeros();
        input_tensor.copy_from(&features);
        let prediction = model.forward(input_tensor);

        for c in features.iter_mut() {
            *c = 0.0;
        }
        (prediction.array()[0][0] + 1.0) / 2.0
    }

    pub fn value_params() -> &'static [f32] {
        match S {
            4 => &VALUE_PARAMS_4S,
            5 => &VALUE_PARAMS_5S,
            6 => &VALUE_PARAMS_6S,
            _ => unimplemented!("{}s is not supported.", S),
        }
    }

    pub fn policy_params() -> &'static [f32] {
        match S {
            4 => &POLICY_PARAMS_4S,
            5 => &POLICY_PARAMS_5S,
            6 => &POLICY_PARAMS_6S,
            _ => unimplemented!("{}s is not supported.", S),
        }
    }

    pub fn static_eval_features(&self, features: &mut [f32]) {
        debug_assert!(self.game_result().is_none());

        let group_data = self.group_data();
        let mut value_features = ValueFeatures::new::<S>(features);
        value_eval::static_eval_game_phase(self, &group_data, &mut value_features);
    }

    pub fn generate_moves_with_params(
        &self,
        params: &[f32],
        group_data: &GroupData<S>,
        simple_moves: &mut Vec<<Self as PositionTrait>::Move>,
        moves: &mut Vec<(<Self as PositionTrait>::Move, f32)>,
        features: &mut Vec<Box<[f32]>>,
    ) {
        debug_assert!(simple_moves.is_empty());
        self.generate_moves(simple_moves);
        match self.side_to_move() {
            Color::White => self.generate_moves_with_probabilities_colortr::<WhiteTr, BlackTr>(
                params,
                group_data,
                simple_moves,
                moves,
                features,
            ),
            Color::Black => self.generate_moves_with_probabilities_colortr::<BlackTr, WhiteTr>(
                params,
                group_data,
                simple_moves,
                moves,
                features,
            ),
        }
    }

    /// Move generation that includes a heuristic probability of each move being played.
    ///
    /// # Arguments
    ///
    /// * `simple_moves` - An empty vector to temporarily store moves without probabilities. The vector will be emptied before the function returns, and only serves to re-use allocated memory.
    /// * `moves` A vector to place the moves and associated probabilities.
    pub fn generate_moves_with_probabilities(
        &self,
        group_data: &GroupData<S>,
        simple_moves: &mut Vec<Move>,
        moves: &mut Vec<(Move, search::Score)>,
        features: &mut Vec<Box<[f32]>>,
    ) {
        self.generate_moves_with_params(
            Self::policy_params(),
            group_data,
            simple_moves,
            moves,
            features,
        )
    }

    pub fn perft(&mut self, depth: u16) -> u64 {
        if depth == 0 || self.game_result().is_some() {
            1
        } else {
            let mut moves = vec![];
            self.generate_moves(&mut moves);
            moves
                .into_iter()
                .map(|mv| {
                    let old_position = self.clone();
                    let reverse_move = self.do_move(mv.clone());
                    let num_moves = self.perft(depth - 1);
                    self.reverse_move(reverse_move);
                    debug_assert_eq!(
                        *self,
                        old_position,
                        "Failed to restore old board after {:?} on\n{:?}",
                        mv.to_string::<S>(),
                        old_position
                    );
                    num_moves
                })
                .sum()
        }
    }

    pub fn bulk_perft(&mut self, depth: u16) -> u64 {
        if depth == 0 || self.game_result().is_some() {
            1
        } else {
            let mut moves = vec![];
            self.generate_moves(&mut moves);

            if depth == 1 {
                moves.len() as u64
            } else {
                moves
                    .into_iter()
                    .map(|mv| {
                        let old_position = self.clone();
                        let reverse_move = self.do_move(mv.clone());
                        let num_moves = self.bulk_perft(depth - 1);
                        self.reverse_move(reverse_move);
                        debug_assert_eq!(
                            *self,
                            old_position,
                            "Failed to restore old board after {:?} on\n{:?}",
                            mv.to_string::<S>(),
                            old_position
                        );
                        num_moves
                    })
                    .sum()
            }
        }
    }
}

impl<const S: usize> PositionTrait for Position<S> {
    type Move = Move;
    type ReverseMove = ReverseMove;
    type Settings = Settings;

    fn start_position_with_settings(settings: &Self::Settings) -> Self {
        Self::start_position_with_komi(settings.komi)
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
    fn generate_moves<E: Extend<Self::Move>>(&self, moves: &mut E) {
        match self.half_moves_played() {
            0 | 1 => moves.extend(
                utils::squares_iterator::<S>()
                    .filter(|square| self[*square].is_empty())
                    .map(|square| Move::Place(Flat, square)),
            ),
            _ => match self.side_to_move() {
                Color::White => self.generate_moves_colortr::<E, WhiteTr, BlackTr>(moves),
                Color::Black => self.generate_moves_colortr::<E, BlackTr, WhiteTr>(moves),
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
                for sq in <MoveIterator<S>>::new(square, direction, stack_movement) {
                    self.hash ^= self.zobrist_hash_for_square(sq);
                }

                let mut to = square;

                let mut pieces_left_behind = ArrayVec::new();
                let mut flattens_stone = false;

                let mut movement_iter = stack_movement.into_iter::<S>();
                let mut moving_pieces: ArrayVec<Piece, 8> = ArrayVec::new();

                for _ in 0..movement_iter.next().unwrap().pieces_to_take {
                    moving_pieces.push(self[square].pop().unwrap());
                }

                for Movement { pieces_to_take } in
                    movement_iter.chain(iter::once(Movement { pieces_to_take: 0 }))
                {
                    to = to.go_direction::<S>(direction).unwrap();

                    if self[to].top_stone.map(Piece::role) == Some(Wall) {
                        flattens_stone = true;
                    }

                    pieces_left_behind.push(moving_pieces.len() as u8 - pieces_to_take);

                    while moving_pieces.len() as u8 > pieces_to_take {
                        self[to].push(moving_pieces.pop().unwrap());
                    }
                }

                for sq in <MoveIterator<S>>::new(square, direction, stack_movement) {
                    self.hash ^= self.zobrist_hash_for_square(sq);
                }

                ReverseMove::Move(
                    square,
                    direction,
                    stack_movement,
                    pieces_left_behind,
                    flattens_stone,
                )
            }
        };

        debug_assert_eq!(
            2 * (starting_stones(S) + starting_capstones(S))
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

            ReverseMove::Move(
                from,
                direction,
                stack_movement,
                pieces_left_behind,
                flattens_wall,
            ) => {
                for square in <MoveIterator<S>>::new(from, direction, stack_movement) {
                    self.hash ^= self.zobrist_hash_for_square(square);
                }

                let mut to = from;

                for piece_left_behind in pieces_left_behind.into_iter() {
                    to = to.go_direction::<S>(direction).unwrap();
                    let temp_pieces: ArrayVec<Piece, 8> = (0..piece_left_behind)
                        .map(|_| self[to].pop().unwrap())
                        .collect();

                    for piece in temp_pieces.into_iter().rev() {
                        self[from].push(piece);
                    }
                }

                if flattens_wall {
                    debug_assert_eq!(self[to].top_stone().map(Piece::role), Some(Flat));
                    match self[to].top_stone().unwrap().color() {
                        Color::White => self[to].replace_top(WhiteWall),
                        Color::Black => self[to].replace_top(BlackWall),
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
            if self.squares_left > 1 {
                self.square = self.square.go_direction::<S>(self.direction).unwrap();
            }
            self.squares_left -= 1;
            Some(next_square)
        }
    }
}

impl<const S: usize> EvalPositionTrait for Position<S> {
    fn static_eval(&self) -> f32 {
        let params = Self::value_params();
        let mut features: Vec<f32> = vec![0.0; Self::value_params().len()];
        self.static_eval_features(&mut features);
        features.iter().zip(params).map(|(a, b)| a * b).sum()
    }
}

impl<const S: usize> pgn_traits::PgnPosition for Position<S> {
    const REQUIRED_TAGS: &'static [(&'static str, &'static str)] = &[
        ("Player1", "?"),
        ("Player2", "?"),
        ("Date", "????.??.??"),
        (
            "Size",
            match S {
                3 => "3",
                4 => "4",
                5 => "5",
                6 => "6",
                7 => "7",
                8 => "8",
                _ => "",
            },
        ),
        ("Result", "*"),
    ];

    const START_POSITION_TAG_NAME: Option<&'static str> = Some("TPS");

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

    fn pgn_game_result(&self) -> Option<&'static str> {
        let group_data = self.group_data();
        self.detailed_game_result(&group_data)
            .map(|result| result.result_str())
    }

    fn full_move_number(&self) -> Option<u32> {
        Some(self.half_moves_played() as u32 / 2 + 1)
    }

    fn from_fen_with_settings(
        fen: &str,
        settings: &Self::Settings,
    ) -> Result<Self, pgn_traits::Error> {
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
        let mut position = Position::start_position_with_settings(settings);
        for square in utils::squares_iterator::<S>() {
            let (file, rank) = (square.file::<S>(), square.rank::<S>());
            let stack = rows[rank as usize][file as usize];
            for piece in stack.into_iter() {
                match piece {
                    WhiteFlat | WhiteWall => position.white_stones_left -= 1,
                    WhiteCap => position.white_caps_left -= 1,
                    BlackFlat | BlackWall => position.black_stones_left -= 1,
                    BlackCap => position.black_caps_left -= 1,
                }
            }
            position[square] = stack;
        }

        match fen_words[1] {
            "1" => position.to_move = Color::White,
            "2" => position.to_move = Color::Black,
            s => {
                return Err(pgn_traits::Error::new_parse_error(format!(
                    "Error parsing TPS \"{}\": Got bad side to move \"{}\"",
                    fen, s
                )))
            }
        }

        match fen_words[2].parse::<usize>() {
            Ok(n) => match position.side_to_move() {
                Color::White => position.half_moves_played = 2 * n - 2,
                Color::Black => position.half_moves_played = 2 * n - 1,
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

        position.hash = position.zobrist_hash_from_scratch();

        return Ok(position);

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
        for rank in 0..S {
            for file in 0..S {
                let square = Square::from_rank_file::<S>(rank as u8, file as u8);
                if self[square].is_empty() {
                    f.push('x')
                } else {
                    for piece in self[square].into_iter() {
                        match piece {
                            WhiteFlat => f.push('1'),
                            BlackFlat => f.push('2'),
                            WhiteWall => f.push_str("1S"),
                            BlackWall => f.push_str("2S"),
                            WhiteCap => f.push_str("1C"),
                            BlackCap => f.push_str("2C"),
                        }
                    }
                }
                if file < S - 1 {
                    f.push(',');
                }
            }
            if rank < S - 1 {
                f.push('/');
            }
        }
        match self.side_to_move() {
            Color::White => f.push_str(" 1 "),
            Color::Black => f.push_str(" 2 "),
        }
        write!(f, "{}", (self.half_moves_played() / 2) + 1).unwrap();
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

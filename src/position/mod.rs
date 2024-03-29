//! Tak move generation, along with all required data types.

use std::fmt::Write;
use std::hash::{Hash, Hasher};
use std::{array, fmt, ops};
use std::{iter, mem};

use arrayvec::ArrayVec;
use board_game_traits::{Color, GameResult};
use board_game_traits::{EvalPosition as EvalPositionTrait, Position as PositionTrait};
use half::f16;
use lazy_static::lazy_static;
use pgn_traits::PgnPosition;
use rand::{Rng, SeedableRng};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use bitboard::BitBoard;
use color_trait::{BlackTr, WhiteTr};

pub use utils::{Direction, Komi, Movement, Piece, Piece::*, Role, Role::*, Stack, StackMovement};

pub use square::{squares_iterator, Square, SquareCacheEntry};

pub use utils::AbstractBoard;

pub use mv::{ExpMove, Move, ReverseMove};

use crate::evaluation::parameters::{self, IncrementalValue, PolicyApplier, ValueApplier};
use crate::evaluation::value_eval;
use crate::position::color_trait::ColorTr;

pub(crate) mod bitboard;
pub(crate) mod color_trait;
mod mv;
mod square;
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

const fn generate_square_symmetries_table<const S: usize>() -> AbstractBoard<usize, S> {
    let mut table = AbstractBoard::new_with_value(0);
    let mut i = 0;
    let mut rank = 0;
    while rank < S.div_ceil(2) {
        let mut file = rank;
        while file < S.div_ceil(2) {
            table.raw[file][rank] = i;
            table.raw[rank][file] = i;
            table.raw[S - file - 1][rank] = i;
            table.raw[S - rank - 1][file] = i;
            table.raw[file][S - rank - 1] = i;
            table.raw[rank][S - file - 1] = i;
            table.raw[S - file - 1][S - rank - 1] = i;
            table.raw[S - rank - 1][S - file - 1] = i;
            file += 1;
            i += 1;
        }
        rank += 1;
    }
    table
}

pub(crate) const SQUARE_SYMMETRIES_4S: AbstractBoard<usize, 4> = generate_square_symmetries_table();
pub(crate) const SQUARE_SYMMETRIES_5S: AbstractBoard<usize, 5> = generate_square_symmetries_table();
pub(crate) const SQUARE_SYMMETRIES_6S: AbstractBoard<usize, 6> = generate_square_symmetries_table();

pub(crate) fn lookup_square_symmetries<const S: usize>(square: Square<S>) -> usize {
    match S {
        4 => SQUARE_SYMMETRIES_4S[square.downcast_size()],
        5 => SQUARE_SYMMETRIES_5S[square.downcast_size()],
        6 => SQUARE_SYMMETRIES_6S[square.downcast_size()],
        _ => unimplemented!("Unsupported size {}", S),
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
    pub const fn empty() -> Self {
        Self { data: 0 }
    }

    pub const fn connect_square_const<const S: usize>(self, square: Square<S>) -> Self {
        let mut edge_connection = self;
        if square.rank() == S as u8 - 1 {
            edge_connection = edge_connection.connect_north();
        }
        if square.rank() == 0 {
            edge_connection = edge_connection.connect_south();
        }
        if square.file() == 0 {
            edge_connection = edge_connection.connect_west();
        }
        if square.file() == S as u8 - 1 {
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

    pub const fn connect_north(self) -> Self {
        GroupEdgeConnection {
            data: self.data | 0b1000,
        }
    }

    pub fn is_connected_west(self) -> bool {
        self.data & 0b100 != 0
    }

    pub const fn connect_west(self) -> Self {
        GroupEdgeConnection {
            data: self.data | 0b100,
        }
    }

    pub fn is_connected_east(self) -> bool {
        self.data & 0b10 != 0
    }

    pub const fn connect_east(self) -> Self {
        GroupEdgeConnection {
            data: self.data | 0b10,
        }
    }

    pub fn is_connected_south(self) -> bool {
        self.data & 1 != 0
    }

    pub const fn connect_south(self) -> Self {
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

#[derive(Clone, Copy, Debug)]
pub struct MovementSynopsis<const S: usize> {
    pub origin: Square<S>,
    pub destination: Square<S>,
}

fn our_last_movement<const S: usize>(position: &Position<S>) -> Option<MovementSynopsis<S>> {
    get_movement_in_history(position, 2)
}

fn their_last_movement<const S: usize>(position: &Position<S>) -> Option<MovementSynopsis<S>> {
    get_movement_in_history(position, 1)
}

fn get_movement_in_history<const S: usize>(
    position: &Position<S>,
    i: usize,
) -> Option<MovementSynopsis<S>> {
    position
        .moves()
        .get(position.moves().len().overflowing_sub(i).0)
        .and_then(|mv| match mv.expand() {
            ExpMove::Place(_, _) => None,
            ExpMove::Move(origin, direction, stack_movement) => Some(MovementSynopsis {
                origin,
                destination: origin.jump_valid_direction(direction, stack_movement.len() as u8),
            }),
        })
}

#[derive(Clone, Debug)]
pub struct GroupData<const S: usize> {
    pub(crate) groups: AbstractBoard<u8, S>,
    pub(crate) amount_in_group: ArrayVec<(u8, GroupEdgeConnection), 65>, // Size is max_size^2 + 1
    pub(crate) white_critical_squares: BitBoard,
    pub(crate) black_critical_squares: BitBoard,
    pub(crate) white_flat_stones: BitBoard,
    pub(crate) black_flat_stones: BitBoard,
    pub(crate) white_caps: BitBoard,
    pub(crate) black_caps: BitBoard,
    pub(crate) white_walls: BitBoard,
    pub(crate) black_walls: BitBoard,
    pub(crate) last_movement: Option<MovementSynopsis<S>>,
    pub(crate) second_to_last_movement: Option<MovementSynopsis<S>>,
}

impl<const S: usize> Default for GroupData<S> {
    fn default() -> Self {
        let mut group_data = GroupData {
            groups: Default::default(),
            amount_in_group: Default::default(),
            white_critical_squares: Default::default(),
            black_critical_squares: Default::default(),
            white_flat_stones: Default::default(),
            black_flat_stones: Default::default(),
            white_caps: Default::default(),
            black_caps: Default::default(),
            white_walls: Default::default(),
            black_walls: Default::default(),
            last_movement: Default::default(),
            second_to_last_movement: Default::default(),
        };
        for _ in 0..S * S + 1 {
            group_data
                .amount_in_group
                .push((0, GroupEdgeConnection::default()));
        }
        group_data
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

    pub fn is_critical_square(&self, square: Square<S>, color: Color) -> bool {
        match color {
            Color::White => WhiteTr::is_critical_square(self, square),
            Color::Black => BlackTr::is_critical_square(self, square),
        }
    }

    pub fn critical_squares(&self, color: Color) -> impl Iterator<Item = Square<S>> {
        match color {
            Color::White => self.white_critical_squares.into_iter(),
            Color::Black => self.black_critical_squares.into_iter(),
        }
    }

    pub fn last_movement(&self) -> Option<MovementSynopsis<S>> {
        self.last_movement
    }

    pub fn second_to_last_movement(&self) -> Option<MovementSynopsis<S>> {
        self.second_to_last_movement
    }
}
#[derive(PartialEq, Eq, Debug)]
pub struct ZobristKeys<const S: usize> {
    top_stones: AbstractBoard<[u64; 6], S>,
    stones_in_stack: [Box<AbstractBoard<[u64; 256], S>>; 8],
    to_move: [u64; 2],
}

pub fn zobrist_top_stones<const S: usize>(square: Square<S>, piece: Piece) -> u64 {
    match S {
        3 => ZOBRIST_KEYS_3S.top_stones[square.downcast_size()][piece as u16 as usize],
        4 => ZOBRIST_KEYS_4S.top_stones[square.downcast_size()][piece as u16 as usize],
        5 => ZOBRIST_KEYS_5S.top_stones[square.downcast_size()][piece as u16 as usize],
        6 => ZOBRIST_KEYS_6S.top_stones[square.downcast_size()][piece as u16 as usize],
        7 => ZOBRIST_KEYS_7S.top_stones[square.downcast_size()][piece as u16 as usize],
        8 => ZOBRIST_KEYS_8S.top_stones[square.downcast_size()][piece as u16 as usize],
        _ => panic!("No zobrist keys for size {}. Size not supported.", S),
    }
}

pub fn zobrist_stones_in_stack<const S: usize>(
    square: Square<S>,
    place_in_stack: usize,
    stack_slice: usize,
) -> u64 {
    match S {
        3 => ZOBRIST_KEYS_3S.stones_in_stack[place_in_stack][square.downcast_size()][stack_slice],
        4 => ZOBRIST_KEYS_4S.stones_in_stack[place_in_stack][square.downcast_size()][stack_slice],
        5 => ZOBRIST_KEYS_5S.stones_in_stack[place_in_stack][square.downcast_size()][stack_slice],
        6 => ZOBRIST_KEYS_6S.stones_in_stack[place_in_stack][square.downcast_size()][stack_slice],
        7 => ZOBRIST_KEYS_7S.stones_in_stack[place_in_stack][square.downcast_size()][stack_slice],
        8 => ZOBRIST_KEYS_8S.stones_in_stack[place_in_stack][square.downcast_size()][stack_slice],
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

        Box::new(ZobristKeys {
            top_stones: AbstractBoard::new_from_fn(|| array::from_fn(|_| rng.gen())),
            stones_in_stack: array::from_fn(|_| {
                Box::new(AbstractBoard::new_from_fn(|| array::from_fn(|_| rng.gen())))
            }),
            to_move: array::from_fn(|_| rng.gen()),
        })
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
pub struct Position<const S: usize> {
    stacks: AbstractBoard<BitBoard, S>,
    stack_heights: AbstractBoard<u8, S>,
    top_stones: AbstractBoard<Option<Piece>, S>,
    to_move: Color,
    white_stones_left: u8,
    black_stones_left: u8,
    white_caps_left: u8,
    black_caps_left: u8,
    half_moves_played: usize,
    moves: Vec<Move<S>>,
    komi: Komi,
    hash: u64,              // Zobrist hash of current position
    hash_history: Vec<u64>, // Zobrist hashes of previous board states, up to the last irreversible move. Does not include the corrent position
}

impl<const S: usize> Clone for Position<S> {
    fn clone(&self) -> Self {
        Self {
            stacks: self.stacks.clone(),
            stack_heights: self.stack_heights.clone(),
            top_stones: self.top_stones.clone(),
            to_move: self.to_move,
            white_stones_left: self.white_stones_left,
            black_stones_left: self.black_stones_left,
            white_caps_left: self.white_caps_left,
            black_caps_left: self.black_caps_left,
            half_moves_played: self.half_moves_played,
            moves: self.moves.clone(),
            komi: self.komi,
            hash: self.hash,
            hash_history: self.hash_history.clone(),
        }
    }
    fn clone_from(&mut self, source: &Self) {
        self.stacks = source.stacks.clone();
        self.stack_heights = source.stack_heights.clone();
        self.top_stones = source.top_stones.clone();
        self.to_move = source.to_move;
        self.white_stones_left = source.white_stones_left;
        self.black_stones_left = source.black_stones_left;
        self.white_caps_left = source.white_caps_left;
        self.black_caps_left = source.black_caps_left;
        self.half_moves_played = source.half_moves_played;
        self.moves.clone_from(&source.moves);
        self.komi = source.komi;
        self.hash = source.hash;
        self.hash_history.clone_from(&source.hash_history);
        debug_assert_eq!(self, source);
        debug_assert_eq!(self.moves, source.moves);
        debug_assert_eq!(self.hash_history, source.hash_history);
    }
}

impl<const S: usize> PartialEq for Position<S> {
    fn eq(&self, other: &Self) -> bool {
        self.stacks == other.stacks
            && self.stack_heights == other.stack_heights
            && self.top_stones == other.top_stones
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
        self.stacks.hash(state);
        self.stack_heights.hash(state);
        self.top_stones.hash(state);
        self.to_move.hash(state);
        self.white_stones_left.hash(state);
        self.black_stones_left.hash(state);
        self.white_caps_left.hash(state);
        self.black_caps_left.hash(state);
        self.half_moves_played.hash(state);
        self.komi.hash(state);
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
                        match self
                            .get_stack(Square::from_rank_file(y as u8, x as u8))
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
            stacks: Default::default(),
            stack_heights: Default::default(),
            top_stones: Default::default(),
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

    pub fn get_stack(&self, square: Square<S>) -> Stack {
        let bitboard = self.stacks[square];
        let top_stone = self.top_stones[square];
        let height = self.stack_heights[square];
        Stack {
            top_stone,
            bitboard,
            height,
        }
    }

    pub fn set_stack(&mut self, square: Square<S>, stack: Stack) {
        self.stacks[square] = stack.bitboard;
        self.stack_heights[square] = stack.height;
        self.top_stones[square] = stack.top_stone;
    }

    pub fn top_stones(&self) -> &AbstractBoard<Option<Piece>, S> {
        &self.top_stones
    }

    pub fn stack_heights(&self) -> &AbstractBoard<u8, S> {
        &self.stack_heights
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
    pub fn moves(&self) -> &Vec<Move<S>> {
        &self.moves
    }

    pub fn null_move(&mut self) {
        self.to_move = !self.to_move;
    }

    pub(crate) fn zobrist_hash_from_scratch(&self) -> u64 {
        let mut hash = 0;
        hash ^= zobrist_to_move::<S>(self.to_move);

        for square in square::squares_iterator::<S>() {
            hash ^= self.zobrist_hash_for_square(square);
        }
        hash
    }

    pub(crate) fn zobrist_hash_for_square(&self, square: Square<S>) -> u64 {
        let mut hash = 0;
        if let Some(top_stone) = self.top_stones[square] {
            hash ^= zobrist_top_stones::<S>(square, top_stone);
            // Only enter this loop if stack.len() is 2 or more
            for i in 0..(self.stack_heights[square] as usize + 6) / 8 {
                hash ^= zobrist_stones_in_stack::<S>(
                    square,
                    i,
                    self.stacks[square].board as usize >> (i * 8) & 255,
                )
            }
        }
        hash
    }

    fn is_critical_square_from_scratch<Us: ColorTr>(
        &self,
        group_data: &GroupData<S>,
        square: Square<S>,
    ) -> bool {
        let neighbors_bitboard = utils::lookup_neighbor_table::<S>(square);
        let our_road_stones = Us::road_stones(group_data);

        let mut our_neighbors = neighbors_bitboard & our_road_stones;
        let mut sum_of_connections = square.group_edge_connection();
        while let Some(neighbor_square) = our_neighbors.occupied_square() {
            sum_of_connections = sum_of_connections
                | group_data.amount_in_group[group_data.groups[neighbor_square] as usize].1;
            our_neighbors = our_neighbors.clear_square(neighbor_square);
        }

        sum_of_connections.is_winning()
    }

    pub fn flip_board_y(&self) -> Position<S> {
        let mut new_board = self.clone();
        for file in 0..S as u8 {
            for rank in 0..S as u8 {
                new_board.stacks[Square::from_rank_file(rank, file)] =
                    self.stacks[Square::from_rank_file(S as u8 - rank - 1, file)];
                new_board.stack_heights[Square::from_rank_file(rank, file)] =
                    self.stack_heights[Square::from_rank_file(S as u8 - rank - 1, file)];
                new_board.top_stones[Square::from_rank_file(rank, file)] =
                    self.top_stones[Square::from_rank_file(S as u8 - rank - 1, file)];
            }
        }
        new_board
    }

    pub fn flip_board_x(&self) -> Position<S> {
        let mut new_board = self.clone();
        for file in 0..S as u8 {
            for rank in 0..S as u8 {
                new_board.stacks[Square::from_rank_file(rank, file)] =
                    self.stacks[Square::from_rank_file(rank, S as u8 - file - 1)];
                new_board.stack_heights[Square::from_rank_file(rank, file)] =
                    self.stack_heights[Square::from_rank_file(rank, S as u8 - file - 1)];
                new_board.top_stones[Square::from_rank_file(rank, file)] =
                    self.top_stones[Square::from_rank_file(rank, S as u8 - file - 1)];
            }
        }
        new_board
    }

    pub fn rotate_board(&self) -> Position<S> {
        let mut new_board = self.clone();
        for file in 0..S as u8 {
            for rank in 0..S as u8 {
                let new_file = rank;
                let new_rank = S as u8 - file - 1;
                new_board.stacks[Square::from_rank_file(rank, file)] =
                    self.stacks[Square::from_rank_file(new_rank, new_file)];
                new_board.stack_heights[Square::from_rank_file(rank, file)] =
                    self.stack_heights[Square::from_rank_file(new_rank, new_file)];
                new_board.top_stones[Square::from_rank_file(rank, file)] =
                    self.top_stones[Square::from_rank_file(new_rank, new_file)];
            }
        }
        new_board
    }

    pub fn flip_colors(&self) -> Position<S> {
        let mut new_board = self.clone();
        for square in square::squares_iterator::<S>() {
            let mut new_stack = Stack::default();
            for piece in self.get_stack(square) {
                new_stack.push(piece.flip_color());
            }
            new_board.stacks[square] = new_stack.bitboard;
            new_board.stack_heights[square] = new_stack.height;
            new_board.top_stones[square] = new_stack.top_stone;
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
            .map(|square| self.stack_heights[square])
            .sum()
    }

    #[inline(never)]
    pub fn group_data(&self) -> GroupData<S> {
        let mut group_data = GroupData {
            last_movement: their_last_movement(self),
            second_to_last_movement: our_last_movement(self),
            ..Default::default()
        };

        for square in square::squares_iterator::<S>() {
            match self.top_stones[square] {
                Some(WhiteFlat) => {
                    group_data.white_flat_stones = group_data.white_flat_stones.set_square(square)
                }
                Some(BlackFlat) => {
                    group_data.black_flat_stones = group_data.black_flat_stones.set_square(square)
                }
                Some(WhiteWall) => {
                    group_data.white_walls = group_data.white_walls.set_square(square)
                }
                Some(BlackWall) => {
                    group_data.black_walls = group_data.black_walls.set_square(square)
                }
                Some(WhiteCap) => group_data.white_caps = group_data.white_caps.set_square(square),
                Some(BlackCap) => group_data.black_caps = group_data.black_caps.set_square(square),
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

        for square in square::squares_iterator::<S>() {
            group_data.amount_in_group[group_data.groups[square] as usize].0 += 1;
            if self.top_stones[square].is_some_and(Piece::is_road_piece) {
                group_data.amount_in_group[group_data.groups[square] as usize].1 =
                    group_data.amount_in_group[group_data.groups[square] as usize].1
                        | square.group_edge_connection();
            }
        }

        for square in square::squares_iterator::<S>() {
            if self.is_critical_square_from_scratch::<WhiteTr>(&group_data, square) {
                group_data.white_critical_squares =
                    group_data.white_critical_squares.set_square(square);
            }
            if self.is_critical_square_from_scratch::<BlackTr>(&group_data, square) {
                group_data.black_critical_squares =
                    group_data.black_critical_squares.set_square(square);
            }
        }
        group_data
    }

    /// An iterator over the top stones left behind after a stack movement
    pub fn top_stones_left_behind_by_move<'a>(
        &'a self,
        square: Square<S>,
        stack_movement: &'a StackMovement<S>,
    ) -> impl Iterator<Item = Option<Piece>> + 'a {
        stack_movement
            .into_iter()
            .map(move |Movement { pieces_to_take }| {
                let piece_index = self.stack_heights[square] - pieces_to_take;
                if piece_index == 0 {
                    None
                } else {
                    Some(self.get_stack(square).get(piece_index - 1).unwrap())
                }
            })
            .chain(std::iter::once(self.top_stones[square]))
    }

    pub(crate) fn fcd_for_move(&self, mv: Move<S>) -> i8 {
        match mv.expand() {
            ExpMove::Place(Role::Flat, _) if self.half_moves_played() > 1 => 1,
            ExpMove::Place(Role::Flat, _) => -1,
            ExpMove::Place(_, _) => 0,
            ExpMove::Move(square, direction, stack_movement) => {
                let mut destination_square =
                    if stack_movement.get_first().pieces_to_take == self.stack_heights[square] {
                        square.go_direction(direction).unwrap()
                    } else {
                        square
                    };

                let mut fcd = 0;

                if self.stack_heights[square] == stack_movement.get_first().pieces_to_take {
                    let top_stone = self.top_stones[square].unwrap();
                    if top_stone.role() == Flat {
                        fcd -= 1;
                    }
                }

                // This iterator skips the first square if we move the whole stack
                for piece in self
                    .top_stones_left_behind_by_move(square, &stack_movement)
                    .flatten()
                {
                    if let Some(captured_piece) = self.top_stones[destination_square] {
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
                        .go_direction(direction)
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
                .find(|(_i, v)| v.0 == 0)
                .map(|(i, _v)| i)
                .unwrap_or(S * S + 1) as u8;

            if let Some(square) = self.is_win_by_road(&group_data.groups, highest_component_id) {
                debug_assert!(self.top_stones[square].unwrap().is_road_piece());
                return if self.top_stones[square].unwrap().color() == Color::White {
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
            let white_points = group_data.white_flat_stones.count() as i8;
            let black_points = group_data.black_flat_stones.count() as i8;

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
    ) -> Option<Square<S>> {
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
                let square = square::squares_iterator::<S>()
                    .find(|&sq| components[sq] == id)
                    .unwrap();
                if self.top_stones[square].unwrap().color() == self.side_to_move() {
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
        params: &'static [f32],
    ) -> f32 {
        let (white_params, black_params) = params.split_at(params.len() / 2);
        let mut white_value_features: IncrementalValue<S> = IncrementalValue::new(white_params);
        let mut black_value_features: IncrementalValue<S> = IncrementalValue::new(black_params);
        value_eval::static_eval_game_phase(
            self,
            group_data,
            &mut white_value_features,
            &mut black_value_features,
        );
        white_value_features.finish() + black_value_features.finish()
    }

    pub fn value_params(komi: Komi) -> &'static [f32] {
        match komi.half_komi() {
            0 => Self::value_params_0komi(),
            4 => Self::value_params_2komi(),
            _ => unimplemented!("{} komi not supported in {}s", komi, S),
        }
    }

    pub fn policy_params(komi: Komi) -> &'static [f32] {
        match komi.half_komi() {
            0 => Self::policy_params_0komi(),
            4 => Self::policy_params_2komi(),
            _ => unimplemented!("{} komi not supported in {}s", komi, S),
        }
    }

    pub fn value_params_0komi() -> &'static [f32] {
        match S {
            4 => &parameters::VALUE_PARAMS_4S_0KOMI,
            5 => &parameters::VALUE_PARAMS_5S_0KOMI,
            6 => &parameters::VALUE_PARAMS_6S_0KOMI,
            _ => unimplemented!("{}s is not supported for 0 komi.", S),
        }
    }

    pub fn value_params_2komi() -> &'static [f32] {
        match S {
            4 => &parameters::VALUE_PARAMS_4S_2KOMI,
            5 => &parameters::VALUE_PARAMS_5S_2KOMI,
            6 => &parameters::VALUE_PARAMS_6S_2KOMI,
            _ => unimplemented!("{}s is not supported for 2 komi.", S),
        }
    }

    pub fn policy_params_0komi() -> &'static [f32] {
        match S {
            4 => &parameters::POLICY_PARAMS_4S_0KOMI,
            5 => &parameters::POLICY_PARAMS_5S_0KOMI,
            6 => &parameters::POLICY_PARAMS_6S_0KOMI,
            _ => unimplemented!("{}s is not supported for 0 komi.", S),
        }
    }

    pub fn policy_params_2komi() -> &'static [f32] {
        match S {
            4 => &parameters::POLICY_PARAMS_4S_2KOMI,
            5 => &parameters::POLICY_PARAMS_5S_2KOMI,
            6 => &parameters::POLICY_PARAMS_6S_2KOMI,
            _ => unimplemented!("{}s is not supported for 2 komi.", S),
        }
    }

    pub fn static_eval_features<V: ValueApplier>(&self, white_value: &mut V, black_value: &mut V) {
        debug_assert!(self.game_result().is_none());

        let group_data = self.group_data();

        value_eval::static_eval_game_phase(self, &group_data, white_value, black_value);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn generate_moves_with_params<P: PolicyApplier>(
        &self,
        params: &'static [f32],
        group_data: &GroupData<S>,
        simple_moves: &mut Vec<<Self as PositionTrait>::Move>,
        moves: &mut Vec<(<Self as PositionTrait>::Move, f16)>,
        fcd_per_move: &mut Vec<i8>,
        policy_feature_sets: &mut Vec<P>,
    ) {
        debug_assert!(simple_moves.is_empty());
        self.generate_moves(simple_moves);
        match self.side_to_move() {
            Color::White => self.generate_moves_with_probabilities_colortr::<WhiteTr, BlackTr, P>(
                params,
                group_data,
                simple_moves,
                fcd_per_move,
                moves,
                policy_feature_sets,
            ),
            Color::Black => self.generate_moves_with_probabilities_colortr::<BlackTr, WhiteTr, P>(
                params,
                group_data,
                simple_moves,
                fcd_per_move,
                moves,
                policy_feature_sets,
            ),
        }
    }

    /// Move generation that includes a heuristic probability of each move being played.
    ///
    /// # Arguments
    ///
    /// * `simple_moves` - An empty vector to temporarily store moves without probabilities. The vector will be emptied before the function returns, and only serves to re-use allocated memory.
    /// * `moves` A vector to place the moves and associated probabilities.
    #[allow(clippy::too_many_arguments)]
    pub fn generate_moves_with_probabilities<P: PolicyApplier>(
        &self,
        group_data: &GroupData<S>,
        simple_moves: &mut Vec<Move<S>>,
        moves: &mut Vec<(Move<S>, f16)>,
        fcd_per_move: &mut Vec<i8>,
        policy_params: &'static [f32],
        policy_feature_sets: &mut Vec<P>,
    ) {
        self.generate_moves_with_params(
            policy_params,
            group_data,
            simple_moves,
            moves,
            fcd_per_move,
            policy_feature_sets,
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
                    let reverse_move = self.do_move(mv);
                    let num_moves = self.perft(depth - 1);
                    self.reverse_move(reverse_move);
                    debug_assert_eq!(
                        *self,
                        old_position,
                        "Failed to restore old board after {:?} on\n{:?}",
                        mv.to_string(),
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
            let mut moves = Vec::with_capacity(S * S * 4);
            self.generate_moves(&mut moves);

            if depth == 1 {
                moves.len() as u64
            } else {
                moves
                    .into_iter()
                    .map(|mv| {
                        let reverse_move = self.do_move(mv);
                        let num_moves = self.bulk_perft(depth - 1);
                        self.reverse_move(reverse_move);
                        num_moves
                    })
                    .sum()
            }
        }
    }
}

impl<const S: usize> PositionTrait for Position<S> {
    type Move = Move<S>;
    type ReverseMove = ReverseMove<S>;
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
                square::squares_iterator::<S>()
                    .filter(|square| self.stack_heights[*square] == 0)
                    .map(|square| Move::placement(Flat, square)),
            ),
            _ => match self.side_to_move() {
                Color::White => self.generate_moves_colortr::<E, WhiteTr, BlackTr>(moves),
                Color::Black => self.generate_moves_colortr::<E, BlackTr, WhiteTr>(moves),
            },
        }
    }

    fn move_is_legal(&self, mv: Self::Move) -> bool {
        match (mv.expand(), self.side_to_move()) {
            (ExpMove::Place(Flat | Wall, square), Color::White) => {
                self.stack_heights[square] == 0 && self.white_reserves_left() > 0
            }
            (ExpMove::Place(Flat | Wall, square), Color::Black) => {
                self.stack_heights[square] == 0 && self.black_reserves_left() > 0
            }
            (ExpMove::Place(Cap, square), Color::White) => {
                self.stack_heights[square] == 0 && self.white_caps_left() > 0
            }
            (ExpMove::Place(Cap, square), Color::Black) => {
                self.stack_heights[square] == 0 && self.black_caps_left() > 0
            }
            (ExpMove::Move(square, _, _), Color::White) => {
                let mut legal_moves = vec![];
                self.generate_moves_for_square_colortr::<_, WhiteTr, BlackTr>(
                    &mut legal_moves,
                    square,
                );
                legal_moves.contains(&mv)
            }
            (ExpMove::Move(square, _, _), Color::Black) => {
                let mut legal_moves = vec![];
                self.generate_moves_for_square_colortr::<_, BlackTr, WhiteTr>(
                    &mut legal_moves,
                    square,
                );
                legal_moves.contains(&mv)
            }
        }
    }

    fn do_move(&mut self, mv: Self::Move) -> Self::ReverseMove {
        self.hash_history.push(self.hash);
        let reverse_move = match mv.expand() {
            ExpMove::Place(role, to) => {
                debug_assert!(self.stack_heights[to] == 0);
                // On the first move, the players place the opponent's color
                let color_to_place = if self.half_moves_played() > 1 {
                    self.side_to_move()
                } else {
                    !self.side_to_move()
                };
                let piece = Piece::from_role_color(role, color_to_place);
                let mut to_stack = self.get_stack(to);
                to_stack.push(piece);
                self.set_stack(to, to_stack);

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
            ExpMove::Move(square, direction, stack_movement) => {
                for sq in <MoveIterator<S>>::new(square, direction, stack_movement) {
                    self.hash ^= self.zobrist_hash_for_square(sq);
                }

                let mut to = square;

                let mut pieces_left_behind = ArrayVec::new();
                let mut flattens_stone = false;

                let mut movement_iter = stack_movement.into_iter();
                let mut moving_pieces: ArrayVec<Piece, 8> = ArrayVec::new();

                let mut stack = self.get_stack(square);
                for _ in 0..movement_iter.next().unwrap().pieces_to_take {
                    moving_pieces.push(stack.pop().unwrap());
                }
                self.set_stack(square, stack);

                for Movement { pieces_to_take } in
                    movement_iter.chain(iter::once(Movement { pieces_to_take: 0 }))
                {
                    to = to.go_direction(direction).unwrap();

                    if self.top_stones[to].map(Piece::role) == Some(Wall) {
                        flattens_stone = true;
                    }

                    pieces_left_behind.push(moving_pieces.len() as u8 - pieces_to_take);

                    let mut to_stack = self.get_stack(to);
                    while moving_pieces.len() as u8 > pieces_to_take {
                        to_stack.push(moving_pieces.pop().unwrap());
                    }
                    self.set_stack(to, to_stack);
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
                let mut stack = self.get_stack(square);
                let piece = stack.pop().unwrap();
                self.set_stack(square, stack);

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
                    to = to.go_direction(direction).unwrap();
                    let mut to_stack = self.get_stack(to);
                    let temp_pieces: ArrayVec<Piece, 8> = (0..piece_left_behind)
                        .map(|_| to_stack.pop().unwrap())
                        .collect();
                    self.set_stack(to, to_stack);

                    let mut from_stack = self.get_stack(from);
                    for piece in temp_pieces.into_iter().rev() {
                        from_stack.push(piece);
                    }
                    self.set_stack(from, from_stack);
                }

                if flattens_wall {
                    debug_assert_eq!(self.top_stones[to].map(Piece::role), Some(Flat));
                    match self.top_stones[to].unwrap().color() {
                        Color::White => self.top_stones[to].replace(WhiteWall),
                        Color::Black => self.top_stones[to].replace(BlackWall),
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
    square: Square<S>,
    direction: Direction,
    squares_left: usize,
    _size: [(); S],
}

impl<const S: usize> MoveIterator<S> {
    pub fn new(square: Square<S>, direction: Direction, stack_movement: StackMovement<S>) -> Self {
        MoveIterator {
            square,
            direction,
            squares_left: stack_movement.len() + 1,
            _size: [(); S],
        }
    }
}

impl<const S: usize> Iterator for MoveIterator<S> {
    type Item = Square<S>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.squares_left == 0 {
            None
        } else {
            let next_square = self.square;
            if self.squares_left > 1 {
                self.square = self.square.go_direction(self.direction).unwrap();
            }
            self.squares_left -= 1;
            Some(next_square)
        }
    }
}

impl<const S: usize> EvalPositionTrait for Position<S> {
    fn static_eval(&self) -> f32 {
        let params = Self::value_params(self.komi());

        let (white_params, black_params) = params.split_at(params.len() / 2);
        let mut white_value: IncrementalValue<S> = IncrementalValue::new(white_params);
        let mut black_value: IncrementalValue<S> = IncrementalValue::new(black_params);

        self.static_eval_features(&mut white_value, &mut black_value);

        white_value.finish() + black_value.finish()
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
        for square in square::squares_iterator::<S>() {
            let (file, rank) = (square.file(), square.rank());
            let stack = rows[rank as usize][file as usize];
            for piece in stack.into_iter() {
                match piece {
                    WhiteFlat | WhiteWall => position.white_stones_left -= 1,
                    WhiteCap => position.white_caps_left -= 1,
                    BlackFlat | BlackWall => position.black_stones_left -= 1,
                    BlackCap => position.black_caps_left -= 1,
                }
            }
            position.set_stack(square, stack);
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
                let square = Square::from_rank_file(rank as u8, file as u8);
                if self.stack_heights[square] == 0 {
                    f.push('x')
                } else {
                    for piece in self.get_stack(square).into_iter() {
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
        Self::Move::from_string(input)
    }

    fn move_to_san(&self, mv: &Self::Move) -> String {
        mv.to_string()
    }

    fn move_from_lan(&self, input: &str) -> Result<Self::Move, pgn_traits::Error> {
        Self::Move::from_string(input)
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
    for square in square::squares_iterator::<S>() {
        if components[square] == 0 && road_pieces.get_square(square) {
            connect_component(road_pieces, components, square, *id);
            *id += 1;
        }
    }
}

fn connect_component<const S: usize>(
    road_pieces: BitBoard,
    components: &mut AbstractBoard<u8, S>,
    square: Square<S>,
    id: u8,
) {
    components[square] = id;
    for neighbour in square.neighbors() {
        if road_pieces.get_square(neighbour) && components[neighbour] == 0 {
            connect_component(road_pieces, components, neighbour, id);
        }
    }
}

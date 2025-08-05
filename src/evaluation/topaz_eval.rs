use board_game_traits::{Color, Position as _};

use crate::position::{self, squares_iterator, Position};

pub const STACK_DEPTH: usize = 10;
pub const HIDDEN_SIZE: usize = 512;
pub const SCALE: i32 = 400;
pub const QA: i16 = 255;
pub const QB: i16 = 64;
pub const WHITE_FLAT: ValidPiece = ValidPiece(0);
pub const BLACK_FLAT: ValidPiece = ValidPiece(1);
pub const WHITE_WALL: ValidPiece = ValidPiece(2);
pub const BLACK_WALL: ValidPiece = ValidPiece(3);
pub const WHITE_CAP: ValidPiece = ValidPiece(4);
pub const BLACK_CAP: ValidPiece = ValidPiece(5);
const _ASS: () = assert!(
    WHITE_FLAT.flip_color().0 == BLACK_FLAT.0
        && BLACK_WALL.flip_color().0 == WHITE_WALL.0
        && BLACK_CAP.flip_color().0 == WHITE_CAP.0
);

pub static NNUE: Network = unsafe {
    let bytes = include_bytes!("../quantised.bin");
    assert!(bytes.len() == std::mem::size_of::<Network>());
    std::mem::transmute(*bytes)
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidPiece(pub u8);

impl ValidPiece {
    pub const fn without_color(self) -> u8 {
        self.0 >> 1
    }
    const fn flip_color(self) -> Self {
        Self(self.0 ^ 1) // Toggle bit 0
    }
    pub const fn promote_cap(self) -> Self {
        Self(self.0 | 4) // Set bit 2
    }
    pub const fn is_white(self) -> bool {
        (self.0 & 1) == 0
    }
    pub const fn color_index(self) -> usize {
        (self.0 & 1) as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PieceSquare(pub u8);

impl PieceSquare {
    pub fn new(square: usize, piece: u8) -> Self {
        Self((square as u8) | piece << 6)
    }
    pub fn square(self) -> u8 {
        self.0 & 63
    }
    pub fn piece(self) -> ValidPiece {
        let masked = 0b1100_0000 & self.0;
        ValidPiece(masked >> 6)
    }
    pub fn promote_wall(&mut self) {
        self.0 |= 128;
    }
    pub fn topaz_piece(self) -> Piece {
        match self.piece() {
            WHITE_FLAT => Piece::WhiteFlat,
            BLACK_FLAT => Piece::BlackFlat,
            WHITE_WALL => Piece::WhiteWall,
            BLACK_WALL => Piece::BlackWall,
            WHITE_CAP => Piece::WhiteCap,
            BLACK_CAP => Piece::BlackCap,
            _ => unimplemented!(),
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BoardData {
    pub caps: [u8; 2],
    pub data: [PieceSquare; 62], // Each stack must be presented from top to bottom sequentially
    pub data_len: u8,
    pub white_to_move: bool,
}

impl<const S: usize> From<Position<S>> for BoardData {
    fn from(position: Position<S>) -> Self {
        let mut data = [PieceSquare(255); 62];
        let mut data_len: u8 = 0;
        let mut caps = [255; 2];
        for square in squares_iterator::<S>() {
            let stack: Vec<position::Piece> = position.get_stack(square).into_iter().collect();
            for piece in stack.iter().rev().take(STACK_DEPTH) {
                let topaz_piece = match piece {
                    position::Piece::WhiteFlat => WHITE_FLAT,
                    position::Piece::BlackFlat => BLACK_FLAT,
                    position::Piece::WhiteWall => WHITE_WALL,
                    position::Piece::BlackWall => BLACK_WALL,
                    position::Piece::WhiteCap => WHITE_FLAT,
                    position::Piece::BlackCap => BLACK_FLAT,
                };
                data[data_len as usize] =
                    PieceSquare::new(square.into_inner() as usize, topaz_piece.0);
                data_len += 1;
            }
            if position.top_stones()[square] == Some(position::Piece::WhiteCap) {
                caps[0] = square.into_inner();
            }
            if position.top_stones()[square] == Some(position::Piece::BlackCap) {
                caps[1] = square.into_inner();
            }
        }
        Self {
            caps,
            data,
            data_len,
            white_to_move: position.side_to_move() == Color::White,
        }
    }
}

impl BoardData {
    const SIZE: u8 = 6;
    const SYM_TABLE: [[u8; 36]; 8] = Self::build_symmetry_table();
    pub fn new(caps: [u8; 2], data: [PieceSquare; 62], data_len: u8, white_to_move: bool) -> Self {
        Self {
            caps,
            data,
            data_len,
            white_to_move,
        }
    }
    pub fn symmetry(mut self, idx: usize) -> Self {
        if idx == 0 {
            return self;
        }
        assert!(idx < 8);
        let table = &Self::SYM_TABLE[idx];
        if self.caps[0] < 36 {
            self.caps[0] = table[self.caps[0] as usize];
        }
        if self.caps[1] < 36 {
            self.caps[1] = table[self.caps[1] as usize];
        }
        for i in 0..(self.data_len as usize) {
            let old = self.data[i];
            self.data[i] = PieceSquare::new(table[old.square() as usize] as usize, old.piece().0);
        }
        self
    }
    const fn build_symmetry_table() -> [[u8; 36]; 8] {
        [
            Self::transform(0),
            Self::transform(1),
            Self::transform(2),
            Self::transform(3),
            Self::transform(4),
            Self::transform(5),
            Self::transform(6),
            Self::transform(7),
        ]
    }
    const fn transform(rotation: usize) -> [u8; 36] {
        let mut data = [(0, 0); 36];
        let mut i = 0;
        while i < 36 {
            let (row, col) = Self::row_col(i as u8);
            data[i] = (row, col);
            i += 1;
        }
        match rotation {
            1 => Self::flip_ns(&mut data),
            2 => Self::flip_ew(&mut data),
            3 => Self::rotate(&mut data),
            4 => {
                Self::rotate(&mut data);
                Self::rotate(&mut data);
            }
            5 => {
                Self::rotate(&mut data);
                Self::rotate(&mut data);
                Self::rotate(&mut data);
            }
            6 => {
                Self::rotate(&mut data);
                Self::flip_ns(&mut data);
            }
            7 => {
                Self::rotate(&mut data);
                Self::flip_ew(&mut data);
            }
            _ => {}
        };
        let mut out = [0; 36];
        let mut i = 0;
        while i < 36 {
            let (row, col) = data[i];
            out[i] = Self::index(row, col);
            i += 1;
        }
        out
    }
    const fn flip_ns(arr: &mut [(u8, u8); 36]) {
        let mut i = 0;
        while i < 36 {
            let (row, _col) = &mut arr[i];
            *row = Self::SIZE - 1 - *row;
            i += 1;
        }
    }
    const fn flip_ew(arr: &mut [(u8, u8); 36]) {
        let mut i = 0;
        while i < 36 {
            let (_row, col) = &mut arr[i];
            *col = Self::SIZE - 1 - *col;
            i += 1;
        }
    }
    const fn rotate(arr: &mut [(u8, u8); 36]) {
        let mut i = 0;
        while i < 36 {
            let (row, col) = &mut arr[i];
            let new_row = Self::SIZE - 1 - *col;
            *col = *row;
            *row = new_row;
            i += 1;
        }
    }
    const fn row_col(index: u8) -> (u8, u8) {
        (index / Self::SIZE, index % Self::SIZE)
    }
    const fn index(row: u8, col: u8) -> u8 {
        row * Self::SIZE + col
    }
}

#[derive(Clone, Copy)]
pub struct TakSimple6 {}

impl TakSimple6 {
    pub const SQUARE_INPUTS: usize = 36 * (6 + 2 * STACK_DEPTH);
    // Squares + Side + Reserves
    pub const NUM_INPUTS: usize = TakSimple6::SQUARE_INPUTS + 8 + 80; // Pad to 1024

    pub fn handle_features<F: FnMut(usize, usize)>(&self, pos: &BoardData, mut f: F) {
        let mut reserves: [usize; 2] = [31, 31];
        for (piece, square, depth_idx) in pos.into_iter() {
            let c = (piece.is_white() ^ pos.white_to_move) as usize; // 0 if matches, else 1
            reserves[c] -= 1;
            let location = usize::from(piece.without_color() + depth_idx);
            let sq = usize::from(square);

            let stm = [0, 468][c] + 36 * location + sq;
            let ntm = [468, 0][c] + 36 * location + sq;
            f(stm, ntm);
        }
        let white_res_adv = (31 + reserves[0] - reserves[1]).clamp(23, 39);
        let black_res_adv = (31 + reserves[1] - reserves[0]).clamp(23, 39);
        if pos.white_to_move {
            // White to move
            f(
                Self::SQUARE_INPUTS + 8 + reserves[0],
                Self::SQUARE_INPUTS + 8 + reserves[1],
            );
            f(975 + white_res_adv, 975 + black_res_adv);
            f(Self::SQUARE_INPUTS, Self::SQUARE_INPUTS + 1);
        } else {
            // Black to move
            f(
                Self::SQUARE_INPUTS + 8 + reserves[1],
                Self::SQUARE_INPUTS + 8 + reserves[0],
            );
            f(975 + black_res_adv, 960 + white_res_adv);
            f(Self::SQUARE_INPUTS + 1, Self::SQUARE_INPUTS);
        }
    }
}

impl IntoIterator for BoardData {
    type Item = (ValidPiece, u8, u8);
    type IntoIter = TakBoardIter;
    fn into_iter(self) -> Self::IntoIter {
        TakBoardIter {
            board: self,
            idx: 0,
            last: u8::MAX,
            depth: 0,
        }
    }
}

pub struct TakBoardIter {
    board: BoardData,
    idx: usize,
    last: u8,
    depth: u8,
}

impl Iterator for TakBoardIter {
    type Item = (ValidPiece, u8, u8); // PieceType, Square, Depth
    fn next(&mut self) -> Option<Self::Item> {
        const DEPTH_TABLE: [u8; 10] = [0, 3, 4, 5, 6, 7, 8, 9, 10, 11];
        if self.idx > self.board.data.len() {
            return None;
        }
        let val = self.board.data[self.idx];
        let square = val.square();
        if square >= 36 {
            return None;
        }
        let mut piece = val.piece();
        if square == self.last {
            self.depth += 1;
        } else {
            self.depth = 0;
            if self.board.caps[0] == square || self.board.caps[1] == square {
                piece = piece.promote_cap();
            }
        }
        self.idx += 1;
        self.last = square;
        Some((piece, square, DEPTH_TABLE[self.depth as usize]))
    }
}

/// A column of the feature-weights matrix.
/// Note the `align(64)`.
#[derive(Clone, Copy)]
#[repr(C, align(64))]
pub struct Accumulator {
    vals: [i16; HIDDEN_SIZE],
}

impl Accumulator {
    /// Initialised with bias so we can just efficiently
    /// operate on it afterwards.
    pub fn new(net: &Network) -> Self {
        net.feature_bias
    }

    pub fn from_old(old: &Self) -> Self {
        old.clone()
    }

    pub fn add_all(&mut self, features: &[u16], net: &Network) {
        for f in features {
            self.add_feature(*f as usize, net);
        }
    }

    pub fn remove_all(&mut self, features: &[u16], net: &Network) {
        for f in features {
            self.remove_feature(*f as usize, net);
        }
    }

    /// Add a feature to an accumulator.
    pub fn add_feature(&mut self, feature_idx: usize, net: &Network) {
        for (i, d) in self
            .vals
            .iter_mut()
            .zip(&net.feature_weights[feature_idx].vals)
        {
            *i += *d
        }
    }

    /// Remove a feature from an accumulator.
    pub fn remove_feature(&mut self, feature_idx: usize, net: &Network) {
        for (i, d) in self
            .vals
            .iter_mut()
            .zip(&net.feature_weights[feature_idx].vals)
        {
            *i -= *d
        }
    }
}

pub struct NNUE6 {
    white: (Incremental, Incremental),
    black: (Incremental, Incremental),
    pub(crate) tempo_offset: i32,
}

impl NNUE6 {
    pub fn incremental_eval(&mut self, takboard: BoardData) -> i32 {
        let (ours, theirs) = build_features(takboard);
        let (old_ours, old_theirs) = if takboard.white_to_move {
            (&self.white.0, &self.white.1)
        } else {
            (&self.black.0, &self.black.1)
        };
        // Ours
        let mut ours_acc = Accumulator::from_old(&old_ours.vec);
        let (sub, add) = ours.diff(&old_ours.state);
        ours_acc.remove_all(&sub, &NNUE);
        ours_acc.add_all(&add, &NNUE);
        let ours = Incremental {
            state: ours,
            vec: ours_acc,
        };
        // Theirs
        let mut theirs_acc = Accumulator::from_old(&old_theirs.vec);
        let (sub, add) = theirs.diff(&old_theirs.state);
        theirs_acc.remove_all(&sub, &NNUE);
        theirs_acc.add_all(&add, &NNUE);
        let theirs = Incremental {
            state: theirs,
            vec: theirs_acc,
        };
        // Output
        let eval = NNUE.evaluate(&ours.vec, &theirs.vec, ours.state.clone().into_iter());
        if takboard.white_to_move {
            self.white = (ours, theirs);
        } else {
            self.black = (ours, theirs);
        }
        eval
    }
    pub(crate) fn manual_eval(takboard: BoardData) -> i32 {
        let (ours, theirs) = build_features(takboard);
        let ours = Incremental::fresh_new(&NNUE, ours);
        let theirs = Incremental::fresh_new(&NNUE, theirs);
        let eval = NNUE.evaluate(&ours.vec, &theirs.vec, ours.state.clone().into_iter());
        eval
    }
}

impl Default for NNUE6 {
    fn default() -> Self {
        Self {
            white: (
                Incremental::fresh_empty(&NNUE),
                Incremental::fresh_empty(&NNUE),
            ),
            black: (
                Incremental::fresh_empty(&NNUE),
                Incremental::fresh_empty(&NNUE),
            ),
            tempo_offset: 100,
        }
    }
}

fn build_features(takboard: BoardData) -> (IncrementalState, IncrementalState) {
    let mut ours = IncrementalState::empty();
    let mut theirs = IncrementalState::empty();
    let simple = TakSimple6 {};
    simple.handle_features(&takboard, |x, y| {
        ours.add_feature(x as u16);
        theirs.add_feature(y as u16);
    });
    (ours, theirs)
}

#[inline]
pub fn screlu(x: i16) -> i32 {
    i32::from(x.clamp(0, QA as i16)).pow(2)
}

/// This is the quantised format that bullet outputs.
#[repr(C)]
pub struct Network {
    /// Column-Major `HIDDEN_SIZE x 768` matrix.
    feature_weights: [Accumulator; TakSimple6::NUM_INPUTS],
    /// Vector with dimension `HIDDEN_SIZE`.
    feature_bias: Accumulator,
    /// Column-Major `1 x (2 * HIDDEN_SIZE)`
    /// matrix, we use it like this to make the
    /// code nicer in `Network::evaluate`.
    output_weights: [i16; 2 * HIDDEN_SIZE],
    /// Piece-Square Table for Input
    pqst: [i16; TakSimple6::NUM_INPUTS],
    /// Scalar output bias.
    output_bias: i16,
}

impl Network {
    /// Calculates the output of the network, starting from the already
    /// calculated hidden layer (done efficiently during makemoves).
    fn evaluate(&self, us: &Accumulator, them: &Accumulator, original: BitSetIterator) -> i32 {
        // Initialise output with bias.
        let mut sum = 0;
        let mut psqt_out = 0;

        // Side-To-Move Accumulator -> Output.
        for (&input, &weight) in us.vals.iter().zip(&self.output_weights[..HIDDEN_SIZE]) {
            let val = screlu(input) * i32::from(weight);
            sum += val;
        }

        // Not-Side-To-Move Accumulator -> Output.
        for (&input, &weight) in them.vals.iter().zip(&self.output_weights[HIDDEN_SIZE..]) {
            sum += screlu(input) * i32::from(weight);
        }

        // Update Piece Square Table
        for idx in original {
            psqt_out += i32::from(self.pqst[idx as usize]);
        }
        // Apply eval scale.
        psqt_out *= SCALE;
        // Remove quantisation.
        let output =
            (sum / (QA as i32) + i32::from(self.output_bias)) * SCALE / (QA as i32 * QB as i32);
        psqt_out /= i32::from(QA);
        output + psqt_out
    }
}

// Sorry this naming convention is so bad
struct Incremental {
    state: IncrementalState,
    vec: Accumulator,
}

impl Incremental {
    fn fresh_empty(net: &Network) -> Self {
        let acc = Accumulator::new(net);
        let inc = IncrementalState::empty();
        Self {
            state: inc,
            vec: acc,
        }
    }
    fn fresh_new(net: &Network, data: IncrementalState) -> Self {
        let mut acc = Accumulator::new(net);
        for d in data.clone().into_iter() {
            acc.add_feature(d as usize, net);
        }
        Self {
            vec: acc,
            state: data,
        }
    }
}

// struct IncrementalState {
//     pub(crate) meta: [u16; 3],
//     pub(crate) piece_data: [u16; 62],
// }

// impl IncrementalState {
//     pub fn from_vec(mut vec: Vec<u16>) -> Self {
//         let mut meta = [0; 3];
//         for i in 0..3 {
//             meta[i] = vec.pop().unwrap();
//         }
//         let mut piece_data = [u16::MAX; 62];
//         piece_data[0..vec.len()].copy_from_slice(&vec);
//         Self { meta, piece_data }
//     }
//     pub fn diff(&self, old: &Self) -> (Vec<u16>, Vec<u16>) {
//         // Todo in the real algorithm, do not allocate vecs. This is just to demonstrate the idea
//         let mut subtract = Vec::new();
//         let mut add = Vec::new();
//         Self::operate(&self.meta, &old.meta, &mut add);
//         Self::operate(&old.meta, &self.meta, &mut subtract);
//         // Piece data is not sorted, but it is grouped by square
//         let mut new_st = 0;
//         let mut old_st = 0;
//         loop {
//             let ol = Self::get_sq(old.piece_data[old_st]);
//             let nw = Self::get_sq(self.piece_data[new_st]);
//             if ol >= 36 && nw >= 36 {
//                 break;
//             }
//             if nw < ol {
//                 let new_end = Self::get_end(&self.piece_data, new_st);
//                 add.extend(self.piece_data[new_st..new_end].iter().copied());
//                 new_st = new_end;
//             } else if ol < nw {
//                 let old_end = Self::get_end(&old.piece_data, old_st);
//                 subtract.extend(old.piece_data[old_st..old_end].iter().copied());
//                 old_st = old_end;
//             } else {
//                 // They are equal
//                 let new_end = Self::get_end(&self.piece_data, new_st);
//                 let old_end = Self::get_end(&old.piece_data, old_st);
//                 Self::operate(
//                     &self.piece_data[new_st..new_end],
//                     &old.piece_data[old_st..old_end],
//                     &mut add,
//                 );
//                 Self::operate(
//                     &old.piece_data[old_st..old_end],
//                     &self.piece_data[new_st..new_end],
//                     &mut subtract,
//                 );
//                 new_st = new_end;
//                 old_st = old_end;
//             }
//         }
//         // End
//         (subtract, add)
//     }
//     fn get_end(slice: &[u16], st: usize) -> usize {
//         let st_val = Self::get_sq(slice[st]);
//         st + slice[st..]
//             .iter()
//             .position(|&x| Self::get_sq(x) != st_val)
//             .unwrap()
//     }
//     /// Extend out with values in left which are not present in right
//     fn operate(left: &[u16], right: &[u16], out: &mut Vec<u16>) {
//         out.extend(left.iter().copied().filter(|x| !right.contains(x)));
//     }
//     fn get_sq(val: u16) -> u16 {
//         if val == u16::MAX {
//             return 64;
//         }
//         val % 36
//     }
// }

struct BitSetIterator {
    bitset: [u64; Self::SIZE],
    idx: usize,
}

impl BitSetIterator {
    const SIZE: usize = 16;
    const END: usize = Self::SIZE - 1;
}

impl Iterator for BitSetIterator {
    type Item = u16;

    fn next(&mut self) -> Option<Self::Item> {
        while self.bitset[self.idx] == 0 {
            if self.idx >= Self::END {
                return None;
            }
            self.idx += 1
        }
        let b_idx = pop_lowest(&mut self.bitset[self.idx]) as u16;
        let out = self.idx as u16 * 64 + b_idx;
        Some(out)
    }
}

#[derive(Clone)]
struct IncrementalState {
    pub(crate) bitset: [u64; 16],
}

impl IntoIterator for IncrementalState {
    type Item = u16;

    type IntoIter = BitSetIterator;

    fn into_iter(self) -> Self::IntoIter {
        BitSetIterator {
            bitset: self.bitset,
            idx: 0,
        }
    }
}

impl IncrementalState {
    pub fn empty() -> Self {
        let bitset = [0; 16];
        Self { bitset }
    }
    pub fn from_vec(vec: Vec<u16>) -> Self {
        let mut bitset = [0; 16];
        for val in vec {
            let b_idx = (val / 64) as usize;
            let b_val = 1 << (val % 64);
            bitset[b_idx] |= b_val
        }
        Self { bitset }
    }
    pub fn add_feature(&mut self, val: u16) {
        let b_idx = (val / 64) as usize;
        let b_val = 1 << (val % 64);
        self.bitset[b_idx] |= b_val
    }
    pub fn diff(&self, old: &Self) -> (Vec<u16>, Vec<u16>) {
        // Todo in the real algorithm, do not allocate vecs. This is just to demonstrate the idea
        let mut subtract_indices = Vec::new();
        let mut add_indices = Vec::new();
        for (idx, (n, o)) in self
            .bitset
            .iter()
            .copied()
            .zip(old.bitset.iter().copied())
            .enumerate()
        {
            let d = n ^ o; // Difference between bitsets
            if d != 0 {
                let mut sub = d & o; // Difference and Old
                let mut add = d & n; // Difference and New
                while sub != 0 {
                    let bit_idx = pop_lowest(&mut sub);
                    subtract_indices.push(idx as u16 * 64 + bit_idx as u16)
                }
                while add != 0 {
                    let bit_idx = pop_lowest(&mut add);
                    add_indices.push(idx as u16 * 64 + bit_idx as u16)
                }
            }
        }
        (subtract_indices, add_indices)
    }
}

fn pop_lowest(x: &mut u64) -> u32 {
    let highest_index = x.trailing_zeros();
    if highest_index == 64 {
        0
    } else {
        let value = 1 << highest_index;
        *x ^= value;
        highest_index
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Hash)]
pub enum Piece {
    WhiteFlat = 1,
    WhiteWall = 2,
    WhiteCap = 3,
    BlackFlat = 4,
    BlackWall = 5,
    BlackCap = 6,
}

impl Piece {
    pub fn from_index(index: u32) -> Self {
        match index {
            1 => Piece::WhiteFlat,
            2 => Piece::WhiteWall,
            3 => Piece::WhiteCap,
            4 => Piece::BlackFlat,
            5 => Piece::BlackWall,
            6 => Piece::BlackCap,
            _ => unimplemented!(),
        }
    }
    pub fn owner(self) -> Color {
        match self {
            Piece::WhiteFlat | Piece::WhiteWall | Piece::WhiteCap => Color::White,
            Piece::BlackFlat | Piece::BlackWall | Piece::BlackCap => Color::Black,
        }
    }
    pub fn kind_index(self) -> usize {
        (self as usize - 1) % 3
    }
    pub fn is_flat(self) -> bool {
        match self {
            Piece::WhiteFlat | Piece::BlackFlat => true,
            _ => false,
        }
    }
    pub fn is_wall(self) -> bool {
        match self {
            Piece::WhiteWall | Piece::BlackWall => true,
            _ => false,
        }
    }
    pub fn is_cap(self) -> bool {
        match self {
            Piece::WhiteCap | Piece::BlackCap => true,
            _ => false,
        }
    }
    pub fn is_blocker(self) -> bool {
        match self {
            Piece::WhiteFlat | Piece::BlackFlat => false,
            _ => true,
        }
    }
    pub fn wall(color: Color) -> Self {
        match color {
            Color::White => Piece::WhiteWall,
            Color::Black => Piece::BlackWall,
        }
    }
    pub fn flat(color: Color) -> Self {
        match color {
            Color::White => Piece::WhiteFlat,
            Color::Black => Piece::BlackFlat,
        }
    }
    pub fn cap(color: Color) -> Self {
        match color {
            Color::White => Piece::WhiteCap,
            Color::Black => Piece::BlackCap,
        }
    }
    pub fn crush(self) -> Option<Piece> {
        match self {
            Piece::WhiteWall => Some(Piece::WhiteFlat),
            Piece::BlackWall => Some(Piece::BlackFlat),
            _ => None,
        }
    }
    pub fn uncrush(self) -> Option<Piece> {
        match self {
            Piece::WhiteFlat => Some(Piece::WhiteWall),
            Piece::BlackFlat => Some(Piece::BlackWall),
            _ => None,
        }
    }
    pub fn swap_color(self) -> Self {
        match self {
            Piece::WhiteFlat => Piece::BlackFlat,
            Piece::BlackFlat => Piece::WhiteFlat,
            Piece::WhiteCap => Piece::BlackCap,
            Piece::BlackCap => Piece::WhiteCap,
            Piece::WhiteWall => Piece::BlackWall,
            Piece::BlackWall => Piece::WhiteWall,
        }
    }
    pub fn road_piece(self, color: Color) -> bool {
        if let Color::White = color {
            match self {
                Piece::WhiteFlat | Piece::WhiteCap => true,
                _ => false,
            }
        } else {
            match self {
                Piece::BlackFlat | Piece::BlackCap => true,
                _ => false,
            }
        }
    }
}

impl std::fmt::Debug for Piece {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let s = match self {
            Piece::WhiteFlat => "w",
            Piece::BlackFlat => "b",
            Piece::WhiteCap => "C",
            Piece::BlackCap => "D",
            Piece::WhiteWall => "S",
            Piece::BlackWall => "T",
        };
        write!(f, "{}", s)
    }
}

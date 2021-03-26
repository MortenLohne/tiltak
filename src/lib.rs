//! A library implementing the rules for Tak, including a fairly strong AI.
//!
//! # Examples
//!
//! Generate legal moves for the start position
//!
//! ```
//! use tiltak::position::Position;
//! use board_game_traits::Position as PositionTrait;
//!
//! let board = <Position<5>>::start_position();
//! let mut moves = vec![];
//! board.generate_moves(&mut moves);
//! assert_eq!(moves.len(), 25);
//! ```
//!
//! Run Monte Carlo Tree Search for the start position
//!
//! ```rust,no_run
//! use tiltak::position::Position;
//! use tiltak::mcts;
//! use pgn_traits::PgnPosition;
//!
//! let board = <Position<5>>::default();
//! let (best_move, score) = mcts(board.clone(), 100_000);
//! println!("Played {} with score {}", board.move_to_san(&best_move), score);
//! ```

extern crate arrayvec;
extern crate board_game_traits;
extern crate pgn_traits;

pub use search::mcts;

#[cfg(any(feature = "aws-lambda-runtime", feature = "aws-lambda-client"))]
pub mod aws;
pub mod minmax;
pub mod move_gen;
pub mod position;
pub mod search;
#[cfg(test)]
mod tests;
#[cfg(feature = "constant-tuning")]
pub mod tune;

pub mod evaluation;
pub mod ptn;

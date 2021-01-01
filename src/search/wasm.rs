#![allow(non_snake_case)]

use crate::board::{Board, Move};
use crate::search::MonteCarloTree;
use board_game_traits::board::{Board as BoardTrait, Color};
use pgn_traits::pgn::PgnBoard;
use std::str::FromStr;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl MonteCarloTree {
    /// Run `n` iterations of MCTS. Subsequent calls to this function will continue the same search, not restart it.
    pub fn doSearchIterations(&mut self, n: u32) {
        for _ in 0..n {
            self.select();
        }
    }

    /// Returns the best move in the position, as determined by the search so far.
    ///
    /// Returns `undefined` if no calls to `doSearchIterations` have been done, or if the game is already decided.
    pub fn bestMove(&self) -> Option<String> {
        if self.visits() < 2 || self.board.game_result().is_some() {
            None
        } else {
            Some(self.best_move().0.to_string())
        }
    }

    /// Returns the score of the position, as determined by the search so far.
    /// The score is represented as winning probability from the side to move's perspective.
    /// For example, 1.0 when you have Tinuë, and 0.5 for a roughly equal position.
    ///
    /// Returns `undefined` if no calls to `doSearchIterations` have been done, or if the game is already decided.
    pub fn score(&self) -> Option<f32> {
        if self.visits() < 2 || self.board.game_result().is_some() {
            None
        } else {
            Some(self.best_move().1)
        }
    }
}
#[wasm_bindgen]
impl Board {
    pub fn doMove(&mut self, move_string: &str) -> JsValue {
        let reverse_move = self.do_move(Move::from_str(move_string).unwrap());
        JsValue::from_serde(&reverse_move).unwrap()
    }

    pub fn reverseMove(&mut self, reverse_move_string: JsValue) {
        let reverse_move = JsValue::into_serde(&reverse_move_string).unwrap();
        self.reverse_move(reverse_move);
    }

    pub fn startBoard() -> Self {
        Self::start_board()
    }

    pub fn legalMoves(&self) -> js_sys::Array {
        let mut moves = vec![];
        self.generate_moves(&mut moves);
        moves
            .into_iter()
            .map(|mv| JsValue::from_str(&mv.to_string()))
            .collect()
    }

    pub fn sideToMove(&self) -> u32 {
        match self.side_to_move() {
            Color::White => 1,
            Color::Black => 2,
        }
    }

    pub fn gameIsOver(&self) -> bool {
        self.game_result().is_some()
    }

    pub fn fromTps(tps_string: &str) -> Result<Board, JsValue> {
        Self::from_fen(tps_string).map_err(|err| JsValue::from_serde(&err.to_string()).unwrap())
    }
}

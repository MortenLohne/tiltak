use crate::board::{Board, Move};
use crate::search::MonteCarloTree;
use board_game_traits::board::{Board as BoardTrait, Color};
use pgn_traits::pgn::PgnBoard;
use std::str::FromStr;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl MonteCarloTree {
    pub fn search_iteration(&mut self) {
        self.select();
    }

    pub fn best_move2(&self) -> Option<String> {
        if self.visits() < 2 {
            None
        } else {
            Some(self.best_move().0.to_string())
        }
    }
}
#[wasm_bindgen]
impl Board {
    pub fn do_move2(&mut self, move_string: &str) -> JsValue {
        let reverse_move = self.do_move(Move::from_str(move_string).unwrap());
        JsValue::from_serde(&reverse_move).unwrap()
    }

    pub fn reverse_move2(&mut self, reverse_move_string: JsValue) {
        let reverse_move = JsValue::into_serde(&reverse_move_string).unwrap();
        self.reverse_move(reverse_move);
    }

    pub fn start_board2() -> Self {
        Self::start_board()
    }

    pub fn legal_moves(&self) -> js_sys::Array {
        let mut moves = vec![];
        self.generate_moves(&mut moves);
        moves
            .into_iter()
            .map(|mv| JsValue::from_str(&mv.to_string()))
            .collect()
    }

    pub fn side_to_move2(&self) -> u32 {
        match self.side_to_move() {
            Color::White => 1,
            Color::Black => 2,
        }
    }

    pub fn game_is_over(&self) -> bool {
        self.game_result().is_some()
    }

    pub fn from_tps(tps_string: &str) -> Result<Board, JsValue> {
        Self::from_fen(tps_string).map_err(|err| JsValue::from_serde(&err.to_string()).unwrap())
    }
}

use crate::board::Board;
use crate::search::MonteCarloTree;
use board_game_traits::board::Board as BoardTrait;
use pgn_traits::pgn::PgnBoard;
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
impl Board {
    pub fn do_move2(&mut self, move_string: String) -> JsValue {
        let dummy_board = Board::start_board();
        let reverse_move = self.do_move(dummy_board.move_from_san(&move_string).unwrap());
        JsValue::from_serde(&reverse_move).unwrap()
    }
}

use std::convert::TryFrom;

use crate::position::{Komi, Move, Position, Role};
use crate::tests::moves_sorted_by_policy;

#[test]
fn place_to_win_no_komi() {
    let position: Position<6> =
    Position::from_fen_with_komi(
        "x2,2,2,1,2S/1S,1,x,2,111112S,1/1,2,x,1211112C,111122221C,x/x,12,1,x2,1/2121212,1,2,2,1,1/2S,x2,2,2,1 1 41", 
        Komi::try_from(0.5f64).unwrap()
    ).unwrap();
    let policy_moves = moves_sorted_by_policy(&position);
    assert!(matches!(&policy_moves[0].0, Move::Place(Role::Flat, _)));
}

#[test]
fn do_not_place_into_komi_loss() {
    let position: Position<6> =
    Position::from_fen_with_komi(
        "x2,2,2,1,2S/1S,1,x,2,111112S,1/1,2,x,1211112C,111122221C,x/x,12,1,x2,1/2121212,1,2,2,1,1/2S,x2,2,2,1 1 41", 
        Komi::try_from(1.5f64).unwrap()
    ).unwrap();
    let policy_moves = moves_sorted_by_policy(&position);
    assert!(matches!(&policy_moves[0].0, Move::Move(_, _, _)));
}

#[test]
fn place_into_komi_win() {
    let position: Position<6> =
    Position::from_fen_with_komi(
        "x2,2,21,x,2S/1S,1,x,2,111112S,1/1,2,x,1211112C,111122221C,x/x,12,1,x2,1/2121212,1,2,2,1,1/2S,x2,2,2,1 2 42", 
        Komi::try_from(1.5f64).unwrap()
    ).unwrap();
    let policy_moves = moves_sorted_by_policy(&position);
    assert!(matches!(&policy_moves[0].0, Move::Place(Role::Flat, _)));
}
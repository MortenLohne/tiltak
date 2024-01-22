use std::convert::TryFrom;

use crate::position::{ExpMove, Komi, Position, Role};
use crate::tests::moves_sorted_by_policy;

#[test]
fn place_to_win_no_komi() {
    let position: Position<6> =
    Position::from_fen_with_komi(
        "x2,2,2,1,2S/1S,1,x,2,111112S,1/1,2,x,1211112C,111122221C,x/x,12,1,x2,1/2121212,1,2,2,1,1/2S,x2,2,2,1 1 41", 
        Komi::try_from(0.5f64).unwrap()
    ).unwrap();
    let policy_moves = moves_sorted_by_policy(&position, Komi::from_half_komi(0).unwrap());
    assert!(matches!(
        &policy_moves[0].0.expand(),
        ExpMove::Place(Role::Flat, _)
    ));
}

#[test]
fn do_not_place_into_komi_loss() {
    let position: Position<6> =
    Position::from_fen_with_komi(
        "x2,2,2,1,2S/1S,1,x,2,111112S,1/1,2,x,1211112C,111122221C,x/x,12,1,x2,1/2121212,1,2,2,1,1/2S,x2,2,2,1 1 41", 
        Komi::try_from(1.5f64).unwrap()
    ).unwrap();
    let policy_moves = moves_sorted_by_policy(&position, Komi::from_half_komi(4).unwrap());
    assert!(matches!(
        &policy_moves[0].0.expand(),
        ExpMove::Move(_, _, _)
    ));
}

#[test]
fn place_into_komi_win() {
    let position: Position<6> =
    Position::from_fen_with_komi(
        "2,x,21,11,x,2221S/1,2121,x,112S,12,2/1,x,2S,21,1112C,12S/1,1,21S,1,21C,2/2,1112S,2,21,21,2/2,2,1,121S,2S,2212 2 49", 
        Komi::try_from(1.5f64).unwrap()
    ).unwrap();
    let policy_moves = moves_sorted_by_policy(&position, Komi::from_half_komi(4).unwrap());
    let (top_move, top_score) = &policy_moves[0];
    assert!(
        matches!(top_move.expand(), ExpMove::Place(Role::Flat, _)),
        "Got move {} with score {:.2}%, expected flat placement",
        top_move,
        top_score.to_f32() * 100.0
    );
}

#[test]
fn do_not_place_to_allow_win() {
    let position: Position<6> =
    Position::from_fen_with_komi(
        "2,x,21,11,221S,2/1,2121,x,112S,12,2/1,x,2S,x,1112C,12S/1,1,21S,11,21C,x/2,1112S,2,21,21,2/2,2,1,121S,2S,2212 2 47", 
        Komi::try_from(0.5f64).unwrap()
    ).unwrap();
    let policy_moves = moves_sorted_by_policy(&position, Komi::from_half_komi(0).unwrap());
    let (top_move, top_score) = &policy_moves[0];
    assert!(
        matches!(top_move.expand(), ExpMove::Move(_, _, _)),
        "Got move {} with score {:.2}%, expected stack movement",
        top_move,
        top_score.to_f32() * 100.0
    );
}

#[test]
fn place_to_allow_komi_loss() {
    let position: Position<6> =
    Position::from_fen_with_komi(
        "2,x,21,11,221S,2/1,2121,x,112S,12,2/1,x,2S,x,1112C,12S/1,1,21S,11,21C,x/2,1112S,2,21,21,2/2,2,1,121S,2S,2212 2 47", 
        Komi::try_from(1.5f64).unwrap()
    ).unwrap();
    let policy_moves = moves_sorted_by_policy(&position, Komi::from_half_komi(4).unwrap());
    let (top_move, top_score) = &policy_moves[0];
    assert!(
        matches!(top_move.expand(), ExpMove::Place(Role::Flat, _)),
        "Got move {} with score {:.2}%, expected flat placement",
        top_move,
        top_score.to_f32() * 100.0
    );
}

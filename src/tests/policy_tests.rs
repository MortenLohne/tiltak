use crate::evaluation::parameters;
use crate::position::{Move, Position, TunableBoard};
use pgn_traits::PgnPosition as PgnPositionTrait;

fn correct_top_policy_move_property<const S: usize>(fen: &str, correct_move_strings: &[&str]) {
    let position: Position<S> = Position::from_fen(fen).unwrap();
    let moves: Vec<Move> = correct_move_strings
        .iter()
        .map(|move_string| position.move_from_san(move_string).unwrap())
        .collect();

    let mut simple_moves = vec![];
    let mut legal_moves = vec![];
    let group_data = position.group_data();
    position.generate_moves_with_probabilities(
        &group_data,
        &mut simple_moves,
        &mut legal_moves,
        &mut vec![0.0; parameters::num_policy_features::<S>()],
    );

    for mv in &moves {
        assert!(legal_moves.iter().any(|(legal_move, _)| legal_move == mv));
    }

    let (highest_policy_move, score) = legal_moves
        .iter()
        .max_by_key(|(_mv, score)| (score * 1000.0) as i64)
        .unwrap();

    assert!(
        moves.contains(highest_policy_move),
        "Expected {:?}, got {:?} with score {:.3}",
        correct_move_strings,
        highest_policy_move.to_string::<S>(),
        score
    );
}

#[test]
fn pure_spread_into_road() {
    let fen = "2,2,2,1S,22,1112/x2,1,2,22,21112C/1,1,1,2,21,11121C/x,1,x,2,x2/x,1,x3,2/x6 1 30";
    correct_top_policy_move_property::<6>(fen, &["4f4<112", "4f4<13", "4f4<22", "3f4<12"]);
}

#[test]
fn two_pure_spreads_into_road() {
    let fen = "x,2,2S,21,1,1/2,212,12,12111122C,x,1/21,x,1,211,x,1S/1,2,1C,x2,1/x2,1,2,11212,2/x2,1,2,x,212 2 36";
    correct_top_policy_move_property::<6>(fen, &["6d5-51", "6d5>51"]);
}

#[test]
fn imperfect_cap_spread_onto_critical_square() {
    let fen = "2,x4/2,1,x,1,2S/1112,1,x,1S,x/2,121121C,x,1,x/1,x,112C,x,111212S 2 24";
    correct_top_policy_move_property::<5>(fen, &["3c1<12", "3c1<21", "2c1<11"]);
}

#[test]
fn simple_flat_movement_onto_critical_square() {
    let fen = "1,1,2,x2/1,1,2,x2/x,1,2,x2/1,2,1,1,x/x,2,2,x,2 2 8";
    correct_top_policy_move_property::<5>(fen, &["b2>", "c1+"]);
}

#[test]
fn pure_spread_onto_critical_square() {
    let fen =
        "x4,11212,221C/x2,2,2,1212,1S/2,2,2,1111,2122C,2S/21,x4,1/x,1112121S,x,1,1,1/2,x4,1 2 36";
    correct_top_policy_move_property::<6>(fen, &["e4>"]);
}

#[test]
fn pure_spread_without_critical_square() {
    let fen = "2,1,1,1,1,2S/1,12,1,x,1C,11112121/x,2,2,212,2C,11/2,21122,x2,1,x/x3,1,1,x/x2,2,21,x,112S 2 34";
    correct_top_policy_move_property::<6>(fen, &["4b3>1111"]);
}

#[test]
fn winning_wall_spread_from_critical_square() {
    let tps =
        "2,2,x,2121S,x,1/x,2,12,221,2,x/x,2,2,221C,12C,12/x,2,x,121,x,1/x,2,x,1,1,1/x3,1,x,1 1 25";
    correct_top_policy_move_property::<6>(tps, &["2d6<", "2d6>", "2d6<11"]);
}

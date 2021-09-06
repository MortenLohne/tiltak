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
        &mut vec![0.0; parameters::num_policy_params::<S>()],
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

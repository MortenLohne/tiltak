//! Test that we don't make one-move blunders on very low node counts

use crate::position::Position;
use crate::search;
use board_game_traits::Position as PositionTrait;
use pgn_traits::PgnPosition;

#[test]
fn do_not_blunder_road_win() {
    let position: Position<6> = Position::from_fen("x2,2,1,x2/x,2,22,21S,2,2/x,22221C,2,221111211112C,1221S,x/2,2,2S,1S,1,1/x,2,1,2,1,12/1,2,1,21,221,2 2 44").unwrap();

    do_not_blunder_property(
        position,
        &["f1<", "d2>", "2f2<", "b1<", "2f2+", "d2-", "b1>", "f2<"],
    )
}

fn do_not_blunder_property<const S: usize>(position: Position<S>, correct_moves: &[&str]) {
    let mut moves = vec![];
    position.generate_moves(&mut moves);

    for move_string in correct_moves {
        assert_eq!(
            *move_string,
            position.move_to_san(&position.move_from_san(move_string).unwrap())
        );
        assert!(
            moves.contains(&position.move_from_san(move_string).unwrap()),
            "Candidate move {} was not among legal moves {:?} in position\n{:?}",
            move_string,
            moves,
            position
        );
    }
    let (best_move, score) = search::mcts(position.clone(), 10_000);

    assert!(
        correct_moves
            .iter()
            .any(|move_string| move_string == &position.move_to_san(&best_move)),
        "{} didn't play one of the correct moves {:?}, {} {:.1}% played instead in position:\n{:?}",
        position.side_to_move(),
        correct_moves,
        position.move_to_san(&best_move),
        score * 100.0,
        position
    );
}

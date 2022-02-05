mod blunder_tests;
mod board_generic_tests;
mod board_tests;
mod mcts_tests;
mod move_gen_5s_tests;
mod move_gen_generic_tests;
mod policy_tests;
mod ptn_tests;
mod tactics_tests_5s;
mod tactics_tests_6s;

use crate::position::{Move, Position};
use crate::search::{self, MctsSetting};
use board_game_traits::Position as PositionTrait;
use pgn_traits::PgnPosition;

fn do_moves_and_check_validity<const S: usize>(position: &mut Position<S>, move_strings: &[&str]) {
    let mut moves = vec![];
    for mv_san in move_strings.iter() {
        let mv = position.move_from_san(mv_san).unwrap();
        position.generate_moves(&mut moves);
        assert!(
            moves.contains(&mv),
            "Move {} was not among legal moves: {:?}\n{:?}",
            position.move_to_san(&mv),
            moves,
            position
        );
        position.do_move(mv);
        moves.clear();
    }
}

fn moves_sorted_by_policy<const S: usize>(position: &Position<S>) -> Vec<(Move, f32)> {
    let mut simple_moves = vec![];
    let mut legal_moves = vec![];
    let group_data = position.group_data();
    position.generate_moves_with_probabilities(
        &group_data,
        &mut simple_moves,
        &mut legal_moves,
        &mut vec![],
        MctsSetting::<S>::default().policy_baseline(),
    );
    legal_moves.sort_by(|(_, score1), (_, score2)| score1.partial_cmp(score2).unwrap().reverse());
    legal_moves
}

fn plays_correct_hard_move_property<const S: usize>(move_strings: &[&str], correct_moves: &[&str]) {
    let mut position = <Position<S>>::default();
    let mut moves = vec![];

    do_moves_and_check_validity(&mut position, move_strings);

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
    let (best_move, score) = search::mcts(position.clone(), 50_000);

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

fn plays_correct_move_easy_tps_property<const S: usize>(tps: &str, correct_moves: &[&str]) {
    let position = <Position<S>>::from_fen(tps).unwrap();
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

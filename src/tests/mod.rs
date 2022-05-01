mod arena_tests;
mod blunder_tests;
mod board_generic_tests;
mod board_tests;
mod komi_policy_tests;
mod mcts_tests;
mod move_gen_5s_tests;
mod move_gen_generic_tests;
mod policy_tests;
mod ptn_tests;
mod tactics_tests_5s;
mod tactics_tests_6s;

use crate::evaluation::parameters::{self, PolicyFeatures};
use crate::position::{Komi, Move, Position};
use crate::search;
use board_game_traits::Position as PositionTrait;
use pgn_traits::PgnPosition;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TestPosition {
    pub tps_string: Option<&'static str>,
    pub move_strings: &'static [&'static str],
    pub komi: Komi,
}

impl TestPosition {
    pub fn from_tps(tps: &'static str) -> Self {
        Self {
            tps_string: Some(tps),
            ..Default::default()
        }
    }

    pub fn from_move_strings(move_strings: &'static [&'static str]) -> Self {
        Self {
            move_strings,
            ..Default::default()
        }
    }

    pub fn position<const S: usize>(&self) -> Position<S> {
        let mut position = self
            .tps_string
            .map(|tps| Position::from_fen_with_komi(tps, self.komi).unwrap())
            .unwrap_or_else(|| Position::start_position_with_komi(self.komi));
        do_moves_and_check_validity(&mut position, &self.move_strings);
        position
    }

    pub fn plays_correct_move_long_prop<const S: usize>(&self, correct_moves: &[&str]) {
        self.plays_correct_move_prop::<S>(correct_moves, 50_000)
    }

    pub fn plays_correct_move_short_prop<const S: usize>(&self, correct_moves: &[&str]) {
        self.plays_correct_move_prop::<S>(correct_moves, 10_000)
    }

    fn plays_correct_move_prop<const S: usize>(&self, correct_moves: &[&str], nodes: u64) {
        let position: Position<S> = self.position();
        let candidate_moves = check_candidate_moves(&position, correct_moves);

        let (best_move, score) = search::mcts(position.clone(), nodes);

        assert!(
            candidate_moves.contains(&best_move),
            "{} didn't play one of the correct moves {:?}, {} {:.1}% played instead in position:\n{:?}",
            position.side_to_move(),
            correct_moves,
            position.move_to_san(&best_move),
            score * 100.0,
            position
        );
    }

    pub fn top_policy_move_prop<const S: usize>(&self, correct_moves: &[&str]) {
        self.top_n_policy_move::<S>(correct_moves, 1)
    }

    pub fn top_five_policy_move_prop<const S: usize>(&self, correct_moves: &[&str]) {
        self.top_n_policy_move::<S>(correct_moves, 5)
    }

    fn top_n_policy_move<const S: usize>(&self, correct_moves: &[&str], n: usize) {
        let position: Position<S> = self.position();
        let candidate_moves = check_candidate_moves(&position, correct_moves);

        let policy_moves = moves_sorted_by_policy(&position);

        assert!(
            policy_moves
                .iter()
                .take(n)
                .any(|(mv, _)| candidate_moves.contains(mv)),
            "Expected one of {:?}, got {:?} instead",
            correct_moves,
            policy_moves
                .iter()
                .take(n)
                .map(|(mv, score)| format!("{}, {:.1}%", mv.to_string::<S>(), score * 100.0))
                .collect::<Vec<_>>(),
        );
    }

    pub fn sets_winning_flag<const S: usize>(&self) -> bool {
        let position = self.position::<S>();

        let group_data = position.group_data();
        let mut moves = vec![];
        position.generate_moves(&mut moves);

        let mut feature_sets = vec![vec![0.0; parameters::num_policy_features::<S>()]; moves.len()];
        let mut policy_feature_sets: Vec<PolicyFeatures> = feature_sets
            .iter_mut()
            .map(|feature_set| PolicyFeatures::new::<S>(feature_set))
            .collect();

        position.features_for_moves(&mut policy_feature_sets, &moves, &group_data);

        policy_feature_sets
            .iter()
            .any(|features| features.decline_win[0] != 0.0)
    }
}

fn check_candidate_moves<const S: usize>(
    position: &Position<S>,
    candidate_move_strings: &[&str],
) -> Vec<Move> {
    let mut legal_moves = vec![];
    position.generate_moves(&mut legal_moves);
    candidate_move_strings
        .iter()
        .map(|candidate_move_string| {
            let mv = position.move_from_san(candidate_move_string).unwrap();
            assert!(
                legal_moves.contains(&mv),
                "Candidate move {} was not among legal moves {:?} in position\n{:?}",
                mv.to_string::<S>(),
                legal_moves
                    .iter()
                    .map(|mv| mv.to_string::<S>())
                    .collect::<Vec<_>>(),
                position
            );
            mv
        })
        .collect()
}

fn do_moves_and_check_validity<const S: usize>(position: &mut Position<S>, move_strings: &[&str]) {
    let mut moves = vec![];
    for mv_san in move_strings.iter() {
        let mv = position.move_from_san(mv_san).unwrap();
        position.generate_moves(&mut moves);
        assert!(
            moves.contains(&mv),
            "Move {} was not among legal moves: {:?}\n{:?}",
            position.move_to_san(&mv),
            moves
                .iter()
                .map(|mv| mv.to_string::<S>())
                .collect::<Vec<_>>(),
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
            moves
                .iter()
                .map(|mv| mv.to_string::<S>())
                .collect::<Vec<_>>(),
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
            moves
                .iter()
                .map(|mv| mv.to_string::<S>())
                .collect::<Vec<_>>(),
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

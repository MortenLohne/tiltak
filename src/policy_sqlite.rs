use std::{
    collections::HashMap,
    sync::{atomic::AtomicUsize, Mutex},
};

use board_game_traits::{Color, GameResult, Position as BoardTrait};
use pgn_traits::PgnPosition;
use rayon::prelude::*;
use rusqlite::Connection;

use crate::{
    evaluation::parameters::{IncrementalPolicy, PolicyApplier},
    position::{Komi, Move, Position},
};

#[derive(Debug, Clone)]
struct Game<const S: usize> {
    komi: Komi,
    moves: Vec<Move<S>>,
}

impl<const S: usize> Game<S> {
    fn analysis_positions(&self) -> Vec<AnalysisPosition<S>> {
        let mut position = Position::start_position_with_komi(self.komi);
        let mut result = vec![];
        'game_loop: for mv in self.moves.iter() {
            position.do_move(*mv);
            if position.game_result().is_some() {
                break;
            }
            let mut moves = vec![];
            position.generate_moves(&mut moves);

            for legal_moves in moves {
                let reverse_move = position.do_move(legal_moves);
                let game_result = position.game_result();
                let ptn_game_result = position.pgn_game_result();
                position.reverse_move(reverse_move);
                if position.side_to_move() == Color::White
                    && game_result == Some(GameResult::WhiteWin)
                    || position.side_to_move() == Color::Black
                        && game_result == Some(GameResult::BlackWin)
                {
                    result.push(AnalysisPosition {
                        position: position.clone(),
                        solution_result: ptn_game_result.map(|s| s.to_string()),
                    });
                    continue 'game_loop;
                }
            }
            {
                result.push(AnalysisPosition {
                    position: position.clone(),
                    solution_result: None,
                })
            }
        }
        result
    }
}

#[derive(Debug, Clone)]
struct AnalysisPosition<const S: usize> {
    position: Position<S>,
    solution_result: Option<String>,
}

fn policy_finds_win(position: &Position<5>) -> Option<Move<5>> {
    let mut simple_moves = vec![];
    let mut moves = vec![];
    let mut fcd_per_move = vec![];

    position.generate_moves_with_probabilities::<IncrementalPolicy<5>>(
        &position.group_data(),
        &mut simple_moves,
        &mut moves,
        &mut fcd_per_move,
        <Position<5>>::policy_params(position.komi()),
        &mut vec![],
    );

    let parameters = <Position<5>>::policy_params(position.komi());

    let mut policies: Vec<IncrementalPolicy<5>> =
        vec![IncrementalPolicy::new(parameters); moves.len()];

    let simple_moves: Vec<Move<5>> = moves.iter().map(|(mv, _)| *mv).collect();

    position.features_for_moves(
        &mut policies,
        &simple_moves,
        &mut fcd_per_move,
        &position.group_data(),
    );
    if simple_moves
        .iter()
        .zip(policies.iter())
        .any(|(_, score)| score.has_immediate_win())
    {
        simple_moves
            .iter()
            .zip(policies)
            .find(|(_, score)| score.has_immediate_win())
            .map(|(mv, _)| *mv)
    } else {
        None
    }
}

pub fn check_all_games() {
    let games = read_all_games();
    println!("Read {} games from database", games.len());
    let mut true_positives = AtomicUsize::new(0);
    let false_positives = Mutex::new(vec![]);
    let mut true_negative = AtomicUsize::new(0);
    let false_negatives = Mutex::new(vec![]);
    let wrong_groups = Mutex::new(HashMap::new());
    games.par_iter().for_each(|game| {
        for win_position in game.clone().analysis_positions() {
            match (
                policy_finds_win(&win_position.position),
                win_position.solution_result.as_ref(),
            ) {
                (Some(_), Some(_)) => {
                    true_positives.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                (Some(mv), None) => false_positives
                    .lock()
                    .unwrap()
                    .push((win_position.clone(), mv)),
                (None, Some(solution_result)) => {
                    *wrong_groups
                        .lock()
                        .unwrap()
                        .entry(solution_result.clone())
                        .or_insert(0) += 1;
                    false_negatives.lock().unwrap().push(win_position.clone());
                }
                (None, None) => {
                    true_negative.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            }
        }
    });

    let false_positives = false_positives.into_inner().unwrap();
    let false_negatives = false_negatives.into_inner().unwrap();
    let wrong_groups = wrong_groups.into_inner().unwrap();

    println!("\nFalse negatives: ");
    for wrongg in false_negatives.iter() {
        println!(
            "{}: {}",
            wrongg.solution_result.as_ref().unwrap(),
            wrongg.position.to_fen()
        );
    }
    println!("\nFalse positives: ");
    for wrongg in false_positives.iter() {
        println!(
            "{}, {} komi, {}",
            wrongg.0.position.to_fen(),
            wrongg.0.position.komi(),
            wrongg.1
        );
    }
    println!(
        "Analyzed {} games, got {} true positives, {} false positives and {} false negatives",
        *true_positives.get_mut()
            + false_positives.len()
            + *true_negative.get_mut()
            + false_negatives.len(),
        true_positives.get_mut(),
        false_positives.len(),
        false_negatives.len()
    );
    for (result, n) in wrong_groups {
        println!("Got {} wrong {} times", result, n);
    }
}

fn read_all_games() -> Vec<Game<5>> {
    let conn = Connection::open("puzzles.db").unwrap();

    let mut stmt = conn
        .prepare(
            "SELECT notation, komi FROM games
        WHERE size = 5",
        )
        .unwrap();
    let rows = stmt.query([]).unwrap().mapped(|row| {
        Ok((
            row.get(0).unwrap(),
            Komi::from_half_komi(row.get(1).unwrap()).unwrap(),
        ))
    });
    rows.map(|row: Result<(String, Komi), rusqlite::Error>| {
        let (notation, komi) = row.unwrap();

        let mut position: Position<5> = Position::start_position_with_komi(komi);
        let mut moves = vec![];

        for move_string in notation.split_whitespace() {
            let mv = Move::from_string(move_string).unwrap();
            let mut legal_moves = vec![];
            position.generate_moves(&mut legal_moves);

            assert!(legal_moves.contains(&mv));
            assert!(position.game_result().is_none());

            position.do_move(mv);
            moves.push(mv);
        }
        Game { komi, moves }
    })
    .collect()
}

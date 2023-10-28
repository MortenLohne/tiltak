use std::collections::HashMap;

use board_game_traits::{Color, GameResult, Position as BoardTrait};
use pgn_traits::PgnPosition;
use rusqlite::Connection;

use crate::{
    evaluation::parameters,
    position::{Komi, Move, Position},
};

#[derive(Debug, Clone)]
struct Game {
    komi: Komi,
    moves: Vec<Move>,
}

impl Game {
    fn analysis_positions(&self) -> Vec<AnalysisPosition> {
        let mut position = Position::start_position_with_komi(self.komi);
        let mut result = vec![];
        'game_loop: for mv in self.moves.iter() {
            position.do_move(mv.clone());
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
struct AnalysisPosition {
    position: Position<5>,
    solution_result: Option<String>,
}

fn policy_finds_win(position: &Position<5>) -> bool {
    let mut simple_moves = vec![];
    let mut moves = vec![];
    let mut fcd_per_move = vec![];

    position.generate_moves_with_probabilities(
        &position.group_data(),
        &mut simple_moves,
        &mut moves,
        &mut fcd_per_move,
        &mut vec![],
        &mut Some(vec![]),
    );

    let mut feature_sets = vec![vec![0.0; parameters::num_policy_features::<5>()]; moves.len()];

    let mut policy_feature_sets: Vec<_> = feature_sets
        .iter_mut()
        .map(|feature_set| parameters::PolicyFeatures::new::<5>(feature_set))
        .collect();

    let simple_moves: Vec<Move> = moves.iter().map(|(mv, _)| mv.clone()).collect();

    position.features_for_moves(
        &mut policy_feature_sets,
        &simple_moves,
        &mut fcd_per_move,
        &position.group_data(),
    );
    simple_moves
        .iter()
        .zip(policy_feature_sets)
        .any(|(_, score)| score.decline_win[0] != 0.0)
}

pub fn check_all_games() {
    let games = read_all_games();
    println!("Read {} games from database", games.len());
    let mut true_positives = 0;
    let mut false_positives = vec![];
    let mut true_negative = 0;
    let mut false_negatives = vec![];
    let mut wrong_groups = HashMap::new();
    for game in games {
        for win_position in game.clone().analysis_positions() {
            match (
                policy_finds_win(&win_position.position),
                win_position.solution_result.as_ref(),
            ) {
                (true, Some(_)) => true_positives += 1,
                (true, None) => false_positives.push(win_position.clone()),
                (false, Some(solution_result)) => {
                    *wrong_groups.entry(solution_result.clone()).or_insert(0) += 1;
                    false_negatives.push(win_position.clone());
                }
                (false, None) => true_negative += 1,
            }
        }
    }
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
        println!("{}", wrongg.position.to_fen());
    }
    println!(
        "Analyzed {} games, got {} true positives, {} false positives and {} false negatives",
        true_positives + false_positives.len() + true_negative + false_negatives.len(),
        true_positives,
        false_positives.len(),
        false_negatives.len()
    );
    for (result, n) in wrong_groups {
        println!("Got {} wrong {} times", result, n);
    }
}

fn read_all_games() -> Vec<Game> {
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
            let mv = Move::from_string::<5>(move_string).unwrap();
            let mut legal_moves = vec![];
            position.generate_moves(&mut legal_moves);

            assert!(legal_moves.contains(&mv));
            assert!(position.game_result().is_none());

            position.do_move(mv.clone());
            moves.push(mv);
        }
        Game { komi, moves }
    })
    .collect()
}

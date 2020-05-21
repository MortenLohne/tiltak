use crate::tune::gradient_descent_policy::gradient_descent_policy;
use crate::tune::play_match::play_game;
use crate::tune::{gradient_descent_value, pgn_parser, play_match};
use board_game_traits::board::Board as BoardTrait;
use board_game_traits::board::GameResult;
use rand::prelude::*;
use rayon::prelude::*;

use pgn_traits::pgn::PgnBoard;
use std::io::Read;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time;
use std::{error, fs, io, iter};
use taik::board::TunableBoard;
use taik::board::{Board, Move};
use taik::pgn_writer::Game;

pub fn train_from_scratch(training_id: usize) -> Result<(), Box<dyn error::Error>> {
    let mut rng = rand::thread_rng();

    let initial_value_params: Vec<f32> = iter::from_fn(|| Some(rng.gen_range(-0.1, 0.1)))
        .take(Board::VALUE_PARAMS.len())
        .collect();
    let initial_policy_params: Vec<f32> = iter::from_fn(|| Some(rng.gen_range(-0.1, 0.1)))
        .take(Board::POLICY_PARAMS.len())
        .collect();

    train_perpetually(training_id, &initial_value_params, &initial_policy_params)
}

pub fn train_perpetually(
    training_id: usize,
    initial_value_params: &[f32],
    initial_policy_params: &[f32],
) -> Result<(), Box<dyn error::Error>> {
    const BATCH_SIZE: usize = 1000;
    // Only train from the last n batches
    const BATCHES_FOR_TRAINING: usize = 20;

    let mut all_games = vec![];
    let mut all_move_scores = vec![];

    let mut last_value_params = initial_value_params.to_vec();
    let mut last_policy_params = initial_policy_params.to_vec();

    let mut value_params = initial_value_params.to_vec();
    let mut policy_params = initial_policy_params.to_vec();

    let mut batch_id = 0;
    let start_time = time::Instant::now();
    let mut playing_time = time::Duration::default();
    let mut tuning_time = time::Duration::default();

    loop {
        let current_params_wins: AtomicU64 = AtomicU64::new(0);
        let last_params_wins: AtomicU64 = AtomicU64::new(0);

        let playing_start_time = time::Instant::now();
        let (games, move_scores): (Vec<_>, Vec<_>) = (0..BATCH_SIZE)
            .into_par_iter()
            .map(|i| {
                play_game_pair(
                    &last_value_params,
                    &last_policy_params,
                    &value_params,
                    &policy_params,
                    &current_params_wins,
                    &last_params_wins,
                    i,
                )
            })
            .unzip();
        playing_time += playing_start_time.elapsed();

        all_move_scores.extend_from_slice(&move_scores[..]);
        all_games.extend_from_slice(&games[..]);

        let file_name = format!("games{}_batch{}.ptn", training_id, batch_id);

        let outfile = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_name)
            .unwrap();

        let mut writer = io::BufWriter::new(outfile);

        for game in games.iter() {
            play_match::game_to_ptn(game, &mut writer)?;
        }

        let games_and_move_scores_outfile = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(format!("move_scores{}_batch{}.ptn", training_id, batch_id))
            .unwrap();

        let mut writer = io::BufWriter::new(games_and_move_scores_outfile);

        for (game, move_scores) in games.iter().zip(move_scores) {
            for (mv, move_scores) in game.moves.iter().map(|(mv, _comment)| mv).zip(move_scores) {
                write!(writer, "{}: ", mv)?;
                for (mv, score) in move_scores {
                    write!(writer, "{} {}, ", mv, score)?;
                }
                writeln!(writer)?;
            }
            writeln!(writer)?;
        }

        let game_stats = GameStats::from_games(&games);

        let wins = current_params_wins.into_inner();
        let losses = last_params_wins.into_inner();
        let draws = BATCH_SIZE as u64 - wins - losses;

        println!("Finished playing batch of {} games. {} games played in total. {} white wins, {} draws, {} black wins, {} aborted. New vs old parameters was +{}-{}={}.",
            games.len(), all_games.len(), game_stats.white_wins, game_stats.draws, game_stats.black_wins, game_stats.aborted, wins, losses, draws
        );

        let games_in_training_batch = all_games
            .iter()
            .cloned()
            .rev()
            .take(BATCH_SIZE * BATCHES_FOR_TRAINING)
            .collect::<Vec<_>>();

        let move_scores_in_training_batch = all_move_scores
            .iter()
            .cloned()
            .rev()
            .take(BATCH_SIZE * BATCHES_FOR_TRAINING)
            .collect::<Vec<_>>();

        let value_tuning_start_time = time::Instant::now();

        let (new_value_params, new_policy_params) = tune_value_and_policy(
            &value_params,
            &policy_params,
            &move_scores_in_training_batch,
            &games_in_training_batch,
        )?;

        last_value_params = value_params;
        last_policy_params = policy_params;

        value_params = new_value_params;
        policy_params = new_policy_params;

        tuning_time += value_tuning_start_time.elapsed();

        batch_id += 1;
        println!(
            "{}s elapsed. Time use breakdown: {:.2}% playing games, {:.2}% tuning parameters.",
            start_time.elapsed().as_secs(),
            100.0 * playing_time.as_secs_f64() / start_time.elapsed().as_secs_f64(),
            100.0 * tuning_time.as_secs_f64() / start_time.elapsed().as_secs_f64()
        );
    }
}

fn play_game_pair(
    last_value_params: &[f32],
    last_policy_params: &[f32],
    value_params: &[f32],
    policy_params: &[f32],
    current_params_wins: &AtomicU64,
    last_params_wins: &AtomicU64,
    i: usize,
) -> (Game<Board>, Vec<Vec<(Move, f32)>>) {
    if i % 2 == 0 {
        let game = play_game(
            &value_params,
            &policy_params,
            &last_value_params,
            &last_policy_params,
        );
        match game.0.game_result {
            Some(GameResult::WhiteWin) => {
                current_params_wins.fetch_add(1, Ordering::Relaxed);
            }
            Some(GameResult::BlackWin) => {
                last_params_wins.fetch_add(1, Ordering::Relaxed);
            }
            Some(GameResult::Draw) | None => (),
        };
        game
    } else {
        let game = play_game(
            &last_value_params,
            &last_policy_params,
            &value_params,
            &policy_params,
        );
        match game.0.game_result {
            Some(GameResult::BlackWin) => {
                current_params_wins.fetch_add(1, Ordering::Relaxed);
            }
            Some(GameResult::WhiteWin) => {
                last_params_wins.fetch_add(1, Ordering::Relaxed);
            }
            Some(GameResult::Draw) | None => (),
        };
        game
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct GameStats {
    pub white_wins: u64,
    pub draws: u64,
    pub black_wins: u64,
    pub aborted: u64,
}

impl GameStats {
    pub fn from_games(games: &[Game<Board>]) -> Self {
        let mut stats = GameStats::default();
        for game in games {
            match game.game_result {
                Some(GameResult::WhiteWin) => stats.white_wins += 1,
                Some(GameResult::BlackWin) => stats.black_wins += 1,
                Some(GameResult::Draw) => stats.draws += 1,
                None => stats.aborted += 1,
            }
        }
        stats
    }
}

pub fn read_games_from_file() -> Result<Vec<Game<Board>>, Box<dyn error::Error>> {
    let mut file = fs::File::open("games23_all.ptn")?;
    let mut input = String::new();
    file.read_to_string(&mut input)?;
    pgn_parser::parse_pgn(&input)
}

pub fn tune_from_file() -> Result<(), Box<dyn error::Error>> {
    let games = read_games_from_file()?;

    let (positions, results) = positions_and_results_from_games(games);

    let middle_index = positions.len() / 2;

    let mut rng = rand::thread_rng();
    let params: Vec<f32> = vec![rng.gen_range(-0.1, 0.1); Board::VALUE_PARAMS.len()];

    println!(
        "Final parameters: {:?}",
        gradient_descent_value::gradient_descent(
            &positions[0..middle_index],
            &results[0..middle_index],
            &positions[middle_index..],
            &results[middle_index..],
            &params,
        )
    );

    Ok(())
}

pub fn tune_value_and_policy_from_file() -> Result<(Vec<f32>, Vec<f32>), Box<dyn error::Error>> {
    let move_scoress = read_move_scores_from_file()?;
    let games = read_games_from_file()?;

    for (game, move_scores) in games.iter().zip(&move_scoress) {
        let mut board = game.start_board.clone();
        for (mv, move_score) in game.moves.iter().map(|(mv, _)| mv).zip(move_scores) {
            assert!(
                move_score
                    .iter()
                    .any(|(scored_move, _score)| *mv == *scored_move),
                "Played move {} not among move scores {:?}\nBoard:\n{:?}",
                mv,
                move_score,
                board
            );
            board.do_move(mv.clone());
        }
    }

    tune_value_and_policy(
        Board::VALUE_PARAMS,
        Board::POLICY_PARAMS,
        &move_scoress,
        &games,
    )
}

pub fn tune_value_and_policy(
    value_params: &[f32],
    policy_params: &[f32],
    move_scores: &[Vec<Vec<(Move, f32)>>],
    games: &[Game<Board>],
) -> Result<(Vec<f32>, Vec<f32>), Box<dyn error::Error>> {
    let mut games_and_move_scores: Vec<(Game<Board>, Vec<Vec<(Move, f32)>>)> = games
        .iter()
        .cloned()
        .zip(move_scores.iter().cloned())
        .collect::<Vec<_>>();

    let mut rng = rand::thread_rng();
    games_and_move_scores.shuffle(&mut rng);

    let middle_index = games_and_move_scores.len() / 2;

    let (training_games, training_move_scores): (Vec<_>, Vec<_>) = games_and_move_scores
        .iter()
        .take(middle_index)
        .cloned()
        .unzip();

    let (test_games, test_move_scores): (Vec<_>, Vec<_>) = games_and_move_scores
        .iter()
        .skip(middle_index)
        .cloned()
        .unzip();

    let (training_positions, training_results) = positions_and_results_from_games(training_games);

    let (test_positions, test_results) = positions_and_results_from_games(test_games);

    let value_params = gradient_descent_value::gradient_descent(
        &training_positions,
        &training_results,
        &test_positions,
        &test_results,
        &value_params,
    );

    let flat_training_move_scores = training_move_scores
        .iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();

    let flat_test_move_scores = test_move_scores
        .iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();

    let policy_params = gradient_descent_policy(
        &training_positions,
        &flat_training_move_scores,
        &test_positions,
        &flat_test_move_scores,
        &policy_params,
    );
    Ok((value_params, policy_params))
}

pub fn read_move_scores_from_file() -> Result<Vec<Vec<Vec<(Move, f32)>>>, Box<dyn error::Error>> {
    let mut file = fs::File::open("move_scores23_all.txt")?;
    let mut input = String::new();
    file.read_to_string(&mut input)?;

    let board = Board::start_board();

    // Move scores grouped by the game they were played
    let mut move_scoress: Vec<Vec<Vec<(Move, f32)>>> = vec![vec![]];
    for line in input.lines() {
        // Start a new game
        if line.trim().is_empty() {
            move_scoress.push(vec![]);
            continue;
        }
        let mut scores_for_this_move = vec![];
        let _played_move = line.split(':').next().unwrap();
        let possible_moves = line.split(':').nth(1).unwrap();
        for move_score_string in possible_moves.split(',') {
            if move_score_string.len() < 3 {
                continue;
            }
            let mut words = move_score_string.split_whitespace();
            let mv = board.move_from_san(words.next().unwrap())?;
            let score = str::parse::<f32>(words.next().unwrap())?;
            scores_for_this_move.push((mv, score));
        }
        move_scoress.last_mut().unwrap().push(scores_for_this_move);
    }
    Ok(move_scoress)
}

pub fn positions_and_results_from_games(games: Vec<Game<Board>>) -> (Vec<Board>, Vec<GameResult>) {
    let mut positions = vec![];
    let mut results = vec![];
    for game in games.into_iter() {
        let mut board = game.start_board;
        for (mv, _) in game.moves {
            positions.push(board.clone());
            results.push(game.game_result.unwrap_or(GameResult::Draw));
            board.do_move(mv);
            // Deliberately skip the final position
        }
    }
    (positions, results)
}

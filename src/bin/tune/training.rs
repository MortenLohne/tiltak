use crate::tune::gradient_descent_policy::gradient_descent_policy;
use crate::tune::pgn_parse::Game;
use crate::tune::play_match::play_game;
use crate::tune::{gradient_descent_value, pgn_parse, play_match};
use board_game_traits::board::Board as BoardTrait;
use board_game_traits::board::GameResult;
use rand::prelude::*;
use rayon::prelude::*;
use std::io::Read;
use std::time;
use std::{error, fs, io, iter};
use taik::board::Board;
use taik::board::TunableBoard;

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

    let mut rng = rand::thread_rng();

    let mut all_games = vec![];
    let mut all_move_scores = vec![];
    let mut value_params = initial_value_params.to_vec();
    let mut policy_params = initial_policy_params.to_vec();

    let mut batch_id = 0;
    let start_time = time::Instant::now();
    let mut playing_time = time::Duration::default();
    let mut value_tuning_time = time::Duration::default();
    let mut policy_tuning_time = time::Duration::default();

    loop {
        let playing_start_time = time::Instant::now();
        let (games, move_scores): (Vec<_>, Vec<_>) = (0..BATCH_SIZE)
            .into_par_iter()
            .map(|_| play_game(&value_params, &policy_params))
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

        let game_stats = GameStats::from_games(&games);

        println!("Finished playing batch of {} games. {} games played in total. {} white wins, {} draws, {} black wins, {} aborted.",
            games.len(), all_games.len(), game_stats.white_wins, game_stats.draws, game_stats.black_wins, game_stats.aborted
        );

        let mut games_and_move_scores = all_games
            .iter()
            .cloned()
            .zip(all_move_scores.iter().cloned())
            .rev()
            .take(BATCH_SIZE * BATCHES_FOR_TRAINING)
            .collect::<Vec<_>>();

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

        let (training_positions, training_results) =
            positions_and_results_from_games(training_games);

        let (test_positions, test_results) = positions_and_results_from_games(test_games);

        let value_tuning_start_time = time::Instant::now();
        value_params = gradient_descent_value::gradient_descent(
            &training_positions,
            &training_results,
            &test_positions,
            &test_results,
            &value_params,
        );
        value_tuning_time += value_tuning_start_time.elapsed();

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

        let policy_tuning_start_time = time::Instant::now();
        policy_params = gradient_descent_policy(
            &training_positions,
            &flat_training_move_scores,
            &test_positions,
            &flat_test_move_scores,
            &policy_params,
        );
        policy_tuning_time += policy_tuning_start_time.elapsed();

        batch_id += 1;
        println!("{}s elapsed. Time use breakdown: {:.2}% playing games, {:.2}% tuning value parameters, {:.2}% tuning policy parameters.",
                 start_time.elapsed().as_secs(),
                 100.0 * playing_time.as_secs_f64() / start_time.elapsed().as_secs_f64(),
                 100.0 * value_tuning_time.as_secs_f64() / start_time.elapsed().as_secs_f64(),
                 100.0 * policy_tuning_time.as_secs_f64() / start_time.elapsed().as_secs_f64());
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

pub fn tune_from_file() -> Result<(), Box<dyn error::Error>> {
    let mut file = fs::File::open("games0_all.ptn")?;
    let mut input = String::new();
    file.read_to_string(&mut input)?;
    let games: Vec<Game<Board>> = pgn_parse::parse_pgn(&input)?;

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

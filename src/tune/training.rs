use crate::tune::play_match::play_game;
use crate::tune::{gradient_descent, play_match};
use board_game_traits::board::Board as BoardTrait;
use board_game_traits::board::GameResult;
use rand::prelude::*;
use rayon::prelude::*;

use crate::board::TunableBoard;
use crate::board::{Board, Move};
use crate::pgn_parser;
use crate::pgn_writer::Game;
use crate::search::MctsSetting;
use pgn_traits::pgn::PgnBoard;
use std::io::Read;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time;
use std::{error, fs, io};

// The score, or probability of being played, for a given move
type MoveScore = (Move, f32);

// The probability of each possible move being played, through a whole game.
type MoveScoresForGame = Vec<Vec<MoveScore>>;

pub fn train_from_scratch(training_id: usize) -> Result<(), Box<dyn error::Error>> {
    let mut rng = rand::thread_rng();

    let initial_value_params: [f32; Board::VALUE_PARAMS.len()] =
        array_from_fn(|| rng.gen_range(-0.01, 0.01));

    let mut initial_policy_params: [f32; Board::POLICY_PARAMS.len()] =
        array_from_fn(|| rng.gen_range(-0.01, 0.01));

    // The move number parameter should always be around 1.0, so start it here
    // If we don't, variation of this parameter completely dominates the other parameters
    initial_policy_params[0] = 1.0;

    train_perpetually(training_id, &initial_value_params, &initial_policy_params)
}

pub fn train_perpetually(
    training_id: usize,
    initial_value_params: &[f32; Board::VALUE_PARAMS.len()],
    initial_policy_params: &[f32; Board::POLICY_PARAMS.len()],
) -> Result<(), Box<dyn error::Error>> {
    const BATCH_SIZE: usize = 100;
    // Only train from the last n batches
    const BATCHES_FOR_TRAINING: usize = 10;

    let mut all_games = vec![];
    let mut all_move_scores = vec![];

    let mut last_value_params = initial_value_params.clone();
    let mut last_policy_params = initial_policy_params.clone();

    let mut value_params = initial_value_params.clone();
    let mut policy_params = initial_policy_params.clone();

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

        // Only take the most recent half of the games, to avoid training on bad, old games
        let max_training_games = all_games.len() / 2;

        let games_in_training_batch = all_games
            .iter()
            .cloned()
            .rev()
            .take(usize::min(
                max_training_games,
                BATCH_SIZE * BATCHES_FOR_TRAINING,
            ))
            .collect::<Vec<_>>();

        let move_scores_in_training_batch = all_move_scores
            .iter()
            .cloned()
            .rev()
            .take(usize::min(
                max_training_games,
                BATCH_SIZE * BATCHES_FOR_TRAINING,
            ))
            .collect::<Vec<_>>();

        let value_tuning_start_time = time::Instant::now();

        let (new_value_params, new_policy_params): (
            [f32; Board::VALUE_PARAMS.len()],
            [f32; Board::POLICY_PARAMS.len()],
        ) = tune_value_and_policy(
            &games_in_training_batch,
            &move_scores_in_training_batch,
            &value_params,
            &policy_params,
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
    let settings = MctsSetting::default()
        .add_value_params(value_params.to_vec())
        .add_policy_params(policy_params.to_vec())
        .add_dirichlet(0.2);
    let last_settings = MctsSetting::default()
        .add_value_params(last_value_params.to_vec())
        .add_policy_params(last_policy_params.to_vec())
        .add_dirichlet(0.2);
    if i % 2 == 0 {
        let game = play_game(&settings, &last_settings, &[], 1.0);
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
        let game = play_game(&last_settings, &settings, &[], 1.0);
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

pub fn read_games_from_file(file_name: &str) -> Result<Vec<Game<Board>>, Box<dyn error::Error>> {
    let mut file = fs::File::open(file_name)?;
    let mut input = String::new();
    file.read_to_string(&mut input)?;
    pgn_parser::parse_pgn(&input)
}

pub fn tune_value_from_file(
    file_name: &str,
) -> Result<[f32; Board::VALUE_PARAMS.len()], Box<dyn error::Error>> {
    let games = read_games_from_file(file_name)?;
    const N: usize = Board::VALUE_PARAMS.len();

    let (positions, results) = positions_and_results_from_games(games);

    let coefficient_sets = positions
        .iter()
        .map(|position| {
            let mut coefficients = [0.0; N];
            position.static_eval_coefficients(&mut coefficients);
            coefficients
        })
        .collect::<Vec<[f32; N]>>();

    let f32_results = results
        .iter()
        .map(|res| match res {
            GameResult::WhiteWin => 1.0,
            GameResult::Draw => 0.5,
            GameResult::BlackWin => 0.0,
        })
        .collect::<Vec<f32>>();

    let middle_index = positions.len() / 2;

    let mut rng = rand::thread_rng();
    let mut initial_params = [0.00; N];

    for param in initial_params.iter_mut() {
        *param = rng.gen_range(-0.01, 0.01)
    }
    let tuned_parameters = gradient_descent::gradient_descent(
        &coefficient_sets[0..middle_index],
        &f32_results[0..middle_index],
        &coefficient_sets[middle_index..],
        &f32_results[middle_index..],
        &initial_params,
        50.0,
    );

    println!("Final parameters: {:?}", tuned_parameters);

    Ok(tuned_parameters)
}

pub fn tune_value_and_policy<const N: usize, const M: usize>(
    games: &[Game<Board>],
    move_scoress: &[MoveScoresForGame],
    initial_value_params: &[f32; N],
    initial_policy_params: &[f32; M],
) -> Result<([f32; N], [f32; M]), Box<dyn error::Error>> {
    let mut games_and_move_scoress: Vec<(&Game<Board>, &MoveScoresForGame)> =
        games.iter().zip(move_scoress).collect();

    let mut rng = rand::rngs::StdRng::from_seed([0; 32]);

    games_and_move_scoress.shuffle(&mut rng);

    let (games, move_scoress): (Vec<_>, Vec<_>) = games_and_move_scoress.into_iter().unzip();

    let (positions, results) =
        positions_and_results_from_games(games.iter().cloned().cloned().collect());

    let value_coefficient_sets = positions
        .iter()
        .map(|position| {
            let mut coefficients = [0.0; N];
            position.static_eval_coefficients(&mut coefficients);
            coefficients
        })
        .collect::<Vec<[f32; N]>>();

    let value_results = results
        .iter()
        .map(|res| match res {
            GameResult::WhiteWin => 1.0,
            GameResult::Draw => 0.5,
            GameResult::BlackWin => 0.0,
        })
        .collect::<Vec<f32>>();

    let number_of_coefficient_sets = move_scoress.iter().flat_map(|a| *a).flatten().count();

    let mut policy_coefficients_sets: Vec<[f32; M]> =
        Vec::with_capacity(number_of_coefficient_sets);
    let mut policy_results: Vec<f32> = Vec::with_capacity(number_of_coefficient_sets);

    for (game, move_scores) in games.iter().zip(move_scoress) {
        let mut board = game.start_board.clone();

        for (mv, move_scores) in game.moves.iter().map(|(mv, _)| mv).zip(move_scores) {
            let group_data = board.group_data();
            for (possible_move, score) in move_scores {
                let mut coefficients = [0.0; M];
                board.coefficients_for_move(
                    &mut coefficients,
                    possible_move,
                    &group_data,
                    move_scores.len(),
                );

                policy_coefficients_sets.push(coefficients);
                policy_results.push(*score);
            }
            board.do_move(mv.clone());
        }
    }

    let middle_index = value_coefficient_sets.len() / 2;

    let tuned_value_parameters = gradient_descent::gradient_descent(
        &value_coefficient_sets[0..middle_index],
        &value_results[0..middle_index],
        &value_coefficient_sets[middle_index..],
        &value_results[middle_index..],
        &initial_value_params,
        10.0,
    );

    println!("Final parameters: {:?}", tuned_value_parameters);

    let middle_index = policy_coefficients_sets.len() / 2;

    let tuned_policy_parameters = gradient_descent::gradient_descent(
        &policy_coefficients_sets[0..middle_index],
        &policy_results[0..middle_index],
        &policy_coefficients_sets[middle_index..],
        &policy_results[middle_index..],
        &initial_policy_params,
        10000.0,
    );

    println!("Final parameters: {:?}", tuned_policy_parameters);

    Ok((tuned_value_parameters, tuned_policy_parameters))
}

pub fn tune_value_and_policy_from_file(
    value_file_name: &str,
    policy_file_name: &str,
) -> Result<
    (
        [f32; Board::VALUE_PARAMS.len()],
        [f32; Board::POLICY_PARAMS.len()],
    ),
    Box<dyn error::Error>,
> {
    let (games, move_scoress) =
        games_and_move_scoress_from_file(value_file_name, policy_file_name)?;

    let mut rng = rand::thread_rng();

    let initial_value_params: [f32; Board::VALUE_PARAMS.len()] =
        array_from_fn(|| rng.gen_range(-0.01, 0.01));

    let mut initial_policy_params: [f32; Board::POLICY_PARAMS.len()] =
        array_from_fn(|| rng.gen_range(-0.01, 0.01));

    // The move number parameter should always be around 1.0, so start it here
    // If we don't, variation of this parameter completely dominates the other parameters
    initial_policy_params[0] = 1.0;
    tune_value_and_policy(
        &games,
        &move_scoress,
        &initial_value_params,
        &initial_policy_params,
    )
}

pub fn games_and_move_scoress_from_file(
    value_file_name: &str,
    policy_file_name: &str,
) -> Result<(Vec<Game<Board>>, Vec<MoveScoresForGame>), Box<dyn error::Error>> {
    let mut move_scoress = read_move_scores_from_file(policy_file_name)?;
    let mut games = read_games_from_file(value_file_name)?;

    // Only keep the last n games, since all the training data doesn't fit in memory while training
    move_scoress.reverse();
    games.reverse();

    move_scoress.truncate(4000);
    games.truncate(4000);

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
    Ok((games, move_scoress))
}

pub fn read_move_scores_from_file(
    file_name: &str,
) -> Result<Vec<MoveScoresForGame>, Box<dyn error::Error>> {
    let mut file = fs::File::open(file_name)?;
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
            if board.game_result().is_some() {
                break;
            }
            positions.push(board.clone());
            results.push(game.game_result.unwrap_or(GameResult::Draw));
            board.do_move(mv);
            // Deliberately skip the final position
        }
    }
    (positions, results)
}

fn array_from_fn<F, T, const N: usize>(mut f: F) -> [T; N]
where
    F: FnMut() -> T,
    T: Default + Copy,
{
    let mut output = [T::default(); N];
    for e in output.iter_mut() {
        *e = f();
    }
    output
}

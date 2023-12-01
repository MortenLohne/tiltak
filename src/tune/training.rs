use std::convert::TryFrom;
use std::io::Read;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time;
use std::{error, fs, io};

use crate::evaluation::parameters::PolicyFeatures;
use crate::position::Komi;
use crate::search::TimeControl;
use board_game_traits::GameResult;
use board_game_traits::Position as PositionTrait;
use pgn_traits::PgnPosition;
use rand::prelude::*;
use rayon::prelude::*;

use crate::evaluation::policy_eval::inverse_sigmoid;
use crate::position::Move;
use crate::position::Position;
use crate::ptn::Game;
use crate::ptn::{ptn_parser, PtnMove};
use crate::search::MctsSetting;
use crate::tune::gradient_descent;
use crate::tune::gradient_descent::TrainingSample;
use crate::tune::play_match::play_game;

// The score, or probability of being played, for a given move
type MoveScore = (Move, f32);

// The probability of each possible move being played, through a whole game.
type MoveScoresForGame = Vec<Vec<MoveScore>>;

pub fn train_from_scratch<const S: usize, const N: usize, const M: usize>(
    training_id: usize,
    komi: Komi,
) -> Result<(), DynError> {
    let mut rng = rand::rngs::StdRng::from_seed([0; 32]);

    let initial_value_params: [f32; N] = array_from_fn(|| rng.gen_range(-0.01..0.01));

    let initial_policy_params: [f32; M] = array_from_fn(|| rng.gen_range(-0.01..0.01));

    train_perpetually::<S, N, M>(
        training_id,
        komi,
        &initial_value_params,
        &initial_policy_params,
        vec![],
        vec![],
        0,
    )
}

pub fn continue_training<const S: usize, const N: usize, const M: usize>(
    training_id: usize,
    komi: Komi,
) -> Result<(), DynError> {
    let mut games = vec![];
    let mut move_scores = vec![];
    let mut batch_id = 0;
    loop {
        match read_games_from_file::<S>(&format!(
            "games{}_{}s_batch{}.ptn",
            training_id, S, batch_id
        )) {
            Ok(mut game_batch) => {
                games.append(&mut game_batch);
            }
            Err(error) => {
                let io_error = error.downcast::<io::Error>()?;
                if io_error.kind() == io::ErrorKind::NotFound {
                    break;
                } else {
                    return Err(io_error);
                }
            }
        }
        let mut move_scores_batch = read_move_scores_from_file::<S>(&format!(
            "move_scores{}_{}s_batch{}.ptn",
            training_id, S, batch_id
        ))?;
        move_scores.append(&mut move_scores_batch);
        batch_id += 1;
    }

    assert_eq!(games.len(), move_scores.len());
    println!(
        "Resumed training with {} games and {} moves",
        games.len(),
        move_scores.iter().map(Vec::len).sum::<usize>()
    );

    let value_params = <Position<S>>::value_params(komi);

    let policy_params = <Position<S>>::policy_params(komi);

    train_perpetually::<S, N, M>(
        training_id,
        komi,
        &<[f32; N]>::try_from(value_params).unwrap(),
        &<[f32; M]>::try_from(policy_params).unwrap(),
        games,
        move_scores,
        batch_id,
    )
}

pub fn train_perpetually<const S: usize, const N: usize, const M: usize>(
    training_id: usize,
    komi: Komi,
    initial_value_params: &[f32; N],
    initial_policy_params: &[f32; M],
    mut all_games: Vec<Game<Position<S>>>,
    mut all_move_scores: Vec<MoveScoresForGame>,
    mut batch_id: usize,
) -> Result<(), DynError> {
    const BATCH_SIZE: usize = 500;
    // Only train from the last n batches
    const BATCHES_FOR_TRAINING: usize = 30;

    let mut last_value_params = *initial_value_params;
    let mut last_policy_params = *initial_policy_params;

    let mut value_params = *initial_value_params;
    let mut policy_params = *initial_policy_params;

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
                play_game_pair::<S>(
                    komi,
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

        let file_name = format!("games{}_{}s_batch{}.ptn", training_id, S, batch_id);

        let outfile = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_name)
            .unwrap();

        let mut writer = io::BufWriter::new(outfile);

        for game in games.iter() {
            game.game_to_ptn(&mut writer)?;
        }

        let games_and_move_scores_outfile = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(format!(
                "move_scores{}_{}s_batch{}.ptn",
                training_id, S, batch_id
            ))
            .unwrap();
        writer.flush()?;

        let mut writer = io::BufWriter::new(games_and_move_scores_outfile);

        for (game, move_scores) in games.iter().zip(move_scores) {
            for (mv, move_scores) in game
                .moves
                .iter()
                .map(|PtnMove { mv, .. }| mv)
                .zip(move_scores)
            {
                write!(writer, "{}: ", mv.to_string::<S>())?;
                for (mv, score) in move_scores {
                    write!(writer, "{} {}, ", mv.to_string::<S>(), score)?;
                }
                writeln!(writer)?;
            }
            writeln!(writer)?;
        }
        writer.flush()?;

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

        let (new_value_params, new_policy_params): ([f32; N], [f32; M]) = tune_value_and_policy(
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

#[allow(clippy::too_many_arguments)]
fn play_game_pair<const S: usize>(
    komi: Komi,
    last_value_params: &[f32],
    last_policy_params: &[f32],
    value_params: &[f32],
    policy_params: &[f32],
    current_params_wins: &AtomicU64,
    last_params_wins: &AtomicU64,
    i: usize,
) -> (Game<Position<S>>, Vec<Vec<(Move, f32)>>) {
    let settings = MctsSetting::default()
        .add_value_params(value_params.into())
        .add_policy_params(policy_params.into())
        .add_dirichlet(0.2);
    let last_settings = MctsSetting::default()
        .add_value_params(last_value_params.into())
        .add_policy_params(last_policy_params.into())
        .add_dirichlet(0.2);
    if i % 2 == 0 {
        let game = play_game::<S>(
            &settings,
            &last_settings,
            komi,
            &[],
            1.0,
            &TimeControl::FixedNodes(100_000),
        );
        match game.0.game_result() {
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
        let game = play_game::<S>(
            &last_settings,
            &settings,
            komi,
            &[],
            1.0,
            &TimeControl::FixedNodes(100_000),
        );
        match game.0.game_result() {
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
    pub fn from_games<const N: usize>(games: &[Game<Position<N>>]) -> Self {
        let mut stats = GameStats::default();
        for game in games {
            match game.game_result() {
                Some(GameResult::WhiteWin) => stats.white_wins += 1,
                Some(GameResult::BlackWin) => stats.black_wins += 1,
                Some(GameResult::Draw) => stats.draws += 1,
                None => stats.aborted += 1,
            }
        }
        stats
    }
}

pub fn read_games_from_file<const S: usize>(
    file_name: &str,
) -> Result<Vec<Game<Position<S>>>, DynError> {
    let mut file = fs::File::open(file_name)?;
    let mut input = String::new();
    file.read_to_string(&mut input)?;
    let games = ptn_parser::parse_ptn(&input)?;
    println!("Read {} games from PTN", games.len());
    Ok(games)
}

pub fn tune_value_from_file<const S: usize, const N: usize>(
    file_name: &str,
) -> Result<[f32; N], DynError> {
    let games = read_games_from_file::<S>(file_name)?;

    let (positions, results) = positions_and_results_from_games(games);

    let samples = positions
        .iter()
        .zip(results)
        .map(|(position, game_result)| {
            let mut features = [0.0; N];
            position.static_eval_features(&mut features);
            let result = match game_result {
                GameResult::WhiteWin => 1.0,
                GameResult::Draw => 0.5,
                GameResult::BlackWin => 0.0,
            };
            TrainingSample {
                features,
                offset: 0.0,
                result,
            }
        })
        .collect::<Vec<_>>();

    let mut rng = rand::rngs::StdRng::from_seed([0; 32]);
    let mut initial_params = [0.00; N];

    for param in initial_params.iter_mut() {
        *param = rng.gen_range(-0.01..0.01)
    }

    let tuned_parameters = gradient_descent::gradient_descent(&samples, &initial_params, 100.0);

    println!("Final parameters: {:?}", tuned_parameters);

    Ok(tuned_parameters)
}

pub fn tune_value_and_policy<const S: usize, const N: usize, const M: usize>(
    games: &[Game<Position<S>>],
    move_scoress: &[MoveScoresForGame],
    initial_value_params: &[f32; N],
    initial_policy_params: &[f32; M],
) -> Result<([f32; N], [f32; M]), DynError> {
    let mut games_and_move_scoress: Vec<(&Game<Position<S>>, &MoveScoresForGame)> =
        games.iter().zip(move_scoress).collect();

    let mut rng = rand::rngs::StdRng::from_seed([0; 32]);

    games_and_move_scoress.shuffle(&mut rng);

    let (games, move_scoress): (Vec<_>, Vec<_>) = games_and_move_scoress.into_iter().unzip();

    let (positions, results) =
        positions_and_results_from_games(games.iter().cloned().cloned().collect());

    let value_training_samples = positions
        .iter()
        .zip(results)
        .map(|(position, game_result)| {
            let mut features = [0.0; N];
            position.static_eval_features(&mut features);
            let result = match game_result {
                GameResult::WhiteWin => 1.0,
                GameResult::Draw => 0.5,
                GameResult::BlackWin => 0.0,
            };
            TrainingSample {
                features,
                offset: 0.0,
                result,
            }
        })
        .collect::<Vec<_>>();

    let number_of_feature_sets = move_scoress.iter().flat_map(|a| *a).flatten().count();

    let mut policy_training_samples = Vec::with_capacity(number_of_feature_sets);

    for (game, move_scores) in games.iter().zip(move_scoress) {
        let mut position = game.start_position.clone();

        for (mv, move_scores) in game
            .moves
            .iter()
            .map(|PtnMove { mv, .. }| mv)
            .zip(move_scores)
        {
            let group_data = position.group_data();

            let mut feature_sets = vec![[0.0; M]; move_scores.len()];
            let mut policy_feature_sets: Vec<PolicyFeatures> = feature_sets
                .iter_mut()
                .map(|feature_set| PolicyFeatures::new::<S>(feature_set))
                .collect();
            let moves: Vec<Move> = move_scores.iter().map(|(mv, _score)| mv.clone()).collect();

            position.features_for_moves(&mut policy_feature_sets, &moves, &mut vec![], &group_data);

            for ((_, result), features) in move_scores.iter().zip(feature_sets) {
                let offset = inverse_sigmoid(1.0 / move_scores.len().max(2) as f32);

                policy_training_samples.push({
                    TrainingSample {
                        features,
                        offset,
                        result: *result,
                    }
                });
            }
            position.do_move(mv.clone());
        }
    }

    let tuned_value_parameters =
        gradient_descent::gradient_descent(&value_training_samples, initial_value_params, 100.0);

    println!("Final parameters: {:?}", tuned_value_parameters);

    let tuned_policy_parameters =
        gradient_descent::gradient_descent(&policy_training_samples, initial_policy_params, 5000.0);

    println!("Final parameters: {:?}", tuned_policy_parameters);

    Ok((tuned_value_parameters, tuned_policy_parameters))
}

pub fn tune_value_and_policy_from_file<const S: usize, const N: usize, const M: usize>(
    value_file_name: &str,
    policy_file_name: &str,
) -> Result<([f32; N], [f32; M]), DynError> {
    let (games, move_scoress) =
        games_and_move_scoress_from_file::<S>(value_file_name, policy_file_name)?;

    let mut rng = rand::rngs::StdRng::from_seed([0; 32]);

    let initial_value_params: [f32; N] = array_from_fn(|| rng.gen_range(-0.01..0.01));

    let initial_policy_params: [f32; M] = array_from_fn(|| rng.gen_range(-0.01..0.01));

    tune_value_and_policy(
        &games,
        &move_scoress,
        &initial_value_params,
        &initial_policy_params,
    )
}

type DynError = Box<dyn error::Error + Send + Sync>;

pub fn games_and_move_scoress_from_file<const S: usize>(
    value_file_name: &str,
    policy_file_name: &str,
) -> Result<(Vec<Game<Position<S>>>, Vec<MoveScoresForGame>), DynError> {
    let mut move_scoress = read_move_scores_from_file::<S>(policy_file_name)?;
    let mut games = read_games_from_file(value_file_name)?;

    // Only keep the last n games, since all the training data doesn't fit in memory while training
    move_scoress.reverse();
    games.reverse();

    match S {
        5 => {
            move_scoress.truncate(10_000);
            games.truncate(10_000);
        }
        6 => {
            move_scoress.truncate(8000);
            games.truncate(8000);
        }
        _ => (),
    }

    for ((i, game), move_scores) in games.iter().enumerate().zip(&move_scoress) {
        let mut position = game.start_position.clone();
        for (mv, move_score) in game
            .moves
            .iter()
            .map(|PtnMove { mv, .. }| mv)
            .zip(move_scores)
        {
            assert!(
                move_score
                    .iter()
                    .any(|(scored_move, _score)| *mv == *scored_move),
                "Played move {} in game {} not among move scores {:?}\nGame: {:?}\nBoard:\n{:?}",
                mv.to_string::<S>(),
                i,
                move_score
                    .iter()
                    .map(|(mv, score)| format!("{}: {:.2}%", mv.to_string::<S>(), score * 100.0))
                    .collect::<Vec<_>>(),
                game.moves
                    .iter()
                    .map(|PtnMove { mv, .. }| mv.to_string::<S>())
                    .collect::<Vec<_>>(),
                position
            );
            position.do_move(mv.clone());
        }
    }
    Ok((games, move_scoress))
}

pub fn read_move_scores_from_file<const S: usize>(
    file_name: &str,
) -> Result<Vec<MoveScoresForGame>, DynError> {
    let mut file = fs::File::open(file_name)?;
    let mut input = String::new();
    file.read_to_string(&mut input)?;

    let position = <Position<S>>::start_position();

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
            let mv = position.move_from_san(words.next().unwrap())?;
            let score = str::parse::<f32>(words.next().unwrap())?;
            scores_for_this_move.push((mv, score));
        }
        move_scoress.last_mut().unwrap().push(scores_for_this_move);
    }
    move_scoress.retain(|move_scores| !move_scores.is_empty());

    println!(
        "Read {} move scores from {} games",
        move_scoress.iter().map(Vec::len).sum::<usize>(),
        move_scoress.len()
    );
    // Extra empty lines may be interpreted as empty games, remove them
    Ok(move_scoress)
}

pub fn positions_and_results_from_games<const S: usize>(
    games: Vec<Game<Position<S>>>,
) -> (Vec<Position<S>>, Vec<GameResult>) {
    let mut positions = vec![];
    let mut results = vec![];
    for game in games.into_iter() {
        let game_result = game.game_result();
        let mut position = game.start_position;
        for PtnMove { mv, .. } in game.moves {
            if position.game_result().is_some() {
                break;
            }
            positions.push(position.clone());
            results.push(game_result.unwrap_or(GameResult::Draw));
            position.do_move(mv.clone());
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

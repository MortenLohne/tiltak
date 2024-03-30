use std::convert::TryFrom;
use std::io::Read;
use std::io::Write;
use std::mem;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time;
use std::{error, fs, io};

use crate::evaluation::parameters::Policy;
use crate::evaluation::parameters::PolicyApplier;
use crate::evaluation::parameters::Value;
use crate::evaluation::parameters::ValueApplier;
use crate::evaluation::policy_eval::policy_offset;
use crate::position::Komi;
use crate::search::TimeControl;
use board_game_traits::GameResult;
use board_game_traits::Position as PositionTrait;
use half::f16;
use rand::prelude::*;
use rayon::prelude::*;

use crate::position::Move;
use crate::position::Position;
use crate::ptn::Game;
use crate::ptn::{ptn_parser, PtnMove};
use crate::search::MctsSetting;
use crate::tune::gradient_descent;
use crate::tune::gradient_descent::TrainingSample;
use crate::tune::play_match::play_game;

// The score, or probability of being played, for a given move
type MoveScore<const S: usize> = (Move<S>, f16);

// The probability of each possible move being played, through a whole game.
type MoveScoresForGame<const S: usize> = Vec<Vec<MoveScore<S>>>;

pub struct TrainingOptions {
    pub training_id: usize,
    pub batch_size: usize,
    pub num_games_for_tuning: usize,
    pub nodes_per_game: usize,
}

pub fn train_from_scratch<const S: usize, const N: usize, const M: usize>(
    options: TrainingOptions,
    komi: Komi,
) -> Result<(), DynError> {
    let mut rng = rand::rngs::StdRng::from_seed([0; 32]);

    let initial_value_params: [f32; N] = array_from_fn(|| rng.gen_range(-0.01..0.01));

    let initial_policy_params: [f32; M] = array_from_fn(|| rng.gen_range(-0.01..0.01));

    train_perpetually::<S, N, M>(
        options,
        komi,
        initial_value_params,
        initial_policy_params,
        vec![],
        vec![],
        0,
    )
}

pub fn continue_training<const S: usize, const N: usize, const M: usize>(
    options: TrainingOptions,
    komi: Komi,
) -> Result<(), DynError> {
    let mut games = vec![];
    let mut move_scores = vec![];
    let mut batch_id = 0;
    loop {
        match read_games_from_file::<S>(
            &format!("games{}_{}s_batch{}.ptn", options.training_id, S, batch_id),
            komi,
        ) {
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
            options.training_id, S, batch_id
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
        options,
        komi,
        <[f32; N]>::try_from(value_params).unwrap(),
        <[f32; M]>::try_from(policy_params).unwrap(),
        games,
        move_scores,
        batch_id,
    )
}

pub fn train_perpetually<const S: usize, const N: usize, const M: usize>(
    options: TrainingOptions,
    komi: Komi,
    initial_value_params: [f32; N],
    initial_policy_params: [f32; M],
    mut all_games: Vec<Game<Position<S>>>,
    mut all_move_scores: Vec<MoveScoresForGame<S>>,
    mut batch_id: usize,
) -> Result<(), DynError> {
    let mut last_value_params: &'static [f32; N] = Box::leak(Box::new(initial_value_params));
    let mut last_policy_params: &'static [f32; M] = Box::leak(Box::new(initial_policy_params));

    let mut value_params: &'static [f32; N] = last_value_params;
    let mut policy_params: &'static [f32; M] = last_policy_params;

    let start_time = time::Instant::now();
    let mut playing_time = time::Duration::default();
    let mut tuning_time = time::Duration::default();

    loop {
        let current_params_wins: AtomicU64 = AtomicU64::new(0);
        let last_params_wins: AtomicU64 = AtomicU64::new(0);

        let playing_start_time = time::Instant::now();
        let (games, move_scores): (Vec<_>, Vec<_>) = (0..options.batch_size)
            .into_par_iter()
            .map(|i| {
                play_game_pair::<S>(
                    komi,
                    last_value_params,
                    last_policy_params,
                    value_params,
                    policy_params,
                    &current_params_wins,
                    &last_params_wins,
                    i,
                )
            })
            .unzip();
        playing_time += playing_start_time.elapsed();

        all_move_scores.extend_from_slice(&move_scores[..]);
        all_games.extend_from_slice(&games[..]);

        let file_name = format!("games{}_{}s_batch{}.ptn", options.training_id, S, batch_id);

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
                options.training_id, S, batch_id
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
                write!(writer, "{}: ", mv)?;
                for (mv, score) in move_scores {
                    write!(writer, "{} {}, ", mv, score)?;
                }
                writeln!(writer)?;
            }
            writeln!(writer)?;
        }
        writer.flush()?;

        let game_stats = GameStats::from_games(&games);

        let wins = current_params_wins.into_inner();
        let losses = last_params_wins.into_inner();
        let draws = options.batch_size as u64 - wins - losses;

        println!("Finished playing batch of {} games. {} games played in total. {} white wins, {} draws, {} black wins, {} aborted. New vs old parameters was +{}-{}={}.",
            games.len(), all_games.len(), game_stats.white_wins, game_stats.draws, game_stats.black_wins, game_stats.aborted, wins, losses, draws
        );

        // Only take the most recent half of the games, to avoid training on bad, old games
        let max_training_games = all_games.len() / 2;

        let games_in_training_batch = all_games
            .iter()
            .cloned()
            .rev()
            .take(usize::min(max_training_games, options.num_games_for_tuning))
            .collect::<Vec<_>>();

        let move_scores_in_training_batch = all_move_scores
            .iter()
            .cloned()
            .rev()
            .take(usize::min(max_training_games, options.num_games_for_tuning))
            .collect::<Vec<_>>();

        let value_tuning_start_time = time::Instant::now();

        let (new_value_params, new_policy_params): ([f32; N], [f32; M]) = tune_value_and_policy(
            &games_in_training_batch,
            &move_scores_in_training_batch,
            komi,
            value_params,
            policy_params,
        )?;

        last_value_params = value_params;
        last_policy_params = policy_params;

        value_params = Box::leak(Box::new(new_value_params));
        policy_params = Box::leak(Box::new(new_policy_params));

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
    last_value_params: &'static [f32],
    last_policy_params: &'static [f32],
    value_params: &'static [f32],
    policy_params: &'static [f32],
    current_params_wins: &AtomicU64,
    last_params_wins: &AtomicU64,
    i: usize,
) -> (Game<Position<S>>, MoveScoresForGame<S>) {
    let settings = MctsSetting::default()
        .add_value_params(value_params)
        .add_policy_params(policy_params)
        .add_dirichlet(0.2);
    let last_settings = MctsSetting::default()
        .add_value_params(last_value_params)
        .add_policy_params(last_policy_params)
        .add_dirichlet(0.2);
    if i % 2 == 0 {
        let game = play_game::<S>(
            &settings,
            &last_settings,
            komi,
            &[],
            1.0,
            &TimeControl::FixedNodes(50_000),
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
            &TimeControl::FixedNodes(50_000),
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
    komi: Komi,
) -> Result<Vec<Game<Position<S>>>, DynError> {
    let start_time = time::Instant::now();
    let mut file = fs::File::open(file_name)?;
    let mut input = String::new();
    file.read_to_string(&mut input)?;
    let mut games = ptn_parser::parse_ptn::<Position<S>>(&input)?;
    for game in games.iter_mut() {
        if let Some((_, komi_str)) = game
            .tags
            .iter()
            .find(|(tag, _)| tag.eq_ignore_ascii_case("Komi"))
        {
            game.start_position
                .set_komi(Komi::from_str(komi_str).unwrap());
        }
        assert_eq!(komi, game.start_position.komi());
    }
    println!(
        "Read {} games from PTN in {:.1}s",
        games.len(),
        start_time.elapsed().as_secs_f32()
    );
    Ok(games)
}

pub fn tune_value_from_file<const S: usize, const N: usize>(
    file_name: &str,
    komi: Komi,
) -> Result<[f32; N], DynError> {
    let games = read_games_from_file::<S>(file_name, komi)?;

    let start_time = time::Instant::now();
    let (positions, results) = positions_and_results_from_games(&games, komi);
    println!(
        "Extracted {} positions in {:.1}s",
        positions.len(),
        start_time.elapsed().as_secs_f32()
    );

    let start_time = time::Instant::now();
    let mut samples = positions
        .par_iter()
        .zip(results)
        .map(|(position, game_result)| {
            let mut white_features: Value<S> = Value::new(&[]);
            let mut black_features: Value<S> = Value::new(&[]);
            position.static_eval_features(&mut white_features, &mut black_features);

            let features = white_features
                .features
                .into_iter()
                .chain(black_features.features)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();

            let result = match game_result {
                GameResult::WhiteWin => f16::ONE,
                GameResult::Draw => f16::ONE / (f16::ONE + f16::ONE),
                GameResult::BlackWin => f16::ZERO,
            };
            TrainingSample {
                features,
                offset: 0.0,
                result,
            }
        })
        .collect::<Vec<_>>();

    println!(
        "Vectorized {} training samples in {:.1}s",
        samples.len(),
        start_time.elapsed().as_secs_f32()
    );

    let mut rng = rand::rngs::StdRng::from_seed([0; 32]);
    let mut initial_params = [0.00; N];

    for param in initial_params.iter_mut() {
        *param = rng.gen_range(-0.01..0.01)
    }

    samples.shuffle(&mut rng);

    let tuned_parameters =
        gradient_descent::gradient_descent(&samples, &initial_params, 10.0, &mut rng);

    Ok(tuned_parameters)
}

pub fn tune_value_and_policy<const S: usize, const N: usize, const M: usize>(
    games: &[Game<Position<S>>],
    move_scoress: &[MoveScoresForGame<S>],
    komi: Komi,
    initial_value_params: &[f32; N],
    initial_policy_params: &[f32; M],
) -> Result<([f32; N], [f32; M]), DynError> {
    let mut rng = rand::rngs::StdRng::from_seed([0; 32]);

    let (positions, results) = positions_and_results_from_games(games, komi);

    let start_time = time::Instant::now();
    let mut value_training_samples = positions
        .par_iter()
        .zip(results)
        .map(|(position, game_result)| {
            let mut white_features: Value<S> = Value::new(&[]);
            let mut black_features: Value<S> = Value::new(&[]);
            position.static_eval_features(&mut white_features, &mut black_features);

            let features = white_features
                .features
                .into_iter()
                .chain(black_features.features)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();

            let result = match game_result {
                GameResult::WhiteWin => f16::ONE,
                GameResult::Draw => f16::ONE / (f16::ONE + f16::ONE),
                GameResult::BlackWin => f16::ZERO,
            };
            TrainingSample {
                features,
                offset: 0.0,
                result,
            }
        })
        .collect::<Vec<_>>();

    value_training_samples.shuffle(&mut rng);

    println!(
        "Generated {} value training samples in {:.1}s, {:.2}GiB total",
        value_training_samples.len(),
        start_time.elapsed().as_secs_f32(),
        (value_training_samples.len() * mem::size_of::<TrainingSample<N>>()) as f32
            / f32::powf(2.0, 30.0),
    );

    let number_of_feature_sets = move_scoress.iter().flatten().flatten().count();

    let start_time: time::Instant = time::Instant::now();

    let mut policy_training_samples: Vec<TrainingSample<M>> =
        Vec::with_capacity(number_of_feature_sets);

    policy_training_samples.extend(games.iter().zip(move_scoress.iter()).flat_map(
        |(game, move_scores)| {
            let mut position = game.start_position.clone();

            game.moves
                .iter()
                .map(|PtnMove { mv, .. }| mv)
                .zip(move_scores.iter())
                .flat_map(move |(mv, move_scores)| {
                    let group_data = position.group_data();

                    let mut policies: Vec<Policy<S>> = vec![Policy::new(&[]); move_scores.len()];
                    let moves: Vec<Move<S>> = move_scores.iter().map(|(mv, _score)| *mv).collect();

                    position.features_for_moves(
                        &mut policies,
                        &moves,
                        &mut Vec::with_capacity(moves.len()),
                        &group_data,
                    );

                    position.do_move(*mv);

                    move_scores
                        .iter()
                        .zip(
                            policies
                                .into_iter()
                                .map(|pol| pol.features.try_into().unwrap()),
                        )
                        .map(|((_, result), features)| {
                            let offset = policy_offset(move_scores.len());
                            {
                                TrainingSample {
                                    features,
                                    offset,
                                    result: *result,
                                }
                            }
                        })
                })
        },
    ));

    policy_training_samples.shuffle(&mut rng);
    println!(
        "Generated {} policy training samples in {:.1}s, {:.2}GiB total",
        policy_training_samples.len(),
        start_time.elapsed().as_secs_f32(),
        (policy_training_samples.len() * mem::size_of::<TrainingSample<M>>()) as f32
            / f32::powf(2.0, 30.0),
    );

    let tuned_value_parameters = gradient_descent::gradient_descent(
        &value_training_samples,
        initial_value_params,
        50.0,
        &mut rng,
    );

    let tuned_policy_parameters = gradient_descent::gradient_descent(
        &policy_training_samples,
        initial_policy_params,
        500.0,
        &mut rng,
    );

    Ok((tuned_value_parameters, tuned_policy_parameters))
}

pub fn tune_value_and_policy_from_file<const S: usize, const N: usize, const M: usize>(
    value_file_name: &str,
    policy_file_name: &str,
    komi: Komi,
) -> Result<([f32; N], [f32; M]), DynError> {
    let (games, move_scoress) =
        games_and_move_scoress_from_file::<S>(value_file_name, policy_file_name, komi)?;
    let mut rng = rand::rngs::StdRng::from_seed([0; 32]);

    let initial_value_params: [f32; N] = array_from_fn(|| rng.gen_range(-0.01..0.01));

    let initial_policy_params: [f32; M] = array_from_fn(|| rng.gen_range(-0.01..0.01));

    tune_value_and_policy(
        &games,
        &move_scoress,
        komi,
        &initial_value_params,
        &initial_policy_params,
    )
}

type DynError = Box<dyn error::Error + Send + Sync>;

pub fn games_and_move_scoress_from_file<const S: usize>(
    value_file_name: &str,
    policy_file_name: &str,
    komi: Komi,
) -> Result<(Vec<Game<Position<S>>>, Vec<MoveScoresForGame<S>>), DynError> {
    let mut move_scoress = read_move_scores_from_file::<S>(policy_file_name)?;
    let mut games = read_games_from_file(value_file_name, komi)?;

    // Only keep the last n games, since all the training data doesn't fit in memory while training
    move_scoress.reverse();
    games.reverse();

    match S {
        5 => {
            move_scoress.truncate(16_000);
            games.truncate(16_000);
        }
        6 => {
            move_scoress.truncate(12000);
            games.truncate(12000);
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
                mv,
                i,
                move_score
                    .iter()
                    .map(|(mv, score)| format!("{}: {:.2}%", mv, score.to_f32() * 100.0))
                    .collect::<Vec<_>>(),
                game.moves
                    .iter()
                    .map(|PtnMove { mv, .. }| mv.to_string())
                    .collect::<Vec<_>>(),
                position
            );
            position.do_move(*mv);
        }
    }
    Ok((games, move_scoress))
}

pub fn read_move_scores_from_file<const S: usize>(
    file_name: &str,
) -> Result<Vec<MoveScoresForGame<S>>, DynError> {
    let start_time = time::Instant::now();

    // Read entire file into memory. Because it's a single allocation, this allows the memory to be cleanly reclaimed later
    let contents = fs::read_to_string(file_name)?;

    // Games are separated by empty lines. Split here, to allow parallel parsing later
    let games: Vec<&str> = contents.split("\n\n").collect();

    let mut move_scoress: Vec<MoveScoresForGame<S>> = games
        .into_par_iter()
        .map(|line_group| {
            line_group
                .lines()
                .map(|line| {
                    let mut scores_for_this_move =
                        Vec::with_capacity(line.chars().filter(|ch| *ch == ',').count());
                    let _played_move = line.split(':').next().unwrap();
                    let possible_moves = line.split(':').nth(1).unwrap();
                    for move_score_string in possible_moves.split(',') {
                        if move_score_string.len() < 3 {
                            continue;
                        }
                        let mut words = move_score_string.split_whitespace();
                        let mv = Move::from_string(words.next().unwrap()).unwrap();
                        let score = str::parse::<f16>(words.next().unwrap()).unwrap();
                        scores_for_this_move.push((mv, score));
                    }
                    // This assert is only a performance check
                    assert_eq!(scores_for_this_move.len(), scores_for_this_move.capacity());
                    scores_for_this_move
                })
                .collect()
        })
        .collect();

    // Extra empty lines may be interpreted as empty games, remove them
    move_scoress.retain(|move_scores| !move_scores.is_empty());

    println!(
        "Read {} move scores from {} games in {:.1}s",
        move_scoress.iter().map(Vec::len).sum::<usize>(),
        move_scoress.len(),
        start_time.elapsed().as_secs_f32()
    );
    Ok(move_scoress)
}

pub fn positions_and_results_from_games<const S: usize>(
    games: &[Game<Position<S>>],
    komi: Komi,
) -> (Vec<Position<S>>, Vec<GameResult>) {
    games
        .into_par_iter()
        .flat_map_iter(|game| {
            let game_result = game.game_result();
            let mut position = game.start_position.clone();
            if let Some((_, komi_str)) = game
                .tags
                .iter()
                .find(|(tag, _)| tag.eq_ignore_ascii_case("Komi"))
            {
                position.set_komi(Komi::from_str(komi_str).unwrap());
            }
            assert_eq!(komi, position.komi());
            let mut output: Vec<(Position<S>, GameResult)> = Vec::with_capacity(200);
            for PtnMove { mv, .. } in game.moves.iter() {
                if position.game_result().is_some() {
                    break;
                }
                output.push((position.clone(), game_result.unwrap_or(GameResult::Draw)));
                position.do_move(*mv);
                // Deliberately skip the final position
            }
            output
        })
        .unzip()
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

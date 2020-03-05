use crate::board::Board;
use crate::tune::auto_tune::TunableBoard;
use crate::tune::pgn_parse::Game;
use crate::tune::play_match::play_game;
use crate::tune::{auto_tune, pgn_parse, play_match};
use arrayvec::ArrayVec;
use board_game_traits::board::Board as BoardTrait;
use board_game_traits::board::GameResult;
use rand::prelude::*;
use rayon::prelude::*;
use std::io::Read;
use std::iter::FromIterator;
use std::{error, fs, io, iter, sync};

pub fn train_from_scratch(training_id: usize) -> Result<(), Box<dyn error::Error>> {
    const BATCH_SIZE: usize = 400;
    // Only train from the last n batches
    const BATCHES_FOR_TRAINING: usize = 25;

    let mut rng = rand::thread_rng();
    let initial_params: ArrayVec<[f32; Board::PARAMS.len()]> =
        ArrayVec::from_iter(iter::from_fn(|| Some(rng.gen())));

    let mut all_games = vec![];
    let mut params = initial_params;

    let mut batch_id = 0;

    loop {
        let games: Vec<Game<Board>> = (0..BATCH_SIZE)
            .into_par_iter()
            .map(|_| play_game(&params))
            .collect();
        all_games.extend_from_slice(&games[..]);

        let file_name = format!("games{}_batch{}.ptn", training_id, batch_id);

        let outfile = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_name)
            .unwrap();

        let mut writer = io::BufWriter::new(outfile);

        for game in games.iter() {
            play_match::game_to_pgn(game, &mut writer)?;
        }

        let game_stats = GameStats::from_games(&games);

        println!("Finished playing batch of {} games. {} games played in total. {} white wins, {} draws, {} black wins, {} aborted.",
            games.len(), all_games.len(), game_stats.white_wins, game_stats.draws, game_stats.black_wins, game_stats.aborted
        );

        let mut training_games = all_games
            .iter()
            .cloned()
            .rev()
            .take(BATCH_SIZE * BATCHES_FOR_TRAINING)
            .collect::<Vec<_>>();

        training_games.shuffle(&mut rng);

        let (positions, results) = positions_and_results_from_games(training_games);
        let middle_index = positions.len() / 2;

        params = ArrayVec::from_iter(
            auto_tune::gradient_descent(
                &positions[0..middle_index],
                &results[0..middle_index],
                &positions[middle_index..],
                &results[middle_index..],
                &params,
            )
            .into_iter(),
        );

        batch_id += 1;
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

pub fn tune() {
    let outfile = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("output4.ptn")
        .unwrap();
    let locked_writer = sync::Mutex::new(io::BufWriter::new(outfile));

    use std::sync::atomic::AtomicU64;
    use std::sync::atomic::Ordering;

    let mut white_wins: AtomicU64 = AtomicU64::new(0);
    let mut draws = AtomicU64::new(0);
    let mut black_wins = AtomicU64::new(0);
    let mut aborted = AtomicU64::new(0);
    loop {
        let games = play_match::play_match();
        games.for_each(|ref game| {
            {
                let mut writer = locked_writer.lock().unwrap();
                play_match::game_to_pgn(game, &mut *writer).unwrap();
            }
            match game.game_result {
                None => aborted.fetch_add(1, Ordering::Relaxed),
                Some(GameResult::WhiteWin) => white_wins.fetch_add(1, Ordering::Relaxed),
                Some(GameResult::BlackWin) => black_wins.fetch_add(1, Ordering::Relaxed),
                Some(GameResult::Draw) => draws.fetch_add(1, Ordering::Relaxed),
            };
        });
        println!(
            "{} white wins, {} draws, {} black wins, {} aborted.",
            white_wins.get_mut(),
            draws.get_mut(),
            black_wins.get_mut(),
            aborted.get_mut()
        );
    }
}

pub fn tune_from_file() -> Result<(), Box<dyn error::Error>> {
    let mut file = fs::File::open("output4.ptn")?;
    let mut input = String::new();
    file.read_to_string(&mut input)?;
    let games: Vec<Game<Board>> = pgn_parse::parse_pgn(&input)?;

    let (positions, results) = positions_and_results_from_games(games);

    let middle_index = positions.len() / 2;

    let params = [0.01; Board::PARAMS.len()];

    println!(
        "Final parameters: {:?}",
        auto_tune::gradient_descent(
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
    for game in games.into_iter().filter(|game| game.game_result.is_some()) {
        let mut board = game.start_board;
        for (mv, _) in game.moves {
            board.do_move(mv);
            positions.push(board.clone());
            results.push(game.game_result.unwrap());
        }
    }
    (positions, results)
}

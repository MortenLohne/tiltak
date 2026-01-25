use crate::position::Role;
use std::time::{self, Duration};

use board_game_traits::Position as _;
use chrono::{DateTime, Utc};
use rusqlite::Connection;

use crate::position::{self, ExpMove, Komi, Move, Position};

pub fn analyze_playtak_db<const S: usize>(playtak_db_name: &str) {
    let mut db_conn = Connection::open(playtak_db_name).unwrap();

    let start_time = time::Instant::now();
    let games = read_all_games(&mut db_conn).unwrap();
    println!(
        "Read {} games in {:.2} seconds",
        games.len(),
        start_time.elapsed().as_secs_f32()
    );

    let start_time = time::Instant::now();
    let relevant_games: Vec<PlaytakGame<S>> = games
        .into_iter()
        .filter(|game| {
            let is_legal = game.game_is_legal();
            if !is_legal {
                println!(
                    "Game #{}, {} vs {} was illegal",
                    game.id, game.player_white, game.player_black
                );
            }
            game.rating_white.is_some_and(|r| r > 1200)
                && game.rating_black.is_some_and(|r| r > 1200)
                && !game.is_bot_game()
                && is_legal
                && game.has_standard_piece_count()
        })
        .collect();
    println!(
        "Extracted {} relevant games in {:.2} seconds",
        relevant_games.len(),
        start_time.elapsed().as_secs_f32()
    );

    let mut longest_reversible = 0;
    let mut longest_movements = 0;

    for game in relevant_games {
        let result = check_longest_reversible_sequence(&game);
        if result.max_reversible > 15 || result.max_reversible >= longest_reversible {
            if result.max_reversible >= longest_reversible {
                println!("New longest reversible sequence");
                longest_reversible = result.max_reversible;
            }
            println!(
                "Reversibles: Game #{}, {} vs {} had {} reversible ply in a row",
                game.id, game.player_white, game.player_black, result.max_reversible
            );
            println!();
        }

        if result.max_movements > 30 || result.max_movements >= longest_movements {
            if result.max_movements >= longest_movements {
                println!("New longest movement sequence");
                longest_movements = result.max_movements;
            }
            println!(
                "Movements: Game #{}, {} vs {} had {} movement ply in a row",
                game.id, game.player_white, game.player_black, result.max_movements
            );
            println!();
        }
    }
}

struct AnalysisResult {
    max_reversible: u16,
    max_movements: u16,
}

fn check_longest_reversible_sequence<const S: usize>(game: &PlaytakGame<S>) -> AnalysisResult {
    let mut num_reversible = 0;
    let mut num_movements = 0;

    let mut result = AnalysisResult {
        max_reversible: 0,
        max_movements: 0,
    };
    let mut position = Position::start_position_with_komi(game.komi);

    for mv in &game.moves {
        num_reversible += 1;
        num_movements += 1;
        if mv.is_placement() {
            num_reversible = 0;
            num_movements = 0;
        } else if let ExpMove::Move(sq, direction, movement) = mv.expand() {
            let destination_square = sq.jump_direction(direction, movement.len() as u8).unwrap();
            if position.top_stones()[destination_square]
                .is_some_and(|piece| piece.role() == Role::Wall)
            {
                num_reversible = 0;
                num_movements += 1;
            }
        } else {
            num_reversible += 1;
            num_movements += 1;
        }
        result.max_reversible = result.max_reversible.max(num_reversible);
        result.max_movements = result.max_movements.max(num_movements);

        position.do_move(*mv);
    }
    result
}

#[derive(Debug, Clone)]
struct PlaytakGame<const S: usize> {
    id: u64,
    #[allow(dead_code)]
    date_time: DateTime<Utc>,
    player_white: String,
    player_black: String,
    moves: Vec<Move<S>>,
    #[allow(dead_code)]
    result_string: String,
    #[allow(dead_code)]
    game_time: Duration,
    #[allow(dead_code)]
    increment: Duration,
    rating_white: Option<i64>,
    rating_black: Option<i64>,
    #[allow(dead_code)]
    is_rated: bool,
    #[allow(dead_code)]
    is_tournament: bool,
    komi: Komi,
    flats: i64,
    caps: i64,
}

impl<const S: usize> PlaytakGame<S> {
    pub fn has_standard_piece_count(&self) -> bool {
        position::starting_stones(S) as i64 == self.flats
            && position::starting_capstones(S) as i64 == self.caps
    }

    #[allow(dead_code)]
    pub fn is_guest_game(&self) -> bool {
        self.player_white.starts_with("Guest") || self.player_black.starts_with("Guest")
    }

    pub fn is_bot_game(&self) -> bool {
        const BOTS: &[&str] = &[
            "TakticianBot",
            "alphatak_bot",
            "alphabot",
            "cutak_bot",
            "TakticianBotDev",
            "takkybot",
            "ShlktBot",
            "AlphaTakBot_5x5",
            "BeginnerBot",
            "TakkerusBot",
            "IntuitionBot",
            "AaaarghBot",
            "kriTakBot",
            "TakkenBot",
            "robot",
            "TakkerBot",
            "Geust93",
            "CairnBot",
            "VerekaiBot1",
            "BloodlessBot",
            "Tiltak_Bot",
            "Taik",
            "FlashBot",
            "FriendlyBot",
            "FPABot",
            "sTAKbot1",
            "sTAKbot2",
            "DoubleStackBot",
            "antakonistbot",
            "CrumBot",
            "SlateBot",
            "CobbleBot",
        ];
        BOTS.contains(&self.player_white.as_str()) || BOTS.contains(&self.player_black.as_str())
    }

    pub fn game_is_legal(&self) -> bool {
        let mut position: Position<S> = Position::start_position_with_komi(self.komi);
        for mv in &self.moves {
            if position.game_result().is_some() {
                return false;
            }
            if !position.move_is_legal(*mv) {
                return false;
            }
            position.do_move(*mv);
        }
        true
    }
}

impl<const S: usize> TryFrom<GameRow> for PlaytakGame<S> {
    type Error = ();
    fn try_from(row: GameRow) -> Result<Self, ()> {
        if row.size as usize != S {
            return Err(());
        }
        let notation = parse_notation::<S>(&row.notation);
        Ok(PlaytakGame {
            id: row.id as u64,
            date_time: row.date,
            player_white: row.player_white,
            player_black: row.player_black,
            moves: notation,
            result_string: row.result,
            game_time: row.timertime,
            increment: row.timerinc,
            rating_white: if row.rating_white == 0 {
                None
            } else {
                Some(row.rating_white)
            },
            rating_black: if row.rating_black == 0 {
                None
            } else {
                Some(row.rating_black)
            },
            is_rated: !row.unrated,
            is_tournament: row.tournament,
            komi: Komi::from_half_komi(row.komi.try_into().map_err(|_| ())?).ok_or(())?,
            flats: if row.pieces == -1 {
                position::starting_stones(row.size as usize) as i64
            } else {
                row.pieces
            },
            caps: if row.capstones == -1 {
                position::starting_capstones(row.size as usize) as i64
            } else {
                row.capstones
            },
        })
    }
}

#[derive(Debug)]
struct GameRow {
    id: u32,
    date: DateTime<Utc>,
    size: u8,
    player_white: String,
    player_black: String,
    notation: String,
    result: String,
    timertime: Duration,
    timerinc: Duration,
    rating_white: i64,
    rating_black: i64,
    unrated: bool,
    tournament: bool,
    komi: u8,
    pieces: i64,
    capstones: i64,
}

fn parse_notation<const S: usize>(notation: &str) -> Vec<Move<S>> {
    if notation.is_empty() {
        Vec::new()
    } else {
        notation
            .split(',')
            .map(Move::<S>::from_string_playtak)
            .collect()
    }
}

fn read_all_games<const S: usize>(conn: &mut Connection) -> Option<Vec<PlaytakGame<S>>> {
    let mut stmt = conn.prepare("SELECT id, date, size, player_white, player_black, notation, result, timertime, timerinc, rating_white, rating_black, unrated, tournament, komi, pieces, capstones FROM games
        WHERE size = $1")
    .unwrap();
    let rows = stmt.query([S]).unwrap().mapped(|row| {
        Ok(GameRow {
            id: row.get(0).unwrap(),
            date: DateTime::from_naive_utc_and_offset(
                DateTime::from_timestamp(row.get::<_, i64>(1).unwrap() / 1000, 0)
                    .unwrap()
                    .naive_local(),
                Utc,
            ),
            size: row.get(2).unwrap(),
            player_white: row.get(3).unwrap(),
            player_black: row.get(4).unwrap(),
            notation: row.get(5).unwrap(),
            result: row.get(6).unwrap(),
            timertime: Duration::from_secs(row.get(7).unwrap()),
            timerinc: Duration::from_secs(row.get(8).unwrap()),
            rating_white: row.get(9).unwrap(),
            rating_black: row.get(10).unwrap(),
            unrated: row.get(11).unwrap(),
            tournament: row.get(12).unwrap(),
            komi: row.get(13).unwrap(),
            pieces: row.get(14).unwrap(),
            capstones: row.get(15).unwrap(),
        })
    });
    rows.map(|row| PlaytakGame::try_from(row.unwrap()).ok())
        .collect()
}

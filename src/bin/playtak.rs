use board_game_traits::{Color, GameResult, Position as PositionTrait};
use bufstream::BufStream;
use clap::{App, Arg};
use log::error;
use std::fmt::Write as FmtWrite;
use std::io::{BufRead, Result, Write};
use std::net::TcpStream;
use std::str::FromStr;
use std::time::Duration;
use std::{io, net, thread};
#[cfg(feature = "aws-lambda-client")]
use tiltak::aws;
use tiltak::position::Board;

use log::{debug, info, warn};

pub fn main() -> Result<()> {
    let mut app = App::new("Tiltak playtak client")
        .version("0.1")
        .author("Morten Lohne")
        .arg(
            Arg::with_name("username")
                .requires("password")
                .short("u")
                .long("username")
                .value_name("USER")
                .help("playtak.com username")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("password")
                .short("p")
                .long("password")
                .value_name("PASS")
                .help("playtak.com password")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("size")
                .short("s")
                .long("size")
                .help("Board size")
                .takes_value(true)
                .default_value("5")
                .possible_values(&["4", "5", "6"]),
        )
        .arg(
            Arg::with_name("logfile")
                .short("l")
                .long("logfile")
                .value_name("tiltak.log")
                .help("Name of debug logfile")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("playBot")
                .long("play-bot")
                .value_name("botname")
                .help("Instead of seeking any game, accept any seek from the specified bot")
                .takes_value(true),
        );
    if cfg!(feature = "aws-lambda-client") {
        app = app.arg(
            Arg::with_name("aws-function-name")
                .long("aws-function-name")
                .value_name("tiltak")
                .required(true)
                .help(
                    "Run the engine on AWS instead of locally. Requires aws cli installed locally.",
                )
                .takes_value(true),
        );
    }
    let matches = app.get_matches();

    let log_dispatcher = fern::Dispatch::new().format(|out, message, record| {
        out.finish(format_args!(
            "{}[{}][{}] {}",
            chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
            record.target(),
            record.level(),
            message
        ))
    });

    if let Some(log_file) = matches.value_of("logfile") {
        log_dispatcher
            .chain(
                fern::Dispatch::new()
                    .level(log::LevelFilter::Debug)
                    .chain(fern::log_file(log_file)?),
            )
            .chain(
                fern::Dispatch::new()
                    .level(log::LevelFilter::Warn)
                    .chain(io::stderr()),
            )
            .apply()
            .unwrap()
    } else {
        log_dispatcher
            .level(log::LevelFilter::Warn)
            .chain(io::stderr())
            .apply()
            .unwrap()
    }

    let size: usize = matches.value_of("size").unwrap().parse().unwrap();

    loop {
        #[cfg(feature = "aws-lambda-client")]
        let connection_result =
            if let Some(aws_function_name) = matches.value_of("aws-function-name") {
                PlaytakSession::with_aws(aws_function_name.to_string())
            } else {
                PlaytakSession::new()
            };
        #[cfg(not(feature = "aws-lambda-client"))]
        let connection_result = PlaytakSession::new();

        let mut session = match connection_result {
            Ok(ok) => ok,
            Err(err) => {
                warn!("Failed to connect due to \"{}\", retrying...", err);
                thread::sleep(Duration::from_secs(2));
                continue;
            }
        };

        if let (Some(user), Some(pwd)) =
            (matches.value_of("username"), matches.value_of("password"))
        {
            session.login("Tiltak", &user, &pwd)?;
        } else {
            warn!("No username/password provided, logging in as guest");
            session.login_guest()?;
        }
        let seekmode = match matches.value_of("playBot") {
            Some(name) => SeekMode::PlayOtherBot(name.to_string()),
            None => SeekMode::OpenSeek,
        };

        let result = match size {
            4 => session.seek_game::<4>(seekmode),
            5 => session.seek_game::<5>(seekmode),
            6 => session.seek_game::<6>(seekmode),
            s => panic!("Unsupported size {}", s),
        };

        match result {
            Err(err) => match err.kind() {
                io::ErrorKind::ConnectionAborted | io::ErrorKind::ConnectionReset => {
                    warn!("Server connection interrupted, caused by \"{}\". This may be due to a server restart, attempting to reconnect.", err)
                }
                _ => {
                    error!("Fatal error: {}", err);
                    return Err(err);
                }
            },
            Ok(_) => unreachable!(),
        }
    }
}

struct PlaytakSession {
    #[cfg(feature = "aws-lambda-client")]
    aws_function_name: Option<String>,
    connection: BufStream<TcpStream>,
    // The server requires regular pings, to not kick the user
    // This thread does nothing but provide those pings
    ping_thread: Option<thread::JoinHandle<io::Result<()>>>,
}

#[derive(Debug, PartialEq, Eq)]
enum SeekMode {
    OpenSeek,
    PlayOtherBot(String),
}

#[derive(Debug, PartialEq, Eq)]
struct PlaytakGame<'a> {
    game_no: u64,
    _board_size: usize,
    white_player: &'a str,
    black_player: &'a str,
    our_color: Color,
    time_left: Duration,
    increment: Duration,
}

impl PlaytakSession {
    /// Initialize a connection to playtak.com. Does not log in or play games.
    fn new() -> Result<Self> {
        let connection = connect()?;
        let mut ping_thread_connection = connection.get_ref().try_clone()?;
        let ping_thread = Some(thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(30));
            writeln!(ping_thread_connection, "PING")?;
            ping_thread_connection.flush()?;
        }));
        Ok(PlaytakSession {
            #[cfg(feature = "aws-lambda-client")]
            aws_function_name: None,
            connection,
            ping_thread,
        })
    }

    #[cfg(feature = "aws-lambda-client")]
    fn with_aws(aws_function_name: String) -> Result<Self> {
        let mut session = Self::new()?;
        session.aws_function_name = Some(aws_function_name);
        Ok(session)
    }

    /// Login with the provided name, username and password
    fn login(&mut self, client_name: &str, user: &str, pwd: &str) -> Result<()> {
        loop {
            let line = self.read_line()?;
            if line.starts_with("Login") {
                break;
            }
        }

        self.send_line(&format!("client {}", client_name))?;
        self.send_line(&format!("Login {} {}", user, pwd))?;

        loop {
            let line = self.read_line()?;
            if line.starts_with("Welcome ") {
                break;
            }
        }
        Ok(())
    }

    fn login_guest(&mut self) -> Result<()> {
        loop {
            let line = self.read_line()?;
            if line.starts_with("Login") {
                break;
            }
        }

        self.send_line("Login Guest")?;

        loop {
            let line = self.read_line()?;
            if line.starts_with("Welcome ") {
                break;
            }
        }
        Ok(())
    }

    fn read_line(&mut self) -> Result<String> {
        let mut input = String::new();
        let bytes_read = self.connection.read_line(&mut input)?;
        if bytes_read == 0 {
            info!("Got EOF from server. Shutting down connection.");
            self.connection.get_mut().shutdown(net::Shutdown::Both)?;
            info!("Waiting for ping thread to exit");
            match self.ping_thread.take().unwrap().join() {
                Ok(Ok(())) => unreachable!(),
                Ok(Err(err)) => info!("Ping thread exited successfully with {}", err),
                Err(err) => error!("Failed to join ping thread {:?}", err),
            }
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "Received EOF from server",
            ));
        }
        info!("> {}", input.trim());
        Ok(input)
    }

    fn send_line(&mut self, output: &str) -> Result<()> {
        writeln!(self.connection, "{}", output)?;
        self.connection.flush()?;
        info!("< {}", output);
        Ok(())
    }

    /// Place a game seek (challenge) on playtak, and wait for somebody to accept
    /// Mutually recursive with `play_game` when the challenge is accepted
    pub fn seek_game<const S: usize>(
        &mut self,
        seek_mode: SeekMode,
    ) -> io::Result<std::convert::Infallible> {
        let mut time_for_game = Duration::from_secs(900);
        let mut increment = Duration::from_secs(30);

        if seek_mode == SeekMode::OpenSeek {
            self.send_line(&format!(
                "Seek {} {} {}",
                S,
                time_for_game.as_secs(),
                increment.as_secs()
            ))?;
        }

        loop {
            let input = self.read_line()?;
            let words: Vec<&str> = input.split_whitespace().collect();
            if words.is_empty() {
                continue;
            }
            match words[0] {
                "Game" => {
                    let playtak_game = PlaytakGame {
                        game_no: u64::from_str(words[2]).unwrap(),
                        _board_size: usize::from_str(words[3]).unwrap(),
                        white_player: words[4],
                        black_player: words[6],
                        our_color: match words[7] {
                            "white" => Color::White,
                            "black" => Color::Black,
                            color => panic!("Bad color \"{}\"", color),
                        },
                        time_left: time_for_game,
                        increment,
                    };
                    self.play_game::<S>(seek_mode, playtak_game)?;
                    unreachable!()
                }

                "Seek" => {
                    if let SeekMode::PlayOtherBot(ref bot_name) = seek_mode {
                        if words[1] == "new" {
                            let number = u64::from_str(words[2]).unwrap();
                            let name = words[3];
                            let time = Duration::from_secs(u64::from_str(words[5]).unwrap());
                            let inc = Duration::from_secs(u64::from_str(words[6]).unwrap());
                            if name.eq_ignore_ascii_case(bot_name) {
                                self.send_line(&format!("Accept {}", number))?;
                                time_for_game = time;
                                increment = inc;
                            }
                        }
                    }
                }
                "NOK" => {
                    warn!("Received NOK from server, ignoring. This may happen if the game was aborted while we were thinking");
                }
                _ => debug!("Ignoring server message \"{}\"", input.trim()),
            }
        }
    }

    /// The main game loop of a playtak game.
    /// Mutually recursive with `seek_game`, which places a new seek as soon as the game finishes.
    fn play_game<const S: usize>(
        &mut self,
        seek_mode: SeekMode,
        game: PlaytakGame,
    ) -> io::Result<Infallible> {
        info!(
            "Starting game #{}, {} vs {} as {}, {}+{:.1}",
            game.game_no,
            game.white_player,
            game.black_player,
            game.our_color,
            game.time_left.as_secs(),
            game.increment.as_secs_f32()
        );
        let mut board = <Board<S>>::start_position();
        let mut moves = vec![];
        let mut our_time_left = game.time_left;
        'gameloop: loop {
            if board.game_result().is_some() {
                break;
            }
            if board.side_to_move() == game.our_color {
                #[cfg(feature = "aws-lambda-client")]
                let (best_move, score) = {
                    let aws_function_name = self.aws_function_name.as_ref().unwrap();
                    let event = aws::Event {
                        size: S,
                        moves: moves
                            .iter()
                            .map(|PtnMove { mv, .. }: &PtnMove<Move>| mv.clone())
                            .collect(),
                        time_left: our_time_left,
                        increment: game.increment,
                    };
                    let aws::Output { best_move, score } =
                        aws::client::best_move_aws(aws_function_name, &event)?;
                    (best_move, score)
                };

                #[cfg(not(feature = "aws-lambda-client"))]
                let (best_move, score) = {
                    let maximum_time = our_time_left / 5 + game.increment;
                    let settings = MctsSetting::default().add_dirichlet(0.1);
                    search::play_move_time(board.clone(), maximum_time, settings)
                };

                board.do_move(best_move.clone());
                moves.push(PtnMove {
                    mv: best_move.clone(),
                    annotations: vec![],
                    comment: score.to_string(),
                });

                let mut output_string = format!("Game#{} ", game.game_no);
                write_move::<S>(best_move, &mut output_string);
                self.send_line(&output_string)?;

                // Say "Tak" whenever there is a threat to win
                // Only do this vs Shigewara
                if game.white_player == "shigewara" || game.black_player == "shigewara" {
                    let mut board_clone = board.clone();
                    board_clone.null_move();
                    let mut moves = vec![];
                    board_clone.generate_moves(&mut moves);
                    for mv in moves {
                        let reverse_move = board_clone.do_move(mv);
                        match (board_clone.side_to_move(), board_clone.game_result()) {
                            (Color::White, Some(GameResult::BlackWin))
                            | (Color::Black, Some(GameResult::WhiteWin)) => {
                                self.send_line("Tell shigewara Tak!")?;
                                break;
                            }
                            _ => (),
                        }
                        board_clone.reverse_move(reverse_move);
                    }
                }
            } else {
                // Wait for the opponent's move. The server may send other messages in the meantime
                loop {
                    let line = self.read_line()?;
                    let words: Vec<&str> = line.split_whitespace().collect();
                    if words.is_empty() {
                        continue;
                    }
                    if words[0] == format!("Game#{}", game.game_no) {
                        match words[1] {
                            "P" | "M" => {
                                let move_string = words[1..].join(" ");
                                let move_played = parse_move::<S>(&move_string);
                                board.do_move(move_played.clone());
                                moves.push(PtnMove {
                                    mv: move_played,
                                    annotations: vec![],
                                    comment: "0.0".to_string(),
                                });
                                break;
                            }
                            "Time" => {
                                let white_time_left =
                                    Duration::from_secs(u64::from_str(&words[2]).unwrap());
                                let black_time_left =
                                    Duration::from_secs(u64::from_str(&words[3]).unwrap());
                                our_time_left = match game.our_color {
                                    Color::White => white_time_left,
                                    Color::Black => black_time_left,
                                };
                            }
                            "Abandoned" | "Abandoned." | "Over" => break 'gameloop,
                            _ => debug!("Ignoring server message \"{}\"", line),
                        }
                    } else if words[0] == "NOK" {
                        warn!("Received NOK from server, ignoring.");
                    }
                }
            }
        }

        {
            info!("Game finished. Pgn: ");

            let date = Local::today();

            let tags = vec![
                ("Event".to_string(), "Playtak challenge".to_string()),
                ("Site".to_string(), "playtak.com".to_string()),
                ("Player1".to_string(), game.white_player.to_string()),
                ("Player2".to_string(), game.black_player.to_string()),
                ("Size".to_string(), S.to_string()),
                (
                    "Date".to_string(),
                    format!("{}.{:0>2}.{:0>2}", date.year(), date.month(), date.day()),
                ),
            ];

            let game = Game {
                start_position: <Board<S>>::start_position(),
                moves: moves.clone(),
                game_result: board.game_result(),
                tags,
            };

            let mut ptn = Vec::new();

            game.game_to_ptn(&mut ptn)?;

            info!("{}", String::from_utf8(ptn).unwrap());
        }

        let mut move_list = vec![];

        for PtnMove { mv, .. } in moves {
            move_list.push(mv.to_string::<S>());
        }

        info!("Move list: {}", move_list.join(" "));

        self.seek_game::<S>(seek_mode)
    }
}

fn connect() -> Result<BufStream<TcpStream>> {
    let connection = dial()?;
    Ok(connection)
}

fn dial() -> Result<BufStream<TcpStream>> {
    net::TcpStream::connect("playtak.com:10000").map(BufStream::new)
}

use std::cmp::Ordering;
use std::iter;

use chrono::{Datelike, Local};
use std::convert::Infallible;
use tiltak::position;
use tiltak::position::{Direction, Move, Movement, Role, StackMovement};
use tiltak::ptn::{Game, PtnMove};
#[cfg(not(feature = "aws-lambda-client"))]
use tiltak::search;
#[cfg(not(feature = "aws-lambda-client"))]
use tiltak::search::MctsSetting;

pub fn parse_move<const S: usize>(input: &str) -> position::Move {
    let words: Vec<&str> = input.split_whitespace().collect();
    if words[0] == "P" {
        let square = position::Square::parse_square::<S>(&words[1].to_lowercase()).unwrap();
        let role = match words.get(2) {
            Some(&"C") => Role::Cap,
            Some(&"W") => Role::Wall,
            None => Role::Flat,
            Some(s) => panic!("Unknown role {} for move {}", s, input),
        };
        position::Move::Place(role, square)
    } else if words[0] == "M" {
        let start_square = position::Square::parse_square::<S>(&words[1].to_lowercase()).unwrap();
        let end_square = position::Square::parse_square::<S>(&words[2].to_lowercase()).unwrap();
        let pieces_dropped: Vec<u8> = words
            .iter()
            .skip(3)
            .map(|s| u8::from_str(s).unwrap())
            .collect();

        let num_pieces_taken: u8 = pieces_dropped.iter().sum();

        let mut pieces_held = num_pieces_taken;

        let pieces_taken: StackMovement = iter::once(num_pieces_taken)
            .chain(
                pieces_dropped
                    .iter()
                    .take(pieces_dropped.len() - 1)
                    .map(|pieces_to_drop| {
                        pieces_held -= pieces_to_drop;
                        pieces_held
                    }),
            )
            .map(|pieces_to_take| Movement { pieces_to_take })
            .collect();

        let direction = match (
            start_square.rank::<S>().cmp(&end_square.rank::<S>()),
            start_square.file::<S>().cmp(&end_square.file::<S>()),
        ) {
            (Ordering::Equal, Ordering::Less) => Direction::East,
            (Ordering::Equal, Ordering::Greater) => Direction::West,
            (Ordering::Less, Ordering::Equal) => Direction::South,
            (Ordering::Greater, Ordering::Equal) => Direction::North,
            _ => panic!("Diagonal move string {}", input),
        };

        position::Move::Move(start_square, direction, pieces_taken)
    } else {
        unreachable!()
    }
}

pub fn write_move<const S: usize>(mv: position::Move, w: &mut String) {
    match mv {
        position::Move::Place(role, square) => {
            let role_string = match role {
                Role::Flat => "",
                Role::Wall => " W",
                Role::Cap => " C",
            };
            let square_string = square.to_string::<S>().to_uppercase();
            write!(w, "P {}{}", square_string, role_string).unwrap();
        }
        Move::Move(start_square, direction, stack_movement) => {
            let mut end_square = start_square;
            let mut pieces_held = stack_movement.get(0).pieces_to_take;
            let pieces_to_leave: Vec<u8> = stack_movement
                .into_iter()
                .skip(1)
                .map(|Movement { pieces_to_take }| {
                    end_square = end_square.go_direction::<S>(direction).unwrap();
                    let pieces_to_leave = pieces_held - pieces_to_take;
                    pieces_held = pieces_to_take;
                    pieces_to_leave
                })
                .collect();

            end_square = end_square.go_direction::<S>(direction).unwrap();

            write!(
                w,
                "M {} {} ",
                start_square.to_string::<S>().to_uppercase(),
                end_square.to_string::<S>().to_uppercase()
            )
            .unwrap();
            for num_to_leave in pieces_to_leave {
                write!(w, "{} ", num_to_leave).unwrap();
            }
            write!(w, "{}", pieces_held).unwrap();
        }
    }
}

use std::convert::Infallible;
use std::io::{BufRead, Result, Write};
use std::net::TcpStream;
use std::str::FromStr;
use std::time::Duration;
#[cfg(feature = "aws-lambda-client")]
use std::time::Instant;
use std::{io, net, thread};

use board_game_traits::{Color, GameResult, Position as PositionTrait};
use bufstream::BufStream;
use chrono::{Datelike, Local};
use clap::{Arg, ArgAction, Command};
use log::error;
use log::{debug, info, warn};
use pgn_traits::PgnPosition;

use rand::seq::SliceRandom;
use rand::Rng;
#[cfg(feature = "aws-lambda-client")]
use tiltak::aws;
use tiltak::position;
use tiltak::position::{squares_iterator, Move, Role, Square};
use tiltak::position::{Komi, Position};
use tiltak::ptn::{Game, PtnMove};
use tiltak::search;
use tiltak::search::MctsSetting;

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct PlaytakSettings {
    default_seek_size: usize,
    default_seek_color: Option<Color>,
    allow_choosing_size: bool,
    allow_choosing_color: bool,
    fixed_nodes: Option<u64>,
    dirichlet_noise: Option<f32>,
    rollout_depth: u16,
    rollout_temperature: f64,
    seek_game_time: Duration,
    seek_increment: Duration,
    seek_unrated: bool,
    target_move_time: Option<Duration>,
    komi: Komi,
}

impl PlaytakSettings {
    pub fn to_mcts_setting<const S: usize>(&self) -> MctsSetting<S> {
        if let Some(dirichlet) = self.dirichlet_noise {
            MctsSetting::default()
                .add_dirichlet(dirichlet)
                .add_rollout_depth(self.rollout_depth)
                .add_rollout_temperature(self.rollout_temperature)
        } else {
            MctsSetting::default()
                .add_rollout_depth(self.rollout_depth)
                .add_rollout_temperature(self.rollout_temperature)
        }
    }
}

pub fn main() -> Result<()> {
    let mut app = Command::new("Tiltak playtak client")
        .version("0.1")
        .author("Morten Lohne")
        .arg(
            Arg::new("username")
                .requires("password")
                .short('u')
                .long("username")
                .env("PLAYTAK_USERNAME")
                .value_name("USER")
                .help("playtak.com username")
                .num_args(1),
        )
        .arg(
            Arg::new("password")
                .short('p')
                .long("password")
                .env("PLAYTAK_PASSWORD")
                .value_name("PASS")
                .help("playtak.com password")
                .num_args(1),
        )
        .arg(
            Arg::new("size")
                .short('s')
                .long("size")
                .env("SIZE")
                .help("Board size")
                .num_args(1)
                .default_value("5")
                .value_parser(clap::value_parser!(u64).range(4..=8)),
        )
        .arg(
            Arg::new("logfile")
                .short('l')
                .long("logfile")
                .env("LOGFILE")
                .value_name("tiltak.log")
                .help("Name of debug logfile")
                .num_args(1),
        )
        .arg(
            Arg::new("playBot")
                .long("play-bot")
                .env("PLAY_BOT")
                .value_name("botname")
                .help("Instead of seeking any game, accept any seek from the specified bot. Mutually exclusive with --tc")
                .conflicts_with("tc")
                .num_args(1)
                .required(true),

        )
        .arg(
            Arg::new("tc")
                 .long("tc")
                 .env("TC")
                 .help("Time control to seek games for. Mutually exclusive with --play-bot")
                 .conflicts_with("playBot")
                 .num_args(1)
                 .required(true),
        )
        .arg(
            Arg::new("targetMoveTime")
                .long("target-move-time")
                .env("TARGET_MOVE_TIME")
                .conflicts_with("fixedNodes")
                .help("Try spending no more than this number of seconds per move. Will occasionally search longer, assuming the time control allows")
                .num_args(1))
        .arg(Arg::new("allowChoosingColor")
            .long("allow-choosing-color")
            .env("ALLOW_CHOOSING_COLOR")
            .help("Allow users to change the bot's seek color through chat")
            .action(ArgAction::SetTrue)
            .num_args(0))
        .arg(Arg::new("allowChoosingSize")
            .long("allow-choosing-size")
            .env("ALLOW_CHOOSING_SIZE")
            .help("Allow users to change the bot's board size through chat")
            .action(ArgAction::SetTrue)
            .num_args(0))
        .arg(Arg::new("seekColor")
            .long("seek-color")
            .env("SEEK_COLOR")
            .help("Color of games to seek")
            .num_args(1)
            .value_parser(["white", "black", "either"])
            .default_value("either"))
        .arg(Arg::new("policyNoise")
            .long("policy-noise")
            .env("POLICY_NOISE")
            .help("Add dirichlet noise to the policy scores of the root node in search. This gives the bot a small amount of randomness in its play, especially on low nodecounts.")
            .num_args(1)
            .value_parser(["none", "low", "medium", "high"])
            .default_value("none"))
        .arg(Arg::new("rolloutDepth")
            .long("rollout-depth")
            .env("ROLLOUT_DEPTH")
            .help("Depth of MCTS rollouts. Once a rollout reaches the maximum depth, the heuristic eval function is returned. Can be set to 0 to disable rollouts entirely.")
            .num_args(1)
            .default_value("0")
            .value_parser(clap::value_parser!(u16)))
        .arg(Arg::new("rolloutNoise")
            .long("rollout-noise")
            .env("ROLLOUT_NOISE")
            .help("Add a random component to move selection in MCTS rollouts. Has no effect if --rollout-depth is 0. For full rollouts, even the 'low' setting is enough to give highly variable play.")
            .num_args(1)
            .value_parser(["low", "medium", "high"])
            .default_value("low"))
        .arg(Arg::new("fixedNodes")
            .long("fixed-nodes")
            .env("FIXED_NODES")
            .help("Normally, the bot will search a variable number of nodes, depending on hardware on time control. This option overrides that to calculate a fixed amount of nodes each move")
            .num_args(1))
        .arg(Arg::new("komi")
            .long("komi")
            .env("KOMI")
            .help("Seek games with komi")
            .num_args(1)
            .default_value("0"))
        .arg(Arg::new("seekUnrated")
            .long("seek-unrated")
            .env("SEEK_UNRATED")
            .help("Seek unrated games")
            .action(ArgAction::SetTrue)
            .num_args(0))
        .arg(Arg::new("playtakBaseUrl")
            .long("playtak-base-url")
            .env("PLAYTAK_BASE_URL")
            .help("Base URL to connect to")
            .num_args(1)
            .default_value("playtak.com"))
        .arg(Arg::new("playtakPort")
            .long("playtak-url")
            .env("PLAYTAK_PORT")
            .help("Network port to connect to")
            .num_args(1)
            .default_value("10000")
            .value_parser(clap::value_parser!(u16)));

    if cfg!(feature = "aws-lambda-client") {
        app = app.arg(
            Arg::new("aws-function-name")
                .long("aws-function-name")
                .env("AWS_FUNCTION_NAME")
                .value_name("tiltak")
                .required(true)
                .conflicts_with("fixedNodes")
                .help(
                    "Run the engine on AWS instead of locally. Requires aws cli installed locally.",
                )
                .num_args(1),
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

    if let Some(log_file) = matches.get_one::<String>("logfile") {
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

    let size: usize = *matches.get_one::<u64>("size").unwrap() as usize;

    let allow_choosing_color = matches.get_flag("allowChoosingColor");
    let allow_choosing_size = matches.get_flag("allowChoosingSize");
    let default_seek_color = match matches.get_one::<String>("seekColor").unwrap().as_str() {
        "white" => Some(Color::White),
        "black" => Some(Color::Black),
        "either" => None,
        _ => unreachable!(),
    };

    let dirichlet_noise: Option<f32> =
        match matches.get_one::<String>("policyNoise").unwrap().as_ref() {
            "none" => None,
            "low" => Some(0.5),
            "medium" => Some(0.25),
            "high" => Some(0.1),
            s => panic!("policyNoise cannot be {}", s),
        };

    let rollout_depth: u16 = *matches.get_one::<u16>("rolloutDepth").unwrap();
    let rollout_temperature: f64 = match matches.get_one::<String>("rolloutNoise").unwrap().as_ref()
    {
        "low" => 0.2,
        "medium" => 0.3,
        "high" => 0.5,
        s => panic!("rolloutTemperature cannot be {}", s),
    };

    let fixed_nodes: Option<u64> = matches
        .get_one::<String>("fixedNodes")
        .map(|v| v.parse().unwrap());

    let tc = matches.get_one::<String>("tc").map(|tc| parse_tc(tc));

    let target_move_time: Option<Duration> = matches
        .get_one::<String>("targetMoveTime")
        .map(|v| v.parse().unwrap())
        .map(Duration::from_secs_f32);

    let komi = matches.get_one::<String>("komi").unwrap().parse().unwrap();

    let seek_unrated = matches.get_flag("seekUnrated");

    let playtak_base_url = matches.get_one::<String>("playtakBaseUrl").unwrap();
    let playtak_port = *matches.get_one::<u16>("playtakPort").unwrap();

    let playtak_url = format!("{}:{}", playtak_base_url, playtak_port);

    let playtak_settings = PlaytakSettings {
        allow_choosing_size,
        allow_choosing_color,
        default_seek_color,
        default_seek_size: size,
        fixed_nodes,
        dirichlet_noise,
        rollout_depth,
        rollout_temperature,
        seek_game_time: tc.unwrap_or_default().0,
        seek_increment: tc.unwrap_or_default().1,
        seek_unrated,
        target_move_time,
        komi,
    };

    loop {
        #[cfg(feature = "aws-lambda-client")]
        let connection_result =
            if let Some(aws_function_name) = matches.get_one::<String>("aws-function-name") {
                PlaytakSession::with_aws(&playtak_url, aws_function_name.to_string())
            } else {
                PlaytakSession::new(&playtak_url)
            };
        #[cfg(not(feature = "aws-lambda-client"))]
        let connection_result = PlaytakSession::new(&playtak_url);

        let mut session = match connection_result {
            Ok(ok) => ok,
            Err(err) => {
                warn!("Failed to connect due to \"{}\", retrying...", err);
                thread::sleep(Duration::from_secs(10));
                continue;
            }
        };

        if let (Some(user), Some(pwd)) = (
            matches.get_one::<String>("username"),
            matches.get_one::<String>("password"),
        ) {
            session.username = Some(user.to_string());
            session.login("Tiltak", user, pwd)?;
        } else {
            warn!("No username/password provided, logging in as guest");
            session.login_guest()?;
        }

        // Re-connect if we get disconnected from the server
        let error = match matches.get_one::<String>("playBot") {
            Some(bot_name) => {
                match match size {
                    4 => session.accept_seek::<4>(playtak_settings, bot_name),
                    5 => session.accept_seek::<5>(playtak_settings, bot_name),
                    6 => session.accept_seek::<6>(playtak_settings, bot_name),
                    s => panic!("Unsupported size {}", s),
                } {
                    Ok(_game) => return Ok(()),
                    Err(err) => err,
                }
            }
            None => match size {
                4..=6 => session.seek_playtak_games(playtak_settings),
                s => panic!("Unsupported size {}", s),
            }
            .unwrap_err(),
        };

        match error.kind() {
            io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::TimedOut => {
                warn!("Server connection interrupted, caused by \"{}\". This may be due to a server restart, attempting to reconnect.", error);
                thread::sleep(Duration::from_secs(2));
            }
            _ => {
                error!("Fatal error of kind {:?}: {}", error.kind(), error);
                return Err(error);
            }
        }
    }
}

pub fn parse_tc(input: &str) -> (Duration, Duration) {
    let mut parts = input.split('+');
    let time_part = parts.next().expect("Couldn't parse tc");
    let inc_part = parts.next();
    let time = Duration::from_millis((f64::from_str(time_part).unwrap() * 1000.0) as u64);

    let inc = if let Some(inc_part) = inc_part {
        Duration::from_millis((f64::from_str(inc_part).unwrap() * 1000.0) as u64)
    } else {
        Duration::default()
    };
    (time, inc)
}

struct ChatCommand<'a> {
    command: &'a str,
    argument: Option<&'a str>,
    response_command: &'a str,
    sender_name: &'a str,
}

impl<'a> ChatCommand<'a> {
    pub fn parse_engine_command(engine_name: &str, input: &'a str) -> Option<ChatCommand<'a>> {
        let mut words_iterator = input.split_whitespace();
        let response_command = words_iterator.next().unwrap();
        assert!(response_command == "Tell" || response_command == "Shout");
        let raw_name = words_iterator.next()?;
        // Strip < and > from name
        let sender_name = &raw_name[1..raw_name.len() - 1];
        if !words_iterator.next()?.starts_with(engine_name) {
            return None;
        }
        Some(ChatCommand {
            command: words_iterator.next()?,
            argument: words_iterator.next(),
            response_command,
            sender_name,
        })
    }

    pub fn respond(&self, session: &mut PlaytakSession, response: &str) -> Result<()> {
        let full_response = format!(
            "{} {} {}",
            self.response_command, self.sender_name, response
        );
        session.send_line(&full_response)
    }

    pub fn process_color_command(
        &self,
        session: &mut PlaytakSession,
    ) -> Result<Option<Option<Color>>> {
        let next_game_color = match self.argument {
            Some("white") => Some(Color::White),
            Some("black") => Some(Color::Black),
            Some("either") => None,
            s => {
                self.respond(
                    session,
                    &format!(
                        "Unknown color {}. Must be \"white\", \"black\" or \"either\"",
                        s.unwrap_or_default()
                    ),
                )?;
                return Ok(None);
            }
        };
        let color_string: String = next_game_color
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "either color".to_string());
        self.respond(session, &format!("Seeking next game with {}", color_string))?;
        Ok(Some(next_game_color))
    }

    pub fn process_size_command(&self, session: &mut PlaytakSession) -> Result<Option<usize>> {
        let next_game_size = match self.argument {
            Some("4") => 4,
            Some("5") => 5,
            Some("6") => 6,
            s => {
                self.respond(
                    session,
                    &format!(
                        "Unsupported size {}. Must be 4, 5 or 6",
                        s.unwrap_or_default()
                    ),
                )?;
                return Ok(None);
            }
        };
        self.respond(
            session,
            &format!("Seeking next game with size {}", next_game_size),
        )?;
        Ok(Some(next_game_size))
    }
}

struct PlaytakSession {
    username: Option<String>,
    #[cfg(feature = "aws-lambda-client")]
    aws_function_name: Option<String>,
    connection: BufStream<TcpStream>,
    // The server requires regular pings, to not kick the user
    // This thread does nothing but provide those pings
    ping_thread: Option<thread::JoinHandle<io::Result<()>>>,
}

#[derive(Debug, PartialEq, Eq)]
struct PlaytakGame<'a> {
    game_no: u64,
    size: usize,
    white_player: &'a str,
    black_player: &'a str,
    our_color: Color,
    time_left: Duration,
    increment: Duration,
    komi: Komi,
}

impl<'a> PlaytakGame<'a> {
    pub fn from_playtak_game_words(words: &[&'a str], increment: Duration) -> PlaytakGame<'a> {
        PlaytakGame {
            game_no: u64::from_str(words[2]).unwrap(),
            size: usize::from_str(words[3]).unwrap(),
            white_player: words[4],
            black_player: words[6],
            our_color: match words[7] {
                "white" => Color::White,
                "black" => Color::Black,
                color => panic!("Bad color \"{}\"", color),
            },
            time_left: Duration::from_secs(u64::from_str(words[8]).unwrap()),
            increment,
            komi: words
                .get(9)
                .map(|komi_str| Komi::from_half_komi(i8::from_str(komi_str).unwrap()).unwrap())
                .unwrap_or_default(),
        }
    }
}

impl PlaytakSession {
    /// Initialize a connection to playtak.com. Does not log in or play games.
    fn new(playtak_url: &str) -> Result<Self> {
        let connection = connect(playtak_url)?;
        let mut ping_thread_connection = connection.get_ref().try_clone()?;
        let ping_thread = Some(thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(30));
            writeln!(ping_thread_connection, "PING")?;
            ping_thread_connection.flush()?;
        }));
        Ok(PlaytakSession {
            username: None,
            #[cfg(feature = "aws-lambda-client")]
            aws_function_name: None,
            connection,
            ping_thread,
        })
    }

    #[cfg(feature = "aws-lambda-client")]
    fn with_aws(playtak_url: &str, aws_function_name: String) -> Result<Self> {
        let mut session = Self::new(playtak_url)?;
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

    fn send_seek(
        &mut self,
        playtak_settings: PlaytakSettings,
        size: usize,
        color: Option<Color>,
    ) -> Result<()> {
        self.send_line(&format!(
            "Seek {} {} {} {} {} {} {} {} 0 ",
            size,
            playtak_settings.seek_game_time.as_secs(),
            playtak_settings.seek_increment.as_secs(),
            match color {
                Some(Color::White) => "W",
                Some(Color::Black) => "B",
                None => "A",
            },
            playtak_settings.komi.half_komi(),
            position::starting_stones(size),
            position::starting_capstones(size),
            if playtak_settings.seek_unrated { 1 } else { 0 },
        ))
    }

    fn seek_playtak_games(&mut self, playtak_settings: PlaytakSettings) -> io::Result<Infallible> {
        let mut restoring_previous_session = true;
        let mut next_seek_color = playtak_settings.default_seek_color;
        let mut next_seek_size = playtak_settings.default_seek_size;

        loop {
            let input = self.read_line()?;
            let words: Vec<&str> = input.split_whitespace().collect();
            if words.is_empty() {
                continue;
            }
            match words[0] {
                "Game" => {
                    let playtak_game = PlaytakGame::from_playtak_game_words(
                        &words,
                        playtak_settings.seek_increment,
                    );
                    let (updated_seek_size, updated_seek_color) = match playtak_game.size {
                        4 => self.play_game::<4>(
                            playtak_game,
                            playtak_settings,
                            restoring_previous_session,
                        )?,
                        5 => self.play_game::<5>(
                            playtak_game,
                            playtak_settings,
                            restoring_previous_session,
                        )?,
                        6 => self.play_game::<6>(
                            playtak_game,
                            playtak_settings,
                            restoring_previous_session,
                        )?,
                        s => panic!("Unsupported size {}", s),
                    };
                    restoring_previous_session = false;
                    self.send_seek(playtak_settings, updated_seek_size, updated_seek_color)?;
                    next_seek_color = updated_seek_color;
                    next_seek_size = updated_seek_size;
                }
                "NOK" => {
                    warn!("Received NOK from server, ignoring. This may happen if the game was aborted while we were thinking");
                }
                "Tell" | "Shout" => {
                    if let Some(chat_command) = self
                        .username
                        .as_ref()
                        .and_then(|username| ChatCommand::parse_engine_command(username, &input))
                    {
                        if chat_command.command == "color" {
                            if !playtak_settings.allow_choosing_color {
                                chat_command.respond(self, "Cannot choose color for this bot")?
                            } else if let Some(updated_seek_color) =
                                chat_command.process_color_command(self)?
                            {
                                next_seek_color = updated_seek_color;
                                self.send_seek(playtak_settings, next_seek_size, next_seek_color)?;
                            }
                        } else if chat_command.command == "size" {
                            if !playtak_settings.allow_choosing_size {
                                chat_command.respond(self, "Cannot choose size for this bot")?
                            } else if let Some(updated_seek_size) =
                                chat_command.process_size_command(self)?
                            {
                                next_seek_size = updated_seek_size;
                                self.send_seek(playtak_settings, next_seek_size, next_seek_color)?;
                            }
                        } else {
                            chat_command.respond(self, "Unknown command")?
                        }
                    }
                }
                _ => {
                    if restoring_previous_session {
                        debug!("No longer restoring previous session");
                        restoring_previous_session = false;
                        self.send_seek(playtak_settings, next_seek_size, next_seek_color)?;
                    }
                    debug!("Ignoring server message \"{}\"", input.trim())
                }
            }
        }
    }

    pub fn accept_seek<const S: usize>(
        &mut self,
        playtak_settings: PlaytakSettings,
        bot_name: &str,
    ) -> io::Result<()> {
        // The server doesn't send increment when the game starts
        // We have to keep track of it ourselves, depending on the seekmode
        let mut increment = Duration::from_secs(0);
        loop {
            let input = self.read_line()?;
            let words: Vec<&str> = input.split_whitespace().collect();
            if words.is_empty() {
                continue;
            }
            match words[0] {
                "Game" => {
                    let playtak_game = PlaytakGame::from_playtak_game_words(&words, increment);
                    self.play_game::<S>(playtak_game, playtak_settings, false)?;
                    return Ok(());
                }

                "Seek" => {
                    if words[1] == "new" {
                        let number = u64::from_str(words[2]).unwrap();
                        let name = words[3];
                        let inc = Duration::from_secs(u64::from_str(words[6]).unwrap());
                        if name.eq_ignore_ascii_case(bot_name) {
                            self.send_line(&format!("Accept {}", number))?;
                            increment = inc;
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
        game: PlaytakGame,
        playtak_settings: PlaytakSettings,
        mut restoring_previous_session: bool,
    ) -> io::Result<(usize, Option<Color>)> {
        info!(
            "Starting game #{}, {} vs {} as {}, {}+{:.1}, {} komi",
            game.game_no,
            game.white_player,
            game.black_player,
            game.our_color,
            game.time_left.as_secs(),
            game.increment.as_secs_f32(),
            game.komi
        );
        let mut next_seek_size = playtak_settings.default_seek_size;
        let mut next_seek_color = playtak_settings.default_seek_color;
        let mut position = <Position<S>>::start_position_with_komi(game.komi);
        let mut moves = vec![];
        let mut our_time_left = game.time_left;
        'gameloop: loop {
            if position.game_result().is_some() {
                // Double check that the game is still over, if we remove information about move repetitions
                // Playtak does not have this rule, so we want to play on, even if the position is a repetition
                let position_without_history = <Position<S>>::from_fen(&position.to_fen()).unwrap();
                if position_without_history.game_result().is_some() {
                    break;
                } else {
                    position = position_without_history;
                }
            }
            if position.side_to_move() == game.our_color && !restoring_previous_session {
                let (best_move, score) =
                    // On the very first move, always place instantly in a random corner
                    if squares_iterator::<S>().all(|square| position[square].is_empty()) {
                        let mut rng = rand::thread_rng();
                        let moves = vec![
                            Move::Place(Role::Flat, Square(0)),
                            Move::Place(Role::Flat, Square(S as u8 - 1)),
                            Move::Place(Role::Flat, Square((S * (S - 1)) as u8)),
                            Move::Place(Role::Flat, Square((S * S - 1) as u8)),
                        ];
                        (moves.choose(&mut rng).unwrap().clone(), 0.0)
                    } else if let Some(fixed_nodes) = playtak_settings.fixed_nodes {
                        let settings =
                            playtak_settings.to_mcts_setting()
                            .arena_size_for_nodes(fixed_nodes as u32);
                        let mut tree = search::MonteCarloTree::with_settings(position.clone(), settings);
                        for _ in 0..fixed_nodes {
                            if tree.select().is_none() {
                                eprintln!("Warning: Search stopped early due to OOM");
                                break;
                            };
                        }

                        // Wait for a bit
                        let mut rng = rand::thread_rng();
                        let sleep_duration = Duration::from_millis(rng.gen_range(1000..2500));
                        thread::sleep(sleep_duration);

                        tree.best_move()
                    } else {
                        #[cfg(feature = "aws-lambda-client")]
                        {
                            let aws_function_name = self.aws_function_name.as_ref().unwrap();
                            let start_time = Instant::now();
                            let event = aws::Event {
                                size: S,
                                tps: None,
                                moves: moves
                                    .iter()
                                    .map(|PtnMove { mv, .. }: &PtnMove<Move>| mv.to_string::<S>())
                                    .collect(),
                                time_control: search::TimeControl::Time(our_time_left, game.increment),
                                komi: position.komi().into(),
                                eval_komi: None,
                                dirichlet_noise: playtak_settings.dirichlet_noise,
                                rollout_depth: playtak_settings.rollout_depth,
                                rollout_temperature: playtak_settings.rollout_temperature,
                            };
                            let aws::Output { pv, score, nodes, mem_usage, time_taken } =
                                aws::client::best_move_aws(aws_function_name, &event)?;

                            debug!("{} nodes, {}MB, {:.1}s taken, {}ms overhead",
                                nodes,
                                mem_usage / (1024 * 1024),
                                time_taken.as_secs_f32(),
                                (start_time.elapsed() - time_taken).as_millis()
                            );

                            (position.move_from_san(&pv[0]).unwrap(), score)
                        }

                        #[cfg(not(feature = "aws-lambda-client"))]
                        {
                            let maximum_time = if let Some(target_move_time) =  playtak_settings.target_move_time {
                                (our_time_left / 6 + game.increment / 2).min(2 * target_move_time)
                            } else {
                                our_time_left / 6 + game.increment / 2
                            };

                            // Give enough memory for a CPU calculating at 500K nps.
                            let max_nodes = (maximum_time.as_secs() as u32).saturating_mul(500_000);

                            // For 6s, the toughest position I've found required 40 elements/node searched
                            // This formula gives 108, which is hopefully plenty
                            let max_arena_size = (S * S) as u32 * 3 * max_nodes;

                            let settings =
                                playtak_settings.to_mcts_setting()
                                .arena_size(max_arena_size.min(2_u32.pow(31)));

                            search::play_move_time(position.clone(), maximum_time, settings)
                        }
                    };

                position.do_move(best_move.clone());
                moves.push(PtnMove {
                    mv: best_move.clone(),
                    annotations: vec![],
                    comment: score.to_string(),
                });

                let output_string = format!(
                    "Game#{} {}",
                    game.game_no,
                    best_move.to_string_playtak::<S>()
                );
                self.send_line(&output_string)?;

                // Say "Tak" whenever there is a threat to win
                // Only do this vs Shigewara
                if game.white_player == "shigewara" || game.black_player == "shigewara" {
                    let mut position_clone = position.clone();
                    position_clone.null_move();
                    let mut moves = vec![];
                    position_clone.generate_moves(&mut moves);
                    for mv in moves {
                        let reverse_move = position_clone.do_move(mv);
                        match (position_clone.side_to_move(), position_clone.game_result()) {
                            (Color::White, Some(GameResult::BlackWin))
                            | (Color::Black, Some(GameResult::WhiteWin)) => {
                                self.send_line("Tell shigewara Tak!")?;
                                break;
                            }
                            _ => (),
                        }
                        position_clone.reverse_move(reverse_move);
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
                    if line.trim() == "Message Your game is resumed" {
                        restoring_previous_session = false;
                        break;
                    } else if matches!(words[0], "Shout" | "Tell") {
                        if let Some(chat_command) = self
                            .username
                            .as_ref()
                            .and_then(|username| ChatCommand::parse_engine_command(username, &line))
                        {
                            if chat_command.command == "color" {
                                if !playtak_settings.allow_choosing_color {
                                    chat_command
                                        .respond(self, "Cannot choose color for this bot")?
                                } else if let Some(updated_seek_color) =
                                    chat_command.process_color_command(self)?
                                {
                                    next_seek_color = updated_seek_color;
                                }
                            } else if chat_command.command == "size" {
                                if !playtak_settings.allow_choosing_size {
                                    chat_command.respond(self, "Cannot choose size for this bot")?
                                } else if let Some(updated_seek_size) =
                                    chat_command.process_size_command(self)?
                                {
                                    next_seek_size = updated_seek_size;
                                }
                            } else {
                                chat_command.respond(self, "Unknown command")?
                            }
                        }
                    } else if words[0] == format!("Game#{}", game.game_no) {
                        match words[1] {
                            "P" | "M" => {
                                let move_string = words[1..].join(" ");
                                let move_played = Move::from_string_playtak::<S>(&move_string);
                                position.do_move(move_played.clone());
                                moves.push(PtnMove {
                                    mv: move_played,
                                    annotations: vec![],
                                    comment: "0.0".to_string(),
                                });
                                break;
                            }
                            "Time" => {
                                let white_time_left =
                                    Duration::from_secs(u64::from_str(words[2]).unwrap());
                                let black_time_left =
                                    Duration::from_secs(u64::from_str(words[3]).unwrap());
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

        info!("Game finished. Pgn: ");

        let date = Local::now();

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
            ("Komi".to_string(), position.komi().to_string()),
        ];

        let game = Game {
            start_position: <Position<S>>::start_position(),
            moves: moves.clone(),
            game_result_str: position.pgn_game_result(),
            tags,
        };

        let mut ptn = Vec::new();

        game.game_to_ptn(&mut ptn)?;

        info!("{}", String::from_utf8(ptn).unwrap());

        let mut move_list = vec![];
        for PtnMove { mv, .. } in moves {
            move_list.push(mv.to_string::<S>());
        }
        info!("Move list: {}", move_list.join(" "));

        Ok((next_seek_size, next_seek_color))
    }
}

fn connect(playtak_url: &str) -> Result<BufStream<TcpStream>> {
    let connection = dial(playtak_url)?;
    Ok(connection)
}

fn dial(playtak_url: &str) -> Result<BufStream<TcpStream>> {
    net::TcpStream::connect(playtak_url).map(BufStream::new)
}

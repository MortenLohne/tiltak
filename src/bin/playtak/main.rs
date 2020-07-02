use board_game_traits::board::Color;
use bufstream::BufStream;
use std::io::{BufRead, Result, Write};
use std::net::TcpStream;
use std::str::FromStr;
use std::time::Duration;
use std::{io, net, thread};

pub fn main() -> Result<()> {
    let mut input = String::new();

    print!("Username: ");
    io::stdout().flush()?;
    io::stdin().read_line(&mut input)?;
    let user = input.trim().to_string();
    input.clear();

    print!("Password: ");
    io::stdout().flush()?;
    io::stdin().read_line(&mut input)?;
    let pwd = input.trim();

    let mut session = PlaytakSession::new()?;
    session.login("Taik", &user, &pwd)?;
    session.wait_for_game()?;
    Ok(())
}

struct PlaytakSession {
    connection: BufStream<TcpStream>,
    ping_thread: thread::JoinHandle<io::Result<()>>,
}

impl PlaytakSession {
    fn new() -> Result<Self> {
        let connection = connect()?;
        let mut ping_thread_connection = connection.get_ref().try_clone()?;
        let ping_thread = thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(30));
            writeln!(ping_thread_connection, "PING")?;
            ping_thread_connection.flush()?;
        });
        Ok(PlaytakSession {
            connection,
            ping_thread,
        })
    }

    fn login(&mut self, client_name: &str, user: &str, pwd: &str) -> Result<()> {
        loop {
            let line = self.read_line()?;
            if line.starts_with("Login") {
                break;
            }
        }
        println!("Logging in: ");
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

    fn read_line(&mut self) -> Result<String> {
        let mut input = String::new();
        self.connection.read_line(&mut input)?;
        println!("> {}", input.trim());
        Ok(input)
    }

    fn send_line(&mut self, output: &str) -> Result<()> {
        writeln!(self.connection, "{}", output)?;
        self.connection.flush()?;
        println!("< {}", output);
        Ok(())
    }

    pub fn wait_for_game(&mut self) -> io::Result<()> {
        self.send_line("Seek 5 900 10")?;

        loop {
            let input = self.read_line()?;
            let words: Vec<&str> = input.split_whitespace().collect();

            match words[0] {
                "Game" => {
                    let game_no: u64 = u64::from_str(words[2]).unwrap();
                    let board_size = usize::from_str(words[3]).unwrap();
                    let white_player = words[4];
                    let black_player = words[6];
                    let color = match words[7] {
                        "white" => Color::White,
                        "black" => Color::Black,
                        color => panic!("Bad color \"{}\"", color),
                    };
                    self.start_game(game_no, board_size, white_player, black_player, color)?;
                    return Ok(());
                }
                _ => println!("Unrecognized message \"{}\"", input.trim()),
            }
        }
    }

    fn start_game(
        &mut self,
        game_no: u64,
        board_size: usize,
        white_player: &str,
        black_player: &str,
        color: Color,
    ) -> io::Result<()> {
        println!(
            "Starting game #{}, {} vs {} as {}",
            game_no, white_player, black_player, color
        );
        self.send_line("quit")
    }
}

fn connect() -> Result<BufStream<TcpStream>> {
    let connection = dial()?;
    Ok(connection)
}

fn dial() -> Result<BufStream<TcpStream>> {
    net::TcpStream::connect("playtak.com:10000").map(BufStream::new)
}

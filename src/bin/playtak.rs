use board_game_traits::board::{Board as BoardTrait, Color};
use bufstream::BufStream;
use std::fmt::Write as FmtWrite;
use std::io::{BufRead, Result, Write};
use std::net::TcpStream;
use std::str::FromStr;
use std::time::Duration;
use std::{io, net, thread};
use taik::board::Board;
use taik::mcts;

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
        our_color: Color,
    ) -> io::Result<()> {
        println!(
            "Starting game #{}, {} vs {} as {}",
            game_no, white_player, black_player, our_color
        );
        let mut board = Board::start_board();
        let mut moves = vec![];
        loop {
            if board.game_result().is_some() {
                break;
            }
            if board.side_to_move() == our_color {
                let (best_move, score) = mcts::mcts(board.clone(), 1_000_000);
                board.do_move(best_move.clone());
                moves.push((best_move.clone(), score));
                let mut output_string = format!("Game{} ", game_no);
                write_move(best_move, &mut output_string);
                self.send_line(&output_string)?;
            } else {
            }
        }
        self.wait_for_game()
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

use arrayvec::ArrayVec;
use taik::board;
use taik::board::{Direction, Move, Movement, Role, StackMovement};

pub fn parse_move(input: &str) -> board::Move {
    let words: Vec<&str> = input.split_whitespace().collect();
    if words[0] == "P" {
        let square = board::Square::parse_square(words[1]);
        let role = match words.get(2) {
            Some(&"C") => Role::Cap,
            Some(&"W") => Role::Standing,
            None => Role::Flat,
            Some(s) => panic!("Unknown role {} for move {}", s, input),
        };
        board::Move::Place(role, square)
    } else if words[0] == "M" {
        let start_square = board::Square::parse_square(words[1]);
        let end_square = board::Square::parse_square(words[2]);
        let pieces_dropped: ArrayVec<[u8; board::BOARD_SIZE - 1]> = words
            .iter()
            .skip(3)
            .map(|s| u8::from_str(s).unwrap())
            .collect();

        let num_pieces_taken: u8 = pieces_dropped.iter().sum();

        let mut pieces_held = num_pieces_taken;

        let pieces_taken: ArrayVec<[Movement; board::BOARD_SIZE - 1]> =
            iter::once(num_pieces_taken)
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
            start_square.rank().cmp(&end_square.rank()),
            start_square.file().cmp(&end_square.file()),
        ) {
            (Ordering::Equal, Ordering::Less) => Direction::East,
            (Ordering::Equal, Ordering::Greater) => Direction::West,
            (Ordering::Less, Ordering::Equal) => Direction::South,
            (Ordering::Greater, Ordering::Equal) => Direction::North,
            _ => panic!("Diagonal move string {}", input),
        };

        board::Move::Move(
            start_square,
            direction,
            StackMovement {
                movements: pieces_taken,
            },
        )
    } else {
        unreachable!()
    }
}

pub fn write_move(mv: board::Move, w: &mut String) {
    match mv {
        board::Move::Place(Role::Flat, square) => write!(w, "P {}", square).unwrap(),
        board::Move::Place(Role::Standing, square) => write!(w, "P {} W", square).unwrap(),
        board::Move::Place(Role::Cap, square) => write!(w, "P {} C", square).unwrap(),
        Move::Move(start_square, direction, stack_movement) => {
            let mut end_square = start_square;
            let mut pieces_held = stack_movement.movements[0].pieces_to_take;
            let pieces_to_leave: Vec<u8> = stack_movement
                .movements
                .iter()
                .skip(1)
                .map(|Movement { pieces_to_take }| {
                    end_square = end_square.go_direction(direction).unwrap();
                    let pieces_to_leave = pieces_held - pieces_to_take;
                    pieces_held = *pieces_to_take;
                    pieces_to_leave
                })
                .collect();

            end_square = end_square.go_direction(direction).unwrap();

            write!(w, "M {} {} ", start_square, end_square).unwrap();
            for num_to_leave in pieces_to_leave {
                write!(w, "{} ", num_to_leave).unwrap();
            }
            write!(w, "{}", pieces_held).unwrap();
        }
    }
}

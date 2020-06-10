use arrayvec::ArrayVec;
use bufstream::BufStream;
use std::cmp::Ordering;
use std::io::{BufRead, Result, Write};
use std::net;
use std::net::TcpStream;
use std::str::FromStr;
use taik::board;
use taik::board::{Direction, Movement, Role, StackMovement, BOARD_SIZE};

pub fn main() -> Result<()> {
    Ok(())
}

pub fn connect(client_name: &str, user: &str, pwd: &str) -> Result<BufStream<TcpStream>> {
    let mut connection = dial()?;
    login(&mut connection, client_name, user, pwd)?;
    Ok(connection)
}

fn dial() -> Result<BufStream<TcpStream>> {
    net::TcpStream::connect("playtak.com:10000").map(BufStream::new)
}

fn login(conn: &mut BufStream<TcpStream>, client_name: &str, user: &str, pwd: &str) -> Result<()> {
    let mut line = String::new();
    loop {
        conn.read_line(&mut line)?;
        println!("{}", line);
        if line.starts_with("Login") {
            break;
        }
        line.clear();
    }
    println!("Logging in: ");
    writeln!(conn, "client {}", client_name)?;
    writeln!(conn, "Login {} {}", user, pwd)?;
    conn.flush()?;
    Ok(())
}

fn parse_game_command(input: &str) {}

fn parse_move(input: &str) -> board::Move {
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
        let pieces_dropped: ArrayVec<[Movement; BOARD_SIZE - 1]> = words
            .iter()
            .skip(3)
            .map(|s| board::Movement {
                pieces_to_take: u8::from_str(s).unwrap(),
            })
            .collect();

        let direction = match (
            start_square.rank().cmp(&end_square.rank()),
            start_square.file().cmp(&end_square.file()),
        ) {
            (Ordering::Equal, Ordering::Less) => Direction::West,
            (Ordering::Equal, Ordering::Greater) => Direction::East,
            (Ordering::Less, Ordering::Equal) => Direction::North,
            (Ordering::Greater, Ordering::Equal) => Direction::South,
            _ => panic!("Diagonal move string {}", input),
        };

        board::Move::Move(
            start_square,
            direction,
            StackMovement {
                movements: pieces_dropped,
            },
        )
    } else {
        unreachable!()
    }
}

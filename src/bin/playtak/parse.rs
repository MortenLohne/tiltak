use arrayvec::ArrayVec;
use std::cmp::Ordering;
use std::str::FromStr;
use taik::board;
use taik::board::{Direction, Movement, Role, StackMovement};

pub fn parse_game_command(input: &str) {}

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
        let pieces_dropped: ArrayVec<[Movement; board::BOARD_SIZE - 1]> = words
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

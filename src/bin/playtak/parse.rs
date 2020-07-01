use std::cmp::Ordering;
use std::str::FromStr;
use std::iter;

use arrayvec::ArrayVec;
use std::fmt::Write;
use taik::board;
use taik::board::{Direction, Move, Movement, Role, StackMovement};

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

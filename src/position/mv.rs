#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::fmt::Write;

use crate::position::utils::Role::{Cap, Flat, Wall};
use crate::position::utils::{Role, Square};
use crate::position::Direction::{East, North, South, West};
use crate::position::{Direction, Movement, StackMovement};

/// A legal move for a position.
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Move {
    Place(Role, Square),
    Move(Square, Direction, StackMovement), // Number of stones to take
}

impl Move {
    pub fn to_string<const S: usize>(&self) -> String {
        let mut string = String::new();
        match self {
            Move::Place(role, square) => match role {
                Cap => write!(string, "C{}", square.to_string::<S>()).unwrap(),
                Flat => write!(string, "{}", square.to_string::<S>()).unwrap(),
                Wall => write!(string, "S{}", square.to_string::<S>()).unwrap(),
            },
            Move::Move(square, direction, stack_movements) => {
                let mut pieces_held = stack_movements.get(0).pieces_to_take;
                if pieces_held == 1 {
                    write!(string, "{}", square.to_string::<S>()).unwrap();
                } else {
                    write!(string, "{}{}", pieces_held, square.to_string::<S>()).unwrap();
                }
                match direction {
                    North => string.push('+'),
                    West => string.push('<'),
                    East => string.push('>'),
                    South => string.push('-'),
                }
                // Omit number of pieces dropped, if all stones are dropped immediately
                if stack_movements.len() > 1 {
                    for movement in stack_movements.into_iter().skip(1) {
                        let pieces_to_drop = pieces_held - movement.pieces_to_take;
                        write!(string, "{}", pieces_to_drop).unwrap();
                        pieces_held -= pieces_to_drop;
                    }
                    write!(string, "{}", pieces_held).unwrap();
                }
            }
        }
        string
    }

    pub fn from_string<const S: usize>(input: &str) -> Result<Self, pgn_traits::Error> {
        if input.len() < 2 {
            return Err(pgn_traits::Error::new(
                pgn_traits::ErrorKind::ParseError,
                "Input move too short.",
            ));
        }
        if !input.is_ascii() {
            return Err(pgn_traits::Error::new(
                pgn_traits::ErrorKind::ParseError,
                "Input move contained non-ascii characters.",
            ));
        }
        let first_char = input.chars().next().unwrap();
        match first_char {
            'a'..='h' if input.len() == 2 => {
                let square = Square::parse_square::<S>(input)?;
                Ok(Move::Place(Flat, square))
            }
            'a'..='h' if input.len() == 3 => {
                let square = Square::parse_square::<S>(&input[0..2])?;
                let direction = Direction::parse(input.chars().nth(2).unwrap());
                // Moves in the simplified move notation always move one piece
                let mut movement = StackMovement::new();
                movement.push(Movement { pieces_to_take: 1 });
                Ok(Move::Move(square, direction, movement))
            }
            'C' if input.len() == 3 => {
                Ok(Move::Place(Cap, Square::parse_square::<S>(&input[1..])?))
            }
            'S' if input.len() == 3 => {
                Ok(Move::Place(Wall, Square::parse_square::<S>(&input[1..])?))
            }
            '1'..='8' if input.len() > 3 => {
                let square = Square::parse_square::<S>(&input[1..3])?;
                let direction = Direction::parse(input.chars().nth(3).unwrap());
                let pieces_taken = first_char.to_digit(10).unwrap() as u8;
                let mut pieces_held = pieces_taken;

                let mut amounts_to_drop: Vec<u8> = input
                    .chars()
                    .skip(4)
                    .map(|ch| ch.to_digit(10).map(|i| i as u8))
                    .collect::<Option<Vec<u8>>>()
                    .ok_or_else(|| {
                        pgn_traits::Error::new_parse_error(
                            format!("Couldn't parse move \"{}\": found non-integer when expecting number of pieces to drop", input
                        ))
                    })?;
                amounts_to_drop.pop(); //

                let mut movements = StackMovement::new();
                movements.push(Movement {
                    pieces_to_take: pieces_taken,
                });

                for amount_to_drop in amounts_to_drop {
                    movements.push(Movement {
                        pieces_to_take: pieces_held - amount_to_drop,
                    });
                    pieces_held -= amount_to_drop;
                }
                Ok(Move::Move(square, direction, movements))
            }
            _ => Err(pgn_traits::Error::new(
                pgn_traits::ErrorKind::ParseError,
                format!(
                    "Couldn't parse move \"{}\". Moves cannot start with {} and have length {}.",
                    input,
                    first_char,
                    input.len()
                ),
            )),
        }
    }
}

/// The counterpart of `Move`. When applied to a `Board`, it fully reverses the accompanying `Move`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ReverseMove {
    Place(Square),
    Move(Square, Direction, StackMovement, bool),
}

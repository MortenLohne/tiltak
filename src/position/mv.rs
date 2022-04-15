use std::fmt::Write;
use std::iter;
use std::str::FromStr;

use arrayvec::ArrayVec;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::position::utils;
use crate::position::utils::Direction::{East, North, South, West};
use crate::position::utils::Role::{Cap, Flat, Wall};
use crate::position::utils::{Direction, Movement, Role, Square, StackMovement};
use std::cmp::Ordering;

/// A legal move for a position.
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Move {
    Place(Role, Square),
    Move(Square, Direction, StackMovement), // Number of stones to take
}

impl Move {
    pub fn origin_square(&self) -> Square {
        match self {
            Move::Place(_, square) => *square,
            Move::Move(square, _, _) => *square,
        }
    }

    pub fn to_string<const S: usize>(&self) -> String {
        let mut string = String::new();
        match self {
            Move::Place(role, square) => match role {
                Cap => write!(string, "C{}", square.to_string::<S>()).unwrap(),
                Flat => write!(string, "{}", square.to_string::<S>()).unwrap(),
                Wall => write!(string, "S{}", square.to_string::<S>()).unwrap(),
            },
            Move::Move(square, direction, stack_movements) => {
                let mut pieces_held = stack_movements.get::<S>(0).pieces_to_take;
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
                    for movement in stack_movements.into_iter::<S>().skip(1) {
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
                let direction = Direction::parse(input.chars().nth(2).unwrap())
                    .ok_or_else(|| pgn_traits::Error::new_parse_error("Bad direction"))?;
                // Moves in the simplified move notation always move one piece
                let mut movement = StackMovement::new();
                movement.push::<S>(Movement { pieces_to_take: 1 }, 1);
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
                let direction = Direction::parse(input.chars().nth(3).unwrap())
                    .ok_or_else(|| pgn_traits::Error::new_parse_error("Bad direction"))?;
                let pieces_taken = first_char.to_digit(10).unwrap() as u8;
                if pieces_taken as usize > S {
                    return Err(pgn_traits::Error::new_parse_error(format!(
                        "{} too large for {}s",
                        input, S
                    )));
                }
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
                amounts_to_drop.pop();

                let mut movements = StackMovement::new();
                movements.push::<S>(
                    Movement {
                        pieces_to_take: pieces_taken,
                    },
                    S as u8,
                );

                for amount_to_drop in amounts_to_drop {
                    movements.push::<S>(
                        Movement {
                            pieces_to_take: pieces_held - amount_to_drop,
                        },
                        pieces_held,
                    );
                    pieces_held -= amount_to_drop;
                }

                // Finish the movement
                movements.push::<S>(Movement { pieces_to_take: 0 }, pieces_held);

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

    pub fn to_string_playtak<const S: usize>(&self) -> String {
        match self {
            Move::Place(role, square) => {
                let role_string = match role {
                    Role::Flat => "",
                    Role::Wall => " W",
                    Role::Cap => " C",
                };
                let square_string = square.to_string::<S>().to_uppercase();
                format!("P {}{}", square_string, role_string)
            }
            Move::Move(start_square, direction, stack_movement) => {
                let mut output = String::new();
                let mut end_square = *start_square;
                let mut pieces_held = stack_movement.get::<S>(0).pieces_to_take;
                let pieces_to_leave: Vec<u8> = stack_movement
                    .into_iter::<S>()
                    .skip(1)
                    .map(|Movement { pieces_to_take }| {
                        end_square = end_square.go_direction::<S>(*direction).unwrap();
                        let pieces_to_leave = pieces_held - pieces_to_take;
                        pieces_held = pieces_to_take;
                        pieces_to_leave
                    })
                    .collect();

                end_square = end_square.go_direction::<S>(*direction).unwrap();

                write!(
                    output,
                    "M {} {} ",
                    start_square.to_string::<S>().to_uppercase(),
                    end_square.to_string::<S>().to_uppercase()
                )
                .unwrap();
                for num_to_leave in pieces_to_leave {
                    write!(output, "{} ", num_to_leave).unwrap();
                }
                write!(output, "{}", pieces_held).unwrap();
                output
            }
        }
    }

    pub fn from_string_playtak<const S: usize>(input: &str) -> Self {
        let words: Vec<&str> = input.split_whitespace().collect();
        if words[0] == "P" {
            let square = Square::parse_square::<S>(&words[1].to_lowercase()).unwrap();
            let role = match words.get(2) {
                Some(&"C") => Role::Cap,
                Some(&"W") => Role::Wall,
                None => Role::Flat,
                Some(s) => panic!("Unknown role {} for move {}", s, input),
            };
            Move::Place(role, square)
        } else if words[0] == "M" {
            let start_square = utils::Square::parse_square::<S>(&words[1].to_lowercase()).unwrap();
            let end_square = utils::Square::parse_square::<S>(&words[2].to_lowercase()).unwrap();
            let pieces_dropped: Vec<u8> = words
                .iter()
                .skip(3)
                .map(|s| u8::from_str(s).unwrap())
                .collect();

            let num_pieces_taken: u8 = pieces_dropped.iter().sum();

            let mut pieces_held = num_pieces_taken;

            let pieces_taken: StackMovement = StackMovement::from_movements::<S, _>(
                iter::once(num_pieces_taken)
                    .chain(pieces_dropped.iter().take(pieces_dropped.len() - 1).map(
                        |pieces_to_drop| {
                            pieces_held -= pieces_to_drop;
                            pieces_held
                        },
                    ))
                    .map(|pieces_to_take| Movement { pieces_to_take }),
            );

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

            Move::Move(start_square, direction, pieces_taken)
        } else {
            unreachable!()
        }
    }
}

/// The counterpart of `Move`. When applied to a `Board`, it fully reverses the accompanying `Move`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ReverseMove {
    Place(Square),
    Move(Square, Direction, StackMovement, ArrayVec<u8, 8>, bool),
}

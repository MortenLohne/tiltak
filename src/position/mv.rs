use std::fmt::{self, Write};
use std::iter;
use std::str::FromStr;

use arrayvec::ArrayVec;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::position::utils::Direction::{East, North, South, West};
use crate::position::utils::Role::{Cap, Flat, Wall};
use crate::position::utils::{Direction, Movement, Role, StackMovement};
use std::cmp::Ordering;

use super::Square;

/// A legal move for a position.
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ExpMove<const S: usize> {
    Place(Role, Square<S>),
    Move(Square<S>, Direction, StackMovement<S>), // Number of stones to take
}

/// A legal move for a position.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Move<const S: usize> {
    inner: u16,
}

impl<const S: usize> Move<S> {
    pub fn compress(mv: ExpMove<S>) -> Move<S> {
        match mv {
            ExpMove::Place(role, square) => Self::placement(role, square),
            ExpMove::Move(square, direction, stack_movement) => {
                Self::movement(square, direction, stack_movement)
            }
        }
    }

    pub fn placement(role: Role, square: Square<S>) -> Move<S> {
        Move {
            inner: square.into_inner() as u16 | (role as u16) << 6,
        }
    }

    pub fn movement(
        square: Square<S>,
        direction: Direction,
        stack_movement: StackMovement<S>,
    ) -> Move<S> {
        Move {
            inner: square.into_inner() as u16
                | (direction as u16) << 6
                | (stack_movement.into_inner() as u16) << 8,
        }
    }

    pub fn expand(self) -> ExpMove<S> {
        if self.inner >> 8 == 0 {
            unsafe {
                ExpMove::Place(
                    Role::from_disc_unchecked(self.inner as u8 >> 6),
                    Square::from_u8(self.inner as u8 & 63),
                )
            }
        } else {
            ExpMove::Move(
                Square::from_u8(self.inner as u8 & 63),
                Direction::from_disc((self.inner as u8 >> 6) & 3),
                StackMovement::from_u8((self.inner >> 8) as u8),
            )
        }
    }

    pub fn is_placement(self) -> bool {
        self.inner >> 8 == 0
    }

    pub fn origin_square(self) -> Square<S> {
        Square::from_u8(self.inner as u8 & 63)
    }

    pub fn from_string(input: &str) -> Result<Self, pgn_traits::Error> {
        // Trim crush notation
        let input = input.trim_end_matches('*');
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
                let square = Square::parse_square(input)?;
                Ok(Self::placement(Flat, square))
            }
            'a'..='h' if input.len() == 3 => {
                let square = Square::parse_square(&input[0..2])?;
                let direction = Direction::parse(input.chars().nth(2).unwrap())
                    .ok_or_else(|| pgn_traits::Error::new_parse_error("Bad direction"))?;
                // Moves in the simplified move notation always move one piece
                let mut movement = StackMovement::new();
                movement.push(Movement { pieces_to_take: 1 }, 1);
                Ok(Self::movement(square, direction, movement))
            }
            'C' if input.len() == 3 => Ok(Self::placement(Cap, Square::parse_square(&input[1..])?)),
            'S' if input.len() == 3 => {
                Ok(Self::placement(Wall, Square::parse_square(&input[1..])?))
            }
            '1'..='8' if input.len() > 3 => {
                let square = Square::parse_square(&input[1..3])?;
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
                movements.push(
                    Movement {
                        pieces_to_take: pieces_taken,
                    },
                    S as u8,
                );

                for amount_to_drop in amounts_to_drop {
                    movements.push(
                        Movement {
                            pieces_to_take: pieces_held - amount_to_drop,
                        },
                        pieces_held,
                    );
                    pieces_held -= amount_to_drop;
                }

                // Finish the movement
                movements.push(Movement { pieces_to_take: 0 }, pieces_held);

                Ok(Self::movement(square, direction, movements))
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

    pub fn from_string_playtak(input: &str) -> Self {
        let words: Vec<&str> = input.split_whitespace().collect();
        if words[0] == "P" {
            let square = Square::parse_square(&words[1].to_lowercase()).unwrap();
            let role = match words.get(2) {
                Some(&"C") => Role::Cap,
                Some(&"W") => Role::Wall,
                None => Role::Flat,
                Some(s) => panic!("Unknown role {} for move {}", s, input),
            };
            Self::placement(role, square)
        } else if words[0] == "M" {
            let start_square = Square::parse_square(&words[1].to_lowercase()).unwrap();
            let end_square: Square<S> = Square::parse_square(&words[2].to_lowercase()).unwrap();
            let pieces_dropped: Vec<u8> = words
                .iter()
                .skip(3)
                .map(|s| u8::from_str(s).unwrap())
                .collect();

            let num_pieces_taken: u8 = pieces_dropped.iter().sum();

            let mut pieces_held = num_pieces_taken;

            let pieces_taken: StackMovement<S> = StackMovement::from_movements(
                iter::once(num_pieces_taken)
                    .chain(pieces_dropped.iter().take(pieces_dropped.len() - 1).map(
                        |pieces_to_drop| {
                            pieces_held -= pieces_to_drop;
                            pieces_held
                        },
                    ))
                    .chain(iter::once(0))
                    .map(|pieces_to_take| Movement { pieces_to_take }),
            );

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

            Self::movement(start_square, direction, pieces_taken)
        } else {
            unreachable!()
        }
    }

    pub fn to_string_playtak(self) -> String {
        self.expand().to_string_playtak()
    }
}

impl<const S: usize> fmt::Display for Move<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.expand().fmt(f)
    }
}

impl<const S: usize> ExpMove<S> {
    pub fn origin_square(&self) -> Square<S> {
        Move::compress(self.clone()).origin_square()
    }

    pub fn from_string(input: &str) -> Result<Self, pgn_traits::Error> {
        Move::from_string(input).map(Move::expand)
    }

    pub fn to_string_playtak(&self) -> String {
        match self {
            ExpMove::Place(role, square) => {
                let role_string = match role {
                    Role::Flat => "",
                    Role::Wall => " W",
                    Role::Cap => " C",
                };
                let square_string = square.to_string().to_uppercase();
                format!("P {}{}", square_string, role_string)
            }
            ExpMove::Move(start_square, direction, stack_movement) => {
                let mut output = String::new();
                let mut end_square = *start_square;
                let mut pieces_held = stack_movement.get_first().pieces_to_take;
                let pieces_to_leave: Vec<u8> = stack_movement
                    .into_iter()
                    .skip(1)
                    .map(|Movement { pieces_to_take }| {
                        end_square = end_square.go_direction(*direction).unwrap();
                        let pieces_to_leave = pieces_held - pieces_to_take;
                        pieces_held = pieces_to_take;
                        pieces_to_leave
                    })
                    .collect();

                end_square = end_square.go_direction(*direction).unwrap();

                write!(
                    output,
                    "M {} {} ",
                    start_square.to_string().to_uppercase(),
                    end_square.to_string().to_uppercase()
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
}

impl<const S: usize> fmt::Display for ExpMove<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExpMove::Place(role, square) => match role {
                Cap => write!(f, "C{}", square),
                Flat => write!(f, "{}", square),
                Wall => write!(f, "S{}", square),
            },
            ExpMove::Move(square, direction, stack_movements) => {
                let mut pieces_held = stack_movements.get_first().pieces_to_take;
                if pieces_held == 1 {
                    write!(f, "{}", square).unwrap();
                } else {
                    write!(f, "{}{}", pieces_held, square).unwrap();
                }
                match direction {
                    North => f.write_char('+')?,
                    West => f.write_char('<')?,
                    East => f.write_char('>')?,
                    South => f.write_char('-')?,
                }
                // Omit number of pieces dropped, if all stones are dropped immediately
                if stack_movements.len() > 1 {
                    for movement in stack_movements.into_iter().skip(1) {
                        let pieces_to_drop = pieces_held - movement.pieces_to_take;
                        write!(f, "{}", pieces_to_drop)?;
                        pieces_held -= pieces_to_drop;
                    }
                    write!(f, "{}", pieces_held)
                } else {
                    Ok(())
                }
            }
        }
    }
}

/// The counterpart of `Move`. When applied to a `Board`, it fully reverses the accompanying `Move`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ReverseMove<const S: usize> {
    Place(Square<S>),
    Move(
        Square<S>,
        Direction,
        StackMovement<S>,
        ArrayVec<u8, 8>,
        bool,
    ),
}

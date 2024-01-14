use crate::ptn::{Game, ParseError, PtnMove};
use pgn_traits::PgnPosition;
use std::str::FromStr;

pub fn parse_ptn<B: PgnPosition>(input: &str) -> Result<Vec<Game<B>>, ParseError> {
    let mut parser = ParserData { input };
    let mut games = vec![];
    loop {
        if parser.input.chars().any(|ch| !ch.is_whitespace()) {
            match parse_game(&mut parser) {
                Ok(game) => games.push(game),
                Err(err) => {
                    eprintln!("Couldn't parse game: {}", err);
                    if games.is_empty() {
                        return Err(err);
                    } else {
                        return Ok(games);
                    }
                }
            }
        } else {
            return Ok(games);
        }
    }
}

fn parse_game<B: PgnPosition>(input: &mut ParserData) -> Result<Game<B>, ParseError> {
    let mut tags = vec![];
    input.skip_whitespaces();
    while input.peek() == Some('[') {
        let (tag, value) = parse_tag(input)?;
        input.skip_whitespaces();
        tags.push((tag.to_string(), value));
    }

    // Thunk to get the game's start position
    // It can't be a regular variable, because there is no `B: Clone` bound
    let start_position = || {
        if let Some(fen_tag) = B::START_POSITION_TAG_NAME {
            if let Some((_, tps)) = tags
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(fen_tag))
            {
                B::from_fen(tps)
            } else {
                Ok(B::start_position())
            }
        } else {
            Ok(B::start_position())
        }
    };

    let (moves, game_result_str) = parse_moves(input, start_position()?)?;

    Ok(Game {
        start_position: start_position()?,
        moves,
        game_result_str,
        tags,
    })
}

fn parse_tag<'a>(input: &mut ParserData<'a>) -> Result<(&'a str, String), pgn_traits::Error> {
    assert_eq!(input.take(), Some('['));
    let tag: &'a str = input.take_word();

    input.skip_whitespaces();
    if input.take() != Some('"') {
        return Err(pgn_traits::Error::new_parse_error(format!(
            "Tag value for {} didn't start with \"",
            tag
        )));
    }

    let mut value = String::new();
    loop {
        match input.take() {
            Some('"') => break,
            Some('\\') => match input.take().unwrap() {
                '"' => value.push('"'),
                '\\' => value.push('\\'),
                ch => {
                    value.push('\\');
                    value.push(ch);
                }
            },
            Some(ch) => value.push(ch),
            None => {
                return Err(pgn_traits::Error::new_parse_error(format!(
                    "Unexpected EOF parsing tag value for {}",
                    tag
                )))
            }
        }
    }
    input.skip_whitespaces();
    if input.take() == Some(']') {
        Ok((tag, value))
    } else {
        Err(pgn_traits::Error::new_parse_error(format!(
            "Unexpected EOF parsing tag value for {}",
            tag
        )))
    }
}

#[allow(clippy::type_complexity)]
fn parse_moves<B: PgnPosition>(
    input: &mut ParserData,
    mut position: B,
) -> Result<(Vec<PtnMove<B::Move>>, Option<&'static str>), ParseError> {
    let mut moves: Vec<PtnMove<B::Move>> = vec![];
    let mut _ply_counter = 0; // Last ply seen
    loop {
        input.skip_whitespaces();
        if input.peek().is_none() || input.peek() == Some('[') {
            // Games without a result aren't allowed by the spec,
            // but try to accept it anyway and return a `None` result
            if !moves.is_empty() {
                return Ok((moves, None));
            }
            // Return an error if we've read tags, but no moves
            return Err(Box::new(pgn_traits::Error::new_parse_error(
                "Unexpected EOF, expected a move or a game result.".to_string(),
            )));
        }
        let word = input.take_word();

        assert!(!word.is_empty());

        if let Some(num_string) = word.strip_suffix("...") {
            let _num = u64::from_str(num_string)?;
            _ply_counter = _num * 2 - 1;
        } else if let Some(num_string) = word.strip_suffix('.') {
            let _num = u64::from_str(num_string)?;
            _ply_counter = _num * 2 - 2;
        } else if let Some((result_str, _)) = B::POSSIBLE_GAME_RESULTS
            .iter()
            .find(|(s, _result)| *s == word)
        {
            return Ok((moves, Some(*result_str)));
        } else {
            let mut move_string = word;
            let mut annotations = vec![];
            while let Some(annotation) = B::POSSIBLE_MOVE_ANNOTATIONS
                .iter()
                .find(|annotation| move_string.strip_suffix(*annotation).is_some())
            {
                move_string = move_string.strip_suffix(*annotation).unwrap();
                annotations.insert(0, *annotation);
            }

            match position.move_from_san(move_string) {
                Ok(mv) => {
                    if !position.move_is_legal(mv.clone()) {
                        return Err(Box::new(pgn_traits::Error::new(
                            pgn_traits::ErrorKind::IllegalMove,
                            word,
                        )));
                    }
                    position.do_move(mv.clone());
                    input.skip_whitespaces();
                    if input.peek() == Some('{') {
                        input.take();
                        let comment = input.take_while(|ch| ch != '}');
                        input.take();
                        moves.push(PtnMove {
                            mv,
                            annotations,
                            comment: comment.to_string(),
                        })
                    } else {
                        moves.push(PtnMove {
                            mv,
                            annotations,
                            comment: String::new(),
                        });
                    }
                }
                Err(err) => {
                    return Err(Box::new(pgn_traits::Error::new_parse_error(format!(
                        "Couldn't parse move {}: {}",
                        word, err
                    ))));
                }
            }
        }
    }
}

struct ParserData<'a> {
    input: &'a str,
}

impl<'a> ParserData<'a> {
    fn skip_whitespaces(&mut self) {
        self.input = self.input.trim_start_matches(char::is_whitespace);
    }

    fn take_word(&mut self) -> &'a str {
        self.skip_whitespaces();
        self.take_while(|ch| !ch.is_whitespace())
    }

    fn take_while<F: Fn(char) -> bool>(&mut self, f: F) -> &'a str {
        for (i, ch) in self.input.char_indices() {
            if !f(ch) {
                let output = &self.input[0..i];
                self.input = &self.input[i..];
                return output;
            }
        }
        let output = self.input;
        self.input = &self.input[self.input.len()..];
        output
    }

    fn peek(&mut self) -> Option<char> {
        self.input.chars().next()
    }

    fn take(&mut self) -> Option<char> {
        if let Some(ch) = self.input.chars().next() {
            self.input = &self.input[ch.len_utf8()..];
            Some(ch)
        } else {
            None
        }
    }
}

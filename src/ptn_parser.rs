use crate::ptn_writer::Game;
use board_game_traits::GameResult;
use pgn_traits::PgnPosition;
use std::error;
use std::error::Error;
use std::fmt::Debug;
use std::str::FromStr;

pub fn parse_ptn<B: PgnPosition + Debug + Clone>(
    input: &str,
) -> Result<Vec<Game<B>>, Box<dyn error::Error>> {
    let mut parser = ParserData { input };
    let mut games = vec![];
    loop {
        match parse_game(&mut parser) {
            Ok(game) => games.push(game),
            Err(err) => {
                if !parser.input.is_empty() {
                    eprintln!("Couldn't parse game: {}", err);
                }
                return Ok(games);
            }
        }
    }
}

fn parse_game<B: PgnPosition + Debug + Clone>(
    input: &mut ParserData,
) -> Result<Game<B>, Box<dyn Error>> {
    let mut tags = vec![];
    input.skip_whitespaces();
    while input.peek() == Some('[') {
        let (tag, value) = parse_tag(input)?;
        input.skip_whitespaces();
        tags.push((tag.to_string(), value));
    }
    let position = B::start_position();

    let (moves, game_result) = parse_moves(input, position.clone())?;

    Ok(Game {
        start_position: position,
        moves,
        game_result,
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

fn parse_moves<B: PgnPosition + Debug + Clone>(
    input: &mut ParserData,
    mut position: B,
) -> Result<(Vec<(B::Move, String)>, Option<GameResult>), Box<dyn Error>> {
    let mut moves: Vec<(B::Move, String)> = vec![];
    let mut _ply_counter = 0; // Last ply seen
    loop {
        input.skip_whitespaces();
        let word = input.take_word();
        if let Some(num_string) = word.strip_suffix("...") {
            let _num = u64::from_str(num_string)?;
            _ply_counter = _num * 2 - 1;
        } else if let Some(num_string) = word.strip_suffix('.') {
            let _num = u64::from_str(num_string)?;
            _ply_counter = _num * 2 - 2;
        } else if let Some((_, result)) = B::POSSIBLE_GAME_RESULTS
            .iter()
            .find(|(s, _result)| *s == word)
        {
            return Ok((moves, *result));
        } else if let Ok(mv) = position.move_from_san(&word) {
            position.do_move(mv.clone());
            input.skip_whitespaces();
            if input.peek() == Some('{') {
                input.take();
                let comment = input.take_while(|ch| ch != '}');
                input.take();
                moves.push((mv, comment.to_string()))
            } else {
                moves.push((mv, String::new()));
            }
        } else {
            return Err(Box::new(pgn_traits::Error::new_parse_error(format!(
                "Couldn't parse move {}",
                word
            ))));
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

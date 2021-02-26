use crate::pgn_writer::Game;
use board_game_traits::GameResult;
use pgn_traits::PgnPosition;
use std::error;
use std::fmt::Debug;
use std::str::FromStr;

pub fn parse_pgn<B: PgnPosition + Debug + Clone>(
    input: &str,
) -> Result<Vec<Game<B>>, Box<dyn error::Error>> {
    let mut parser = ParserData { input };
    let mut games = vec![];
    while let Some(game) = parse_game(&mut parser) {
        games.push(game);
    }
    Ok(games)
}

fn parse_game<B: PgnPosition + Debug + Clone>(input: &mut ParserData) -> Option<Game<B>> {
    let mut tags = vec![];
    input.skip_whitespaces();
    while input.peek()? == '[' {
        let (tag, value) = parse_tag(input)?;
        tags.push((tag.to_string(), value));
    }
    let position = B::start_position();

    let (moves, game_result) = parse_moves(input, position.clone())?;

    Some(Game {
        start_position: position,
        moves,
        game_result,
        tags,
    })
}

fn parse_tag<'a>(input: &mut ParserData<'a>) -> Option<(&'a str, String)> {
    assert_eq!(input.take(), Some('['));
    let tag: &'a str = input.take_while(|ch| !ch.is_whitespace());

    input.skip_whitespaces();
    if input.take() != Some('"') {
        return None;
    }

    let mut value = String::new();
    loop {
        match input.take()? {
            '"' => break,
            '\\' => match input.take()? {
                '"' => value.push('"'),
                '\\' => value.push('\\'),
                ch => {
                    value.push('\\');
                    value.push(ch);
                }
            },
            ch => value.push(ch),
        }
    }
    input.skip_whitespaces();
    if input.take()? == ']' {
        Some((tag, value))
    }
    else {
        None
    }
}

fn parse_moves<B: PgnPosition + Debug + Clone>(
    input: &mut ParserData,
    mut position: B,
) -> Option<(Vec<(B::Move, String)>, Option<GameResult>)> {
    let mut moves: Vec<(B::Move, String)> = vec![];
    let mut _ply_counter = 0; // Last ply seen
    loop {
        let word = input.take_word();
        if word.ends_with("...") {
            let _num = u64::from_str(&word[..word.len() - 4]).ok()?;
            _ply_counter = _num * 2 - 1;
        } else if word.ends_with('.') {
            let _num = u64::from_str(&word[..word.len() - 2]).ok()?;
            _ply_counter = _num * 2 - 2;
        } else if let Some((_, result)) = B::POSSIBLE_GAME_RESULTS
            .iter()
            .find(|(s, _result)| *s == word)
        {
            return Some((moves, *result));
        } else if let Ok(mv) = position.move_from_san(&word) {
            position.do_move(mv.clone());
            input.skip_whitespaces();
            if input.peek() == Some('{') {
                input.take();
                let comment = input.take_while(|ch| ch != '}');
                moves.push((mv, comment.to_string()))
            } else {
                moves.push((mv, String::new()));
            }
        } else {
            return None;
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
                self.input = &self.input[i + ch.len_utf8()..];
                return &self.input[0..i];
            }
        }
        self.input = &self.input[self.input.len()..];
        self.input
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

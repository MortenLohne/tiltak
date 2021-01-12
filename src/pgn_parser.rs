use crate::pgn_writer::Game;
use board_game_traits::board::GameResult;
use nom::{
    alt, char, complete, dbg, do_parse, many0, many1, many_till, named, opt, return_error, tag,
    take_until, take_until_and_consume,
};
use pgn_traits::pgn::PgnBoard;
use std::error;
use std::fmt::Debug;
use std::io;
use std::io::Write;

pub fn parse_pgn<B: PgnBoard + Debug + Clone>(
    mut input: &str,
) -> Result<Vec<Game<B>>, Box<dyn error::Error>> {
    let mut games = vec![];

    loop {
        let result = parse_game(input);
        match result {
            Ok((rem_input, (tag_pairs, move_texts))) => {
                let mut board = B::start_board();
                let mut moves = vec![];
                for (ref move_text, ref comment) in move_texts.iter() {
                    let mv = board.move_from_san(move_text);
                    match mv {
                        Err(err) => {
                            println!(
                                "Failed to parse move text \"{}\" on board\n{:?}\n{}",
                                move_text, board, err
                            );
                            return Err(err.into());
                        }
                        Ok(mv) => {
                            // Checking for move legality is too expensive for release builds
                            let mut legal_moves = vec![];
                            board.generate_moves(&mut legal_moves);
                            debug_assert!(legal_moves.contains(&mv));
                            board.do_move(mv.clone());
                            moves.push((mv, comment.unwrap_or("").to_string()));
                        }
                    }
                }

                input = rem_input;

                let tags: Vec<(String, String)> = tag_pairs
                    .iter()
                    .map(|(a, b)| ((*a).to_string(), (*b).to_string()))
                    .collect();

                let game_result = tags
                    .iter()
                    .find(|(name, _)| name == "Result")
                    .map(|(_, result)| match result.as_ref() {
                        "1-0" => Some(GameResult::WhiteWin),
                        "1/2-1/2" => Some(GameResult::Draw),
                        "0-1" => Some(GameResult::BlackWin),
                        "*" => None,
                        _ => panic!("No result for game."), // TODO: Failure to read a single game should be recoverable
                    })
                    .flatten();

                let start_board = tags
                    .iter()
                    .find(|(name, _)| name == "TPS")
                    .map(|(_, result)| B::from_fen(result))
                    .unwrap_or_else(|| Ok(B::start_board()))?;

                let game = Game {
                    start_board,
                    moves,
                    game_result,
                    tags,
                };

                games.push(game);
            }
            Err(err) => {
                match err {
                    nom::Err::Incomplete(i) => {
                        writeln!(io::stderr(), "Couldn't parse incomplete game: {:?}", i)?
                    }
                    nom::Err::Error(nom::Context::Code(i, error_kind)) => writeln!(
                        io::stderr(),
                        "Parse error of kind {:?} around {}",
                        error_kind,
                        &i[0..100]
                    )?,
                    nom::Err::Error(nom::Context::List(errs)) => {
                        writeln!(io::stderr(), "Parse error: {:?}", errs)?
                    }
                    nom::Err::Failure(nom::Context::Code(i, error_kind)) => writeln!(
                        io::stderr(),
                        "Parse failure of kind {:?} around {}",
                        error_kind,
                        i
                    )?,
                    nom::Err::Failure(nom::Context::List(errs)) => {
                        writeln!(io::stderr(), "Parse failure: {:?}", errs)?
                    }
                }
                break;
            }
        }
    }

    Ok(games)
}

named!(parse_game<&str, (Vec<(&str, &str)>, Vec<(&str, Option<&str>)>)>,
    do_parse!(
        tag_pairs: parse_tag_pairs >>
        many0!(alt!(tag!(" ") | tag!("\n"))) >>
        moves: parse_game_movetext >>
        many0!(alt!(tag!(" ") | tag!("\n"))) >>
        ((tag_pairs, moves))
    )
);

named!(parse_game_movetext<&str, Vec<(&str, Option<&str>)>>,
    do_parse!(
        result: complete!(dbg!(
        many_till!(parse_move_text,
            alt!(tag!("0-1") | tag!("1-0") | tag!("1/2-1/2") | tag!("*"))
        ))) >>
        (result.0)
    )
);

named!(parse_move_text<&str, (&str, Option<&str>)>,
    do_parse!(
        dbg!(opt!(complete!(many_till!(nom::digit, alt!(tag!("... ") | tag!(". ")))))) >>
        movetext: return_error!(dbg!(complete!(take_until!(" ")))) >>
        comment: opt!(complete!(do_parse!(
                many1!(alt!(tag!(" ") | tag!("\n"))) >>
                char!('{') >>
                comment: take_until!("}") >>
                char!('}') >>
                (comment)
            ))
        ) >>
        many1!(complete!(alt!(tag!(" ") | tag!("\n")))) >>
        (movetext, comment)
    )
);

named!(parse_tag_pairs<&str, Vec<(&str, &str)>>,
    many0!(do_parse!(
        tag: parse_tag_pair >>
        tag!("\n") >>
        (tag)
    ))
);

named!(parse_tag_pair<&str, (&str, &str)>,
    do_parse!(
        char!('[') >>
        name: take_until_and_consume!(" ") >>
        char!('"') >>
        value: take_until_and_consume!("\"]") >>
        ((name, value))
    )
);

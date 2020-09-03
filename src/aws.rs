use crate::board::{Board, Move};
use crate::mcts;
use board_game_traits::board::Board as EvalBoard;
use lambda_runtime::error::HandlerError;
use lambda_runtime::Context;
use serde::Deserialize;
use serde::Serialize;
use std::io::BufReader;
use std::process::Command;
use std::time::Duration;
use std::{fs, io};

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Event {
    pub moves: Vec<Move>,
    pub time_left: Duration,
    pub increment: Duration,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Output {
    pub best_move: Move,
    pub score: f32,
}

/// AWS serverside handler
pub fn handle_aws_event(e: Event, _c: Context) -> Result<Output, HandlerError> {
    let mut board = Board::default();
    for mv in e.moves {
        board.do_move(mv);
    }

    let max_time = Duration::min(e.time_left / 40 + e.increment, Duration::from_secs(60));

    let (best_move, score) = mcts::play_move_time(board, max_time);

    Ok(Output { best_move, score })
}

/// Clientside function for receiving moves from AWS
pub fn best_move_aws(aws_function_name: &str, payload: &Event) -> io::Result<Output> {
    let mut aws_out_file_name = std::env::temp_dir();
    aws_out_file_name.push("aws_response.json");
    {
        fs::File::create(aws_out_file_name.clone()).map_err(|err| {
            io::Error::new(
                err.kind(),
                format!(
                    "Failed to create temporary aws output file \"{}\"",
                    aws_out_file_name.to_string_lossy()
                ),
            )
        })?;

        let mut child = Command::new("aws")
            .arg("lambda")
            .arg("invoke")
            .arg("--function-name")
            .arg(aws_function_name)
            .arg("--cli-binary-format")
            .arg("raw-in-base64-out")
            .arg("--payload")
            .arg(serde_json::to_string(payload).unwrap())
            .arg(aws_out_file_name.as_os_str())
            .spawn()
            .map_err(|err| io::Error::new(err.kind(), "Failed to start aws cli"))?;
        child.wait()?;
    }

    let aws_out_file = fs::File::open(aws_out_file_name.clone()).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!(
                "Failed to read from temporary aws output file \"{}\"",
                aws_out_file_name.to_string_lossy()
            ),
        )
    })?;
    let output = serde_json::from_reader(BufReader::new(aws_out_file)).unwrap();
    fs::remove_file(&aws_out_file_name).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!(
                "Failed to delete temporary aws output file \"{}\"",
                aws_out_file_name.to_string_lossy()
            ),
        )
    })?;
    Ok(output)
}

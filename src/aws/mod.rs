use crate::position::Move;
use serde::Deserialize;
use serde::Serialize;
use std::time::Duration;

#[cfg(feature = "aws-lambda-client")]
pub mod client;
#[cfg(feature = "aws-lambda-runtime")]
pub mod server;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Event {
    pub size: usize,
    pub moves: Vec<Move>,
    pub time_left: Duration,
    pub increment: Duration,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Output {
    pub best_move: Move,
    pub score: f32,
}

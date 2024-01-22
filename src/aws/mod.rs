use std::time;

use serde::Deserialize;
use serde::Serialize;

use crate::search::TimeControl;

#[cfg(feature = "aws-lambda-client")]
pub mod client;
#[cfg(feature = "aws-lambda-runtime")]
pub mod server;

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Event {
    pub size: usize,
    pub tps: Option<String>,
    pub moves: Vec<String>,
    pub time_control: TimeControl,
    pub komi: f64, // "Main" komi setting, used to determine the game result at terminal nodes
    pub eval_komi: Option<f64>, // Komi used for heuristic evaluation. Default to the main komi, but not all komis are supported
    pub dirichlet_noise: Option<f32>,
    pub rollout_depth: u16,
    pub rollout_temperature: f64,
}

#[derive(Debug, Default, PartialEq, Clone, Serialize, Deserialize)]
pub struct Output {
    pub pv: Vec<String>,
    pub score: f32,
    pub nodes: u32,
    pub mem_usage: u64,
    pub time_taken: time::Duration,
}

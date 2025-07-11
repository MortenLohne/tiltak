#![allow(clippy::uninlined_format_args)]

use std::path::Path;
use std::process::exit;

use clap::{Arg, Command};

use tiltak::evaluation::parameters::{
    self, NUM_POLICY_FEATURES_4S, NUM_POLICY_FEATURES_5S, NUM_POLICY_FEATURES_6S,
    NUM_VALUE_FEATURES_4S, NUM_VALUE_FEATURES_5S, NUM_VALUE_FEATURES_6S,
};
use tiltak::position::Komi;
use tiltak::tune::training::TrainingOptions;
use tiltak::tune::{spsa, training};

fn main() {
    let app = Command::new("Tiltak variable tuning")
        .version("0.1")
        .author("Morten Lohne")
        .arg(
            Arg::new("size")
                .global(true)
                .short('s')
                .long("size")
                .help("Board size")
                .num_args(1)
                .value_parser(clap::value_parser!(u64).range(4..=6)))
        .arg(
            Arg::new("komi")
                .global(true)
                .long("komi")
                .num_args(1)
                .allow_hyphen_values(true)
                .value_parser(|input: &str| {
                    input.parse::<Komi>()
                }))
        .subcommand(Command::new("selfplay")
            .about("Tune value and policy constants by playing against itself. Will write the games to text files in the working directory.")
            .arg(
                Arg::new("nodes")
                    .long("nodes")
                    .help("Number of MCTS nodes per selfplay game.")
                    .default_value("50000")
                    .num_args(1)
                    .value_parser(clap::value_parser!(u64)))
            .arg(
                Arg::new("batch-size")
                    .long("batch-size")
                    .help("Number of games per training batch. Eval parameters are re-tuned after each batch.")
                    .default_value("1000")
                    .num_args(1)
                    .value_parser(clap::value_parser!(u64)))
            .arg(
                Arg::new("policy-tuning-games")
                    .num_args(1)
                    .long("policy-tuning-games")
                    .default_value("2000")
                    .help("Maximum number of games to tune the policy parameters from. Higher numbers will increase peak memory usage.")
                    .value_parser(clap::value_parser!(u64))
            ))
        .subcommand(Command::new("selfplay-from-scratch")
            .about("Tune value and policy constants from randomly initialized values by playing against itself. Will write the games to text files in the working directory.")
            .arg(
                Arg::new("nodes")
                    .long("nodes")
                    .help("Number of MCTS nodes per selfplay game.")
                    .default_value("50000")
                    .num_args(1)
                    .value_parser(clap::value_parser!(u64)))
            .arg(
                Arg::new("batch-size")
                    .long("batch-size")
                    .help("Number of games per training batch. Eval parameters are re-tuned after each batch.")
                    .default_value("1000")
                    .num_args(1)
                    .value_parser(clap::value_parser!(u64)))
            .arg(
                Arg::new("policy-tuning-games")
                    .num_args(1)
                    .long("policy-tuning-games")
                    .default_value("2000")
                    .help("Maximum number of games to tune the policy parameters from. Higher numbers will increase peak memory usage.")
                    .value_parser(clap::value_parser!(u64))
            ))
        .subcommand(Command::new("continue-selfplay")
            .about("Continue selfplay training")
            .arg(
                Arg::new("nodes")
                    .long("nodes")
                    .help("Number of MCTS nodes per selfplay game.")
                    .default_value("50000")
                    .num_args(1)
                    .value_parser(clap::value_parser!(u64)))
            .arg(
                Arg::new("batch-size")
                    .long("batch-size")
                    .help("Number of games per training batch. Eval parameters are re-tuned after each batch.")
                    .default_value("1000")
                    .num_args(1)
                    .value_parser(clap::value_parser!(u64)))
            .arg(Arg::new("training-id")
                .long("training-id")
                .num_args(1)
                .required(true)
                .value_parser(clap::value_parser!(u64)),
            ).arg(Arg::new("policy-tuning-games")
                .num_args(1)
                .long("policy-tuning-games")
                .default_value("2000")
                .help("Maximum number of games to tune the policy parameters from. Higher numbers will increase peak memory usage.")
                .value_parser(clap::value_parser!(u64))
            ))
        .subcommand(Command::new("value-from-file")
                .about("Tune value constants from randomly initialized values, using the given ptn file. Note that the ptn parser is completely broken, and will probably fail on any files not generated by this program itself.")
                .arg(Arg::new("file-name")
                    .index(1)
                    .required(true)
                    .value_name("games.ptn")))
        .subcommand(
            Command::new("both-from-file")
                .about("Tune value and policy constants from randomly initialized values, using the given text file")
                .arg(Arg::new("value-file-name")
                    .index(1)
                    .required(true)
                    .value_name("games.ptn"))
                .arg(Arg::new("policy-file-name")
                    .index(2)
                    .required(true)
                    .value_name("move_scores.txt"))
        )
        .subcommand(Command::new("spsa")
            .about("Tune exploration parameters using SPSA. Starting values are hard-coded.")
            .arg(Arg::new("book")
                .num_args(1)
                .long("book")
                .help("Opening book for the games.")
                .value_name("book.txt")
            )).arg_required_else_help(true);

    let matches = app.get_matches();
    // Required global options doesn't work properly in Clap,
    // so manually check that they are present
    let Some(size) = matches.get_one::<u64>("size") else {
        eprintln!("Error: --size is required");
        exit(1)
    };
    let Some(komi) = matches.get_one::<Komi>("komi") else {
        eprintln!("Error: --komi is required");
        exit(1)
    };

    match matches.subcommand() {
        Some(("selfplay", arg)) => {
            let num_games_for_policy_tuning = matches
                .get_one::<u64>("policy-tuning-games")
                .map(|&n| n as usize)
                .unwrap();
            for training_id in 0.. {
                let file_name = format!("games{}_s{}_batch0.ptn", training_id, size);
                if !Path::new(&file_name).exists() {
                    let batch_size = *arg.get_one::<u64>("batch-size").unwrap() as usize;
                    let nodes_per_game = *arg.get_one::<u64>("nodes").unwrap();
                    let options = TrainingOptions {
                        training_id,
                        batch_size,
                        num_games_for_policy_tuning,
                        nodes_per_game,
                    };
                    match size {
                        4 => training::train_perpetually::<
                            4,
                            NUM_VALUE_FEATURES_4S,
                            NUM_POLICY_FEATURES_4S,
                        >(
                            options,
                            *komi,
                            *parameters::value_features_4s(*komi),
                            *parameters::policy_features_4s(*komi),
                            vec![],
                            vec![],
                            0,
                        )
                        .unwrap(),
                        5 => training::train_perpetually::<
                            5,
                            NUM_VALUE_FEATURES_5S,
                            NUM_POLICY_FEATURES_5S,
                        >(
                            options,
                            *komi,
                            *parameters::value_features_5s(*komi),
                            *parameters::policy_features_5s(*komi),
                            vec![],
                            vec![],
                            0,
                        )
                        .unwrap(),
                        6 => training::train_perpetually::<
                            6,
                            NUM_VALUE_FEATURES_6S,
                            NUM_POLICY_FEATURES_6S,
                        >(
                            options,
                            *komi,
                            *parameters::value_features_6s(*komi),
                            *parameters::policy_features_6s(*komi),
                            vec![],
                            vec![],
                            0,
                        )
                        .unwrap(),
                        _ => panic!("Size {} not supported.", size),
                    }
                    break;
                } else {
                    println!("File {} already exists, trying next.", file_name);
                }
            }
        }
        Some(("selfplay-from-scratch", arg)) => {
            let num_games_for_policy_tuning = arg
                .get_one::<u64>("policy-tuning-games")
                .map(|&n| n as usize)
                .unwrap();
            for training_id in 0.. {
                let file_name = format!("games{}_{}s_batch0.ptn", training_id, size);
                if !Path::new(&file_name).exists() {
                    let batch_size = *arg.get_one::<u64>("batch-size").unwrap() as usize;
                    let nodes_per_game = *arg.get_one::<u64>("nodes").unwrap();
                    let options = TrainingOptions {
                        training_id,
                        batch_size,
                        num_games_for_policy_tuning,
                        nodes_per_game,
                    };
                    match size {
                        4 => training::train_from_scratch::<
                            4,
                            NUM_VALUE_FEATURES_4S,
                            NUM_POLICY_FEATURES_4S,
                        >(options, *komi)
                        .unwrap(),
                        5 => training::train_from_scratch::<
                            5,
                            NUM_VALUE_FEATURES_5S,
                            NUM_POLICY_FEATURES_5S,
                        >(options, *komi)
                        .unwrap(),
                        6 => training::train_from_scratch::<
                            6,
                            NUM_VALUE_FEATURES_6S,
                            NUM_POLICY_FEATURES_6S,
                        >(options, *komi)
                        .unwrap(),
                        _ => panic!("Size {} not supported.", size),
                    }
                    break;
                } else {
                    println!("File {} already exists, trying next.", file_name);
                }
            }
        }
        Some(("continue-selfplay", arg)) => {
            let num_games_for_policy_tuning = matches
                .get_one::<u64>("policy-tuning-games")
                .map(|&n| n as usize)
                .unwrap();
            let training_id = *arg.get_one::<u64>("training-id").unwrap() as usize;
            let batch_size = *arg.get_one::<u64>("batch-size").unwrap() as usize;
            let nodes_per_game = *arg.get_one::<u64>("nodes").unwrap();
            let options = TrainingOptions {
                training_id,
                batch_size,
                num_games_for_policy_tuning,
                nodes_per_game,
            };
            match size {
                4 => {
                    training::continue_training::<4, NUM_VALUE_FEATURES_4S, NUM_POLICY_FEATURES_4S>(
                        options, *komi,
                    )
                    .unwrap()
                }
                5 => {
                    training::continue_training::<5, NUM_VALUE_FEATURES_5S, NUM_POLICY_FEATURES_5S>(
                        options, *komi,
                    )
                    .unwrap()
                }
                6 => {
                    training::continue_training::<6, NUM_VALUE_FEATURES_6S, NUM_POLICY_FEATURES_6S>(
                        options, *komi,
                    )
                    .unwrap()
                }
                _ => panic!("Size {} not supported.", size),
            }
        }
        Some(("value-from-file", arg)) => {
            let file_name = arg.get_one::<String>("file-name").unwrap();
            match size {
                4 => {
                    let value_params = training::tune_value_from_file::<4, NUM_VALUE_FEATURES_4S>(
                        file_name, *komi,
                    )
                    .unwrap();
                    println!("{:?}", value_params);
                }
                5 => {
                    let value_params = training::tune_value_from_file::<5, NUM_VALUE_FEATURES_5S>(
                        file_name, *komi,
                    )
                    .unwrap();
                    println!("{:?}", value_params);
                }
                6 => {
                    let value_params = training::tune_value_from_file::<6, NUM_VALUE_FEATURES_6S>(
                        file_name, *komi,
                    )
                    .unwrap();
                    println!("{:?}", value_params);
                }
                _ => panic!("Size {} not supported.", size),
            }
        }
        Some(("both-from-file", arg)) => {
            let value_file_name = arg.get_one::<String>("value-file-name").unwrap();
            let policy_file_name = arg.get_one::<String>("policy-file-name").unwrap();
            match size {
                4 => {
                    let (value_params, policy_params) =
                        training::tune_value_and_policy_from_file::<
                            4,
                            NUM_VALUE_FEATURES_4S,
                            NUM_POLICY_FEATURES_4S,
                        >(value_file_name, policy_file_name, *komi)
                        .unwrap();
                    println!("Value: {:?}", value_params);
                    println!("Policy: {:?}", policy_params);
                }
                5 => {
                    let (value_params, policy_params) =
                        training::tune_value_and_policy_from_file::<
                            5,
                            NUM_VALUE_FEATURES_5S,
                            NUM_POLICY_FEATURES_5S,
                        >(value_file_name, policy_file_name, *komi)
                        .unwrap();
                    println!("Value: {:?}", value_params);
                    println!("Policy: {:?}", policy_params);
                }
                6 => {
                    let (value_params, policy_params) =
                        training::tune_value_and_policy_from_file::<
                            6,
                            NUM_VALUE_FEATURES_6S,
                            NUM_POLICY_FEATURES_6S,
                        >(value_file_name, policy_file_name, *komi)
                        .unwrap();
                    println!("Value: {:?}", value_params);
                    println!("Policy: {:?}", policy_params);
                }
                _ => panic!("Size {} not supported.", size),
            }
        }
        Some(("spsa", arg)) => {
            let mut variables = vec![
                spsa::Variable {
                    value: 1.50,
                    delta: 0.20,
                    apply_factor: 0.005,
                },
                spsa::Variable {
                    value: 2200.0,
                    delta: 1000.0,
                    apply_factor: 0.005,
                },
                spsa::Variable {
                    value: 0.61,
                    delta: 0.05,
                    apply_factor: 0.005,
                },
            ];
            match size {
                4 => spsa::tune::<4>(
                    &mut variables,
                    arg.get_one::<String>("book").map(|s| s.as_ref()),
                    *komi,
                ),
                5 => spsa::tune::<5>(
                    &mut variables,
                    arg.get_one::<String>("book").map(|s| s.as_ref()),
                    *komi,
                ),
                6 => spsa::tune::<6>(
                    &mut variables,
                    arg.get_one::<String>("book").map(|s| s.as_ref()),
                    *komi,
                ),
                _ => panic!("Size {} not supported.", size),
            }
        }
        Some((command, args)) => panic!("Invalid command {} with arguments {:?}", command, args),
        None => {
            println!("Error: No subcommand selected. Try the 'help' subcommand for a list.");
        }
    }
}

use clap::{App, Arg, SubCommand};
use std::path::Path;
use tiltak::position::{
    NUM_POLICY_PARAMS_4S, NUM_POLICY_PARAMS_5S, NUM_POLICY_PARAMS_6S, NUM_VALUE_PARAMS_4S,
    NUM_VALUE_PARAMS_5S, NUM_VALUE_PARAMS_6S, POLICY_PARAMS_4S, POLICY_PARAMS_5S, POLICY_PARAMS_6S,
    VALUE_PARAMS_4S, VALUE_PARAMS_5S, VALUE_PARAMS_6S,
};
use tiltak::tune::{spsa, training};

fn main() {
    let app = App::new("Tiltak variable tuning")
        .version("0.1")
        .author("Morten Lohne")
        .arg(
            Arg::with_name("size")
                .global(true)
                .short("s")
                .long("size")
                .help("Board size")
                .takes_value(true)
                .default_value("5")
                .possible_values(&["4", "5", "6"]),
        )
        .subcommand(SubCommand::with_name("selfplay")
            .about("Tune value and policy constants by playing against itself. Will write the games to text files in the working directory."))
        .subcommand(SubCommand::with_name("selfplay-from-scratch")
                        .about("Tune value and policy constants from randomly initialized values by playing against itself. Will write the games to text files in the working directory."))
        .subcommand(SubCommand::with_name("value-from-file")
                .about("Tune value constants from randomly initialized values, using the given ptn file. Note that the ptn parser is completely broken, and will probably fail on any files not generated by this program itself.")
                .arg(Arg::with_name("file-name")
                    .index(1)
                    .required(true)
                    .value_name("games.ptn")))
        .subcommand(
            SubCommand::with_name("both-from-file")
                .about("Tune value and policy constants from randomly initialized values, using the given text file")
                .arg(Arg::with_name("value-file-name")
                    .index(1)
                    .required(true)
                    .value_name("games.ptn"))
                .arg(Arg::with_name("policy-file-name")
                    .index(2)
                    .required(true)
                    .value_name("move_scores.txt"))
        )
        .subcommand(SubCommand::with_name("spsa")
            .about("Tune exploration parameters using SPSA. Starting values are hard-coded.")
            .arg(Arg::with_name("book")
                .takes_value(true)
                .long("book")
                .help("Opening book for the games.")
                .value_name("book.txt")
            ));

    let matches = app.get_matches();
    let size: usize = matches.value_of("size").unwrap().parse().unwrap();

    match matches.subcommand() {
        ("selfplay", _) => {
            for i in 0.. {
                let file_name = format!("games{}_s{}_batch0.ptn", i, size);
                if !Path::new(&file_name).exists() {
                    match size {
                        4 => training::train_perpetually::<
                            4,
                            NUM_VALUE_PARAMS_4S,
                            NUM_POLICY_PARAMS_4S,
                        >(i, &VALUE_PARAMS_4S, &POLICY_PARAMS_4S)
                        .unwrap(),
                        5 => training::train_perpetually::<
                            5,
                            NUM_VALUE_PARAMS_5S,
                            NUM_POLICY_PARAMS_5S,
                        >(i, &VALUE_PARAMS_5S, &POLICY_PARAMS_5S)
                        .unwrap(),
                        6 => training::train_perpetually::<
                            6,
                            NUM_VALUE_PARAMS_6S,
                            NUM_POLICY_PARAMS_6S,
                        >(i, &VALUE_PARAMS_6S, &POLICY_PARAMS_6S)
                        .unwrap(),
                        _ => panic!("Size {} not supported.", size),
                    }
                    break;
                } else {
                    println!("File {} already exists, trying next.", file_name);
                }
            }
        }
        ("selfplay-from-scratch", _) => {
            for i in 0.. {
                let file_name = format!("games{}_s{}_batch0.ptn", i, size);
                if !Path::new(&file_name).exists() {
                    match size {
                        4 => training::train_from_scratch::<
                            5,
                            NUM_VALUE_PARAMS_4S,
                            NUM_POLICY_PARAMS_4S,
                        >(i)
                        .unwrap(),
                        5 => training::train_from_scratch::<
                            5,
                            NUM_VALUE_PARAMS_5S,
                            NUM_POLICY_PARAMS_5S,
                        >(i)
                        .unwrap(),
                        6 => training::train_from_scratch::<
                            6,
                            NUM_VALUE_PARAMS_6S,
                            NUM_POLICY_PARAMS_6S,
                        >(i)
                        .unwrap(),
                        _ => panic!("Size {} not supported.", size),
                    }
                    break;
                } else {
                    println!("File {} already exists, trying next.", file_name);
                }
            }
        }
        ("value-from-file", Some(arg)) => {
            let file_name = arg.value_of("file-name").unwrap();
            match size {
                4 => {
                    let value_params =
                        training::tune_value_from_file::<4, NUM_VALUE_PARAMS_4S>(file_name)
                            .unwrap();
                    println!("{:?}", value_params);
                }
                5 => {
                    let value_params =
                        training::tune_value_from_file::<5, NUM_VALUE_PARAMS_5S>(file_name)
                            .unwrap();
                    println!("{:?}", value_params);
                }
                6 => {
                    let value_params =
                        training::tune_value_from_file::<6, NUM_VALUE_PARAMS_6S>(file_name)
                            .unwrap();
                    println!("{:?}", value_params);
                }
                _ => panic!("Size {} not supported.", size),
            }
        }
        ("both-from-file", Some(arg)) => {
            let value_file_name = arg.value_of("value-file-name").unwrap();
            let policy_file_name = arg.value_of("policy-file-name").unwrap();
            match size {
                4 => {
                    let (value_params, policy_params) =
                        training::tune_value_and_policy_from_file::<
                            4,
                            NUM_VALUE_PARAMS_4S,
                            NUM_POLICY_PARAMS_4S,
                        >(value_file_name, policy_file_name)
                        .unwrap();
                    println!("Value: {:?}", value_params);
                    println!("Policy: {:?}", policy_params);
                }
                5 => {
                    let (value_params, policy_params) =
                        training::tune_value_and_policy_from_file::<
                            5,
                            NUM_VALUE_PARAMS_5S,
                            NUM_POLICY_PARAMS_5S,
                        >(value_file_name, policy_file_name)
                        .unwrap();
                    println!("Value: {:?}", value_params);
                    println!("Policy: {:?}", policy_params);
                }
                6 => {
                    let (value_params, policy_params) =
                        training::tune_value_and_policy_from_file::<
                            6,
                            NUM_VALUE_PARAMS_6S,
                            NUM_POLICY_PARAMS_6S,
                        >(value_file_name, policy_file_name)
                        .unwrap();
                    println!("Value: {:?}", value_params);
                    println!("Policy: {:?}", policy_params);
                }
                _ => panic!("Size {} not supported.", size),
            }
        }
        ("spsa", Some(arg)) => {
            let mut variables = vec![
                spsa::Variable {
                    value: 1.2,
                    delta: 0.20,
                    apply_factor: 0.002,
                },
                spsa::Variable {
                    value: 3500.0,
                    delta: 1000.0,
                    apply_factor: 0.002,
                },
            ];
            match size {
                4 => spsa::tune::<4>(&mut variables, arg.value_of("book")),
                5 => spsa::tune::<5>(&mut variables, arg.value_of("book")),
                6 => spsa::tune::<6>(&mut variables, arg.value_of("book")),
                _ => panic!("Size {} not supported.", size),
            }
        }
        ("", None) => {
            println!("Error: No subcommand selected. Try the 'help' subcommand for a list.");
            println!("{}", matches.usage());
        }
        _ => unreachable!(),
    }
}

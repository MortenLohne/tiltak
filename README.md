# Taik

Taik is a simple AI for the board game [Tak](https://en.wikipedia.org/wiki/Tak_(game)). The project can be used as an analysis tool, or connect as a bot to the playtak.com server. 

# Overview

The project consists of 5 different binaries, that use the core engine in various ways:
 
 * **main** Various commands, mostly for debugging and experimentation.
 * **playtak** Connect to the `playtak.com` server, and seek games as a bot.
 * **uti** Run the engine through a [uci-like](https://en.wikipedia.org/wiki/Universal_Chess_Interface) text interface.
 * **tune** Automatically tune the engine's parameters. 
 * **bootstrap** Engine worker for running on AWS Lambda.
 
 The first 3 binaries will be built by default, while `tune` and `bootstrap` require specific commands, see their sections. 

# Usage

## main

Three experimental commands entered through stdin:

* play: Play against the minmax-based AI.
* aimatch: Watch the minmax and mcts AIs play.
* analyze: Mcts analysis of a position, provided from a simple move list.

## playtak

Connect to the playtak.com server, and seek games as a bot. If no username/password is provided, the bot will login as guest. 

Example usage: 
````
playtak -u <username> -p <password>
````

## uti 

Run the engine through a [uci-like](https://en.wikipedia.org/wiki/Universal_Chess_Interface) text interface.

Only a small subset of uci works. To analyze a position for 1 second, run the uti binary and enter:

````
position startpos moves e1 a1
go movetime 1000
````

## tune
To build and run this binary:
```
cargo build --release --features "constant-tuning" --bin tune
cargo run --release --features "constant-tuning" --bin tune
```

Automatically tune the engine's parameters through several subcommands. 

The engine's static evaluation (value parameters) and move evaluation (policy parameters) are tuned from a `.ptn` file, using gradient descent. The search exploration parameters are tuned using [SPSA.](https://en.wikipedia.org/wiki/Simultaneous_perturbation_stochastic_approximation) 

This is otherwise not well documented, try `tune --help` for more. 

## bootstrap 
To build this binary:
```
cargo build --release --target x86_64-unknown-linux-musl --bin bootstrap --features aws-lambda
```
This is otherwise undocumented.

# Build

Building the project from source requires the Rust compiler and Cargo (Rust's package manager) installed, both included in the [Rust downloads.](https://www.rust-lang.org/tools/install)

To build and run:
```
cargo build --release
cargo run --release
```


This command will automatically fetch and build dependencies. The resulting binaries are written to `taik/target/release`.

To build and run a specific command, run `cargo run --release --bin playtak` or similar.

# Implementation details 

The engine uses a modified version of Monte Carlo Tree Search, with a heuristic evaluation function in the simulation step, instead of full rollouts. The engine also uses a *policy* heuristic function to decide which child moves to prioritize in the tree.

These heuristic functions include around 100 tune-able parameters, which have been optimized using gradient descent on a training set of 10000 self-play games.

# Tests

Use `cargo test` to run tests, `cargo test --release` to run without debugging checks (recommended).

# License

This project is licensed under the GPLv3 (or any later version at your option). See the LICENSE file for the full license text.


[reference]: https://en.wikipedia.org/wiki/Universal_Chess_Interface)[uci] -like
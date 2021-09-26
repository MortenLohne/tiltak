# Tiltak

Tiltak is an AI for the board game [Tak](https://en.wikipedia.org/wiki/Tak_(game)). The project can be used as an analysis tool, or connect as a playable bot to the playtak.com server. 

It is most likely the strongest bot available. In a 2000-game match against [Taktician](https://github.com/nelhage/taktician), which was previously regarded as the strongest, it won 1276 games and lost 684.

The core engine is built using [Monte Carlo Tree Search](https://en.wikipedia.org/wiki/Monte_Carlo_tree_search), but without full simulation rollouts. This is similar to the implementation in AlphaZero or Leela Zero. 

It prunes the search tree very aggressively while searching, and will quickly reach depths of 10+ moves in the longest lines. On the other hand, it may also miss 2-move winning sequences, even with significant thinking time. 

# Overview

The project consists of 5 different binaries, that use the core engine in various ways:
 
 * **main** Various commands, mostly for debugging and experimentation.
 * **playtak** Connect to the `playtak.com` server, and seek games as a bot.
 * **tei** Run the engine through Tak Engine Interface, a [uci-like](https://en.wikipedia.org/wiki/Universal_Chess_Interface) text interface.
 * **tune** Automatically tune the engine's parameters. 
 * **bootstrap** Engine worker for running on AWS Lambda.
 
 The first 3 binaries will be built by default, while `tune` and `bootstrap` require specific commands, see their sections. 

# Usage

## main

Five experimental commands entered through stdin:

* play: Play against the engine through the command line.
* aimatch: Watch the engine play against a very simple minmax implementation.
* analyze <size>: Analyze a given position, provided from a PTN or a simple move list.
* tps <size>: Analyze a given position, provided from a tps string.
* game <size>: Analyze a whole game, provided from a PTN or a simple move list.

## playtak

Connect to the playtak.com server, and seek games as a bot. If no username/password is provided, the bot will login as guest. 

At the time of writing, three bots based on this project are running on Playtak. They are configured as follows:
````
playtak -s 6 --tc 900+30 --policy-noise high -u Tiltak_Bot -p <password> -l tiltak.log
playtak -s 6 --tc 600+20 --rollout-depth 200 --rollout-noise low -u SlateBot -p <password> -l slate.log
playtak -s 5 --tc 900+30 --fixed-nodes 100 --policy-noise medium --rollout-depth 200 --rollout-noise low -u CobbleBot -p <password> -l cobble.log
````

## tei 

Run the engine through Tak Engine Interface, a [uci-like](https://en.wikipedia.org/wiki/Universal_Chess_Interface) text interface.

Only a small subset of uci works. To analyze a position for 1 second, run the tei binary and enter:

````
tei
teinewgame 5
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
cargo build --release --target x86_64-unknown-linux-musl --bin bootstrap --features aws-lambda-runtime
```
This is otherwise undocumented.

# Build

Building the project from source requires the Rust compiler and Cargo (Rust's package manager) installed, both included in the [Rust downloads.](https://www.rust-lang.org/tools/install)

The minimum required Rust version is 1.51, released March 25th 2021.

To build and run:
```
cargo build --release
cargo run --release
```


This command will automatically fetch and build dependencies. The resulting binaries are written to `tiltak/target/release`.

To build and run a specific command, run `cargo run --release --bin playtak` or similar.

# Tests

Use `cargo test` to run tests, `cargo test --release` to run without debugging checks (recommended).

# License

This project is licensed under the GPLv3 (or any later version at your option). See the LICENSE file for the full license text.


[reference]: https://en.wikipedia.org/wiki/Universal_Chess_Interface)[uci] -like

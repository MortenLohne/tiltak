# Taik

Taik is a simple AI for the board game [Tak](https://en.wikipedia.org/wiki/Tak_(game)). The project contains a move generator and two different search algorithms, Minmax and Monte Carlo Tree Search.

# Usage

Although mostly a library at this stage, it supports three commands through stdin:

* play: Play against the minmax-based AI.
* aimatch: Watch the minmax and mcts AIs play.
* analyze: Mcts analysis of a hardcoded position.

# Build

Building the project from source requires the Rust compiler and Cargo (Rust's package manager) installed, both included in the [Rust downloads.](https://www.rust-lang.org/tools/install)

To build and run:
```
cargo build --release
cargo run --release 
```

Either command will automatically fetch and build dependencies. 

A standalone binary will also be written to `taik/target/release`.

# Tests

Use `cargo test` to run tests, `cargo test --release` to run without debugging checks (recommended).

# License

This project is licensed under the GPLv3 (or any later version at your option). See the LICENSE file for the full license text.

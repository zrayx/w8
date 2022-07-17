#!/bin/bash

clear

cargo fmt
cargo clippy &&
#RUST_BACKTRACE=1 cargo run --example data_types
RUST_BACKTRACE=1 cargo build
echo --------------------------------------------------------------------------------
inotifywait -q -e close_write src Cargo.toml run.sh

exec ./run.sh
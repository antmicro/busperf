#!/bin/bash

. ~/.cargo/env

# Pre build checks
cargo fmt --check
cargo-deny check
cargo clippy -- -Dwarnings
cargo hack check --feature-powerset

# Build
cargo build
cargo build --release

#!/bin/bash

# Build dependencies
apt update -qq
apt install -qqy curl gcc python3 libpython3-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y &> /dev/null
. ~/.cargo/env
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.105

# For checking all combination of features
cargo install cargo-hack
# Cargo deny
cargo install cargo-deny


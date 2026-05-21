#!/bin/bash

set -e

# Build dependencies
$APT_PREFIX apt update -qq
$APT_PREFIX apt install -qqy curl gcc python3 libpython3-dev git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y &> /dev/null
. ~/.cargo/env

# For checking all combination of features
cargo install cargo-hack
# Cargo deny
cargo install cargo-deny


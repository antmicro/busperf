#!/bin/bash

set -e

. ~/.cargo/env

cargo test
cargo test --release

# Check if install works
cargo install --path .

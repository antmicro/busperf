#!/bin/bash

set -e

. ~/.cargo/env

cargo test
cargo test --release

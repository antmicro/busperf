#!/bin/bash

. ~/.cargo/env

cargo test
cargo test --release

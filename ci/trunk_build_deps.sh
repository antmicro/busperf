#!/bin/bash

set -e

$APT_PREFIX apt update -qq
$APT_PREFIX apt install -qqy curl gcc wget
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y &> /dev/null
. ~/.cargo/env
wget -qO- https://github.com/trunk-rs/trunk/releases/download/v0.21.13/trunk-x86_64-unknown-linux-gnu.tar.gz | tar -xzf-
rustup target add wasm32-unknown-unknown

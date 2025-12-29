#!/bin/bash

set -e

# Cargo
$APT_PREFIX apt update -qq
$APT_PREFIX apt install -qqy curl gcc python3 python3-setuptools libpython3-dev python3-more-itertools git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y &> /dev/null
. ~/.cargo/env
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.105

# tuttest
git clone https://github.com/antmicro/tuttest.git
pushd tuttest
$APT_PREFIX python3 setup.py build &> /dev/null
$APT_PREFIX python3 setup.py install &> /dev/null
popd

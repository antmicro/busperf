#!/bin/bash

set -e

apt update -qq &> /dev/null
apt install -qqy verilator git python3-venv libpython3-dev python3-more-itertools make &> /dev/null

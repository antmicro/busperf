#!/bin/bash

apt update -qq &> /dev/null
apt install --yes verilator git python3 python3-venv libpython3-dev python3-more-itertools make &> /dev/null

#!/bin/bash

set -e

$APT_PREFIX apt update -qq &> /dev/null
$APT_PREFIX apt install -qqy verilator git python3-venv libpython3-dev python3-more-itertools make &> /dev/null

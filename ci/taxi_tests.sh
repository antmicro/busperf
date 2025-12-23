#!/bin/bash

set -e

for TEST in $(ls tests/taxi_descriptions)
do
    # Each test case starts with name of used simulation trace
    TRACE=$(echo $TEST | sed 's/\([a-z]*_[a-z]*\).*/\1/')

    # We capture stderr and make sure nothing was outputed there
    target/debug/busperf analyze taxi/src/axi/tb/taxi_$TRACE/dump.fst tests/taxi_descriptions/$TEST --text 3>&1 1>&2 2>&3 | tee /dev/fd/2 | [ -z "$(cat)" ]
    target/release/busperf analyze taxi/src/axi/tb/taxi_$TRACE/dump.fst tests/taxi_descriptions/$TEST --text 3>&1 1>&2 2>&3 | tee /dev/fd/2 | [ -z "$(cat)" ]
done


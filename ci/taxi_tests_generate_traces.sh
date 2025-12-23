#!/bin/bash

set -e

git clone https://github.com/fpganinja/taxi.git
python3 -m venv env
. ./env/bin/activate
pip install cocotb==2.0.0 &> /dev/null
pip install cocotb-bus==0.2.1 &> /dev/null
pip install cocotbext-axi==0.1.26 &> /dev/null
pip install cocotb-test==0.2.6 &> /dev/null

export WAVES=1

pushd taxi/src/axi/tb/

for BUS in "axi_ram" "axi_fifo" "axil_ram" "axil_register"
do
    pushd taxi_$BUS/
    make &> /dev/null &
    popd
done

wait
popd

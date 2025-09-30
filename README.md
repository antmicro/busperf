# Busperf - post-simulation bus performance analysis

## Motivation

When improving, or creating new, data processing IP cores it is hard to find which submodule(s) are 
bottlenecking the pipeline and causing lower than expected performance. 
Manual analysis of the system busses is tiresome and error prone. 


## Goals

* BusPerf will analyse and provide bus activity statistics based on the simulation trace, in order to 
guide developer to the buses with lowest utilisation and highest backpressure.
* It will be done by  ingesting VCD/FST files, analysing traces and providing statistics in visual and text forms.
* It should also provide API, so that user can extend functionality with features such as transfer-to-transfer delay, command-to-response delay, or which side of the channel creates bottleneck.
* It will read VCD file with accompanying YAML file.
  YAML file contains buses descriptions:
  * names of the bus signals,
  * their handshake type (valid/ready, valid/credit, valid/stall),
  * optionally bus type.
* (Optional) YAML files would be created using topwarp with some modifications, and ideally this file could also 
be used for the uvm-dvgen to generate testing environments.

## YAML bus description

Example .yaml for `tests/test_dumps/dump.vcd`:

```
interfaces:
  "a_":
    scope: "some_module"
    clock: "clk_i"
    reset: "rst_ni"
    reset_type: "low"

    handshake: "ReadyValid"
    ready: "a_ready"
    valid: "a_valid"
 
  "b_":
    scope: "some_module"
    clock: "clk_i"
    reset: "rst_ni"
    reset_type: "low"

    handshake: "ReadyValid"
    ready: "b_ready"
    valid: "b_valid"
```

- "a_", "b_": names of buses
- reset_type: "low" or "high"
- handshake: possible values: "ReadyValid", TBA

## Output

For each described bus busperf will calculate and display:
- `bus_name`: name of bus
- `busy`: number of clock cycles performing transaction
- `busy but no transaction`: both sides are ready but transaction is not performed
- `backpressure`: [backpressure](https://en.wikipedia.org/wiki/Back_pressure)
- `no data to send`: receiver ready but no data is avaible to tranfer
- `free`: bus is not used
- `delays between transaction`: delays in clock cycles between transactions
- `burst lengths`: lengths of bursts including delays during burst
- `burst_delays`: cycles wasted during bursts

## Usage

### Build

`$ cargo build`

### Run

`$ busperf [--max-burst-delay <delay>] <trace> <description> [--output <filename> --output-type <type>]`  
or  
`$ cargo run -- [--max-burst-delay <delay>] <trace> <description> [--output <filename> --output-type <type>]`

- \<trace\>: vcd file with simulation trace.
- \<description\>: yaml file with profiled buses descriptions
- \[--max-burst-delay\]: maximum number of consecutive wasted cycles in burst
- \[--output\]: file to which print output
- \[output-type\]: filetype of output \[possible values: csv, md\]

### Examples

- Prints statistics about bus described in test.yaml trace in test.vcd
<!-- name="example-test" -->
```
cargo run -- tests/test_dumps/test.vcd tests/test_dumps/test.yaml --text
```

- Prints all statistics sets max burst delay to 1
<!-- name="example-test-verbose" -->
```
cargo run -- tests/test_dumps/test.vcd tests/test_dumps/test.yaml --verbose -m 1 --text
```

- Writes statistics to `stat.csv` formated as csv
<!-- name="example-csv" -->
```
cargo run -- tests/test_dumps/test.vcd tests/test_dumps/test.yaml -o stat.csv --csv
```

- Prints statistics to stdout as md
<!-- name="example-md" -->
```
cargo run -- tests/test_dumps/test.vcd tests/test_dumps/test.yaml --md
```

- Writes pretty printed statistics to `out`
<!-- name="example-pretty" -->
```
cargo run -- tests/test_dumps/test.vcd tests/test_dumps/test.yaml -o out --text
```

- Clean files generated from examples
<!-- name="example-clean" -->
```
rm out stat.csv
```

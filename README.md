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

### Single channel bus

Example `.yaml` for `tests/test_dumps/dump.vcd`:

```
interfaces:
  "a_":
    scope: ["some_module"]
    clock: "clk_i"
    reset: "rst_ni"
    reset_type: "low"

    handshake: "ReadyValid"
    ready: "a_ready"
    valid: "a_valid"
 
  "b_":
    scope: ["some_module"]
    clock: "clk_i"
    reset: "rst_ni"
    reset_type: "low"

    handshake: "Custom"
    custom_handshake: "PythonReadyValid"
    ready: "b_ready"
    valid: "b_valid"
```

- "a_", "b_": names of buses
- reset_type: "low" or "high"
- handshake: possible values: "ReadyValid", "CreditValid", "AHB", "APB", Custom"
- custom_handshake: if handshake is set to "Custom" a name of python plugin should be provided

### Multi channel bus

Example `.yaml` for multi channel bus

```
interfaces:
  "ram_rd":
    scope: ["test_taxi_axi_ram", "uut"]
    clock: "clk"
    reset: "rst"
    reset_type: "high"

    custom_analyzer: "AXIRdAnalyzer"
    ar:
      ready: ["s_axi_rd", "arready"]
      valid: ["s_axi_rd", "arvalid"]
    r:
      ready: ["s_axi_rd", "rready"]
      valid: ["s_axi_rd", "rvalid"]
      rresp: ["s_axi_rd", "rresp"]

  "ram_wr":
    scope: ["test_taxi_axi_ram", "uut"]
    clock: "clk"
    reset: "rst"
    reset_type: "high"

    custom_analyzer: "AXIWrAnalyzer"
    aw:
      ready: ["s_axi_wr", "awready"]
      valid: ["s_axi_wr", "awvalid"]
    w:
      ready: ["s_axi_wr", "wready"]
      valid: ["s_axi_wr", "wvalid"]
      wlast: ["s_axi_wr", "wlast"]
    b:
      ready: ["s_axi_wr", "bready"]
      valid: ["s_axi_wr", "bvalid"]
      bresp: ["s_axi_wr", "bresp"]
```

For a multi channel bus an analyzer has to be specified alongside with signals required by that analyzer.
- custom_analyzer: possible values: "AXIRdAnalyzer", "AXIWrAnalyzer", "\<name of custom python analyzer\>"

## Output

For each described bus busperf will calculate and display:

### Single channel

- `bus_name`: name of bus
- `busy`: number of clock cycles performing transaction
- `free`: bus is not used
- `no transaction`: transaction is not performed
- `backpressure`: [backpressure](https://en.wikipedia.org/wiki/Back_pressure)
- `no data`: receiver ready but no data is avaible to tranfer
- `delays between transaction`: delays in clock cycles between transactions
- `burst lengths`: lengths of bursts including delays during burst

Table matching state of the bus with busperf statistic name:

| busperf        | busy                  | free               | no transaction     | backpressure      | no data         | unknown        |
|----------------|-----------------------|--------------------|--------------------|-------------------|-----------------|----------------|
| axi            | ready && valid        | !ready && !valid   | not used           | !ready && valid   | ready && !valid | no used        |
| ahb            | seq / no seq          | idle               | not used           | hready            | trans=BUSY      | other          |
| credit valid   | credit>0 && valid     | credit>0 && !valid | credit=0 && !valid | not used          | not used        | other          |
| apb            | setup or access phase | !psel              | not used           | access && !pready | not used        | other          |

### Multi channel
- `Cmd to completion`: Number of clock cycles from issuing a command to receving a reponse.
- `Cmd to first data`: Number of clock cycles from issuing a command to first data being transfered.
- `Last data to completion`: Number of clock cycles from last data being transfered to transaction end.
- `Transaction delays`: Delays between transactions in clock cycles
- `Error rate`: Percentage of transactions that resulted in error.
- `Bandwidth`: Averaged bandwidth in transactions per clock cycle.

## Usage

### Docs

`$ cargo doc --no-deps --open`

### Build

`$ cargo build`

### Run

`$ cargo run -- [OPTIONS] [-t | --trace] <SIMULATION_TRACE> [-b | --bus-config] <BUS_DESCRIPTION>`  
or  
`$ busperf [OPTIONS] [-t | --trace] <SIMULATION_TRACE> [-b | --bus-config] <BUS_DESCRIPTION>`

```
Arguments:
  <SIMULATION_TRACE>                       vcd/fst file with simulation trace
  <BUS_DESCRIPTION>                        yaml with description of buses

Options:
  -o, --output <OUTPUT_FILENAME>
      --csv                                Format output as csv
      --md                                 Format output as md table
      --gui                                Run GUI
      --text                               Format output as table
  -m, --max-burst-delay <MAX_BURST_DELAY>  [default: 0]
  -w, --window <WINDOW_SIZE>               Set size of the rolling window [default: 10000]
  -x, --x-rate <VALUE>                     Set x_rate for bandwidth above x_rate [default: 0.0001]
  -y, --y-rate <VALUE>                     Set y_rate for bandwidth below y_rate [default: 0.00001]
  -t, --trace <SIMULATION_TRACE>           Path to the trace file. Can be specified as option or a positional argument
  -b, --bus-config <BUS_DESCRIPTION>       Path to the bus description yaml. Can be specified as option or a positional argument
  -v, --verbose                            
  -h, --help                               Print help
```

### GUI

On left panel there is a list for selection of any of the analyzer buses.
On the main panel on top there is a overview of the statistics of selected
bus. Below there are two plot areas, for each you can select what
type of statistics you want to view in it.

#### Shortcuts

- up arrow: move bus selection up
- down arrow: move bus selection down
- Plots:
  - double left click: reset plot view
  - right click: open in surfer

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

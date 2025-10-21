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

    handshake: "Custom"
    custom_handshake: "PythonReadyValid"
    ready: "b_ready"
    valid: "b_valid"
```

- "a_", "b_": names of buses
- reset_type: "low" or "high"
- handshake: possible values: "ReadyValid", "CreditValid", "AHB", "APB", Custom"
- custom_handshake: if handshake is set to "Custom" a name of python plugin should be provided

Scopes can also be nested. Example `.yaml` for `tests/test_dumps/nested_scopes.vcd`:

```
base: &base_scope
  - top
  - tb
  
interfaces:
  "a_":
    scope: [*base_scope, "$rootio"]
    clock: "clk_i"
    reset: "rst_ni"
    reset_type: "low"

    handshake: "ReadyValid"
    ready: "a_ready"
    valid: "a_valid"
 
  "b_":
    scope: [*base_scope, "some_module"]
    clock: "clk_i"
    reset: "rst_ni"
    reset_type: "low"

    handshake: "ReadyValid"
    ready: "b_ready"
    valid: "b_valid"
```

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
    intervals:
      - [0, 5000000]
      - [1234567890,1324567890]
    ar:
      id:    ["s_axi_rd", "arid"]
      ready: ["s_axi_rd", "arready"]
      valid: ["s_axi_rd", "arvalid"]
    r:
      id:    ["s_axi_rd", "rid"]
      ready: ["s_axi_rd", "rready"]
      valid: ["s_axi_rd", "rvalid"]
      rresp: ["s_axi_rd", "rresp"]
      rlast: ["s_axi_rd", "rlast"]

  "ram_wr":
    scope: ["test_taxi_axi_ram", "uut"]
    clock: "clk"
    reset: "rst"
    reset_type: "high"

    custom_analyzer: "AXIWrAnalyzer"
    aw:
      id:    ["s_axi_rd", "awid"]
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
      id:    ["s_axi_rd", "bid"]
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

### Example output

**Single channel buses**
```
╭──────────┬──────┬──────────────┬─────────┬────────────────┬──────┬───────┬──────────────────────┬──────────────────────╮
│ bus name │ Busy │ Backpressure │ No data │ No transaction │ Free │ Reset │ Transaction delays   │ Burst lengths        │
├──────────┼──────┼──────────────┼─────────┼────────────────┼──────┼───────┼──────────────────────┼──────────────────────┤
│ test     │ 9    │ 5            │ 3       │ 0              │ 3    │ 2     │ 1 x1; 4-7 x1; 2-3 x3 │ 4-7 x1; 2-3 x1; 1 x3 │
╰──────────┴──────┴──────────────┴─────────┴────────────────┴──────┴───────┴──────────────────────┴──────────────────────╯
```
```
╭──────────┬──────┬──────────────┬─────────┬────────────────┬──────┬───────┬────────────────────┬───────────────╮
│ bus name │ Busy │ Backpressure │ No data │ No transaction │ Free │ Reset │ Transaction delays │ Burst lengths │
├──────────┼──────┼──────────────┼─────────┼────────────────┼──────┼───────┼────────────────────┼───────────────┤
│ a_       │ 0    │ 0            │ 15      │ 0              │ 0    │ 15    │ 16-31 x1           │               │
│ b_       │ 0    │ 0            │ 15      │ 0              │ 0    │ 15    │ 16-31 x1           │               │
╰──────────┴──────┴──────────────┴─────────┴────────────────┴──────┴───────┴────────────────────┴───────────────╯
```

**Multi channel buses**
```
╭──────────┬───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┬──────────────────────┬─────────────────────────────┬───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┬───────────────────┬─────────────────────────┬─────────────────────────────────┬───────────────────────────────╮
│ bus name │ Cmd to completion                                                                                                         │ Cmd to first data    │ Last data to completion     │ Transaction delays                                                                                                                                                        │ Error rate        │ Bandwidth               │ x rate                          │ y rate                        │
├──────────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┼──────────────────────┼─────────────────────────────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┼───────────────────┼─────────────────────────┼─────────────────────────────────┼───────────────────────────────┤
│ ram_rd   │ 1-2k x120; 16-31 x126; 4-7 x650; 32-63 x158; 8-15 x364; 2-3 x979; 256-511 x417; 64-127 x333; 512-1023 x44; 128-255 x141   │ 4-7 x1282; 2-3 x2050 │ 0 x3332                     │ 32-63 x127; 2-4k x32; 512-1023 x32; 16-31 x483; -2 x682; -1 x106; 4-7 x200; 128-255 x71; 1-2k x64; 64-127 x27; 0 x638; 4-8k x16; 2-3 x108; 256-511 x147; 8-15 x565; 1 x34 │ Error rate: 0.00% │ Bandwidth: 0.0046 t/clk │ Bandwidth above x rate: 100.00% │ Bandwidth below y rate: 0.00% │
│ ram_wr   │ 16-31 x136; 2-3 x1082; 8-15 x445; 256-511 x467; 128-255 x141; 1-2k x84; 64-127 x324; 32-63 x164; 4-7 x1536; 512-1023 x129 │ 1 x3624; 2-3 x884    │ 4-7 x685; 2-3 x414; 1 x3409 │ 0 x1567; 8-15 x906; -2 x165; 128-255 x70; -1 x157; 2-4k x16; 4-8k x17; 32-63 x19; 2-3 x877; 256-511 x148; 1-2k x80; 512-1023 x17; 64-127 x27; 16-31 x179; 4-7 x263        │ Error rate: 0.00% │ Bandwidth: 0.0062 t/clk │ Bandwidth above x rate: 100.00% │ Bandwidth below y rate: 0.00% │
╰──────────┴───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┴──────────────────────┴─────────────────────────────┴───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┴───────────────────┴─────────────────────────┴─────────────────────────────────┴───────────────────────────────╯
```

![gui example](screenshots/example.png)

## Usage

### Docs

```sh
$ cargo doc --no-deps --open
```

### Install and run

```sh
$ cargo install --path .
$ busperf --help
```

Shell completion can be generated as described [here](https://github.com/pacak/bpaf?tab=readme-ov-file#dynamic-shell-completion).

### Run without install

Release mode:
```sh
$ cargo run -r -- --help
```
Debug mode:
```sh
$ cargo run -- --help
```

### Usage help

```
Usage: busperf COMMAND ...

Available options:
    -h, --help  Prints help information

Available commands:
    analyze     Analyze given trace
    show        Show statistics from a file
```

**busperf analyze**
```
Usage: busperf analyze (--gui | --csv | --md | --text) [-o=OUT] [-s=FILENAME] [-m=BURST] [-w=WINDOW]
[-x=X_RATE] [-y=Y_RATE] [-v] (-t=TRACE -b=BUS_CONFIG | TRACE BUS)

Available positional items:
    TRACE                vcd/fst file with simulation trace
    BUS                  yaml with description of buses

Available options:
        --gui            Run GUI
        --csv            Format output as csv
        --md             Format output as md table
        --text           Format output as table
    -o=OUT               Output filename
    -s, --save=FILENAME  Save analyzed statistics for later view
    -m, --max_burst_delay=BURST  Max delay during a burst [default: 0]
    -w, --window=WINDOW  Set size of the rolling window [default: 10000]
    -x, --x_rate=X_RATE  Set x_rate for bandwidth above x_rate [default: 0.0001]
    -y, --y_rate=Y_RATE  Set y_rate for bandwidth below y_rate [default: 0.00001]
    -v, --verbose
    -t, --trace=TRACE    vcd/fst file with simulation trace
    -b, --bus-config=BUS_CONFIG  yaml with description of buses
    -h, --help           Prints help information
```

**busperf show**
```
Usage: busperf show (--gui | --csv | --md | --text) [-v] FILENAME

Available positional items:
    FILENAME       File to load statistics from

Available options:
        --gui      Run GUI
        --csv      Format output as csv
        --md       Format output as md table
        --text     Format output as table
    -v, --verbose
    -h, --help     Prints help information
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
```sh
cargo run -- analyze tests/test_dumps/test.vcd tests/test_dumps/test.yaml --text
```

- Prints all statistics sets max burst delay to 1
<!-- name="example-test-verbose" -->
```sh
cargo run -- analyze tests/test_dumps/test.vcd tests/test_dumps/test.yaml --verbose -m 1 --text
```

- Writes statistics to `stat.csv` formated as csv
<!-- name="example-csv" -->
```sh
cargo run -- analyze tests/test_dumps/test.vcd tests/test_dumps/test.yaml -o stat.csv --csv
```

- Prints statistics to stdout as md
<!-- name="example-md" -->
```sh
cargo run -- analyze tests/test_dumps/test.vcd tests/test_dumps/test.yaml --md
```

- Writes pretty printed statistics to `out`
<!-- name="example-pretty" -->
```sh
cargo run -- analyze tests/test_dumps/test.vcd tests/test_dumps/test.yaml -o out --text
```

- Clean files generated from examples
<!-- name="example-clean" -->
```sh
rm out stat.csv
```

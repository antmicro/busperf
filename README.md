# Busperf

Copyright (c) 2025 [Antmicro](https://www.antmicro.com)

Busperf helps analyze bus performance and identify throughput bottlenecks based on simulation traces.
It ingests VCD/FST files with an accompanying bus description in YAML, then generates both visual and textual statistics on bus activity.
This helps users quickly identify buses with low utilization or high backpressure.
Additonally, the tool supports Python plugins for analyzing custom bus protocols.

## Usage

### Docs

Project documentation is available in `docs/` directory. Rustdoc can be generated with:
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

### Build for WASM

Build and serve with trunk:
```sh
$ cd busperf_web
$ trunk serve --release
```

Only build:
```sh
$ cd busperf_web
$ trunk build --release
```
Output of the build will be in busperf_web/dist directory it can be served with any http server.

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
Usage: busperf analyze (--gui | --csv | --md | --text) [-o=OUT] [--skip=SKIPPED_STATS]
[-s=FILENAME] [-m=BURST] [-w=WINDOW] [-x=X_RATE] [-y=Y_RATE] [-v] [-p=PATH] TRACE BUS_CONFIG

Available positional items:
    TRACE                     vcd/fst file with simulation trace
    BUS_CONFIG                yaml with description of buses

Available options:
        --gui                 Run GUI
        --csv                 Format output as csv
        --md                  Format output as md table
        --text                Format output as table
        --save                Save data in busperf format (requires setting -o)
        --html                Generate HTML with embeded busperf_web (requires setting -o)
    -o, --output=OUT          Output filename
        --skip=SKIPPED_STATS  Stats to skip separated by a comma.
    -m, --max_burst_delay=BURST  Max delay during a burst [default: 0]
    -w, --window=WINDOW       Set size of the rolling window [default: 10000]
    -x, --x_rate=X_RATE       Set x_rate for bandwidth above x_rate [default: 0.0001]
    -y, --y_rate=Y_RATE       Set y_rate for bandwidth below y_rate [default: 0.00001]
    -v, --verbose
    -p, --plugins_path=PATH   Path to python plugins [default: "./plugins/python]"
    -h, --help                Prints help information
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

![gui example](docs/screenshots/example.png)

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

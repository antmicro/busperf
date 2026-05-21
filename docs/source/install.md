# Installation

## Prerequisities

To install and/or build Busperf [cargo](https://doc.rust-lang.org/cargo/) is required.
It can be installed as described [here](https://rust-lang.org/tools/install/).

:::{note}
Make sure that `~/.cargo/bin` was added to `PATH`.
:::

## Install from crates.io

Busperf can be installed from [crates.io](https://crates.io/crates/busperf).

~~~sh
$ cargo install busperf
~~~

## Build from source

Alternatively, you can build from source.

1. Clone the repository
~~~sh
$ git clone https://github.com/antmicro/busperf.git
~~~
2. Build and install
~~~sh
$ cargo install --path busperf
~~~

## Shell completion

Shell completion can be generated as described [here](https://github.com/pacak/bpaf?tab=readme-ov-file#dynamic-shell-completion) in point 3.

# YAML bus description

Bus description is a YAML file which describes the bus(es) we want to analyze.
It consists of 3 parts:
- `common_clk_rst_ifs` (optional) - for defining common clock and reset pairs that can be later reused
- `scopes` (optional) - for defining scope paths to be reused later
- `interfaces` - the most important part of the description. This is where the buses are defined.

Top level overview

```yaml
common_clk_rst_ifs:
  # ifs definitions

scopes:
  # scopes definitions

interfaces:
  # interfaces definitions
```

## Interfaces section

```yaml
interfaces:
  "a_bus":
    # a_bus description
    scope: [$rootio, some_module, a]
    clock: "clk_i"
    reset: "rst_ni"
    reset_type: "low"

    handshake: "ReadyValid"
    ready: "a_ready"
    valid: "a_valid"

  "b_bus":
    # b_bus description
    # ...

  "axi_basic":
    # axi_basic description
    # ...

  "axi_wr":
    # axi_full_wr
    # ...
```

In this block we define all the buses that we need to analyze.
Each bus always must define those 4 elements:
- `scope` - scope path to the signals
- `clock` - name of the clock signal
- `reset` - name of the reset signal
- `reset_type` - reset active polarity "high" or "low"

It also has to define either `handshake` or `custom_analyzer`.
- `handshake` is used for single channel buses
- `custom_analyzer` is used for multichannel bus analyzers

In the last part there are signal names definitions.
Scope and signal names can be either a single string or an array of strings.
All signal names are relative to the scope.
In the example we defined that `ready` signal in the simulation trace is `$rootio.some_module.a.a_ready` and `valid` is `$rootio.some_module.a.a_valid`.

List of the supported analyzers and their required signals can be found in [Analyzers](analyzers.md).

## `common_clk_rst_ifs` section

To not rewrite same values to `clock`, `reset` and `reset_type` multiple times for different buses this section can be used.
We define common names in `common_clk_rst_ifs` section with a YAML anchor and later in the `interfaces` section we use it.

```yaml
common_clk_rst_ifs:
  first_common_clk: &first_common_clk
    clock: "clk_i"
    reset: "rst_ni"
    reset_type: "low"
  second_common_clk: &second_common_clk
    clock: "clk_second"
    reset: "rst_second"
    reset_type: "high"

interfaces:
  "a_":
    scope: "top"
    clk_rst_if: *first_common_clk

    handshake: "ReadyValid"
    ready: "a_ready"
    valid: "a_valid"

  "b_":
    scope: "top"
    clk_rst_if: *first_common_clk
    # ...

  "c_":
    scope: "top"
    clk_rst_if: *first_common_clk
    # ...

  "d_":
    scope: "top"
    clk_rst_if: *first_common_clk
    # ...

  "E_":
    scope: "top"
    clk_rst_if: *second_common_clk
    # ...
  "F_":
    scope: "top"
    clk_rst_if: *second_common_clk
    # ...
```

For example all 4 buses a-d use the same clock and reset signals. The remaining 2 (E and F) use other signals.

## Scopes section

To not rewrite long scope paths multiple times it can be defined inside `scopes` section and be referred to via YAML anchor.
They can also be nested to define a long common path part separatedly.

```yaml
scopes:
  base: &base_scope
    [top, tb, very, long, scope, name]

interfaces:
  "a_":
    scope: *base_scope
    clock: "clk_i"
    reset: "rst_ni"
    reset_type: "low"

    handshake: "ReadyValid"
    ready: "a_ready"
    valid: "a_valid"

  "b_":
    scope: *base_scope
    clk_rst_if: *common_clk
    # ...

  "A_":
    scope: [*base_scope, "A"]
    clk_rst_if: *common_clk
    # ...

  "B_":
    scope: [*base_scope, "B"]
    clk_rst_if: *common_clk
    # ...

  "C_":
    scope: [*base_scope, "C"]
    clk_rst_if: *common_clk
    # ...
```

In example `a_`'s and `b_`'s scopes are set to "top.tb.very.long.scope.name".
For `A`, `B` and `C` the scopes end with the respective letter eg. A - "top.tb.very.long.name.A".


## More examples

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
- reset_type: `low` or `high`
- handshake: possible values: `ReadyValid`, `CreditValid`, `AHB`, `APB`, `Custom`
- custom_handshake: if handshake is set to `Custom`, a name of a Python plugin should be provided

Scopes can also be nested. Example `.yaml` for `tests/test_dumps/nested_scopes.vcd`:

```
scopes:
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

Example `.yaml` for a multi channel bus:

```
interfaces:
  "ram_rd":
    scope: ["test_taxi_axi_ram", "uut"]
    clock: "clk"
    reset: "rst"
    reset_type: "high"

    custom_analyzer: "AXIRdAnalyzer"
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

For multi channel buses, you need to specify the analyzer, along with signals required by that analyzer.
- custom_analyzer: possible values: `AXIRdAnalyzer`, `AXIWrAnalyzer`, `\<name of custom python analyzer\>`

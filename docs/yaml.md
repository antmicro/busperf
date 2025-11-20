# YAML bus description

## Single channel bus

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

## Multi channel bus

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

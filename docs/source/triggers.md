# Triggers

By default, analyzers gather statistics over the entire simulation trace.
In order to narrow the analysis scope, triggers can be used to control the start/stop of the analyzers.
Triggers are provided by trigger sources and they are consumed by trigger sinks.
Triggers require the DAG format to work, otherwise Busperf will error out.

## Trigger sources

There are multiple trigger sources:
* time - create a trigger when simulation reaches the given time
* finite-state machines - create a trigger when the specified state is reached
* analyzers - create a trigger when specified bus conditions are met

### Analyzers

Triggers and their trigger conditions are defined next to signal definitions.
They can be based on the bus state, specific value of one of the bus signals, or a combination of both.

#### Bus state

For single channel buses, the definition is straightforward:
~~~yaml
triggers:
  "name":
    - state: busy/free/no transaction/backpressure/no data/reset/unknown
~~~

For multichannel buses, the channel must be specified:
~~~yaml
triggers:
  ar:
    "name":
      - state: busy/free/no transaction/backpressure/no data/reset/unknown
~~~

#### Signal value

Triggers can be set on any signal that was defined in the bus's signal definition.
~~~yaml
triggers:
  # whole signal value
  "name":
    - signal: rlast
      value: 1

  # part of signal value (bits 7:0)
  "name2":
    - signal: addr
      range: [7:0]
      value: [0x4]

  # many parts match (bits 31:30 have value 3(decimal) and bits 7:0 0xa(hex)
  "name3":
    - signal: addr
      range: [31:30, 7:0]
      value: [3, 0xa]
~~~

For multichannel buses, if the signal is defined in the channel, trigger definitions must also contain channel name.
~~~yaml
triggers:
  ar:
    "name":
      - signal: id
        range: [7:0]
        value: [0x0]
~~~

#### Combinations

**Any / OR**

The trigger activates when any of the subtriggers activate.

Example: `"trigger_name"` activates when the 7:0 bits of `addr` are set to 0x4 or when `rlast` is set to 1.
~~~yaml
triggers:
  "trigger_name":
    - signal: addr
      range: [7:0]
      value: [0x4]
    - signal: rlast
      value: 1
~~~

**All / AND**

The trigger activates when all subtriggers are active at the same time.

Example: `"addr_set"` activates when the bus is in a busy state and the `addr` signal matches the pattern.
~~~yaml
triggers:
  "addr_set":
    - all:
      - state: busy
      - signal: addr
        range: [7:0]
        value: [0x07]
~~~

#### Transactions

This type of analyzer trigger is only supported in the multichannel bus analyzers, and allows for triggers to be set on starts/ends of a bus transaction.
~~~yaml
triggers:
  _:
    "_start": "name"
    "_end": "name2"
~~~

{#fsm}
### Finite-state Machine

Users can create an FSM that changes states based on triggers activating.
For FSM to provide triggers, states can be marked with the `trigger_set` keyword followed by the name of the trigger.
Transitions are defined within the `transition_to` section.
Firstly it defines the state to which the transition leads, then a list of triggers that cause the transition.
If an empty list is provided, it defines an epsilon transition, meaning it happens without any extra condition.
Transitions are checked sequentially in the order of the definitions, so it is possible to have other transitions before the epsilon one.
This also means that if two triggers activate simultaneously and lead to different states, the transition defined earlier in the list takes precedence.

~~~yaml
"fsm":
  type: fsm
  states:
    "idle":
      transition_to:
        "data": ["control_only.time_range.data", "control_only.other_trigger.trigg"]
        "block": ["control_only.time_range.block"]
        "interrupt": ["interfaces.interrupt.trap"]
    "data":
      transition_to:
        "trigg": ["control_only.time_range.trigger"]
        "block": ["control_only.time_range.block"]
        "idle": ["control_only.time_range.timeout"]
    "interrupt":
      transition_to:
        "end": ["control_only.time_range.end"]
        "idle": []
    "trigg":
      trigger_set: "activate"
      transition_to:
        "idle": []
    "end":
      trigger_set: "end"
~~~

### Timers

Timers are the simplest of the triggers.
They activate on specific timestamps.

~~~yaml
"time_range":
  type: timer
  triggers:
    start: [1, 10, 15, 20]
    stop: [8, 14, 17, 25]
    trigg: [12, 16]
~~~

## Trigger sinks

Currently, triggers can be consumed by the following:
* analyzers
* finite-state machines

### Analyzers

Analyzers can be started and stopped using triggers:
* `activate_on` defines which triggers start the analyzer
* `deactivate_on` defines which triggers stop the analyzer.

Conditions can be combined with `all` and `any`, as in the analyzer trigger source.

~~~yaml
activate_on:
  - "hierarchical.trigger_name.a"

deactivate_on:
  - all:
    - "hierarchical.trigger_name.b"
    - "hierarchical.trigger_name.c"
  - "hierarchical.trigger_name.d"
~~~

### Finite-state Machine

When making transitions, the FSM depends on triggers from other sources (see [FSM](#fsm)).

## Example

Below you can see all types of triggers combined in one example. Trigger names starting with "hierarchical" are placeholders.

~~~yaml
common_clk_rst_ifs:
  common: &common_clk
    clock: "clk"
    reset: "rst"
    reset_type: "high"

"interfaces":
  "ram_rd":
    scope: [test_taxi_axi_ram, uut]
    clk_rst_if: *common_clk

    custom_analyzer: "AXIRdAnalyzer"
    ar:
      id: ["s_axi_rd", "arid"]
      ready: ["s_axi_rd", "arready"]
      valid: ["s_axi_rd", "arvalid"]
    r:
      id: ["s_axi_rd", "rid"]
      ready: ["s_axi_rd", "rready"]
      valid: ["s_axi_rd", "rvalid"]
      resp: ["s_axi_rd", "rresp"]
      last: ["s_axi_rd", "rlast"]

    # analyzer trigger sink
    activate_on:
      - "hierarchical.trigger_name.a"

    deactivate_on:
      - all:
        - "hierarchical.trigger_name.b"
        - "hierarchical.trigger_name.c"
      - "hierarchical.trigger_name.d"

    # analyzer trigger source
    triggers:
      # Channel based
      ar:
        name: # full name -> interfaces.ram_rd.ar.name
          - all:
            - state: busy
            - signal: id # name in "ar" description
              range: [7:0]
              value: [0x0]
      r:
        name: # full name -> interfaces.ram_rd.r.name
          - all:
            - state: busy
            - signal: id # name in "r" description
              range: [7:0]
              value: [0x0]
      # Transaction start/end
      _:
        "_start": "start" # full name -> interfaces.ram_rd.start
        "_end": "end" # full name -> interfaces.ram_rd.end

  ram_raddr:
    scope: [test_taxi_axi_ram, uut]
    clk_rst_if: *common_clk
    handshake: ReadyValid
    ready: [s_axi_rd, arready]
    valid: [s_axi_rd, arvalid]
    # addr is used by the trigger
    addr: [s_axi_rd, araddr]

    # analyzer trigger sink
    activate_on:
      - "interfaces.ram_rd.ar.name"

    deactivate_on:
      - "hierarchical.trigger_name.a"
      - all:
        - "hierarchical.trigger_name.b"
        - "hierarchical.trigger_name.c"
      - "hierarchical.trigger_name.d"

    # analyzer trigger source
    triggers:
      "addr_set": # full name -> ram_raddr.addr_set
        - all:
          - state: busy
          - signal: addr
            range: [7:0]
            value: [0x4]

"control_only":
  "time_range":
    type: timer
    triggers:
      start: [1000000, 1000000000, 3000000000]  # full name -> control_only.time_range.start
      stop:  [900000000, 2500000000]  # full name -> control_only.time_range.stop

  "fsm":
    type: fsm
    states:
      "idle":
        transition_to:
          "data": ["interfaces.ram_rd.ar.name"]
      "data":
        transition_to:
          "trigg": ["interfaces.ram_rd.r.name"]
          "idle": ["interfaces.ram_rd.end"]
      "trigg":
        trigger_set: "name" # full name -> control_only.fsm.name
        transition_to:
          "idle": []
~~~

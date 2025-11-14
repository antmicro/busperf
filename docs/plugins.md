# Busperf plugins

Busperf allows extending its funcionality through python plugins. Custom analyzer can be spcified in YAML file
under the `custom_analyzer` and `custom_handshake` keys. To prevent names collisions between native and python
analyzers all python ones are recommened to start with "Python". Some examples are available in `plugins/python`.

## Analyzer requirements

Plugin should define a `create()` function that returns an analyzer object.
There are 2 types of python plugins `custom_handshake`s and `custom_analyzer`s with different uses.

### Custom handshake

Custom handshakes are used to calculate statistics for single channel buses. The analyzer object should
define the following methods:

* get_signals(self) -> list[str]
  * Return value
    An array of string names of signals that are required by the analyzer.
* interpret_cycle(self, signals) -> CycleType
  * signals - array of signal values (casted to string) during clock cycle
  * Return value
    Interpreted state of the bus during that clock cycle. See [CycleType](#CycleType)

### Custom analyzer

Custom analyzers are used to calculate statistcs for multi channel buses. Their analyzer object must define
the following methods:

* `get_yaml_signals(self) -> list[tuple[SignalType, list[str]]]`
  * Return value
    Returns types and paths to each signal defined in yaml that is required by the analyzer. See [SignalType](#SignalType)
* `analyze(self, clk, rst, ...) -> list[Transaction]`
  * Parameters
    Method takes as arguments signals requested in `get_yaml_signals`, clk and rst signals are always included
    and passed as first 2 arguments, all remaining are passed in same order as in `get_yaml_signals`. Each signal
    is an array of tuples `(time: int, value: str)`.
  * Return value
    Method should return a list of transactions that were present on a bus. See [Transaction](#Transaction)

## Interface types

All types that are used on rust-python interface are made available in a `busperf` module, that is created at
runtime and made accessible for the plugins. It defines the following types based on rust structs:

### CycleType

This enum represents state of single channel.

~~~python
class CycleType:
    Busy = <CycleType.Busy>                     # performing transaction
    Free = <CycleType.Free>                     # bus is not used
    NoTransaction = <CycleType.NoTransaction>   # transaction is not performed
    Backpressure = <CycleType.Backpressure>     # backpressure
    NoData = <CycleType.NoData>                 # receiver ready but no data is avaible to tranfer
    Reset = <CycleType.Reset>                   # reset signal active, bus in reset
    Unknown = <CycleType.Unknown>               # invalid/unknown state
~~~

### SignalType

This enum represents what data about signal(s) does the analyzer expect.

~~~python
class SignalType:
    Signal = <SignalType.Signal>              # passes every signal change's time and value to the analyzer
    RisingSignal = <SignalType.RisingSignal>  # passes times of every rising edge of the signal (useful for e.g. clk)
    ReadyValid = <SignalType.ReadyValid>      # passes all time a transaction is performed on ready/valid channel
~~~

### Transaction

This type represents one transaction of the analyzed multichannel bus.

~~~python
class Transaction:
    def __init__(
        self,
        start: int,
        first_data: int,
        last_data: int,
        resp_time: int,
        resp: str,
        next_start: int
    ):
        self.start = start             # time of command issue
        self.first_data = first_data   # time of first data being tranfered
        self.last_data = last_data     # time of last data transfer
        self.resp_time = resp_time     # time of response
        self.resp = resp               # value of the response
        self.next_start = next_start   # start time of next transaction
~~~

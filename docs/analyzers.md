# Analyzers

Busperf provides analyzers for some common buses. Their list and description is provided here.

## Single channel buses

### Ready/valid

**Handshake name**: ReadyValid

**Required signals**:
* `ready`
* `valid`

### AHB

**Handshake name**: AHB

**Required signals**:
* `htrans`
* `hready`

### APB

**Handshake name**: APB

**Required signals**:
* `psel`
* `penable`
* `pready`

### Credit/valid

**Handshake name**: CreditValid

**Required signals**:
* `credit`
* `valid`

## Multi channel buses

### AXI read

**Analyzer name**: AXIRdAnalyzer

**Required signals**:
* `ar`
  * `id` (not required for AXI Lite)
  * `ready`
  * `valid`
* `r`
  * `id` (not required for AXI Lite)
  * `ready`
  * `valid`
  * `resp`
  * `last` (not required for AXI Lite)

### AXI write

**Analyzer name**: AXIWrAnalyzer

**Required signals**:
* `aw`
  * `id` (not required for AXI Lite)
  * `ready`
  * `valid`
* `w`
  * `ready`
  * `valid`
  * `last` (not required for AXI Lite)
* `b`
  * `id` (not required for AXI Lite)
  * `ready`
  * `valid`
  * `resp`

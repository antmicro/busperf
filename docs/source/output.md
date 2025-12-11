# Output

For each described bus busperf will calculate and display:

## Single channel

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

## Multi channel
- `Cmd to completion`: Number of clock cycles from issuing a command to receving a reponse.
- `Cmd to first data`: Number of clock cycles from issuing a command to first data being transfered.
- `Last data to completion`: Number of clock cycles from last data being transfered to transaction end.
- `Transaction delays`: Delays between transactions in clock cycles
- `Error rate`: Percentage of transactions that resulted in error.
- `Bandwidth`: Averaged bandwidth in transactions per clock cycle.

## Examples

### Single channel buses

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

### Multi channel buses
```
╭──────────┬───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┬──────────────────────┬─────────────────────────────┬───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┬───────────────────┬─────────────────────────┬─────────────────────────────────┬───────────────────────────────╮
│ bus name │ Cmd to completion                                                                                                         │ Cmd to first data    │ Last data to completion     │ Transaction delays                                                                                                                                                        │ Error rate        │ Bandwidth               │ x rate                          │ y rate                        │
├──────────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┼──────────────────────┼─────────────────────────────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┼───────────────────┼─────────────────────────┼─────────────────────────────────┼───────────────────────────────┤
│ ram_rd   │ 1-2k x120; 16-31 x126; 4-7 x650; 32-63 x158; 8-15 x364; 2-3 x979; 256-511 x417; 64-127 x333; 512-1023 x44; 128-255 x141   │ 4-7 x1282; 2-3 x2050 │ 0 x3332                     │ 32-63 x127; 2-4k x32; 512-1023 x32; 16-31 x483; -2 x682; -1 x106; 4-7 x200; 128-255 x71; 1-2k x64; 64-127 x27; 0 x638; 4-8k x16; 2-3 x108; 256-511 x147; 8-15 x565; 1 x34 │ Error rate: 0.00% │ Bandwidth: 0.0046 t/clk │ Bandwidth above x rate: 100.00% │ Bandwidth below y rate: 0.00% │
│ ram_wr   │ 16-31 x136; 2-3 x1082; 8-15 x445; 256-511 x467; 128-255 x141; 1-2k x84; 64-127 x324; 32-63 x164; 4-7 x1536; 512-1023 x129 │ 1 x3624; 2-3 x884    │ 4-7 x685; 2-3 x414; 1 x3409 │ 0 x1567; 8-15 x906; -2 x165; 128-255 x70; -1 x157; 2-4k x16; 4-8k x17; 32-63 x19; 2-3 x877; 256-511 x148; 1-2k x80; 512-1023 x17; 64-127 x27; 16-31 x179; 4-7 x263        │ Error rate: 0.00% │ Bandwidth: 0.0062 t/clk │ Bandwidth above x rate: 100.00% │ Bandwidth below y rate: 0.00% │
╰──────────┴───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┴──────────────────────┴─────────────────────────────┴───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┴───────────────────┴─────────────────────────┴─────────────────────────────────┴───────────────────────────────╯
```

![gui example](../screenshots/example.png)


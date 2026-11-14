# EX264: the virtual BLE controller and its peer from the page (#304)

Base: upstream/main `3d276edf`. The final source is the commit containing this receipt.
Inputs, hashes and the exact commands are in [inputs.json](inputs.json); the checks that ran
and their results in [checks.json](checks.json).

The CLI's `--ble` runs the guest's Bluetooth host against a virtual VHCI controller and a virtual
central on the S3, C3, C6 and classic ESP32, driven by `ble` script commands, with its `[ble]`
lines on stderr. The browser build had none of it: its BLE exports are the C3 full link
controller's (`esp32sim_ble_full` and the rest). This is the "WASM API for the virtual central"
item of #304, on the same `SocBus` calls the CLI makes:

- `esp32sim_ble_virtual(emu, elf_ptr, elf_len)`: before boot, parse the application ELF and call
  `SocBus::enable_ble`, as `--ble` does with `--elf`; then `SocBus::ble_keep_log`. 0, or 1 after
  boot, for a bad ELF, a chip without the controller or an ELF it cannot use, with the reason on
  `host_log`.
- `esp32sim_ble_virtual_command(emu, ptr, len)`: `SocBus::ble_command`, the script's `ble`
  commands (`connect`, `discover`, `read HANDLE`, `write HANDLE HEX`, `subscribe CCC_HANDLE`) at
  the current emulated time. 0, or 1 with the reason on `host_log`.
- `esp32sim_ble_virtual_take(emu)`: the oldest kept `[ble]` line (without the prefix) at
  `esp32sim_ble_ptr`, as `esp32sim_ble_take` hands over the full controller's observations.
- `esp_soc::ble::log::BleLog` (`esp-soc/src/ble/log.rs`): the controller's lines go to stderr as
  before, or, once `keep` is called, into a queue of at most 1024 lines (the oldest give way and
  come back as one `dropped=N` line). The shared VHCI controller (`ble::vhci::Ble`, S3, C3 and
  classic) and the C6's own (`esp32c6::ble::Ble`) write every `[ble]` line through it, the
  `--debug ble` packet dumps excepted; `enable` carries the queue over and a reset leaves it.
  `SocBus::ble_keep_log` / `ble_log_take` (defaults: nothing) reach it from the page.

Nothing changes for the CLI: `keep` is never called there, so every line still goes to stderr in
the same words. Nothing runs per tick.

## Firmware

[ble-server.ino](ble-server.ino), Arduino-ESP32 3.3.12: a GATT server with one characteristic
(read, write, notify every 500 ms) under Arduino's BLE library, NimBLE on the S3 and C3 and
Bluedroid on the classic; UART0 marks advertising, connections and writes. Each build ran twice
for 4 emulated seconds, both from the mask ROM:

- **CLI:** `--ble --elf ble-server.ino.elf --script` with `1.2 ble connect`, `1.4 ble discover`,
  `1.8 ble read V`, `1.9 ble write V 6869`, `2.0 ble subscribe CCCD`;
- **page:** [ble-virtual-run.mjs](ble-virtual-run.mjs) under Node 24, the production
  `esp32sim.wasm`: the same images, `esp32sim_ble_virtual` with the same ELF, then
  `esp32sim_run` up to each command's emulated time, the command through
  `esp32sim_ble_virtual_command`, and every line through `esp32sim_ble_virtual_take`.

V / CCCD are `0x0010` / `0x0011` under NimBLE and `0x002a` / `0x002b` under Bluedroid, from the
discovery.

| Chip | `[ble]` lines, CLI and page | Text and order | Largest time difference | UART `ble:` lines |
| --- | ---: | --- | ---: | --- |
| S3 | 38 and 38 | identical | 35 µs | `advertising`, `connected`, `write hi` on both |
| C3 | 38 and 38 | identical | 2 µs | the same |
| classic | 33 and 33 | identical | 2 µs | the same |
| C6 | refused | `BLE requires ELF symbol r_os_mempool_init` on both | — | — |

Both sides show the advertising data, the connection, the discovered services, characteristics
and attributes, the read (`value=6e3d31 text="n=1"`, `n=0` on the classic), both write
completions and the notifications (`n=2` to `n=5`). A line's time is the emulated time at which
the controller hands the log over, which depends on where a run slice ends, so the page's times
differ from the CLI's by up to 35 µs on the S3 and 2 µs on the others. The four logs of each side
are in [runs/](runs/).

The C6 build of Arduino-ESP32 3.3.12 has no `r_os_mempool_init`, which the C6 controller needs;
the CLI and the page refuse it with the same reason.

## Tests

- `esp-soc/src/ble/log.rs`: nothing is kept before `keep`; lines come back in order; past 1024
  the oldest give way and one `dropped=N` line comes first; a second `keep` keeps the queue.
- `esp32s3/src/ble.rs`, `kept_lines_go_to_ble_log_take_instead_of_stderr`: with the log kept, a
  new `enable` and a reset, a rejected VHCI packet's line reaches `SocBus::ble_log_take`.
- `wasm/tests/abi.rs`, `virtual_ble_is_enabled_from_the_elf_and_takes_peer_commands`: a synthetic
  ELF with the controller's symbols enables the S3's controller; the P4, a non-ELF and an ELF
  missing a symbol are refused; a command before enabling, and an unknown one, are refused; the
  script's commands are taken.
- `wasm/tests/ble-api.mjs` (CI's BLE ABI script): after boot `esp32sim_ble_virtual` is refused
  with "before boot"; on the P4 it is refused, a command is refused with "BLE is not enabled",
  and nothing waits to be taken.

## CPU comparison

Not run: no bus, `device_set!`, interrupt routing or per-tick code. The CLI path only goes
through `BleLog::note` where it called `eprintln!`.

## Limits

- The page drives the virtual central the CLI has: the guest is the peripheral, one connection,
  the commands above. The other direction of #304, the guest as central to a real peripheral, and
  a `navigator.bluetooth` shim for an unchanged Web Bluetooth page are not in this change.
- The devices around the station (`--ble-nearby`) have no page export yet.
- The C6 ran no firmware here (above). The classic and the C3 / S3 ran the same sketch under
  different BLE hosts; no other firmware was run.
- The firmware images are not committed; their hashes are in inputs.json.

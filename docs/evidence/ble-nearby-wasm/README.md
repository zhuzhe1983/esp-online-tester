# EX266: the devices around the station from the page (#304)

Base: upstream/main `3d049f5c`. The final source is the commit containing this receipt.
Inputs, hashes and the exact commands are in [inputs.json](inputs.json); the checks that ran
and their results in [checks.json](checks.json).

The CLI's `--ble-nearby` (EX246) gives the virtual BLE controller devices around the station,
which the guest's scans report. The page got the virtual controller and its peer in #343 (EX264),
whose limits say the devices around have no page export yet. This is that export, on the same
calls the option makes:

- `esp32sim_ble_nearby(emu, ptr, len)` (`wasm/src/ble.rs`): UTF-8 text with one `--ble-nearby`
  spec per line (blank lines skipped; no lines for none), parsed by `peer::Nearby::parse_all` as
  the option's specs are, then handed to `SocBus::ble_nearby`, replacing the devices there were.
  It needs `esp32sim_ble_virtual` and works before or after boot: as with the option, a running
  scan reports a new list at once and later scans report it. 0, or 1 for text that is not UTF-8,
  a spec or list the option refuses (an unknown key, two devices on one address, the peer's or
  the guest's address, more than 32) or no virtual controller, with the reason on `host_log` and
  the devices unchanged.

The CLI is untouched. Nothing runs per tick.

## Firmware

EX246's scanner, [ble-scan-probe.ino](../ble-nearby/ble-scan-probe.ino), the same S3 and C3
builds (hashes in inputs.json match EX246's): an active scan for 2 s every 3 s that prints
`found name=… addr=… rssi=… uuid=…` per device and `scan done n=…`. From the mask ROM, with the
application ELF for the virtual controller:

- **CLI:** `--ble --ble-nearby name=Thermo,rssi=-70,addr=c4:aa:01:02:03:04,type=public,rsp=03030f18
  --ble-nearby name=Scale,rssi=-82`, 8 emulated seconds, UART0;
- **page:** [ble-nearby-run.mjs](ble-nearby-run.mjs) under Node 24 with the production
  `esp32sim.wasm`: `esp32sim_ble_virtual` with the same ELF, then `esp32sim_ble_nearby` with the
  same two specs before boot ([plan-static.json](plan-static.json)), 8 emulated seconds.

| Chip | UART0, CLI and page |
| --- | --- |
| S3 | identical, 19 lines: `Ready`, then twice `found name=esp32sim …`, `found name=Thermo addr=c4:aa:01:02:03:04 rssi=-70 uuid=0000180f-…`, `found name=Scale addr=ce:34:5f:b3:8c:4b rssi=-82 uuid=-`, `scan done n=3` |
| C3 | identical, 18 lines, the same scans |

Then the page changes the list while the guest runs ([plan-changes.json](plan-changes.json), 15
emulated seconds; the scans end at about 2.3, 5.3, 8.3, 11.3 and 14.3 s):

| At | `esp32sim_ble_nearby` | The scan that follows (S3 and C3 alike) |
| --- | --- | --- |
| before boot | Thermo, Scale: 0 | ends 2.3 s: esp32sim, Thermo, Scale, `n=3` |
| 4 s, during the second scan | Lamp (`c4:aa:01:02:03:05`, -60 dBm): 0 | ends 5.3 s: esp32sim, Thermo, Lamp, Scale, `n=4`: the running scan hears Lamp at once and keeps what it heard |
| | | ends 8.3 s: esp32sim, Lamp, `n=2` |
| 9 s, between scans | `name=Thermo` twice: 1, `[emu] ble nearby: two nearby devices share the address c7:b3:92:b1:d8:8d` | ends 11.3 s: esp32sim, Lamp, `n=2`: unchanged |
| 12 s, between scans | no lines: 0 | ends 14.3 s: esp32sim, `n=1` |

The outputs are in [runs/](runs/): `cli-<chip>.txt` (the CLI's stdout), `page-<chip>.txt` and
`page-<chip>-changes.txt` (the latter with the emulated time at which the page took each line).

## Tests

- `wasm/tests/abi.rs`, `ble_nearby_takes_the_options_specs_once_the_virtual_controller_is_on`:
  refused on the P4 and on the S3 before `esp32sim_ble_virtual`; then EX246's two specs, CRLF and
  blank lines, and no lines are taken; two devices on one address, an unknown key, text that is
  not UTF-8 and 33 devices are refused; 32 are taken. The controller ELF of
  `virtual_ble_is_enabled_from_the_elf_and_takes_peer_commands` moved to a helper both use.
- `wasm/tests/ble-api.mjs` (CI's BLE ABI script): on the P4 a list is refused with
  "BLE is not enabled", and a list with two devices on one address with "share the address".

Six mutations each fail the ABI test: the list not handed to `SocBus::ble_nearby`, the specs
parsed one by one without `parse_all`'s checks, text that is not UTF-8 read as empty, lines not
trimmed, blank lines kept, and the bus's refusal answered 0.

## CPU comparison

Not run: no bus, `device_set!`, interrupt routing or per-tick code; the export only calls what
the CLI option calls.

## Limits

- What EX246 says of the devices around holds: legacy advertising, scannable and not
  connectable, reported once per scan start or list change.
- Firmware ran on the S3 and the C3; the C6 and the classic share `SocBus::ble_nearby` and the
  session, and are covered by the build only.
- The firmware images are not committed; their hashes are in inputs.json.

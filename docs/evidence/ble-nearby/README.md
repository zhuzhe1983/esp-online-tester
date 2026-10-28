# EX246: Bluetooth LE devices around the station (`--ble-nearby`)

Base: upstream/main `f1b5c5d5`. The final source is the commit containing this receipt.
Inputs, hashes and the exact commands are in [inputs.json](inputs.json); the checks that ran and
their results in [checks.json](checks.json).

With `--ble` a guest's scan hears one advertiser, the virtual peer `esp32sim`, so firmware that
lists devices, picks one by name or signal, or filters by service cannot be exercised against more
than one. `--ble-nearby SPEC` (repeatable, at most 32, needs `--ble`) adds advertisers around the
station, the Bluetooth counterpart of `--wifi-nearby` (EX237):

- `peer::Nearby`: address (HCI order) and type, RSSI, advertising data and scan response.
  `Nearby::parse` reads `name=…` (a Complete Local Name after the LE General Discoverable Flags,
  1 to 26 bytes so both fit the 31-byte advertising data), `adv=`/`rsp=` (raw data as hex, at most
  31 bytes each; `adv` instead of `name`), `addr=aa:bb:cc:dd:ee:ff` (default: a random static
  address derived from the advertising data, the same every run), `type=random|public`, `rssi=`.
  `Nearby::parse_all` refuses two devices on one address and a device on the virtual peer's or the
  guest's address: two advertisers on one address would answer as one.
- The virtual peer reports each device when the guest starts a scan, after its own report, as an LE
  Advertising Report: ADV_SCAN_IND (scannable, not connectable) with the advertising data, and for
  an active scan SCAN_RSP with the scan response (an empty one too, as a scannable advertiser
  answers every scan request). A list set during a scan is reported at once; after LE Set Scan
  Enable turns scanning off (`LinkEvent::ScanStopped`, new) nothing is. Nothing connects to a
  device around. The list survives a guest reset (`Session::reset`), as the devices are not the
  chip's.
- `SocBus::ble_nearby` hands the list to the virtual controller on the S3, C3, C6 and classic
  ESP32 (`vhci::Ble::set_nearby`, the C6's `Ble::set_nearby`). The CLI checks the specs with the
  other arguments, before the ROM loads, and names each device:
  `[emu] device around: c4:aa:01:02:03:04 (public) -70 dBm name="Thermo"`.

Nothing per tick: the devices matter only when the guest starts a scan or the list changes during
one. Without the option nothing changes; the BLE goldens pass byte-identical. The virtual peer's own
advertising and scan response are unchanged.

## Public definitions

- Bluetooth Core Specification 5.4, Vol 4 Part E 7.7.65.2 (LE Advertising Report event:
  subevent 0x02, Num_Reports, Event_Type 0x02 ADV_SCAN_IND and 0x04 SCAN_RSP, Address_Type, Address,
  Data_Length, Data, RSSI), 7.8.10 (LE Set Scan Parameters: LE_Scan_Type 0x00 passive, 0x01 active)
  and 7.8.11 (LE Set Scan Enable); Vol 6 Part B 1.3.2.1 (a static device address has its two most
  significant bits set); Vol 3 Part C 11 (advertising and scan response data: length, AD type,
  data, at most 31 bytes for legacy advertising).
- Supplement to the Bluetooth Core Specification, Part A 1.2 (Local Name, AD type 0x09 Complete)
  and 1.3 (Flags, AD type 0x01; 0x06 = LE General Discoverable, BR/EDR not supported).

No radio is modelled: a device around is a report the virtual controller sends while the guest
scans, with the given RSSI; no timing, channel or collision claim.

## Tests

`esp-soc/src/ble/peer.rs`:

- `nearby_specs_parse_with_defaults_and_their_own_addresses`: the data `name` builds, the address
  in HCI order, `type`, the derived random static address (stable, distinct per name), every
  invalid key and value, a name too long for the data, and `parse_all` refusing a shared address,
  the virtual peer's and the guest's address and more than 32 devices.
- `a_scan_reports_the_devices_around_after_the_virtual_peer`: an active scan gets ADV_SCAN_IND and
  SCAN_RSP for the device with its address type, data and RSSI, a passive scan ADV_SCAN_IND only, after
  the virtual peer's own report; nothing before a scan.
- `a_running_scan_hears_a_new_list_at_once_and_a_stopped_one_nothing`.
- `the_devices_around_survive_a_guest_reset`.

Each fails with its rule removed (the report at scan start, `ScanStopped`, the list kept over the
reset, the shared-address check).

`cli/tests/ble_nearby_option.rs`: `--ble-nearby` without `--ble`, a spec without a name or data, a
device on the virtual peer's address and two devices on one address exit with status 2 and say why,
before the ROM is needed.

## Firmware

[ble-scan-probe.ino](ble-scan-probe.ino), Arduino-ESP32 3.3.12 (ESP-IDF v5.5.5): an active scan for
2 s every 3 s that prints `found name=… addr=… rssi=… uuid=…` per device and `scan done n=…`. Built
for the ESP32-S3 and the ESP32-C3 (the C3 build's source has one more comment line at the top), run
from the mask ROM with `--ble --elf <app ELF>` for 8 emulated seconds (commands in inputs.json), with
`--ble-nearby name=Thermo,rssi=-70,addr=c4:aa:01:02:03:04,type=public,rsp=03030f18
--ble-nearby name=Scale,rssi=-82`:

| Chip | Console, each of the two scans |
| --- | --- |
| S3 | `found name=esp32sim addr=02:53:49:00:00:02 rssi=-35 uuid=0000180f-…`, `found name=Thermo addr=c4:aa:01:02:03:04 rssi=-70 uuid=0000180f-…` (the service from the scan response), `found name=Scale addr=ce:34:5f:b3:8c:4b rssi=-82 uuid=-` (the derived random static address), `scan done n=3` |
| C3 | the same four lines |
| S3, C3 without `--ble-nearby` | `found name=esp32sim …`, `scan done n=1`, as on main |

## Limits

- Legacy advertising only (ADV_SCAN_IND / SCAN_RSP, 31 bytes each); no extended advertising, no
  connectable device around: no ADV_IND, ADV_DIRECT_IND or ADV_NONCONN_IND.
- A device around is reported once per scan start (or list change during a scan), not repeatedly
  at an advertising interval; a scanner that counts reports over time sees one.
- The full C3 controller (`--ble full`) does not carry the list: the option needs `--ble`.
- Firmware was run on the S3 and the C3; the C6 and classic paths share the session code
  (`peer::Session`) and are checked by the build and the unit tests only.
- The firmware images are not committed and no golden pins them. No CPU comparison: nothing runs
  per tick, and the instruction counts of the goldens are unchanged.

## Review follow-up

Main `ddf753a5` merged into the branch (the catalog row moves below EX233). The points of the
[review](https://github.com/mikroverk/esp32sim/pull/286):

- **Event type.** The devices around report ADV_SCAN_IND (0x02: scannable, not connectable; Vol 4
  Part E 7.7.65.2 Event_Type, Vol 6 Part B 2.3.1.4) instead of ADV_IND (0x00, connectable), so a
  guest that filters for connectable devices does not see them as such; the SCAN_RSP still follows
  for an active scan. The virtual peer's own reports are unchanged.
- **RSSI.** `rssi` takes -127 to +20 dBm, or 127 for "not available", the range an LE Advertising
  Report carries (7.7.65.2).
- **Random address kind.** A random `addr` with the reserved top bits 10 (Vol 6 Part B 1.3.2) is
  refused; a public one is not checked.
- **`name` with `adv`** is an error instead of `name` being ignored.
- **`docs/cli.md`** says a `name` cannot hold `,` (use `adv`), and that the devices are reported as
  scannable and not connectable.

Tests: the scan test expects 0x02; the parse test adds both name and adv, RSSI -128, 21 and 126 and
a random address with top bits 10 (refused), and -127, 20, 127 and a public address with top bits
10 (accepted). Each new rule fails a test when removed. The firmware runs above give the same four
lines on the S3 and the C3 (the scan lists ADV_SCAN_IND devices with their scan response).

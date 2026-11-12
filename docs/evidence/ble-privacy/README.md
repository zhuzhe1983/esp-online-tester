# EX248: LL Privacy and the resolving list on the virtual BLE controller (#296)

Base: upstream/main `2718410a`. The final source is the commit containing this receipt.
Inputs, hashes and the exact commands are in [inputs.json](inputs.json); the checks that ran and
their results in [checks.json](checks.json).

With `--ble`, ESP-IDF v5.5.4 Bluedroid's `gatt_client` example (SMP on, the default) panicked on
the ESP32-S3 before its first scan: StoreProhibited at address 0 in
`btm_ble_enq_resolving_list_pending` (#296). The virtual controller answered LE Read Local
Supported Features with zeros, so it did not report LL Privacy; Bluedroid then never read the
resolving list size nor allocated its pending queue (`device/controller.c:285-289`,
`stack/btm/btm_devctl.c:197-200`), yet with `CONTROLLER_RPA_LIST_ENABLE` it still queued its
default entry once SMP had made the local IRK (`stack/btm/btm_ble.c:2616-2618`,
`stack/btm/btm_ble_privacy.c:71-79, 1179-1202`).

The controller now does what an ESP32-S3's own controller does for these commands:

- LE Read Local Supported Features reports bit 6, LL Privacy (Bluetooth Core 5.4 Vol 6 Part B
  4.6). No other feature bit is added.
- Read Local Supported Commands reports octet 34 bits 3-7 (LE Add / Remove Device To / From
  Resolving List, Clear Resolving List, Read Resolving List Size, Read Peer Resolvable Address),
  octet 35 bits 0-2 (Read Local Resolvable Address, Set Address Resolution Enable, Set Resolvable
  Private Address Timeout) and octet 39 bit 2 (Set Privacy Mode) (Vol 4 Part E 6.27).
- A resolving list of 10 entries (Vol 4 Part E 7.8.38-7.8.45, 7.8.77), with the S3's statuses:
  - Add: 0x12 for an identity already in the list, 0x07 Memory Capacity Exceeded for the eleventh
    entry, else 0.
  - Remove: 0x02 Unknown Connection Identifier for an identity not in the list.
  - Clear: 0. Read Resolving List Size: 10.
  - Read Peer / Local Resolvable Address: 0x02 and a zero address (the S3 answers so for an
    entry too, as long as it has made no resolvable private address).
  - Set Address Resolution Enable: 0x12 for a value above 1.
  - Set Resolvable Private Address Timeout: 0x12 for 0 s only; the S3 also accepts 3601 s,
    above the 0x0E10 the specification gives.
  - Set Privacy Mode: 0x12 for a mode above 1, 0x02 for an identity not in the list.
  - HCI Reset and a guest reset empty the list.

  An address type above 1 (only 0 public and 1 random are valid, 7.8.38) follows the
  specification and was not probed: Add answers it 0x12; Remove, Set Privacy Mode and Read Peer
  Resolvable Address find no entry with it and answer 0x02.

The list keeps the peers' identity addresses only. Nothing is resolved against the IRKs and no
resolvable private address is made: the virtual peer and the devices around use identity or
static addresses, and the virtual controller has no pairing.

## The S3's answers

[hci-privacy-probe.ino](hci-privacy-probe.ino), Arduino-ESP32 core 3.3.7 (ESP-IDF
`v5.5.2-729-g87912cd291`), enables only the controller (`esp_bt_controller_enable`, no host stack)
and sends the commands over VHCI; it prints each Command Complete. Run on an ESP32-S3 board over
its USB-Serial/JTAG port; the controller reports HCI version 0x09, manufacturer 0x02E5 (Espressif),
subversion 0x0016. The output is [s3-answers.txt](s3-answers.txt):

| Command | S3 | Model |
| --- | --- | --- |
| LE Read Local Supported Features | `ff f9 01 08 00 00 00 00` | bit 6 only: `40 00 …` |
| Read Local Supported Commands, octets 34 / 35 / 39 | `ff` / `ff` / `07` | `f8` / `07` / `04` |
| LE Read Resolving List Size | 10 | 10 |
| Clear; Remove from an empty list | 0; 0x02 | 0; 0x02 |
| Add the all-zero entry; again; Remove; Remove again | 0; 0x12; 0; 0x02 | the same |
| Add entries 1 to 10; the eleventh | 0; 0x07 | the same |
| Set Address Resolution Enable 1; 2; Add with it on; 0 | 0; 0x12; 0; 0 | the same |
| Set RPA Timeout 900 s; 0; 3600 s; 3601 s | 0; 0x12; 0; 0 | the same |
| Set Privacy Mode: in the list; not; mode 2 | 0; 0x02; 0x12 | the same |
| Read Peer RPA: in the list; not; Read Local RPA | 0x02 each, zero address | the same |

The model reports only the bits of what it answers: the S3's other feature and command bits stay
off, so a host does not start using extended advertising, 2M PHY or other parts the model lacks.

## Firmware

| Firmware | Main `2718410a` | This change |
| --- | --- | --- |
| IDF v5.5.4 `examples/bluetooth/bluedroid/ble/gatt_client`, S3, default sdkconfig | `GATT client register`, then the StoreProhibited panic in `btm_ble_enq_resolving_list_pending` (backtrace below), and again after every reboot: 40 panics in 8 emulated seconds | `Scanning start successfully`, `Scan result, device 02:53:49:00:00:02, name len 8`, `esp32sim`; no panic |
| `ble-scan-probe.ino` of EX246 (Arduino-ESP32 3.3.12, NimBLE), S3 and C3 | each scan: `found name=esp32sim …`, `scan done n=1` | the same UART0 console |

The panic on main, symbolised from the ELF:

```
btm_ble_enq_resolving_list_pending at stack/btm/btm_ble_privacy.c:75
btm_ble_add_default_entry_to_resolving_list at stack/btm/btm_ble_privacy.c:1201
btm_ble_process_irk at stack/btm/btm_ble.c:2617
btm_ble_process_dhk at stack/btm/btm_ble.c:2665
btm_ble_process_ir2 at stack/btm/btm_ble.c:2699
btm_ble_rand_enc_complete at stack/btm/btm_ble.c:1128
```

NimBLE sees the change too. On main it asked for LE Set Address Resolution Enable (0x202D), got
Unknown Command and went on. Now that command succeeds and NimBLE continues its controller
privacy set-up: LE Clear Resolving List, Set Address Resolution Enable 1, LE Add Device To
Resolving List (its own IRK, the all-zero identity) and LE Set Privacy Mode, each answered 0. Its
scan is unchanged: the EX246 scan sketch's UART0 console is the same on the S3 and the C3. In the
emulator's `[ble]` log the unsupported 0x202D line is gone (the vendor 0xFC01 stays, as on main),
and the virtual peer's scan reports come 10 ms of emulated time later, after the five more
commands at start-up.

## Tests

`esp-soc/src/ble/peer.rs`, `ll_privacy_is_reported_and_the_resolving_list_answers_as_on_an_s3`:
the feature bit, the command bits, the size, then the probe's command sequence with the S3's
status for each, and HCI Reset and a guest reset emptying the list. Each rule removed fails it:
the feature bit, the command bits, the size, the duplicate and capacity checks, Remove's 0x02,
Clear, the list emptied by HCI Reset, Set Address Resolution Enable's 0x12, the RPA timeout's 0,
Set Privacy Mode's value check and list lookup, and the Read Resolvable Address status
(13 mutations, each fails the test).

## Limits

- No address resolution and no resolvable private address: an advertiser or initiator using an
  RPA is not resolved against the list, and Read Peer / Local Resolvable Address always answer
  0x02.
- The IRKs are not kept; the S3's privacy mode (network or device) has no effect.
- Adding to or removing from the list while address resolution is on and the controller is
  advertising, scanning or initiating is accepted; the specification disallows it (0x0C), which
  the probe did not exercise on the S3.
- The size and statuses come from one ESP32-S3; the C3, C6 and classic ESP32 controllers were not
  probed, and the same answers are used for them. The classic may differ: IDF v5.5.4 gives it
  network privacy mode only (`components/bt/host/bluedroid/Kconfig.in:477`) and
  `SOC_BLE_DEVICE_PRIVACY_SUPPORTED 0` (`components/soc/esp32/include/soc/soc_caps.h:454`; 1 on
  the S3, C3 and C6), so its controller may not take Set Privacy Mode as the S3 does. Its
  Bluedroid does not take this path (`CONFIG_BT_BLE_RPA_SUPPORTED` defaults to n there).
- The firmware images are not committed and no golden pins them. No CPU comparison: nothing runs
  per tick, only a few more HCI commands at host start-up.

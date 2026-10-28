# Guest SoftAP station and Ethernet bridge

Implementation: `1f0de1b06cf3780820a38904f4b33eb1f08183c8`, based on `ea69be70`.
This extends [EX202](../../experiments.md#ex202)'s host Ethernet contract in the opposite
Wi-Fi direction: a host station joins the firmware's own AP. It reuses the air scheduler,
RX descriptors and framing used by [EX237](../../experiments.md#ex237). No execution or
scheduler optimization is proposed.

The opt-in station discovers the actual transmitted beacon/probe response, performs open
authentication and association, and exchanges Ethernet through the guest's real 802.11
frames. The S3 WASM exports configure/poll/status, frame injection, and a bounded drained
outbox; the host supplies DHCP/TCP/UDP. Configuration accepts an optional target SSID and
WPA2 passphrase; status excludes the passphrase. Disabled clients add no per-tick work.

## Real firmware

The unchanged M5Burner StopWatch Xiaozhi 2.2.6 merged image was downloaded from
[the public firmware artifact](https://m5burner-cdn.m5stack.com/firmware/05a2cf9f8c71e46dbd0dca65137ad0c1.bin).
SHA-256: `6ed1ed429c275504c1cc32a1535f8f961b16509e87b231fd1dfd15fcff1fc21a`;
11,240,285 bytes. Its boot log identifies ESP-IDF v5.5.2. The test uses the StopWatch board,
16 MiB flash, 8 MiB PSRAM, the S3 mask ROM and a fixed synthetic station MAC. The firmware
image and raw console are not committed.

```sh
ESP32SIM_SOFTAP_FIRMWARE="$FIRMWARE" ESP32SIM_ROM_DIR="$ROMS" \
  cargo test --release -p esp32s3 --test softap -- --ignored --nocapture
```

Result: pass. At 2,360 modeled milliseconds the station is associated; by 2,370 ms the
guest answers an ARP request for `192.168.4.1`. The firmware also emits its own station-join
callback. The check requires a real guest ARP reply, not a host-synthesized response or
merely the emulator's connected flag. It does not establish HTTP, Internet or browser
performance; those are integration checks outside this receipt.

The first run failed authentication because the inherited S3 RX metadata set bits 28 and
29 for every unicast. The IDF v5.5.2
[`esp_wifi_types_native.h`](https://github.com/espressif/esp-idf/blob/v5.5.2/components/esp_wifi/include/local/esp_wifi_types_native.h#L38) declares these bits reserved.
The matching [esp32-wifi-lib revision](https://github.com/espressif/esp32-wifi-lib/tree/01d52d9e69032c486015dc28b08c3bf6aaf348a9)
was checked: in ESP32-S3 `libpp.a`, `ppRxPkt` prefers the STA callback when word 0 bit 28
is set and uses the AP callback only when bit 28 is clear and bit 29 is set.
Archive SHA-256: `8ce670b099162eb91c738afdf60bd8d3073dcf57bf6feb12431d2f4dbddc252d`.
Only frames from the enabled virtual station now carry bit 29 alone. Existing STA metadata
is unchanged. This is inference from the matching driver plus a firmware check, not a
silicon measurement or a claim for other chip layouts.

## Focused checks and limits

- `cargo test -p esp-soc wifi --lib`: 25 passed, including existing STA tests and six new
  client tests for open/WPA2 exchanges, MIC mismatch, required credentials, malformed frames,
  bounded queues, multicast, disable/reboot and target SSID selection.
- `cargo test -p esp32s3 --lib wifi_softap_rx_selects_ap_without_changing_station_metadata`:
  passed. It reads actual RX descriptor flags for enabled and disabled client frames,
  existing AP unicast and broadcast.
- `cargo clippy -p esp-soc -p esp32s3 -p esp32sim-wasm --all-targets -- -D warnings`: passed.
- The external unmodified firmware test above: passed.
- WPA2 four-way handshake and framing are unit-tested against the existing virtual AP;
  real WPA2 SoftAP firmware is not yet checked here. CCMP follows the emulator's existing
  hardware-boundary abstraction. WPA3, WEP and enterprise authentication are rejected.
- The guest SoftAP adapter is enabled for ESP32-S3 only. Other chip adapters report
  unsupported. No driver/NVS success injection, ROM hooks or firmware patches are used.

## Disabled-client CPU qualification

The serial harness [bench.py](bench.py) uses child user CPU time from
`resource.getrusage(RUSAGE_CHILDREN)`, with one warmup pair followed by three measured
pairs in alternating baseline/candidate order. S3 hello uses 3,000 modeled seconds, as in
EX237; C3 hello, C6 hello and Pocket Tank use 30. Every run must have identical per-core
instruction counts and console SHA-256 for its workload. No Wi-Fi/SoftAP flag is enabled.

Both revisions were built with `cargo build --release --bins`, default flags, Rust 1.99.1
(`58cf8f9c3`, 2026-10-13), on Apple M5 Max / arm64 / macOS 27.0.1. Emulator environment
overrides are removed by the harness. No concurrent emulator test or Cargo build was
scheduled during the measurement; unrelated desktop load is uncontrolled and recorded only
as aggregate load averages. This is a regression screen, not a speedup or equivalence claim.

```sh
python3 docs/evidence/softap-client-2026-10-28/bench.py \
  --baseline "$BASE_BINS" --candidate "$NEW_BINS" \
  --firmware web/wasm/fw/public --roms "$ROMS" \
  --model web/wasm/fw/local/model_q4.bin --output "$RESULTS"
```

The summary retains source and binary identities, input hashes, normalized commands, run
order, timings and output hashes. Private filesystem paths, host identity, raw application
inventories and raw console logs are not included. No numeric measurements are redacted. The first harness invocation stopped before C3
measurement because its CLI does not accept `--psram-mb 0`; the corrected harness passes
PSRAM only to S3, retains all completed S3 samples and resumes the remaining workloads.

Results, including all 32 samples and workload hashes: [cpu.json](cpu.json).

| Workload | Baseline median user s (range) | Candidate median user s (range) | Change | Candidate slower pairs |
| --- | ---: | ---: | ---: | ---: |
| s3-hello | 25.583193 (24.934542–25.652839) | 25.703687 (25.060334–26.012418) | +0.47% | 3/3 |
| c3-hello | 2.514738 (2.478050–2.528170) | 2.539754 (2.475295–2.547307) | +0.99% | 2/3 |
| c6-hello | 1.679080 (1.665290–1.712407) | 1.656550 (1.637770–1.672600) | -1.34% | 0/3 |
| pocket-tank | 45.040906 (42.721942–45.551030) | 42.887469 (42.052537–44.996563) | -4.78% | 0/3 |

All instruction totals and console hashes match. Ranges overlap for every workload;
three measured pairs with drifting desktop load do not establish a speedup, formal
equivalence, or a sub-percent regression bound. No sample was rejected for timing.

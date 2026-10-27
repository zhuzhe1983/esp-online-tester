# PaperMono SX1262 digital packet subset

The fixed official UserDemo 1.2.0 now initializes LoRa, sends **Hello!** from its actual FIFO after a touch, and renders **Rx PEER** after explicit digital peer input. No peer means no received data. Lite has no SX1262 and does not drive the absent BUSY/IRQ pins. This is a packet-level digital model, not RF propagation, hardware validation or remote device networking.

## Reproduce the real firmware path

Build `tools/wasm-build.sh`, obtain the documented ROM inputs, then run:

```sh
ESP32SIM_PROBE_CLOCK_TIMEOUT_MS=60000 node tools/probe-official-papermono.mjs \
  https://m5burner-cdn.m5stack.com/firmware/376080df0d31b0026e9a67a75d02d571.bin \
  72c290bc2ffa216041b276660277369bd17ecef92315e367d98bb2c96c8428fa \
  output/papermono-lora-check 15 lora
```

The caller supplies URL/hash. Official BIN bytes stay in RAM; only ignored diagnostic output and images are written. The probe uses the actual page runtime and worker/WASM, taps SEND at (423,157), checks transmitted payload `48656c6c6f21`, confirms RX remains zero without a peer, then explicitly supplies `50454552` at 868 MHz, SF12, BW62.5 kHz, CR4/8, sync 0x3444, explicit header, CRC enabled, normal IQ. Final acceptance requires the visually reviewed 330×50 **Rx PEER** pixel-region SHA-256 `6b6cadc313042a506d2d5b94f0af47656ddd78a8b5f539fe1ef0739e5a55aef4`; an API acknowledgement or arbitrary frame change is insufficient. [Official receipt](official-wasm.json) retains exact input times, radio state, guest outputs and PNG artifact hashes.

The [source-only IDF 5.5.1 fixture](../../../../tools/fixtures/papermono-lora-firmware/README.md) verifies real queued SPI3 RX DMA, FIFO wrap, power/reset, BUSY, GPIO5 IRQ, TX_DONE, no-peer timeout, wrong-channel rejection, PEER FIFO bytes and RSSI/SNR reads. Its explicit GPIO pull-downs define the unconnected Lite inputs; the simulator does not invent a Lite radio to make the fixture pass. [Fixture receipt](fixture.json) pins generated input hashes; BINs remain ignored and unpublished. [Browser receipt](browser.json) covers the final full/Lite fixture, API capabilities and shared product identity with zero page errors. To reproduce, serve the repository root and pass [browser-check.js](browser-check.js) to the required Playwright wrapper's `run-code` inside the platform page; close the owned browser/server afterwards. [Regression receipt](regression.json) preserves official NFC NO CARD and Lite detection results; their earlier WASM revision is explicitly distinguished from the final GPIO correction.

## Wiring and source basis

[Source receipts](sources.json) include exact URL/final URL, UTC whole-second retrieval time, MIME, bytes and SHA-256. Reused earlier GETs keep their original timestamps. The M5-hosted Semtech **SX1261/2 Rev2.2, December 2024** datasheet is the protocol source; Salesforce's HTML responses are retained as unsuccessful document retrievals. Official HAL/app source is pinned to `m5stack/M5PaperMono-UserDemo` `e55877dc04b89e13a8519cee5435dc326499e1ca`; RadioLib is `b91c6af112e423dcb14529ade5827802a14e1015`. Official firmware reports IDF **5.5.1**, matching the register/driver headers used here.

- Manufacturer schematic SHA `b5abecac97cb043a6d1af7a673f2111d40d9254b0b5108635cb924d9b37ad6fa`, visually checked PDF pages 2 and 4: PM1 GPIO2 controls U9's 3V3_L2_LoRa rail; IOE10 controls reset, IOE2 antenna switch; BUSY=21, DIO1=5, SCK=39, MOSI=38, MISO=40, NSS=41. Official HAL uses SPI3 and software NSS. Other host instances may reach the same modeled device only through these physical pin routes.
- M5PM1 manual: MODE 0x10, OUT 0x11, DRV 0x13, FUNC 0x16/17, HOLD 0x07 determine standard GPIO output. An open-drain release does not drive the load-switch enable high. Alternate functions do not act as standard GPIO. Full-system PMIC shutdown gates the radio; analog rail dynamics and arbitrary circuit solving are not implemented.
- Semtech §8, §12 and §13 define NSS-framed commands, BUSY, registers, 256-byte wrapping FIFO, mode/status, IRQ masks/read/clear, TX/RX timeout and packet configuration. RadioLib's fixed **SX1262.h explicitly says SX1262 reports `SX1261`** at 0x0320. Only those six documented family bytes are supplied; the ROM suffix is unknown and left zero.
- Mode transitions use datasheet table 8-2 typical timings; reset/cold wake uses nominal 3.5 ms, ordinary writes nominal 1 µs. TCXO delay uses programmed 15.625 µs ticks. LoRa packet air time uses the fixed driver's integer equation including SF, BW, coding rate, preamble, header, CRC and LDRO. None of these is a silicon calibration or scheduling-speed claim.

## S3 RX GDMA prerequisite (EX235)

This extends existing [EX235](../../../experiments.md#ex235), not a duplicate SPI controller. IDF 5.5.1 `SPI_DMA_RX_ENA` is bit27 at +0x30; RX_EOF_EN is bit21. SPI2/SPI3 use existing GDMA triggers 0/1. A cold completion path feeds MISO bytes into the existing bounded IN descriptor walker when master RX DMA is enabled. Only the last descriptor gets success EOF; owner/length, interrupt masks, descriptor failures and restart use the shared implementation. TX-only and DMA-off operations do not enter it.

Only internal SRAM is accessible to descriptors and data; addresses wrapping or crossing its bounds, MMIO, bad ownership, empty/exhausted/cyclic chains and unsupported byte-count EOF mode fail without recursive peripheral access or fabricated success. S3 default EOF-at-transaction-end mode is covered; PSRAM/cache DMA, slave and segmented modes are not. IDF's matching SPI master driver allocates internal DMA bounce buffers when needed. Both hosts have regression tests; C3/C6 GP-SPI layouts are unchanged. Open upstream PRs were checked before implementation: #299 is an adjacent S3 tick-budget change, not an RX DMA implementation; no competing RX DMA change was found, and the fork had no open PR.

## Explicit limits and retained negative results

Reception requires an explicit peer packet with matching frequency, SF/BW/CR, sync word, header/CRC/IQ settings, and implicit payload length when applicable. Power/reset, antenna, BUSY and RX state are checked; overlapping or too-late packets are rejected. Completion fills FIFO, signals IRQ and lets guest firmware decide what to do. Antenna loss cancels an in-flight reception while preserving a single-RX timeout; reset/power loss cancels activity. Warm sleep retains config and clears FIFO; cold wake resets config. Tests cover CRC-error IRQ/data, command bounds and malformed host inputs.

There is no automatic echo, unsolicited RX, physical RF, propagation/interference, calibrated RSSI/SNR, cross-tab/remote peer relay, GFSK, CAD, LR-FHSS or ranging. CAD/preamble-detector parameter writes are accepted without implementing their operations; unsupported operations reject rather than complete successfully. Analog regulator/PA settings and calibration timing are a digital subset. GetStats supplies received count but does not track hardware CRC/header-error counters. Instant RSSI uses the last explicit packet sample, not an RF noise model. TX history is bounded to 16 and diagnostic counters/history survive chip power/reset until board reconstruction. No ordinary UI peer editor or portable LoRa recording is introduced; the documented agent/iframe API is the access path.

[Negative results](negative-results.json) retain the failed official startup before proper command-status recovery and before replacing an invented `SX1262` string with the driver-documented family value. The final run still reports nine rejected dummy wake-up/NOP commands; they are observable diagnostic counters, not fabricated successful operations. Also retained: source-download failures, missing standard test inputs, a browser-harness async error, and the Lite floating-pin fixture issue. No official BIN was patched or substituted.

## Verification and evidence handling

[Checks](checks.json): stable Rust 1.99.1, both Clippy commands with warnings denied, **1,384 Rust tests** including ignored/non-external tests, **69 platform checks**, **10 standard WASM scenarios**, production WASM section policy and catalog schemas passed. No golden changed. No 3D asset was modified; prior photo/CAD evidence is not claimed as newly rerun.

Raw serial includes emulated identifiers and is not committed. Compact receipts omit those identifiers, full consoles, storage scope details, local paths and session/process inventories; original receipt/PNG hashes retain provenance. Raw files remain in ignored local output. Firmware and private storage bytes are absent from this evidence. The privacy pattern check and manual review apply before commit.

CPU regression is recorded in [native-regression.json](native-regression.json), using the existing caller-configurable [benchmark harness](../papermono-nfc/benchmark.py), five alternating pairs and identical instruction counts, virtual cycles, exception/interrupt counts and stdout hashes. Main source equivalence, binary hashes, numeric samples and aggregate host load are retained; unrelated host load is not controlled. This is an idle-workload regression check, not a speedup claim.

| Workload | Baseline median user s | Candidate | Change |
| --- | ---: | ---: | ---: |
| hello | 0.25 | 0.25 | +0.00% |
| c3-hello | 2.45 | 2.47 | +0.82% |
| c6-hello | 1.63 | 1.62 | -0.61% |
| pocket-tank | 40.02 | 40.55 | +1.32% |

The baseline is main-code-equivalent to `20e75f72`: Rust/Cargo sources are unchanged from its retained `9d072179` executable. Candidate includes the already delivered NFC stage plus LoRa. Ranges overlap for all workloads; the S3 hello measurement resolves only hundredths of a second. One light Node contract check (about 2.4 wall seconds) ran during Pocket Tank; no build, guest probe or browser was concurrent. Other host load was uncontrolled. The values do not establish an improvement or a statistically resolved slowdown.

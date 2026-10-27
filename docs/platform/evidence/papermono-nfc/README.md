# PaperMono NFC reader subset and Lite separation

The fixed official UserDemo 1.2.0 binary now detects full PaperMono as Pro and completes a real A/B/F/V scan with **NO CARD**. Lite still NACKs the NFC address and detects Lite. This validates the reader's no-peer digital path, not card identification/content, RF performance or LoRa operation.

## Reproduction

Build WASM with `tools/wasm-build.sh`, obtain the standard ROM assets, then run:

```sh
ESP32SIM_PROBE_CLOCK_TIMEOUT_MS=60000 node tools/probe-official-papermono.mjs \
  https://m5burner-cdn.m5stack.com/firmware/376080df0d31b0026e9a67a75d02d571.bin \
  72c290bc2ffa216041b276660277369bd17ecef92315e367d98bb2c96c8428fa \
  output/papermono-nfc-check 30 nfc
ESP32SIM_PROBE_CLOCK_TIMEOUT_MS=60000 node tools/probe-official-papermono.mjs \
  https://m5burner-cdn.m5stack.com/firmware/376080df0d31b0026e9a67a75d02d571.bin \
  72c290bc2ffa216041b276660277369bd17ecef92315e367d98bb2c96c8428fa \
  output/papermono-lite-check 15 lite m5stack-c153-lite
```

The caller supplies the URL/hash; firmware bytes remain in RAM. The probe uses the actual catalog board, page runtime and WASM worker, sends a touch at (423,385), and checks the final 230×49 pixel status region against the visually checked **NO CARD** glyph hash. A changing frame or Pro menu alone is insufficient. [Official WASM receipt](official-wasm.json) retains input times, pixel hashes, image artifact hashes and failure status. [Native receipt](native.json) records 296 actual transmissions and 296 no-response expiries, with A/B/F/V modes 0x400e, after a 45-guest-second run. Native and WASM have different tap times; they are separate executions, not a bit-exact comparison.

The internal [source fixture](../../../../tools/fixtures/papermono-nfc-firmware/README.md) boots through the real worker: read-only identity, oscillator IRQ, GPIO6 read-clear/masks, TX_END/NRT with empty RX FIFO, Stop cancellation and the first interrupt-queued SPI3 transfer. The same binary must NACK at 0x50 on Lite. [Fixture receipt](fixture.json) pins the generated inputs; binaries remain ignored and unpublished.

## Electrical and command evidence

[GET receipts](sources.json) record final URL, UTC whole seconds, media type, bytes and SHA-256. The failed st.com GET is retained; the successful ST DS12484 Rev 8 PDF comes from M5Stack's datasheet mirror. Driver source is M5Unit-NFC `93745b547364f310cd64b5155a870103a7800a5d`, pinned by UserDemo `e55877dc04b89e13a8519cee5435dc326499e1ca` `repos.json`.

- ST25R3916 is on SDA47/SCL48 at 0x50; IRQ is active-high GPIO6. Both ESP I2C controllers and existing GPIO bit-bang routing see the same device state.
- The URL named `PaperMono_SCH_V0.6.2_20260522.pdf` currently contains an NFC sheet **V0.2, 2026-07-28**. U1 load switch is NC; R1=0Ω bypasses it. The main sheet explicitly marks PYB_NFC_EN as spare. IOE4 is therefore not used as an invented power gate despite the software method name. The present PMIC abstraction gates the whole system rail on its power-off request. Board revision on the user's hardware has not been established.
- DS12484 Rev 8 §4.3.4 / §4.4 / §4.5 define command encoding, bank access, FIFO (bounded at 512), status MSBs in bits 7:6, readonly registers, oscillator status, read-clear interrupts and mask polarity. Identity 0x2a is the datasheet revision, not a silicon measurement.
- REQA/WUPA and FIFO transmit consume real guest commands/data. TX_END starts the configured 16-bit no-response timer (64/fc or 4096/fc, fc=13.56 MHz). With no RF peer, expiry raises NRE and leaves receive FIFO empty. CRC/framing, analog RF and ISO card state machines are not represented.
- No-field initial/response field-on honors detector enable, the specified initial delay/wait slots and programmed B15 guard timer; APON and CAT are distinct events. RF collisions/active peers are absent.
- Oscillator startup uses a stated nominal 700 µs; regulator adjustment uses a stated nominal 3.3 V digital result and 1 ms completion. These are not calibrated analog models. Wake-up, antenna calibration, card emulation and unsupported commands do not acquire fabricated success. The official scan also issues Stop/field-on/Clear FIFO outside the subset's accepted command state (48 rejected commands in the final native run); these are retained as diagnostic counters, not turned into invented completions.

## SPI queue prerequisite and negative result

This extends the existing **[EX235](../../../experiments.md#ex235)** mechanism, not a new SPI3 controller. ESP-IDF **v5.5.1**, matching the official boot log, `spi_hal_init()` calls `spi_ll_set_int_stat()`, which writes TRANS_DONE bit12 to S3 `DMA_INT_SET` at +0x44. The model previously retained that write as register RAM and raised no interrupt. Official PC 0x4208193e writes 0x1000 to 0x60025044; without the fix SPI3 transfers remain zero, LoRa initialization waits indefinitely and NFC remains READY even after a tap. This earlier failure is retained in `native.json`.

The fix implements write-one software set for bits20:0 on **S3 only**, both SPI2 and SPI3; existing masks, routing and clear behavior apply. C3 uses `DMA_INT_RAW` for software set in its version-specific header, so the shared GP-SPI layout is not changed. Register tests check both hosts, reserved/zero writes, isolation, mask/clear and no fabricated transfer. The guest fixture proves queue dispatch rather than merely a register value.

With that real prerequisite fixed, SX1262 remains absent: the official LoRa initialization eventually returns state -2 and displays INIT FAIL (around guest 25 s). NFC then runs normally. No binary patch, skipped initializer, fake SX1262 response or host-drawn result is used. Full LoRa SPI/BUSY/IRQ/packet behavior is a subsequent stage.

## Persistence and limits

Only execution changes to `papermono-lite`. Existing Lite Flash/SD namespaces retain `papermono` via a product-specific alias, preserving old user data and existing clear/epoch behavior. Product IDs still isolate full/Lite; other products are not aliased. Node regression seeds old Flash and SD data, restores them after the board-name change, clears both, rejects a stale writer and verifies full-product isolation. Family manifest compatibility does not claim firmware compatibility.

No 3D asset, connector geometry or manufacturer model changed; prior CAD/photo evidence is not presented as newly rerun. RTC, IMU, Wi-Fi, sleep, audio and RF peer limitations remain explicit in [runtime documentation](../../papermono-runtime.md). Raw logs/source archives/screenshots remain in ignored local output; compact receipts retain their relevant hashes. Emulated addresses/identifiers, full register dumps and local environment paths are omitted. No private capture or official BIN is committed.


## Checks and idle workload regression

[Check receipt](checks.json): current stable Rust 1.99.1, both Clippy targets with warnings denied, **1370 Rust tests** including ignored/non-external tests, eight standard WASM scenarios, platform and storage regressions, catalog schema and evidence privacy checks passed. The first workspace attempt lacked ignored P4 firmware inputs and was retained as a setup failure; the complete rerun passed after obtaining the standard inputs. No golden was regenerated. [Browser receipt](browser.json): real headless page full/Lite fixture and share-product identity passed, no page errors; task-owned browser and server were closed.

[CPU receipt](native-regression.json), versus `9d072179`, 3 alternating pairs, identical instruction counts, virtual cycles, exception/interrupt counts and stdout hashes:

| Workload | Baseline median user s | Candidate | Change |
| --- | ---: | ---: | ---: |
| S3 hello | 0.26 | 0.27 | +3.85% (+0.01 s) |
| C3 hello | 2.48 | 2.50 | +0.81% |
| C6 hello | 1.64 | 1.62 | −1.22% |
| Pocket Tank | 40.57 | 40.22 | −0.86% |

No task-owned build/browser/guest test ran concurrently with this comparison. Other host load was not controlled; load averages and ranges are retained. Three workloads have overlapping ranges; C6 candidate samples are slightly below the baseline range. This small sample is a regression check, not evidence of a speedup. `benchmark.py` takes caller-supplied built binary directories, requires the standard pinned ROM/firmware/model assets, and reproduces the alternating comparison.

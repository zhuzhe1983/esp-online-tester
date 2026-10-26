# Official StopWatch V0.5: remote source and emulator behavior

The unchanged M5Stack V0.5 merged image boots through the ESP32-S3 ROM in the
actual WASM worker. Its own firmware renders the first-run guide, launcher,
AlarmClock and Stopwatch screens. A/B, the touchscreen and the battery gesture
are delivered to the emulated GPIO/CST820 devices; no host-side page navigation
or substituted application is used. `appDirect` is false and `stubs` is empty.

## Input and source identity

- Firmware: [M5Stack V0.5 release](https://github.com/m5stack/M5StopWatch-UserDemo/releases/tag/V0.5),
  `C152-StopWatch-UserDemo-V0.5-20260528_0x0.bin`, 3,854,192 bytes,
  SHA-256 `6395799089341e0ae96072c2a3418d469d6797b2ab8ee0cab1c9ed24a42acbbe`.
- The official public [M5Burner catalog](https://m5burner-api.m5stack.com/api/firmware)
  lists author `M5Stack`, name `StopWatch User Demo`, category `stopwatch`,
  firmware ID `02f082a61479570df0a2063c9bce7b57`, version `v0.5`, published
  2026-05-28, file `6ea670900726398df06ab72c2915db3e.bin`.
- [Official CDN artifact](https://m5burner-cdn.m5stack.com/firmware/6ea670900726398df06ab72c2915db3e.bin):
  HTTP 200, `Access-Control-Allow-Origin: *`, and the exact release SHA above.
  Actual Chromium fetch and SHA-256 verification passed on 2026-10-26 with
  `cache: no-store`, `credentials: omit`. GitHub's release redirect and
  release-assets endpoint did not provide usable browser CORS in the earlier check.
- Public application source: [V0.5 commit 6b4aa125](https://github.com/m5stack/M5StopWatch-UserDemo/tree/6b4aa125288b6fe9dca661f10159f6e1e5ee785c).
  The binary identifies ESP-IDF v5.5.4 and app version `cf51acc`; the source tag
  and binary build identifier are recorded separately, not asserted identical.
- No firmware file, browser persistent cache or proxy is used. The probe fetches
  into memory, verifies SHA-256 and discards bytes at disposal. Logs and rendered
  frames are evidence, not a redistribution of the firmware image.

## Defects exposed by the original binary

1. **M5IOE1 input readback.** The baseline remained in the HAL's OLED rail loop.
   [HAL IOE initialization](https://github.com/m5stack/M5StopWatch-UserDemo/blob/6b4aa125288b6fe9dca661f10159f6e1e5ee785c/main/hal/hal_ioe.cpp)
   reads IO8 after driving it high; [M5IOE1 1.0.8](https://github.com/m5stack/M5IOE1/tree/37db04861687858b6748f5dbd1a84439a6635c46)
   `digitalRead` accesses GPIO_IN registers 0x07/08. The model now returns driven
   push-pull levels in both banks, with read-only input registers and the high
   bank's unused bits masked. External pulls and analog levels remain unmodeled.
2. **Mask-DROM CPU stores.** After that fix, initialization panicked at PC
   `0x42095748`, store address `0x3ff1c050`. The same failure with WASM JIT disabled
   rules out a JIT-only defect. [ESP-DSP 1.8.0 FFT initialization](https://github.com/espressif/esp-dsp/blob/c22e27b4ff8e02bb59b4d0c301c73d590d3abf5e/modules/fft/float/dsps_fft2r_fc32_ansi.c)
   writes its selected 1024-point coefficient table; [IDF v5.5.4 S3 ROM symbols](https://github.com/espressif/esp-idf/blob/v5.5.4/components/esp_rom/esp32s3/ld/esp32s3.rom.ld)
   provide the ROM-table pointer. The CPU-store discard behavior follows
   [Espressif QEMU S3 ROM mapping](https://github.com/espressif/qemu/blob/febae182e132e4055529be423a818225ebddaa3a/hw/xtensa/esp32s3.c)
   and [its TLB_DISCARD_WRITE handling](https://github.com/espressif/qemu/blob/febae182e132e4055529be423a818225ebddaa3a/accel/tcg/cputlb.c).
   Only CPU stores wholly inside mask DROM are discarded; bytes, code versions,
   DMA write protection, IROM protection and out-of-range faults remain unchanged.
   This is source-backed emulator behavior, not a silicon measurement.
3. **Split QSPI headers.** After the second fix, Launcher opened but the frame
   was effectively black. [M5GFX 0.2.19 Panel_AMOLED](https://github.com/m5stack/M5GFX/blob/53a7184601f3667b030ba141c58b87ce2acfaa2a/src/lgfx/v1/panel/Panel_AMOLED.cpp)
   sends command headers one byte per SPI transfer under software CS on GPIO39.
   StopWatch now retains the header across transfers within that CS assertion,
   distinguishes subsequent pixel data even if it begins `02 00 01 00`, and
   discards abandoned headers at the next assertion. Existing boards keep their
   complete-header interface. This extends [EX092](../../../experiments.md#ex092):
   that earlier fix retained split parameters, while this workload splits the
   command header itself and supplies an explicit CS boundary.
4. **Short-press control probe.** The first complete sequence succeeded and its
   Stopwatch reset frame exactly matched the initial frame. A later run concurrent
   with a Rust build failed to reach the expected menu. Stricter probes rejected
   actual down/up queue intervals of 0.8265 and 0.6933 guest seconds, above the
   official button library's 500 ms hold threshold. The second failure also occurred
   after the Rust tests had finished, so host load alone is not an established
   cause. Polling fresh clock receipts did not solve it. These negative outcomes
   remain recorded; the strict timing probe is **not passing**. The adapter's
   ignored `{}` messages read the existing queue clock and send no guest bytes,
   but they do affect host pacing and are unsuitable for performance measurements.
   Queue receipts do not prove the exact firmware consumption cycle.

## Reproduction and assertions

Build the WASM using `tools/wasm-build.sh`; provide the normal S3 ROM asset.
The harness requires an explicit remote target and hash:

```sh
node tools/probe-official-stopwatch.mjs \
  https://m5burner-cdn.m5stack.com/firmware/6ea670900726398df06ab72c2915db3e.bin \
  6395799089341e0ae96072c2a3418d469d6797b2ab8ee0cab1c9ed24a42acbbe \
  output/official-stopwatch/reproduction 4 controls
```

The sequence enters the launcher with A+B, pages B then A, taps an application,
returns using A+B, drags down the battery indicator, then enters Stopwatch and
uses B/A/B/A for start/lap/pause/reset. It checks nonblack real frames, menu-icon
return equality, timer movement, lap-region changes, a stationary paused timer,
and exact reset-body equality excluding the independent top clock. Inputs use
acknowledged guest time. The `controls` run snapshots were inspected visually and showed the full
sequence. The later stricter harness additionally checks the 500 ms short-press
limit and paused-time stability; those gates currently stop on the documented
short-press overrun, so they are not claimed passed. Exact host-input timing
remains unsupported.

Core checks use stable Rust, both CI Clippy invocations with warnings denied,
and `ESP32SIM_ROM_DIR=web/wasm/fw cargo test --release --workspace -- --include-ignored --skip external_`.
Directed tests cover IOE readback, ROM protection boundaries, split CS commands,
command-like pixels, deasserted CS and interrupted headers. The real diagnostic
runtime suite still checks framebuffer, A/B, CST820, logical UART1 GPS, lifecycle
and delayed-stat regression. No goldens were regenerated.

## Limits and retained evidence

The existing TE waveform and PMIC battery voltage are synthetic, not hardware
measurements. BMI270, RX8130, audio playback and the physical power button are
not modeled. RTC logs do not represent a real date. No USB hardware was selected,
flashed or tested. `simulationVerified: true` and `hardwareValidated: false` are
separate catalog fields; other watchlist products remain reference-only.

`summary.json` records revisions, source/file hashes, numeric outcomes and tests.
Raw logs, frames and full input receipts remain under ignored
`output/official-stopwatch/`; they are not committed per the evidence retention
policy. The compact record names their paths and hashes and omits raw event
arrays and complete console output. No personal device data was collected.

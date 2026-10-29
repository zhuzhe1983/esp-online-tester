# ESP32-S3 light sleep: EDC Book on PaperMono

Correctness follow-up to [EX233](../../experiments.md#ex233). That experiment
implemented classic ESP32 sleep. This change uses the S3 register layout and
adds timer, GPIO, EXT0 and EXT1 level wake with retained CPU state. The chip,
firmware, wake mask, watchdog pause bit and EXT1 semantics differ from EX233;
this is not a renamed timing optimization.

## Input and failure

The caller supplied the M5Burner EDC Book 3.1.2 image:

- [Download](https://burner.m5stack.com/api/v1/firmwares/1897200000000000015/versions/2113660506860756993/download)
- 16,777,216 bytes; SHA-256 `06eb3b595fc47bc5d35f760a16cdc5795ea8c0fad6295bcee8db5268cf906986`.
- Board `papermono`, 16 MiB Flash, 8 MiB PSRAM; complete merged image,
  actual S3 mask-ROM startup, no application-direct bypass or firmware patch.
- Bootloader reports `ESP-IDF 5.5.2.260206`; embedded app version is
  `v3.0.0-34-g0cd03bde-dirty`, while the rendered splash and release metadata
  identify 3.1.2. The exact patched SDK source is not available.

At base `5b1cdf11e3d8583e031a844cc1d73293a38700e1`, native JIT and interpreter
both reset after 1,131,233,666 cycles (4.713 guest seconds). The browser first
reset at about 4.25 seconds, then repeated approximately every 3.52 seconds.
The guest waited at PC `0x40385971` for RTC interrupt RAW bits 0/1. At sleep
entry, `DIG_PWC=0x20020010` selected light sleep, `WAKEUP_STATE=0x00070000`
enabled EXT1/GPIO/timer, and `WDTCONFIG0=0xc007ee00` enabled the sleep safety
watchdog with S3 pause bit9. The old model stored `SLEEP_EN` as ordinary RAM,
never entered sleep and never generated a wake cause. Disabling the watchdog
would conceal that missing behavior.

## Digital model and sources

The S3-only RTC adapter provides RAW/ST/ENA/W1C, read-only wake/reject causes,
EXT1 status clearing and source39 interrupt routing. `SLEEP_EN` with
`DG_WRAP_PD_EN` clear holds both cores, retains PC/registers/CCOUNT/memory and
gates digital timers. RTC time continues; firmware's bit9 controls RWDT pause.
Timer and qualified GPIO/RTC-pad levels resume execution. Board-side EPD BUSY
and touch IRQ events continue to advance, since they are separate devices.
Digital clock accounting advances without issuing gated ticks, so TIMG and
SYSTIMER do not catch up after wake.

Register facts were checked against Espressif ESP-IDF **v5.5.2**:

- [rtc_cntl_reg.h](https://github.com/espressif/esp-idf/blob/v5.5.2/components/soc/esp32s3/register/soc/rtc_cntl_reg.h):
  STATE0 +0x18, wake enable +0x3c bits31:15, interrupt registers +0x40…0x4c,
  external polarity +0x64, reject mask +0x68, PWC +0x88, DIG_PWC +0x90,
  WDT +0x98 bit9, EXT1 +0xe0/+0xe4, reject/wake causes +0x128/+0x130.
- [rtc_io_reg.h](https://github.com/espressif/esp-idf/blob/v5.5.2/components/soc/esp32s3/register/soc/rtc_io_reg.h):
  RTC pins 0…21 at +0x84+4n, function/input/sleep enables and EXT0 selector +0xdc.
- [gpio_reg.h](https://github.com/espressif/esp-idf/blob/v5.5.2/components/soc/esp32s3/register/soc/gpio_reg.h)
  and [io_mux_reg.h](https://github.com/espressif/esp-idf/blob/v5.5.2/components/soc/esp32s3/register/soc/io_mux_reg.h):
  level wake bit10 and low/high type4/5; normal versus sleep input enable.
- [interrupts.h](https://github.com/espressif/esp-idf/blob/v5.5.2/components/soc/esp32s3/include/soc/interrupts.h): RTC source39.
- [rtc_sleep.c](https://github.com/espressif/esp-idf/blob/v5.5.2/components/esp_hw_support/port/esp32s3/rtc_sleep.c)
  and [rtc.h](https://github.com/espressif/esp-idf/blob/v5.5.2/components/esp_hw_support/port/esp32s3/include/soc/rtc.h):
  sleep entry, reject selection and wake source masks.

Classic's WDT pause bit7 and EXT1 ALL_LOW behavior must not be copied to S3:
S3 uses bit9 and ANY_LOW/ANY_HIGH. The existing nominal 150 kHz RTC clock is
retained; this is not oscillator, analog power, optical timing or silicon
validation. Deep sleep/reset wake, RTC memory power-down, ULP/UART/radio wake
and full peripheral power-domain sequencing remain outside this change.

## SSD1677 segmented refresh follow-up

All 35 captured refreshes through the first page turn used mode1. EDC Book
programs 105-byte LUTs with undriven rows to retain pixels outside the current
band. The previous generic four-level fallback repainted group0 white and
group3 black, erasing earlier bands and inverting the white drive.

The correction decodes the active VS phases using TP timing bytes. TP=0
skips the phase; RP=0 still executes once, so collecting voltage kinds needs
no repetition expansion. With an undriven VCOM row, all-VSS rows preserve the
previous optical pixel, pure VSH1 gives a black endpoint and pure VSL a white
endpoint. Existing known M5GFX/INK4 differential and monochrome classes retain
their mappings; mixed-polarity/VSH2 waveforms retain the prior four-level path.
Four endpoint results are computed once per refresh, not per pixel.

Sources are [SSD1677 rev1.0](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1267/SSD1677.pdf)
section 6.6 Tables 6-6/6-7 and the voltage meanings
in fixed [M5GFX Panel_SSD1677.cpp](https://github.com/m5stack/M5GFX/blob/02107b828acb3eb782fccd747863638d64c876d7/src/lgfx/v1/panel/Panel_SSD1677.cpp).
This does not infer uncalibrated grayscale from the last pulse. EDC Book's
short unipolar pulses are a black/white preview, not a physical dose-response
model. No firmware hash, page coordinates or text are special-cased.

Three new regressions cover field-free/zero-duration phases, active voltage
selection and four segmented bands using the actual EDC LUT. The old synthetic
`[0x55;105]` gray fixture was replaced by the real fixed M5GFX `lut_fast`,
including VSH2 gray tails and nonzero timing, to preserve a meaningful four-gray
contract. Native final checks at 21/26/33 guest seconds produce complete first,
second and third guide pages (20/35/50 completed refreshes, BUSY false).

## Reproduction and validation

Download the image to a caller-selected path and verify its SHA-256. Build
native binaries with `cargo build --release --bins`, or browser assets with
`tools/wasm-build.sh`. A native run needs explicit inputs:

```sh
target/release/esp32sim --chip s3 --board papermono --flash-mb 16 --psram-mb 8 \
  --flash-image "$EDCBOOK_BIN" --rom "$ESP32SIM_ROM_DIR/esp32s3_rev0_rom.elf" \
  --boot rom --console uart0 --max-seconds 60 --no-reboot \
  --tft-png output/edcbook.png
cargo test --release -p esp32s3 --test light_sleep
```

The 17 portable tests cover exact/full-width/wrapping RTC deadlines, both
CPU contexts, autonomous board deadlines during sleep, gated digital timers,
qualified GPIO/EXT0/EXT1 and power-domain
selection, reject causes, IRQ/W1C routing, watchdog pause/resume and an actual
guest sleep-entry store in both interpreter and JIT.

For a browser run, serve `web/` with the repository's platform server, configure
`m5stack-papermono` with the pinned `{url,sha256}` above through `espTester`,
load and start. Observe actual guest time and capture real framebuffer PNGs.
After boot, tap `(425,625)`, `(240,625)`, `(400,740)`, then `(400,400)`, keeping
each touch down for at least 0.15 guest seconds and allowing the page to settle.
These select the recent-reading list, open the bundled quick-start guide and
turn its page. Host-time holds alone are unreliable under load.

Native startup survived 60 guest seconds without a reset. The browser run
survived 245.81 guest seconds with exactly one ROM boot banner; recent-reading
and the first guide page were visually verified. The first post-fix page-turn
check exposed a separate SSD1677 partial-refresh defect, also reproduced
natively. That negative result is retained; the segmented-refresh correction above
restores the pages. This is not a claim of full reader compatibility. Browser uses a default virtual 64 MiB SD card;
the native checks above have no SD card, so their instruction totals are not
claimed to be identical.

Raw local logs and PNGs remain under ignored `output/`. The compact receipt
retains hashes, numerical checkpoints and test results, omitting full serial
logs, storage namespaces, browser session details and firmware build paths.
No firmware binary is committed. Open-PR review found no competing S3 sleep
owner; upstream #263/#299 touch neighboring machine/bus code for different
mechanisms and were not duplicated.

The opt-in worker probe accepts the source URL and hash explicitly, uses an
isolated runtime and disposes its worker in `finally`. The fixed EDC scenario checks the visually verified second-page RGBA SHA-256
`60d5ebcaa2c5701afb302770eaa1ace6e6577c4b6d89e98566b1eb01a07a916a`;
merely changing a framebuffer is not a readability assertion:

```sh
ESP32SIM_PROBE_CLOCK_TIMEOUT_MS=60000 node tools/probe-official-papermono.mjs \
  "$EDCBOOK_URL" "$EDCBOOK_SHA256" output/edcbook-worker 30 edcbook
```

Final combined browser validation ran **137.3 guest seconds**, with one ROM boot
banner, no runtime error and 50 completed display refreshes. The recent-reading
list and three consecutive guide pages were visually checked. The independent
Node worker probe also passed the complete second-page pixel oracle. The fixed
official UserDemo 1.2.0 brightness buttons, plus touch and mute/unmute restore
checks pass after the display change.

Final checks: 1,728 native workspace tests (ignored included, only `external_`
excluded), 17 portable light-sleep tests, 7 SSD1677 tests, both required Clippy
commands with warnings denied, 189 platform tests, nine real WASM demos, WASM
section and timing ABI checks. No golden output was regenerated. Before the
combined build, real recording/replay also passed 18 inputs with an equal final
frame. Initial missing-ROM setup failures and the corrected awake GPIO batching
regression are retained in `receipt.json`.

## CPU controls

Fresh native binaries from main `5b1cdf11` and candidate code `7b0abe47` were
compared with the existing EX233 harness. Seven alternating measured pairs
followed one excluded warmup pair. Every instruction count, guest-cycle total
and console SHA-256 matched across both builds and all samples.

| Workload | Guest duration | Main median user CPU | Candidate median user CPU |
| --- | ---: | ---: | ---: |
| classic-hello | 30 s | 0.37 s | 0.37 s |
| hello | 30 s | 0.24 s | 0.24 s |
| c3-hello | 30 s | 3.10 s | 2.48 s |
| c6-hello | 30 s | 1.57 s | 1.59 s |
| pocket-tank | 3 s | 4.91 s | 3.65 s |

The 60-second attempt to obtain three consecutive aggregate CPU readings at or
below 150% failed; all samples are retained in `cpu-idle-gate.json`. The final
run explicitly used an initial 500% threshold. These are background-load
measurements with 10 ms timing resolution, not an isolated performance pass.
The large apparent gains in the unchanged C3 control also prevent attributing
speed differences to the sleep fix. Pocket Tank's three seconds are a short
regression control, not a sustained throughput claim. Per-run load averages,
aggregate CPU and numeric samples are retained without process identities.

```sh
python3 docs/evidence/classic-light-sleep-tdisplay/compare-cpu.py \
  --baseline /tmp/main/target/release/esp32sim \
  --candidate target/release/esp32sim --firmware-dir web/wasm/fw \
  --rom-dir web/wasm/fw --output output/cpu.json --initial-cpu-max 500
```

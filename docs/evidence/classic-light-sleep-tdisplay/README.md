# Classic light sleep: TTGO T-Display factory firmware

Correctness extension of [EX233](../../experiments.md#ex233), also related to
[EX230](../../experiments.md#ex230). The original EX233 receipt covers deep sleep
with EXT0 reset wake. This change adds **light sleep**, which must retain both CPU
contexts and resume execution, without resetting the chip. It is not a speed
optimization and does not change the classic CPU frequency model.

## Input and diagnosis

The original TTGO T-Display factory image is the manufacturer's
[`firmware/firmware.bin`](https://github.com/Xinyuan-LilyGO/TTGO-T-Display/blob/6f89cddcde7152b84f7afc1831b6a1dee1fc3206/firmware/firmware.bin),
807,728 bytes, SHA-256
`85a2598f363a96c4d4eb4aebad5af81c286653eab0d20543110546e4d2c7b9ed`.
It identifies ESP-IDF `v3.2.3`; it is a full flash image and boots from the real
ESP32 revision-3 ROM. No binary is mirrored in this repository, no symbol stubs
are installed, and application-direct startup is not used.

Before this change, the firmware prints `Start` and paints the TTGO logo, then
resets with cause `0x10 RTCWDT_RTC_RESET` every approximately 1.675 guest seconds.
Temporary MMIO instrumentation, removed from the delivered code, identified:

| Reference cycle (240 MHz) | Guest write | Meaning |
| --- | --- | --- |
| 25,313,024 | RTC WDT CONFIG0 `0x0001f880` | Boot has disabled the watchdog normally |
| 162,069,975 | RTC WDT CONFIG0 `0x4001f880` | Guest selects stage-0 RTC reset |
| 162,070,103 | RTC WDT HOLD `0x000249f0` | 150,000 slow ticks, one second |
| 162,070,103 | RTC WDT CONFIG0 `0xc001f880` | Guest enables the watchdog |

The caller at `0x400df1d4` is the light-sleep safety-net sequence in
[IDF v3.2.3 `esp_light_sleep_start`](https://github.com/espressif/esp-idf/blob/v3.2.3/components/esp32/sleep_modes.c).
The previous model explicitly ignored light-sleep entry, so the guest remained
in `rtc_sleep_start` waiting for `SLP_WAKEUP_INT_RAW`. The watchdog was behaving
as programmed; extending its timeout or disabling it would conceal the defect.

The manufacturer's
[`FactoryTest.ino`](https://github.com/Xinyuan-LilyGO/TTGO-T-Display/blob/6f89cddcde7152b84f7afc1831b6a1dee1fc3206/TFT_eSPI/examples/FactoryTest/FactoryTest.ino)
uses `espDelay()` (timer wake plus light sleep), GPIO35 for voltage/long-press
sleep, and GPIO0 for scanning. The source is a behavioral reference, not a claim
that rebuilding the current sketch reproduces the older published image.

## Mechanism and limits

- `SLEEP_EN` with `DG_WRAP_PD_EN` clear holds both CPUs without changing PC,
  INTENABLE, window registers, pending local timers, or CCOUNT. The shared
  scheduler now respects the primary core's existing `CoreState::Held` contract;
  chips whose primary is always Running retain that path.
- The RTC slow clock advances while digital/APB clocks are gated. FRC2, TIMG,
  display DMA, radio and other digital engines keep their state and do not tick
  through light sleep. Firmware performs its normal RTC/FRC time correction.
- The 48-bit RTC sleep timer wakes through `SLP_WAKEUP_INT_RAW`, clears
  `SLEEP_EN`, and records timer cause 8. Wake does not require an enabled CPU
  interrupt. The status retains W1C behavior. EXT0 uses the existing qualified
  RTC pad/domain/level path and resumes light sleep without reset.
- ST7789 display-off/sleep commands and GPIO4 backlight changes invalidate the
  board framebuffer version, so the worker publishes the dark panel without a
  new GRAM write. The prior worker kept the last lit frame; native capture already
  observed display-off. A board test covers command and backlight transitions.
- RWDT remains enabled. The guest's `WDT_PAUSE_IN_SLP` bit controls pausing;
  clearing that bit permits the programmed watchdog reset during sleep. Its
  existing count resumes after wake. Deep-sleep EXT0 still resets with cause 5.
- GPIO/EXT1/UART/touch/ULP/radio wake sources, sleep rejection, analog power
  sequencing, and exact clock/power latency remain unmodeled. RTC wake timing
  uses the existing nominal 150 kHz slow clock. This is a digital execution
  model, not silicon or power-consumption validation.
- No AP is attached in the factory test. `no networks found` proves the button
  reached the firmware's scan path, not real RF reception. `Voltage :0.00V`
  reports the default uninjected ADC input; it is not a physical battery reading.

Register facts were checked against the firmware's
[v3.2.3 `rtc_cntl_reg.h`](https://github.com/espressif/esp-idf/blob/v3.2.3/components/soc/esp32/include/soc/rtc_cntl_reg.h),
[`rtc.h`](https://github.com/espressif/esp-idf/blob/v3.2.3/components/soc/esp32/include/soc/rtc.h),
and [`rtc_sleep.c`](https://github.com/espressif/esp-idf/blob/v3.2.3/components/soc/esp32/rtc_sleep.c).
SLEEP_EN bit31, DG_WRAP_PD_EN bit31, SLP_TIMER0/1 (32+16 bits),
WAKEUP_ENA at bits21:11, timer trigger bit3, EXT0 trigger bit0,
SLP_WAKEUP_INT_RAW bit0, and WDT_PAUSE_IN_SLP bit7 agree with the v5.5.4
classic headers already used by EX233. `rtc_sleep_set_wakeup_time` writes the
sleep target without MAIN_TIMER_ALARM_EN; the wake trigger is WAKEUP_ENA.

## Reproduction

Download the pinned URL above outside the repository and check its SHA-256.
Then, with absolute environment paths:

```sh
TDISPLAY_FACTORY_BIN=/tmp/firmware.bin \
ESP32SIM_ROM_DIR="$PWD/roms" \
cargo test --release -p esp32 --test tdisplay_factory -- --ignored --nocapture
cargo test --release -p esp32 --test light_sleep --test deep_sleep
```

The same image and five framebuffer oracles can be checked through the actual
platform worker and public input API (after `tools/wasm-build.sh`):

```sh
TDISPLAY_FACTORY_BIN=/tmp/firmware.bin ESP32SIM_ROM_DIR="$PWD/roms" \
node tools/tdisplay-factory-test.mjs
```

`TDISPLAY_RECEIPT` optionally writes the worker's numeric checkpoint receipt.
Inputs use observed guest time; no UART commands or synthetic frame writes are
used. The worker is disposed in `finally`, including failures.

`TDISPLAY_CAPTURE_DIR` optionally names an existing output directory for PNGs.
The external test requires the exact image, uses the same ordinary machine and
board as the UI, and checks guest output after each physical GPIO input. It
cannot silently pass without the image. The local-image test is excluded from
CI by its `external_` prefix; the nine portable light-sleep tests run normally.

The factory test reaches the menu at 8 guest seconds, presses GPIO35 for
100 ms, then GPIO0 for 100 ms. It holds GPIO35 for 1.5 seconds, releases it before
the firmware's sleep delay completes, verifies that both instruction counts
stay fixed during deep sleep, and presses GPIO35 again at 24 seconds. Only that
wake requests a reset (cause 5). At 33 seconds the menu is bit-identical to the
initial one, and the console contains exactly two `Start` lines and no RWDT reset.

| Checkpoint | Guest cycles | RGB565 little-endian FNV-1a 64 | Pixels written |
| --- | ---: | --- | ---: |
| [Menu](menu.png) | 1,920,000,000 | `95c5ea57a02cb645` | 195,424 |
| [Voltage](voltage.png) | 2,280,000,000 | `4006054ab84cdaa8` | 260,554 |
| [No networks](scan.png) | 3,600,000,000 | `e84d73cc740787bd` | 326,746 |
| [Deep sleep](deep-sleep.png) | 5,520,000,002 | `26993a53c3c439a5` | 426,346 |
| Wake menu | 7,920,000,000 | `95c5ea57a02cb645` | 195,424 |

Images are the real 135×240 RGB565 framebuffer rendered at 3×. The table's FNV
values are regression oracles, not cryptographic source identification. The
firmware SHA-256 above identifies the downloaded artifact. PNGs were visually
checked for menu text, voltage text, no-network text and a dark sleeping panel.

Portable tests cover dual-core state/CCOUNT retention, gated FRC time, exact
sleep timer boundary, disabled timer, unrelated digital interrupts, full-width
timer and rollover, EXT0 selection, watchdog pause on/off and resume, and entry
by an actual guest store within a scheduling quantum. Existing 15 deep-sleep
regressions also pass. The first worker run exposed stale sleep-frame publication (hash `97728a6cdb508058`
versus the dark-frame oracle); that negative result led to the board correction
above. Full validation and CPU comparison results are recorded
in `receipt.json`.

Open-PR file audit found upstream #263 touching machine/soc files for persistent
flash; it adds no sleep state or RTC mechanism. No competing RTC/light-sleep
register owner was found. Source base `61e1808f` has the same runtime code as
main before this change. Evidence retains only firmware output, numeric samples,
source references and test results; temporary disassembly and full string dumps
(including embedded private build paths) are not retained.

## CPU controls

Fresh native builds of main `2a8aa920` and the final candidate were compared on
identical public inputs. Seven alternating measured pairs followed one warmup
pair for each workload. All instruction totals, guest cycle totals and console
SHA-256 values are identical across both builds and every run. No golden output
was regenerated.

| Workload | Guest duration | Main median user | Candidate median user |
| --- | ---: | ---: | ---: |
| Classic hello | 30 s | 0.37 s | 0.37 s |
| S3 hello | 30 s | 0.25 s | 0.24 s |
| C3 hello | 30 s | 2.44 s | 2.46 s |
| C6 hello | 30 s | 1.64 s | 1.65 s |
| Pocket-tank | 3 s | 3.68 s | 3.65 s |

**These are background-load measurements.** Two independent 60-second attempts
to obtain three consecutive aggregate CPU samples at or below 150% failed;
those negative receipts are retained. The final run used an explicit initial
500% threshold (356.2%, 329.8%, 324.1% at entry). Later pre-run aggregate samples
reached 762.9%. Samples and load averages are retained without process identities.
There is no conspicuous regression in the median numbers, but this host state
and the 0.01-second timing resolution do not establish isolated performance
neutrality. The three-second pocket-tank control is not a sustained throughput
benchmark.

```sh
python3 docs/evidence/classic-light-sleep-tdisplay/compare-cpu.py \
  --baseline /tmp/main/target/release/esp32sim \
  --candidate target/release/esp32sim \
  --firmware-dir web/wasm/fw --rom-dir roms --output /tmp/cpu.json
```

Default initial CPU threshold is 150%; the background run explicitly selected
`--initial-cpu-max 500` (the original `--idle-cpu-max` spelling remains an alias).
The captured legacy `idleGatePassed` field referred to that caller-selected
initial gate. The retained background receipt names it `initialLoadGatePassed`
and separately records the failed strict-idle classification; no measured
number changed. `receipt.json` preserves original and classified file hashes.

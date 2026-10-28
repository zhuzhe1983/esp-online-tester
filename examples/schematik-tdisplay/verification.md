# Verification receipt

Refreshed on main `ce31ceac`, the branch's base.

The existing MicroPython T-Display landing card is unchanged and precedes
Schematik T-Display. This demo changes sketch firmware, static web content,
packaging and verification only; no Rust/emulator code changes.

## Sketch and display contract

This is a restyled public Schematik starter, not an unchanged source export.
Arduino_GFX, display wiring, GPIO35 next and GPIO0 power/EXT0 wake are retained.
No image assets, function stubs or dependency source patches are used by the sketch.
The default font is drawn directly; lines are static, centered and fit 240×135.

GPIO35 advances exactly once per debounced press through these nine screens,
then wraps to the wordmark:

| Screen | Title | Value (line break shown as /) |
| --- | --- | --- |
| 0 | none | schematik (lowercase, size 4, first letter accented) |
| 1 | none | making hardware / less hard |
| 2 | none | from idea to / blinking LED |
| 3 | none | now with 100% fewer / magic smoke |
| 4 | none | it works in / the simulator |
| 5 | processor | Xtensa LX6 / dual core, 240 MHz |
| 6 | storage | 4 MB flash |
| 7 | analog inputs | 18 ADC channels |
| 8 | web demo | running in / WebAssembly |

GPIO0 blanks the panel, disables the backlight, waits for release and enters
real emulated deep sleep with active-low EXT0 wake. A later press resets into
setup and redraws screen 0, even when sleeping from another screen. Releasing
the wake press re-arms power-off. It does not advance the next-button cycle.

## Sources for every displayed specification

- Xtensa LX6 / dual core: `esp32/src/lib.rs:1`,
  `esp32/src/soc.rs:16-30` (`CORES = 2`, each core sets `lx6 = true`).
- 240 MHz: `esp32/src/periph.rs:16-18`,
  `docs/esp32.md:8-18` (240 MHz reference, selectable 40/80/160/240 MHz CPU
  clocks). The pinned PlatformIO board recipe selects the normal 240 MHz
  configuration (`boards/lilygo-t-display.json` in the pinned pioarduino
  platform sets `build.f_cpu` to `240000000L`).
  This is a modeled CPU frequency, not a claim of host real-time throughput.
- 4 MB flash: `esp32/src/board/tdisplay.rs:1`, the `docs/esp32.md` section
  "The board: `--board ttgo-tdisplay`", and this demo's
  `web/wasm/fw/tdisplay-schematik.json` (`flash_mb: 4`).
  `esp32/src/bus.rs:75-85` allocates flash from the chosen size.
- 18 ADC channels: `esp32/src/adc.rs:8-10` maps eight ADC1 and ten ADC2
  channels and validates their pins. `docs/esp32.md:57-62` describes classic
  ADC input support and its nominal-calibration limits. This is the SoC
  model's channel count, not a claim that all channels are exposed on the board.
- running in WebAssembly: `tools/wasm-build.sh:14-15` builds the
  `esp32sim-wasm` package for `wasm32-unknown-unknown`; the landing link uses
  `run.html?wasm&fw=tdisplay-schematik`, and `docs/esp32.md:41` confirms
  classic ESP32 selection. This screen describes the web demo. Native runs
  below are verification of that same firmware, not a claim that native is WASM.

Intentionally omitted: the combined 520 KB SRAM headline (the model exposes
separate RAM/cache windows and aliases; no independent 520 KB capacity inventory
was established here), the 34 physical GPIO headline (shared register support
alone does not establish that physical-pad count), and "Wi-Fi + BT (emulated)".
Classic Wi-Fi is modeled, but classic Bluetooth is not established by the
limited BLE virtual controller in `docs/esp32.md:52-67`; do not advertise both
radio stacks. The independently verifiable flash and ADC portions remain.

## Reproduction

All builds use `nice -n 10`, Rust `cargo +1.99.0`, a separate
`CARGO_TARGET_DIR`, and `RUSTUP_TOOLCHAIN=1.99.0` for the WASM script.
The worktree's `target` symlink points at that target directory because the
WASM build script copies from `target/`.

Pinned inputs: PlatformIO Core 6.1.19; pioarduino 55.03.38-1;
Arduino-ESP32 3.3.8 / ESP-IDF 5.5.4; Xtensa GCC 14.2.0_20260121;
Arduino_GFX 1.6.9. `SHA256SUMS` pins the sketch and all five shipped artifacts.
`platformio.ini` sets `-fmacro-prefix-map=${platformio.packages_dir}=/platformio`
(PlatformIO applies `build_flags` to the framework sources too), so the
Arduino core's `__FILE__` strings read `/platformio/...` instead of the local
package directory.

```sh
nice -n 10 pio run -d examples/schematik-tdisplay
nice -n 10 python3 examples/schematik-tdisplay/package.py
(cd examples/schematik-tdisplay && shasum -a 256 -c SHA256SUMS)
nice -n 10 tools/fetch-demo-assets.sh --no-linux
nice -n 10 cargo +1.99.0 build --release --bin esp32sim
# Requires Pillow and the pinned PlatformIO dependencies above:
OUT=$(mktemp -d)
python3 examples/schematik-tdisplay/verify_frames.py "$OUT"
```

The frame verifier replays the shipped application through ROM, compares
actual landscape panel pixels with an independent expected-text mask using
the pinned library's default glyphs, and rejects wrong text, screen order,
placement, clipping and failed wrap. It also asserts a blank sleeping frame,
unchanged PCs/instruction totals at 2 and 2.5 seconds, EXT0 DEEPSLEEP_RESET,
pixel-identical initial/wake frames, and power-off after wake release.
PNGs are actual emulator captures, not generated mockups. The landing thumbnail
is the captured initial frame converted losslessly to WebP. The bootloader,
partition table and application images contain no local paths:
`strings -n 6 <image> | grep -c /Users/` is 0 for each, the 13 Arduino core
`__FILE__` strings in the application start with `/platformio/`, and the
bootloader only has the IDF's own `/IDF/...` paths. The source archive
has zero owner IDs/timestamps, no local paths, and includes the modified sketch
and pinned build recipe alongside unchanged dependency sources.

## Results

- PlatformIO build, native release build, packaging and all six SHA256SUMS
  entries pass. All 427 source archive members have canonical metadata;
  archived sketch, README and verifier match their working source bytes.
  The regenerated notice includes the current repository MIT copyright line.
  A rebuild reproduces the bootloader, partition table, source archive and
  notice byte for byte; the app image differs only in the compile date/time
  string and the ELF/image hashes that depend on it, so the shipped app is kept.
- Native frame assertions pass for all nine screens plus wrap. Sleeping CPUs
  remain at PCs `0x40084795` / `0x40086b91`, with instruction totals
  6,965,801 / 93,508,590 unchanged at 2 and 2.5 seconds. EXT0 wake gives
  `DEEPSLEEP_RESET` and the first frame's exact RGB bytes; another GPIO0
  press after wake/release sleeps again.
- Against the previous app image (built without the prefix map), all 14
  captured panel PNGs (nine screens, wrap, two sleeping frames, wake and the
  second power-off) are byte-identical, and the sleeping PCs are the same.
  The instruction totals differ (previously 6,969,641 / 93,508,722) because
  the shorter `__FILE__` strings move rodata and code addresses.
- No Rust or Cargo file differs from main, and no Rust test reads the added
  files, so Clippy and the workspace tests are main's.
- `RUSTUP_TOOLCHAIN=1.99.0 tools/wasm-build.sh` passes. The section check
  reports DWARF absent and function names retained.
- `node tools/wasm-test.mjs` with CI's list (`hello c3-hello c6-hello
  p4-hello c6-energy-scan c6-contiki c6-contiki-net c6-rpl-net panel
  tdisplay-prices tdisplay-schematik`): all eleven pass, plus the JIT handoff
  check. The new demo emits one frame.
- Headless Chromium, landing page in light and dark at 1280×900 and 390×844:
  page width equals viewport, all thumbnails loaded, both T-Display thumbnails
  contained in their frames, no overlapping cards, the MicroPython T-Display
  card directly followed by this one, each linking its own demo.
- Headless Chromium live demo at 1440×1000 (light) and 390×844 (dark): each of
  the nine live screen masks and wrap matches native; power-off blanks, wake
  restores the wordmark; no page exceptions.
  UI pulses (3 s press, 3 s gap) allow time for the emulated debounce on a
  loaded host; the native replay is the deterministic timed behavior check,
  not wall-clock UI timing.
- Compile-valid sketch mutations (all killed by the frame verifier): uppercase
  first letter, advancing by two screens, and arming sleep before releasing
  GPIO0. Manifest mutation `min_frames: 1000000` is rejected by the existing
  WASM smoke test (`1 binary frames, wanted 1000000`); original restored.
- Evidence privacy and `git diff --check` pass.

## CPU comparison

CPU: not applicable (web-only, no emulator code changed).
No CPU benchmarks were run. No physical-board verification is claimed.

# Schematik T-Display

A restyled Schematik sketch on the classic ESP32 TTGO T-Display:
an ST7789 135×240 panel, shown landscape in the browser.
Open `run.html?wasm&fw=tdisplay-schematik` from the web demo page.

GPIO35 (next) cycles through the lowercase `schematik` wordmark, four fun
lines, processor/flash/ADC specs and `running in WebAssembly`, then wraps back.
Long lines are split into two centered lines; nothing scrolls or needs assets.
GPIO0 (power) blanks the screen and enters deep sleep;
press it again to wake, reboot and redraw the first message. Release each
press before pressing again. The wake press is not another power-off request.
No physical-board verification has been performed.

## Provenance

Adapted from a public Schematik Arduino starter, fetched on October 16, 2026.
The text, layout and next-button code have been rewritten for this demo;
this is no longer an unchanged starter. The SPI/display pins and GPIO0
deep-sleep/EXT0 wake behavior are preserved. Arduino_GFX is pinned to 1.6.9
(the starter's 1.5.2 does not compile with this core).
No dependency source was modified. The sketch is contributed under this
repository's MIT licence. See verification.md for the source of each spec
and the intentionally omitted hardware claims.

## Build and public images

PlatformIO Core 6.1.19; pioarduino 55.03.38-1, Arduino-ESP32 3.3.8,
ESP-IDF 5.5.4, Xtensa GCC 14.2.0_20260121, Arduino_GFX 1.6.9:

```sh
pio run -d examples/schematik-tdisplay
python3 examples/schematik-tdisplay/package.py
```

The packager copies bootloader, partition table and application to
`web/wasm/fw/public/schematik-tdisplay-*`, like the other committed public
demos. Pages serves these files directly; no additional host or release is
needed. The ROM is fetched by `tools/fetch-demo-assets.sh --no-linux`.
The manifest uses no function stubs and does not ship the debug ELF.
`SHA256SUMS` pins the application source and shipped images.
The recipe maps the PlatformIO package directory to `/platformio` in
`__FILE__` strings, so the images carry no local paths.

`public/schematik-tdisplay-NOTICE.txt` inventories the linked components and
licences. The adjacent source archive contains the unmodified Arduino core,
SPI/Wire libraries, Arduino_GFX, application and build recipe so that the
LGPL library can be modified and the application rebuilt/relinked. Extract
it, replace the installed core or library sources with your modified copies,
and rerun the pinned PlatformIO recipe. The application source is supplied,
not just an object file. Retain the licence and source files alongside
redistributed images. The linked archive list is derived from nonzero
allocated sections in the application map, not the discarded sections.

## Native check

```sh
cargo +1.99.0 build --release --bin esp32sim
tools/fetch-demo-assets.sh --no-linux
target/release/esp32sim --chip esp32 --board ttgo-tdisplay \
  --rom web/wasm/fw/esp32_rev300_rom.elf --boot rom --console uart0 --flash-mb 4 \
  --bootloader web/wasm/fw/public/schematik-tdisplay-bootloader.bin \
  --ptable web/wasm/fw/public/schematik-tdisplay-ptable.bin \
  --app web/wasm/fw/public/schematik-tdisplay-app.bin \
  --max-seconds 4 --tft-png frame.png
```

The native frame is portrait; rotate it 270 degrees clockwise to read it.
For button replay add `--script buttons.txt`: `1 press 35 150` advances the
message; `1 press 0 150` followed by `3 press 0 150` powers off and wakes.
The browser smoke test is `node tools/wasm-test.mjs tdisplay-schematik`.
Its `min_frames` requirement rejects a boot that never draws the display.

For exact text/order/layout assertions on every screen, install Pillow and run
`OUT=$(mktemp -d); python3 examples/schematik-tdisplay/verify_frames.py "$OUT"`
after the native build and PlatformIO build. It replays all nine screens, wrap,
power-off, EXT0 wake and another power-off after releasing the wake press. Actual PNGs,
button scripts and console logs are retained under `$OUT`; no hardware or
paid generation is needed.

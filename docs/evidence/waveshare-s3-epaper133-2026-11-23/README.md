# Waveshare ESP32-S3-ePaper-13.3E6

The unchanged official factory image now boots from the S3 ROM, follows its BOOT
button mode-selection flow, reads a normal FAT32 SD card and completes a six-color
refresh. The 40-second native run produced one **1600 × 1200 framebuffer**, with
480,000 bytes written to each controller half and a 19-second BUSY interval.
The native image was inspected and contains the expected complete six-color scene.

The default factory mode is an online/AP path. A separate six-second run with an
empty card produced no panel traffic or completed frame; its initial white state
is retained as a negative result. This profile remains **trial**, with audio,
networking and optical behavior outside its accepted scope.

## Exact hardware and sources

The board is `ESP32-S3-ePaper-13.3E6`, with WROOM-2-N32R16V, **32 MiB Flash and
16 MiB PSRAM**. The [official overview](https://docs.waveshare.com/ESP32-S3-ePaper-13.3E6)
and [schematic, page 1](https://files.waveshare.com/wiki/ESP32-S3-ePaper-13.3E6/ESP32-S3-ePaper-Driver-Board.pdf)
identify that module. The overview's later onboard-resources paragraph instead
says N16R8; that conflicting paragraph is not used to shrink the physical memory.
The fixed factory image also declares 32 MiB Flash.

| Signal | Exact board connection |
| --- | --- |
| BOOT | GPIO0, active low; no invented power or user button |
| Panel SPI | SPI3: clock9, MOSI46, master CS10, slave CS3 |
| Panel control | DC11, RESET2, BUSY12 active low, power enable1 active high |
| SDMMC | CLK6, CMD7, D0=5, D1=4, D2=16, D3=15 |
| Audio control I²C | SDA41, SCL42; ES8311 address18h and ES7210 address40h |

The source repository is
[`waveshareteam/ESP32-S3-ePaper-13.3E6` at `f8e9456bd1ca520f638786142fc17a74154f4e05`](https://github.com/waveshareteam/ESP32-S3-ePaper-13.3E6/tree/f8e9456bd1ca520f638786142fc17a74154f4e05).
Key files are `components/port_bsp/display_bsp.cpp`, `button_bsp.c`,
`sdcard_bsp.cpp`, and the mode handlers under
`example/ESP-IDF-5.5.1/06_xiaozhi-esp32/`. The board configuration identifies the
audio control bus. The separate `04_E-Paper_Example` establishes the alternative
physical D/C mode; it is not treated as the factory mode.

[sources.json](sources.json) records every new HTTP response's URL, time, byte count
and SHA-256, including unsuccessful requests. A 404 at a candidate metadata path
does not imply that the board lacks firmware. The original responses are retained
outside Git; the public SD configuration containing credential-like content was
not fetched or reused.

`Firmware/01_factory.bin` is published as factory firmware. Its 6,025,696 bytes are
loaded unchanged at flash offset 0. Its SHA-256 is
`cc3af5a5a3a29f62a32e339754561de57229d383f1da6fa73342ee1493ec48a2`.
Its embedded descriptor says `xiaozhi` 2.0.1, IDF 5.5.1, compiled
2026-04-13 14:57:27. That timestamp is a build timestamp, not a release date;
[factory-image.json](factory-image.json) records the inspected header/descriptor.

## Display contract and limits

Each half contains a fixed 600 × 1600 four-bit RAM buffer. Palette indices
0/1/2/3/5/6 represent black/white/yellow/red/blue/green. Invalid colors, overflow,
incomplete RAM and unsupported resolution/mode parameters cannot publish a frame.
The source's panel settings `PSR=DF 69` and `TRES=04 B0 03 20` are explicit checks.

The default BS1-high mode takes a command from the first byte after physical
GPIO-CS assertion. Subsequent transactions, including the manufacturer's
4096-byte chunks, remain data while CS is held. This is distinct from D/C mode,
which requires an explicitly driven DC pin and an explicit `dc_mode()` choice.
The tests check that transaction boundaries and DC changes cannot falsely start
commands in the factory mode.

RAM writes become visible only after the DRF command completes. The BUSY GPIO is
actually propagated into the SoC's input register; firmware must observe it.
The 19-second full-refresh duration is the nominal value from the
[official glass specification](https://www.waveshare.com/13.3inch-e-paper-hat-plus-e.htm).
Power/reset cancels an incomplete operation and retains the already-visible
image. PON/POF completion is an untimed one-cycle digital event, not a measured
power-rail settling delay.

The exact controller die part number is not established by the supplied manuals.
This implementation therefore does not borrow a named controller with different
dimensions. It does not simulate analog LUTs, temperature effects, ghosting,
calibrated optical color, 9-bit/QSPI or Arduino software SPI. Audio support is
only the documented codec control-port subset. No RTC is invented from a generic
product description without an established device and connection in the checked
schematic. Physical USB-relative image orientation remains uncalibrated; the
framebuffer follows the pinned factory source's coordinate mapping.

## Reproduce the accepted input/output flow

Build the standalone probe. It imports only this new board module and does not
require central CLI/WASM/catalog registration:

```sh
cargo test --release --manifest-path docs/evidence/waveshare-s3-epaper133-2026-11-23/probe/Cargo.toml
cargo clippy --release --manifest-path docs/evidence/waveshare-s3-epaper133-2026-11-23/probe/Cargo.toml --all-targets -- -D warnings
cargo build --release --manifest-path docs/evidence/waveshare-s3-epaper133-2026-11-23/probe/Cargo.toml
```

Fetch the fixed factory binary and `SD_files/img/1.bmp` using the URLs in
`sources.json`, checking their recorded hashes. Create the test card:

```sh
node docs/evidence/waveshare-s3-epaper133-2026-11-23/make-sd-card.mjs original-1.bmp basic.img
WAVESHARE_INPUT_SCRIPT=docs/evidence/waveshare-s3-epaper133-2026-11-23/inputs-basic-mode.txt \
  <probe-binary> <esp32s3_rev0_rom.elf> <01_factory.bin> basic.img 40 <output-prefix>
```

The card is a **project-authored 64 MiB FAT32 test fixture**, not a manufacturer
disk image. Only `/img/1.bmp` is original upstream content. The three text files
are generated test configuration, with blank service fields and no credentials.
The guest decodes the BMP through native SDMMC; the host does not copy an image
into the framebuffer. [sd-fixture.json](sd-fixture.json) identifies all files.

The four input events at 2.0, 5.0, 6.5 and 10.0 guest seconds use the source
program's long-press/click flow. It causes two deliberate software resets when
changing modes. There is no NVS injection, application-direct boot, guest-memory
patch, PC skip or host pixel injection.

## Current verification

[verification.json](verification.json) contains the current compiler and source
hashes, commands, independent default/Basic runs, input hashes, SD transactions,
framebuffer hashes and raw artifact identifiers. All five board/protocol tests
and release probe Clippy pass. In the Basic run:

- 1,062,947,202 instructions; two expected source-driven software resets.
- First completed frame observed at guest second 33, still present at second 40.
- BUSY low from cycle 3,324,047,649 to 7,884,047,649: exactly 4,560,000,000 cycles.
- SDMMC read 5,821,552 bytes and wrote 4,096 bytes. Six probe timeouts are retained
  in the report; initialization and the subsequent real reads/writes completed.
- No invalid palette data, overflow, unsupported command or rejected busy write.
- Final RGB888 SHA-256:
  `313454cc97570419845bd3bc8c9d718bcaa57ddf2914b4f994949e69d97fce49`.

Raw native logs, GPIO captures and PNGs stay under ignored
`output/waveshare-s3-epaper133/`; their hashes and sizes remain in the compact
receipt. [integration.json](integration.json) records the exact central
CLI/WASM/catalog registration and memory capacities.

## Public platform runtime acceptance

The production platform runtime, Worker and WASM were executed in Node with the
same pinned factory binary and a freshly generated acceptance card. This checks
the path used by the website, including catalog selection, ROM loading, SD
attachment and public BOOT input messages. It is separate from browser layout
and pointer-hit testing, which remain unaccepted in this environment.

The Basic run reached 40 guest seconds and delivered one 1600 × 1200 frame. Its
RGB888 hash exactly matches the native result above. All four press/release
events have actual guest-time receipts in
[platform-basic-result.json](platform-basic-result.json); the scheduling uses
the public asynchronous input API, so those timings are recorded rather than
claimed identical to the native cycle script. The output image was visually
inspected as a complete landscape scene, with both controller halves populated.

The independent default-mode run reached six guest seconds with **zero frames**.
The Worker correctly did not emit the initial white optical state as a rendered
guest frame. `capture()` returned `NO_FRAME`, as required by the negative test;
see [platform-default-result.json](platform-default-result.json). An initial
acceptance-script expectation that white would be emitted was corrected; no
emulator behavior was changed to satisfy that expectation.

Reproduce after building the production WASM:

```sh
node tools/waveshare-epaper133-acceptance.mjs basic <01_factory.bin> <fresh-basic.img> <output-directory>
node tools/waveshare-epaper133-acceptance.mjs default <01_factory.bin> <fresh-empty-fat32.img> <output-directory>
```

Both runs use isolated transient storage. The normal workbench defaults to an
empty 64 MiB FAT32 card and does not insert the acceptance image into a user's
existing card. The factory entry remains `trial`: these results accept the
offline display path, not all factory modes, audio, networking or real hardware.

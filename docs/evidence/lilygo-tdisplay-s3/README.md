# EX246 — T-Display-S3 physical i80 and lazy PWM display snapshots

This is a board correctness extension, not a scheduler or speed experiment. Searches for LCD_CAM, i8080, parallel LCD and D/C found no prior experiment for this physical ST7789 wiring. Related EX056 covers timed TE/SPI completion and EX134 covers device-tick deferral; neither supplies this board's i80 D/C signal or GPIO WR transport. Neither mechanism is retried here.

The adopted subset preserves physical pin routing, reset/read/select gates, native glass orientation, and the existing LEDC latch. The i80 DMA engine retains its synchronous digital-completion limitation. New PWM observation is opt-in and occurs only before host display publication/capture and board attachment; no new peripheral tick work. Averaged brightness is a rendering approximation, not a calibrated model of the manufacturer's pulse backlight IC.

## Inputs and results

- `sources.json`: pinned manufacturer and IDF source revisions, actually fetched URLs, UTC, media types, byte counts and SHA-256.
- `results.json`: unchanged official BINs, ROM/runtime artifacts, exact native work, framebuffer identities, GPIO instruction tests and retained factory/LVGL failure.
- `worker-tft.json` / `worker-factory.json`: actual platform Worker results, with binaries fetched into RAM.
- Native PNG and raw RGBA frames remain in ignored `output/`; their hashes and normalized pixel comparison are in `results.json`. No captures or upstream firmware are published by this change.

The native 4 s font image and Worker 3.9 s image are the same stable font page: all 170×320 RGBA pixels match, SHA-256 `994ed0dcd5a4d9d86345e53beb619639daf9915941a712d87b19a6430316096e`. Worker later shows another nonblank font page. The raw native PNG was visually inspected; its portrait image is rotated by the existing CAD screen transform in the platform. No 3D calibration was changed.

Factory 20231013 reaches LCD initialization but does not draw. Native JIT and interpreter at 0.7 s agree on both instruction counts and the `LCD_INT_ST` wait. A retained 12 s run still has zero pixels. LVGL 20250109 encounters the same wait. The source correlation with a nonnull zero-length parameter is evidence about this emulation, not a demonstrated defect on silicon. Owner checking and DMA fault behavior are unchanged.

The TFT demo does not read buttons. The portable `esp32s3/tests/tdisplay_s3.rs` probe runs real guest GPIO reads after the same web input messages, independently checking GPIO0/14, combinations, release and pause. It is deliberately distinguished from official firmware behavior.

## Commands

Build the native CLI and production WASM as documented in CONTRIBUTING.md. Set `ESP32SIM_ROM_DIR` to the ROM directory. The caller must supply the exact source URL and hash from `results.json`:

```sh
node tools/probe-tdisplay-s3-official.mjs --url "$TFT_URL" --sha256 "$TFT_SHA256" --expect tft-demo
node tools/probe-tdisplay-s3-official.mjs --url "$FACTORY_URL" --sha256 "$FACTORY_SHA256" --expect factory-blocked
```

Native reproduction uses a caller-fetched, hash-verified image:

```sh
esp32sim --board tdisplay-s3 --flash-mb 16 --psram-mb 8 --boot rom \
  --rom "$ESP32SIM_ROM_DIR/esp32s3_rev0_rom.elf" --flash-image "$IMAGE" \
  --max-seconds 4 --console both --tft-png output/tdisplay-s3.png --no-dump
```

For the factory descriptor diagnosis use `--max-seconds 0.7 --peek 3fca7cc4,3 --peek 60041064,3 --disasm 4206263c,5`; repeat with `--no-jit`. No guest bytes or registers are patched.

## Checks and limits

Board route/DCS tests, shared i80 phase tests and the portable input/PWM tests pass. Both warning-denying Clippy targets pass. Catalog/schema/coverage checks and actual Worker font/blocked probes pass. Full combined workspace CI and cross-workload CPU comparison are owned by the integration checkpoint; this receipt makes no performance claim.

Open PR overlap was inspected: #299 touches S3 write budget refresh and #301 LEDC tick/wrap implementation; neither is duplicated or modified. Existing LEDC state queries are reused. Earlier #312 GP-SPI DMA work was also avoided; only the existing i80 delivery path gained D/C metadata.

No hardware validation. Battery, charger, sleep/wake, touch, analog/electrical pulse timing and external interfaces are outside this delivered subset. Source files from different dates are retained separately; the 2023 TFT source uses LEDC while the current example uses GPIO backlight, so current source is not used to claim old binary behavior.

Privacy: retain source hashes, input artifact hashes, numeric results and selected register facts. Omit raw logs containing upstream build paths, local paths, binary strings, device identities, process/session inventories and author metadata. This curation does not remove any measured value used in a conclusion. Raw logs remain only in the ignored task output directory for diagnosis.

# Camera host input to guest RAM validation

`tools/platform-m5-camera-worker-test.mjs` runs the six exact U174/U174-B,
C126-CAM/K147-CAM and C126-M12/D062-M12 products through the public runtime,
`setCameraPicture`, production Worker and actual compiled WASM.

```sh
ESP32SIM_WASM=/path/to/current/esp32sim.wasm \
ESP32SIM_ROM_DIR=/path/to/roms \
M5_CAMERA_WORKER_RECEIPT=docs/platform/evidence/m5-camera-worker-20261126/receipt.json \
node tools/platform-m5-camera-worker-test.mjs
```

The WASM must include the camera board/capture commits. The test fails rather
than skipping when a board or public input API is missing. `M5_CAMERA_IDS` can
limit an investigative run to comma-separated exact product IDs; the receipt
lists only products actually executed. Full acceptance uses all six.

The original SRAM image generator is
`tools/fixtures/m5-camera-capture-probe.mjs`. Its real LX7 instructions configure
physical GPIO/I²C routes, SCCB registers, XCLK and LCD_CAM/GDMA descriptors.
Every receive buffer is first filled with a guest-written `0xa5` sentinel.
After EOF or a bounded guest CCOUNT timeout, the guest reads its RAM and prints
the first 16 bytes and the full standard CRC32 on UART0. Node's independent
`zlib.crc32` checks the expected wire bytes. No host directly sets sensor
registers, DMA buffers or serial text.

PPM red followed by BMP green must change the already-running guest's captured
RAM. Public Worker replacement removes the private source: the probe first
observes the model's black default, then an explicit resupply restores green.
Wrong DVP routing, missing XCLK and inactive power/reset must retain sentinel
bytes without successful EOF. OV2640/PY260/OV3660 accept deterministic, valid
baseline JPEG fixtures at the configured dimensions and return identical bytes;
GC0308 refuses JPEG. Malformed/over-limit inputs must preserve the previously
accepted source. Private pictures remain excluded from recording/sharing.

`tools/m5-camera-probe-isa-test.mjs` separately executes the same authored CRC,
hex printer and timeout code on a disconnected NoBoard using production
Worker/WASM. Its expected sentinel CRC is an assembler smoke check, not positive
camera evidence. It can run against an older WASM while the six-board build is
pending. JPEG fixtures are first-party solid-red images, with exact hashes,
Pillow version and encoding options in `tools/fixtures/m5-camera-jpeg.json`.

These are original app-direct diagnostics. They do not establish manufacturer
factory firmware compatibility, full ESP-IDF startup, UVC endpoints, optics,
PDM audio, networking, physical power or hardware timing calibration.

## Executed central batch 020 result

The full six-product run passed on 2026-11-26 with WASM SHA-256
`0cc18d96042c0b2707afbe3b80102cabcbdd6868fccfcc84504cabed8b3179a2`.
The public runtime sent 50 sensor-input messages through production Worker
`load(kind=7)`. All 34 recorded positive CRC observations and 18 negative
observations passed in 19.525 seconds. This duration is the integration test's
wall time, not a physical-camera rate or timing measurement.

| Exact product | Sensor | Guest capture | Red PPM CRC32 | Green BMP CRC32 | JPEG CRC32 |
| --- | --- | --- | --- | --- | --- |
| U174 | OV2640 | 8×4 RGB565 | `daf25a9d` | `95d4c22c` | `359c9549` |
| U174-B | PY260 | 96×96 RGB565 | `c9ec00ac` | `cd853ec5` | `3a6ea4d5` |
| C126-CAM | GC0308 | 8×4 RGB565 | `daf25a9d` | `95d4c22c` | rejected |
| K147-CAM | GC0308 | 8×4 RGB565 | `daf25a9d` | `95d4c22c` | rejected |
| C126-M12 | OV3660 | 8×4 RGB565 | `daf25a9d` | `95d4c22c` | `359c9549` |
| D062-M12 | OV3660 | 8×4 RGB565 | `daf25a9d` | `95d4c22c` | `359c9549` |

Every product also passed Worker reset to the explicit black default, explicit
green resupply, wrong data route, missing XCLK, inactive power/reset, malformed
picture rejection and oversized-picture rejection. All three negative capture
conditions reported EOF=0 with the complete guest-written `0xa5` buffer CRC.
The valid JPEG files returned their exact 632 or 882 source bytes; there was no
JPEG encoder or host DMA-buffer write.

[`receipt.json`](evidence/m5-camera-worker-20261126/receipt.json) retains the
guest UART byte heads/CRCs, original probe hashes, independent expected-buffer
hashes, actual WASM/ROM hashes, public input hashes and byte counts, production
runtime/Worker/JIT/pacing hashes and exact catalog-file hashes. The tested
catalog in this working tree activates all six products; later five-product
publication snapshots may activate them in separate batches and must retain
their own catalog identity when revalidating. The generated original
diagnostics are not manufacturer firmware images.

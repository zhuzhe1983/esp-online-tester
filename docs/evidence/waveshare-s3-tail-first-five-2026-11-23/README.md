# Waveshare S3 tail: first five listing families

These records cover **five listing families, eight exact sale SKUs and nine runtime/model profiles**. They are simulator observations and source reconstruction evidence. They do not establish physical hardware, browser WebGL, full application or continuous operation acceptance.

| Listing | Exact profiles | Flash / PSRAM | Runtime boundary |
|---|---|---|---|
| ESP32-S3-Touch-LCD-2.8 | SKU 27690 V1 CST328 and V2 CST3530; non-touch SKU 30223 | 16 / 8 MiB | ST7789 SPI, two physically routed I²C buses, exact touch revision, RTC, QMI8658C, native one-bit TF |
| ESP32-S3-Touch-LCD-1.69 | SKU 27350 | 16 / 8 MiB | ST7789V2 240×320 GRAM / 240×280 visible window, CST816T common contact subset, RTC, QMI8658C; no invented TF |
| ESP32-S3-Matrix | SKU 27119 | 4 / 2 MiB | ESP32-S3FH4R2, 64 WS2812 RGB bytes on GPIO14, QMI8658C; distinct from HUB75 RGB-Matrix |
| ESP32-S3-Touch-LCD-7 | Touch onboard SKU 27078; non-touch SKU 30406; external-antenna SKU 35892 | 16 / 8 MiB | Original CH422G, RGB565 physical routes, GT911 only on touch variants, SPI TF with EX4 chip select |
| ESP32-S3-Touch-LCD-1.28-B | Cased SKU 26959, explicitly documented base PCB 25098 | 16 / 2 MiB | Existing exact Rev3 GC9A01A/CST816S/QMI8658A base path; enclosure is unknown and not rendered |

## Electrical and firmware evidence

The retained source manifest is `docs/platform/research/waveshare-s3-tail-first-five-sources/manifest.json`. Each source entry contains its original URL, retrieval time, original byte count, SHA-256 and retained archive path. ZIP members additionally retain their container/member identity. Original firmware bytes remain remote-only; `factory-image-verification.json` records boot/application chip ID 9, checksum/digest and partition-table MD5 checks. The complete manufacturer Matrix archive was enumerated and contains no complete ESP image in the recorded scope, so no factory default was invented.

`native-runs.json`, `native-run-inputs.json` and the eight board subdirectories record original-image executions bounded at 5,000 guest milliseconds. UART text views normalize line endings and trailing whitespace; unchanged bytes are retained as `uart.original.gz` with their original hashes. `frames` counts accepted RAMWR commands or RGB scan completions, not human-visible frame rate. The three 2.8-inch profiles expose 381 / 81 / 267 RAMWR commands, 46 / 16 / 46 final RGB565 colors, and 2,256 / 1,744 / 2,256 actual TF read bytes. The 1.69-inch image reaches `Setup done`, discovers addresses 15/51/6B, and produces 10 RAMWR commands and 1,929 final colors. The base 1.28-inch image produces 40 RAMWR commands and 50 colors. The three 7-inch runs remain at their original boot-stage stop with black output and no RGB scan completion. These failures remain visible in firmware/product limitations; no successful whole-application outcome is inferred from a process exit code.

The V1 2.8-inch legacy demo image and non-touch 7-inch LVGL image have **example** role because retained purpose evidence does not prove shipping recovery. The V2 2.8-inch FactoryOnly SHA-256 exactly matches its official V2 demo ZIP member. External-antenna 7-inch firmware is explicitly a base-board reference whose exact RF option has not been qualified. Every image remains trial and `hardwareValidated=false` / `simulationVerified=false`.

Two concrete SPI issues were caught by original-image execution and corrected. MOSI is consumed during full-duplex host transfers because the LCD's unimplemented readback remains floating FF; simultaneous host reads must not suppress real writes. Espressif's retained v5.5.1 `spi_ll_master_set_mode` maps both modes 0 and 3 to `CK_OUT_EDGE=0`, with different clock-idle levels. Treating `CK_OUT_EDGE` as CPHA wrongly rejected mode 3. Regression tests now accept both actual modes and reject modes 1/2, wrong pads, non-byte phases and multiple lanes. ST7789 C0 output scan is tracked independently from MADCTL RAM address order, using the original factory portrait reference. XBGR/XMV/XINV, gamma, optical orientation and brightness calibration remain limited as reported by the board model.

The LX7 tests execute real S32I/L32I instructions through the SoC GPIO matrix, SPI, I²C FIFOs and interrupts, and RMT. A separate SDMMC IDMAC test uses actual SoC Bus MMIO and card transfers without executing LX7 instructions. They check exact/wrong pin routes, touch reset and contacts, stale acknowledgement, SD 512-byte write/read, card ejection, unsupported bus width, backlight and reset gating, and Matrix RGB wire order. Original diagnostics are not labeled manufacturer firmware.

## Models and visual review

`model-loader-check.json` contains independent production Three.js CPU loader checks for all nine model JSON files. Six unique geometries have four-view renders and source-photo overlays. `model-review-receipts.json` records model/mesh hashes and explicitly identifies geometry-equivalent variant aliases. These are actual production-loader meshes rendered offline, without a browser/WebGL acceptance claim. Reproducible large mesh dumps were removed after their byte counts and hashes were retained.

The native 2.8-inch assembly retains all 453 source components; 1.69-inch retains 298; 7-inch touch retains 850; 7-inch non-touch retains 851; 1.28-inch base retains 230. Source units and relative coordinates are unchanged; the 7-inch source uses a proper rigid rotation. Native connectors, hollow metal shells, display layers and board topology remain independent of the live optical surface. The Matrix model uses the published 25×25 mm outline and 2.54 mm pitch, a photo-registered 8×8 front array, estimated hole/USB depth dimensions, and an explicit UNKNOWN rear. Its physical LED chain-to-position mapping is not calibrated.

Visual review checked the front, reverse, edge and oblique views, then the original-source overlay. The corrected 2.8-inch overlay follows the published cover silhouette. The 1.69-inch screen uses explicitly estimated rounded corners. The 7-inch touch cover material follows the official black photo where native STEP color conflicted; other uncolored native geometry remains grey. The 7-inch published physical window aspect differs from 800×480 pixels, so the model records this source conflict and does not rescale native CAD or claim a verified active-pixel boundary.

The cased 1.28-B SKU visibly states **bare-board reference; enclosure not modeled** in product selection and knowledge. Repeated source-image 403 responses and absent exact enclosure CAD are retained as collection limits. External-antenna 7-inch likewise identifies its onboard-antenna CAD baseline and unknown RF mechanical parts. Non-touch 2.8-inch resources explicitly link the touch CAD; electronics and optical differences remain unverified. No hidden button targets are enabled through unknown geometry.

## Reproduction

Use the same exact source cache and validated 64 MiB FAT32 image; no firmware patching is performed.

```sh
CARGO_BUILD_JOBS=1 cargo test -p esp32s3 --lib waveshare_
CARGO_BUILD_JOBS=1 cargo test -p esp32s3 --test waveshare_tail_guest --test waveshare_tail_sd --test waveshare_qmi_input
CARGO_BUILD_JOBS=1 cargo clippy -p esp32s3 --all-targets -- -D warnings
CARGO_BUILD_JOBS=1 cargo build -p esp32s3 --release --example waveshare_source_attempt
python tools/waveshare-batches/s3-tail/run_original_images.py --sources SOURCE_CACHE --binary target/release/examples/waveshare_source_attempt --rom web/wasm/fw/esp32s3_rev0_rom.elf --sd DEFAULT_FAT32_IMAGE --evidence EVIDENCE_DIRECTORY
```

`prepare_models.py`, `refine_visual_metadata.py`, `audit_factory_images.py`, `export_first_five.py` and `review_models.py` retain reproducible source/model/export steps. Final committed model plans are directly rebuildable by the repository's manufacturer model builder. Root integration owns central catalog/profile generation from the five fragments; this source commit does not replace central catalogs.

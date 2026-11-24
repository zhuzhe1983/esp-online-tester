# Waveshare S3 tail: next five listing families

This group covers **five listing families and six exact sale SKUs/profiles**. It adds a dedicated GEEK runtime, qualifies the original 4.3-inch touch/non-touch boards, adds an explicitly labeled LCD1.28-B internal reference, and refreshes evidence for two existing bare-board products. It does not count the existing bare boards as newly invented electrical adapters. These are simulator and source-reconstruction results, without physical hardware, browser WebGL or full-application acceptance.

| Listing / SKU | Flash / PSRAM | Exact digital boundary |
|---|---|---|
| LCD1.28-B / 26956 | 16 / 2 MiB | Canonical LCD1.28 base; GC9A01A, RST12, BL40, QMI8658C on 6/7; no touch/TF. Case unknown and not rendered. |
| LCD1.28 bare / 26541 | 16 / 2 MiB | Existing runtime, 207-component model and source history preserved. |
| GEEK / 26405 | 16 / 2 MiB | ST7789, LCD 12/11/10/8/9/7, separate SPI TF 36/35/37/34, BOOT0; no invented touch/RTC/IMU. |
| Original Touch4.3 / 25948 and LCD4.3 / 30493 | 16 / 8 MiB | ST7262 800×480 RGB565, original CH422G, GT911 only on touch SKU, SPI TF with external EX4 chip select. |
| Touch1.28 bare / 25098 | 16 / 2 MiB | Existing Rev3 GC9A01A/CST816S/QMI8658A route with RST14, BL2; existing model preserves UNKNOWN current PCB details. |

## Original firmware observations

All five distinct original image identities passed the retained image/partition checks in `factory-image-verification.json`. Original firmware bytes remain remote-only. `native-run-inputs.json` pins the native runner, ROM, verified-image manifest and prepared 64 MiB FAT32 image. `native-runs.json` records a 5,000 guest-millisecond bound, exact memory, stop reason, UART hashes and framebuffer artifacts. LCD1.28-B explicitly shares one LCD1.28 base run; this is not a separate physical cased-device test.

| Original image / board | Observed guest time | Accepted RAMWR / RGB completion | Final colors | Result |
|---|---:|---:|---:|---|
| LCD1.28 | 5,000 ms | 938 RAMWR commands | 63 | Original demo output visible; continuous application behavior remains unqualified. |
| Touch1.28 | 5,000 ms | 40 RAMWR commands | 50 | Original parameter screen visible; continuous UI and physical touch remain unqualified. |
| GEEK FactoryOnly 20260409 | 35 ms | 0 | 1 | StoreProhibited, EXCVADDR 0x3DFFFFF4, then software reset. |
| Original Touch4.3 FactoryOnly 20260224 | 5,000 ms | 0 RGB completions | 1 | Boot-stage stop with black output. |
| Original LCD4.3 FactoryOnly 20260223 | 5,000 ms | 0 RGB completions | 1 | Boot-stage stop with black output. |

RAMWR counts are accepted commands, not a measurement of visible frame rate. A successful host process exit does not establish application success. GEEK retains its actual 2 MiB PSRAM; no 8 MiB workaround or original image modification hides the failure. Its root cause is unresolved. The retained UART view normalizes line endings and trailing whitespace; `uart.original.gz` preserves the original bytes and recorded hashes. All catalog images remain trial, `hardwareValidated=false` and `simulationVerified=false`.

GEEK's original LCD_Driver sample uses MADCTL 0x70 with 135×240 and +52/+40 bounds, whereas the SD/LCD example selects TFT landscape rotation 1. Both are retained; the runtime presents the 135×240 native aperture as 240×135 landscape without rewriting either source. The dedicated GEEK panel accepts actual SPI modes 0/3 (`CK_OUT_EDGE=0`) and consumes MOSI even during simultaneous host reads; unwired LCD readback remains FF. Unsupported widths/phases and wrong pads are rejected. LCM XBGR/XMV/XINV, analog gamma, physical color and glass calibration remain limited.

The original 4.3-inch pin map was independently checked against its schematic and BSP before reusing the original 7-inch digital device implementation. It is not a 4.3B/C alias. CH422G obeys its multi-address, single-byte WCH protocol and actual datasheet mode bits; no register-prefix substitute or fabricated ACK is used. GT911 reset/address strapping and EX2 backlight are separate. EX3 LCD_RST is behind R20 NC/100R in the 4.3-inch schematic (R112 NC/100R on 7-inch); unknown resistor population is an explicit optional-reset boundary. A related PCF85063 resource link does not establish an onboard RTC, so none is fabricated.

## Electrical diagnostics

The retained final checks are 71 passing Waveshare unit tests (12 pre-existing ignored cases), eight passing real LX7 guest tests plus one SoC MMIO SDMMC test, Clippy with warnings denied, catalog schemas, and 13 catalog/knowledge platform tests. The final tail guest set also passed in release mode. The tests run S32I/L32I instructions through the SoC GPIO matrix, SPI/I²C registers and FIFOs; they are original diagnostics, not manufacturer firmware.

The added guest coverage verifies GEEK's aperture, panel routing, mode 3, backlight and absent I²C peripherals. GEEK, Touch4.3 and LCD4.3 each write and read an exact 512-byte TF sector over their independently declared physical SPI routes, then check wrong MISO and card ejection. Non-touch 4.3 rejects GT911 and RTC addresses. Touch4.3 runs the same actual LX7 CH422 command/reset/contact readback checks as original 7-inch. Existing QMI input and one-bit 2.8-inch SDMMC tests remain passing. SPI card diagnostics use an isolated 1 MiB store; original-image attempts use the prepared 64 MiB FAT32 card. No native SDMMC capability is declared for GEEK or original 4.3-inch.

## Mechanical evidence

All six model JSON files were loaded independently by the production Three.js CPU geometry/raycast checker. `model-review-receipts.json` records model and temporary mesh hashes; four views were inspected for all six geometries. Native source-photo overlays were checked against the retained dimension images. GEEK was compared with its original S3 rear photograph and the dimensions image linked by current S3 documentation. Offline mesh renders are labeled geometry-only; the screen color swatch is not firmware output. Large reproducible mesh dumps were removed after hashing.

The existing LCD1.28 and Touch1.28 asset directories are unchanged. The latter keeps the existing UNKNOWN Rev3 PCB rather than silently replacing it with the unmarked 2024 assembly. LCD1.28-B retains the full 207-component bare native assembly as a qualified internal reference; its product selection says **bare-board reference; enclosure not modeled**. No 3D actuator is exposed through unknown case geometry.

Touch4.3 retains all 700 native source components. The non-touch derivative retains 699 and removes only independent touch-cover component 1, with exact non-touch photo/outline evidence; other PCB/BOM differences are unknown. Original millimetres, relative placement and connector geometry are retained. The published 95.54×54.36 mm touch window and 95.04×53.86 mm non-touch active rectangle differ from the digital 800×480 aspect; this is a documented source conflict, without CAD rescaling or physical pixel-calibration claims.

GEEK is an explicitly estimated dimension/photo reconstruction. Its body and removable USB cap are separate hollow parts, with a real optical opening and UNKNOWN internal PCB. The production loader checks the empty body cavity, inner wall, optical opening, independent rear cover, inspection visibility and shell material isolation. Published outside dimensions are 61×24.5×9 mm, including the 16 mm cap. Wall thickness, connector cutouts, internal clearance and USB/cap fit are unmeasured. BOOT0 is available as a workbench input with no guessed 3D target. The current S3 documentation's dimensions image has a C6-GEEK filename; that conflict is explicit, and S3 electrical pins and rear photos are independently retained. The 24.91×14.86 mm rectangle differs by 5.71% relative to 240×135 framebuffer aspect.

## Sources and reproduction

`docs/platform/research/waveshare-s3-tail-second-five-sources/manifest.json` retains source URL, retrieval time, original byte count/SHA-256 and compressed archive hash. ZIP members retain container and member identity. Public SKU/title/wiki fields are explicitly derived records, not original HTTP bodies; raw storefront pages and their anonymous session fields are not published. Existing bare-board knowledge facts, sources, revisions and asset links are preserved, with the new evidence separately identified.

```sh
CARGO_BUILD_JOBS=1 cargo test -p esp32s3 --lib waveshare_
CARGO_BUILD_JOBS=1 cargo test -p esp32s3 --test waveshare_tail_guest --test waveshare_tail_sd --test waveshare_qmi_input
CARGO_BUILD_JOBS=1 cargo clippy -p esp32s3 --all-targets -- -D warnings
CARGO_BUILD_JOBS=1 cargo build -p esp32s3 --release --example waveshare_source_attempt
python tools/waveshare-batches/s3-tail/run_original_images.py --group second --sources SOURCE_CACHE --binary target/release/examples/waveshare_source_attempt --rom web/wasm/fw/esp32s3_rev0_rom.elf --sd DEFAULT_FAT32_IMAGE --evidence EVIDENCE_DIRECTORY
```

`prepare_second_five_models.py`, `audit_factory_images.py`, `export_second_five.py` and `review_models.py --group second` retain preparation and review steps. Existing bare assets must be preserved. Five source fragments are supplied under `tools/waveshare-batches/s3-tail`; root integration owns central catalogs/profiles and sequential patch export.

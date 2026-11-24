# Waveshare C6: first five current display listings

This batch accounts for five official storefront listings and all twelve selectable SKUs. Listing order is the current category's `Position` descending order with displays prioritized. It is an ordering proxy; no hardware release date is invented from a catalogue position, repository commit or firmware filename.

| Listing | Exact SKUs | Runtime board | Memory | Panel / touch |
| --- | --- | --- | --- | --- |
| ESP32-C6-Touch-LCD-1.28 | 35442 bare, 35443 metal case (-B) | `waveshare-c6-touch-lcd128` | 16 MiB / no PSRAM | GC9A01A 240×240 / CST816S |
| ESP32-C6-LCD-1.28 | 35450 bare, 35451 metal case (-B) | `waveshare-c6-lcd128` | 16 MiB / no PSRAM | GC9A01A 240×240 / none |
| ESP32-C6-LCD-2.73 | 35303 battery, 35302 no battery (-EN) | `waveshare-c6-lcd273` | current N16, 16 MiB / no PSRAM | ILI9488 320×320 visible, 320×480 GRAM / none |
| ESP32-C6-Touch-LCD-2.8 | 35065 battery, 35066 no battery (-EN) | `waveshare-c6-touch28` | 16 MiB / no PSRAM | ST7789 240×320 / CST3530 |
| ESP32-C6-LCD-1.54 | 34658 / 34660 (-EN) non-touch; 34659 / 34661 (-EN) touch | `waveshare-c6-lcd154`, `waveshare-c6-touch-lcd154` | 16 MiB / no PSRAM | ST7789 240×240 / CST816 on touch SKUs only |

The catalogue/knowledge/firmware fragments are under `tools/waveshare-batches/c6/`. All are `partial`; no original manufacturer binary is mirrored into Git. Individual runtime, flash-layout, mechanical, full application and real-hardware acceptance are separate claims.

## Source basis and conflicts

Exact original bytes, SHA-256, byte length, URL, final URL and collection time are retained in `docs/platform/research/waveshare-c6-new-lcd-sources/manifest.json`. Per-model source manifests retain the original mechanical references and unchanged photographs. Factory image verification is in the evidence directory. The public storefront's precise SKU options are retained as explicitly derived field selections; they are not presented as original HTTP bodies.

Pinned official repositories:

| Repository | Commit |
| --- | --- |
| [Touch1.28](https://github.com/waveshareteam/ESP32-C6-Touch-LCD-1.28) | `aef9604e55bb3b7a14a41b421a8a2b97895f68a7` |
| [LCD1.28](https://github.com/waveshareteam/ESP32-C6-LCD-1.28) | `54e704868fef1c997b7a1afdb273750bd02abd2b` |
| [LCD2.73](https://github.com/waveshareteam/ESP32-C6-LCD-2.73) | `246d34db826194faa8cc56ac8e8b7fa96dbf0fcc` |
| [Touch2.8](https://github.com/waveshareteam/ESP32-C6-Touch-LCD-2.8) | `83640b2ccd5521ccd3916cea898526340dfda0f8` |
| [LCD/Touch1.54](https://github.com/waveshareteam/ESP32-C6-Touch-LCD-1.54) | `ac1aee955f29a1c7e7d9ac5feb89ae2462b3a13a` |

The 2.73 overview describes CH32V003, while its schematic identifies CH32V006K8U7 and the exact BSP writes 16-bit little-endian register values. This board uses the shared CH32V006 protocol, distinct from the 2.8 board's 8-bit CH32V003 protocol. Its current overview identifies an N16 module, while the older schematic is marked N8; the conflict is recorded and the runtime is explicitly the current 16 MiB configuration.

The 1.54 board uses external W25Q128 and a QFN40 C6, so it is not the C6FH8 8 MiB package. Its schematic title contains an S3 copy error, but the actual MCU circuit and exact product identify C6. The documentation labels 0x30/0x80 as seven-bit codec addresses; they are eight-bit forms. Schematic straps and the BSP support ES8311 at 0x18 and ES7210 at 0x40. LCD reset and touch reset genuinely share GPIO4, with the documented pulldown network.

## Digital wiring

| Runtime | LCD CLK / MOSI / CS / DC / RESET / backlight | I²C SDA / SCL | TF MISO / CS | Extra controls |
| --- | --- | --- | --- | --- |
| round1.28 variants | 11 / 21 / 10 / 19 / 20 / 7 | 18 / 8 | 22 / 15 | BOOT9; touch reset23 / IRQ4 on touch SKU |
| LCD/Touch1.54 | 1 / 2 / 11 / 3 / 4 / 6 | 8 / 7 | 16 / 17 | BOOT9, user18; touch reset4 / IRQ5 on touch SKU |
| Touch2.8 | 0 / 1 / 11 / 10 / CH32 EX1 / register5 PWM | 6 / 7 | 8 / 23 | BOOT9; CST3530 reset EX0 / IRQ18 |
| LCD2.73 | 0 / 1 / 8 / 19 / CH32 EX0 / register5 PWM | 20 / 2 | 18 / 10 | PREV4, NEXT5, BOOT9 |

SPI2 is opt-in and pin/phase checked. Wrong clock/data pins, QSPI traffic on a single-lane panel, simultaneous LCD/card selection and incorrect SD input routing cannot produce a valid display/card result. The 2.73 panel uses RGB666 with its own controller state rather than a renamed RGB565 parser. Reset, sleep, display-enable, backlight and touch power/reset gates are preserved. Ordinary keys remain GPIO inputs; analogue power keys are not invented as normal GPIO controls.

The shared 8-bit expander, CST3530, 16-bit expander and ILI9488 implementations were moved from C5 without changing their public C5 API. C5 files re-export the shared definitions. The C6 boards supply independent C6 wiring and source records.

## Mechanical scope

The 2.8 model retains all 728 native STEP components in original millimetre positions, grouped into 101 draw meshes. The exact C6 STEP is linewise identical to the preceding C5 mechanical export; their bytes differ only in CRLF/LF line endings. Reusing that verified component grouping does not reuse a C5 chip identity, GPIO map or factory image. The current C6 STEP and receipts are independently retained. No nonuniform scaling occurs.

The round touch/non-touch bare boards and metal cases are distinct models. The measured board radii are 18.75 and 18.25 mm. Both case front diameters are 48 mm, but documented depths are 12.5 and 11.5 mm, and viewing apertures are 35.85 and 33 mm. No internal board placement or exterior actuator is invented for the metal cases. BOOT remains usable through its keyboard/GPIO control when a case actuator is undocumented.

LCD2.73 uses the published 68×68 mm outline, 48.96×48.96 mm active area and 62 mm mounting grid with 2.2 mm holes. LCD/Touch1.54 uses the shared 46×46×22.5 mm enclosure and the specified 27.72×27.72 mm active area, distinguished from the 28.12 mm drawing window. Unmeasured thicknesses, wall construction, package heights, openings, internal PCB structures and battery placement remain estimates or explicit `UNKNOWN` placeholders. Original photos are unchanged; a manufacturer screenshot never substitutes for the live framebuffer.

## Validation boundaries

`cargo test -p esp32c6 waveshare --lib` covers six exact first-batch configurations, existing C6 LCD regressions, RGB data, resets, backlight, exact I²C routing, touch clamp/report/read acknowledgement, distinct expander widths, buttons and SD startup/response routing with wrong-MISO and simultaneous-CS rejection. `cargo clippy -p esp32c6 --lib -- -D warnings` is a separate static gate.

Native original-ROM runs use the manufacturer images without patching, 16 MiB flash, a five-second emulated limit and no attached card. All six bounded runs report zero CPU exceptions. LCD2.73 and Touch2.8 produce multicolour output; round1.28 and LCD/Touch1.54 finish with a single-colour snapshot despite real display writes. Full GUI operation, orientation calibration, font/render fidelity, Wi-Fi, audio PCM, battery/charge ADC, SHTC3, physical timing, WebGL and real hardware remain unaccepted. See `docs/evidence/waveshare-c6-new-lcd-2026-11-23/README.md`.

Manufacturer, exact Xiaozhi and UIFlow source trees were investigated. Exact Xiaozhi directories exist for Touch2.8 and Touch1.54 in the retained tree; source-directory presence is not a verified release binary. No Waveshare-named UIFlow board paths were found in the pinned inspected tree. That scoped negative result is not a universal claim of incompatibility.

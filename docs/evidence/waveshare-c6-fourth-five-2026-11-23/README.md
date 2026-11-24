# Fourth five C6 listing groups

## Scope

Five official listing groups, eight distinct sale SKUs, eight products, five board profiles and seven model assets. The two Touch-LCD-1.47 native assets are reused unchanged. New records are integration fragments under `tools/waveshare-batches/c6`; central catalogs and public builds are integrated separately.

| Listing | SKUs | Physical display / touch | Flash | Original complete-image observation |
|---|---|---|---|---|
| ESP32-C6-Touch-AMOLED-2.06 | 32769, 32770 | CO5300 / FT3168 | 16 MiB | 5 seconds, 0 exceptions, 2027 colors, DrawPanel / AIChats home visible |
| ESP32-C6-Touch-AMOLED-1.43 | 31897, 31905 | CO5300 / FT6146 common report subset | 16 MiB | BLE encryption assertion at 4.594 seconds; green/performance overlay, not application acceptance |
| ESP32-C6-Touch-LCD-1.69 | 31538 | ST7789V2 / CST816T | 16 MiB | 5 seconds, 0 exceptions, 16 colors, touch testing prompt |
| ESP32-C6-LCD-1.69 | 31693 | ST7789V2 / no touch | 16 MiB | 5 seconds, 0 exceptions, solid green; I2C/temperature errors retained |
| ESP32-C6-Touch-LCD-1.47 | 31203, 31201 | JD9853 / AXS5106L | 8 MiB | 5 seconds, 0 exceptions, 524 colors, icon UI with unresolved left white strip |

The 1.43 image is an official comprehensive **example trial**: retained source does not explicitly establish shipping recovery purpose. The other four images have explicit manufacturer factory-purpose evidence. No image is patched, replaced with an app-only build, or accepted solely on process exit status. Source/member hashes and image layout checks are in `factory-image-verification.json`; exact observations and framebuffers are retained alongside original logs. No TF card was inserted for these native snapshots. Public Worker/default-card checks are separate.

## Verification

- `cargo test -p esp32c6 waveshare --lib`: 20 passed, 0 failed; exact pins, physical reset wiring, negative bus routing, controller phases, SD contention, no-slot rejection and separately acknowledged expander interrupt paths.
- `cargo clippy -p esp32c6 --lib -- -D warnings`: passed.
- Release `esp32sim-c6` CLI build: passed.
- Actual production model loader: all seven passed with final model hashes, finite geometry/ranges, source hashes, dimensions, UNKNOWN visibility and CPU ray tests.
- Seven four-view plots and two unchanged-source photo-registration plots visually inspected. These are geometry evidence, not WebGL/browser or physical-fit acceptance.
- Current integration JSON schemas: eight products, five firmware entries, eight knowledge records passed. Knowledge linkage validated with the unchanged validator isolated from unrelated stale central records.

## Source and transport boundaries

144 retained original-source receipts cover exact fixed repository revisions, published diagrams, original HTML, selected demo ZIP members and the official component-registry BSP. Raw sources are compressed without modification under `docs/platform/research/waveshare-c6-fourth-five-sources`. Firmware remains remote-only. Source target names are not accepted binary compatibility claims; Xiaozhi and UIFlow investigation uses the shared pinned tree evidence.

LCD-1.69 uses the exact 142,983,431-byte manufacturer ZIP and `Firmware/01_factory.bin`, with independent container and member SHA-256. The `zip-bundle` source requires the integration branch's bounded 256 MiB compressed / 1 GiB declared / 16,384-entry decoder. The official ESP-IDF page explicitly calls `01_factory` the factory default flashed example. There is no invented direct BIN URL.

The ES7210 first run failed under an overly strict 3.3 V-only I2C supply gate; that negative evidence remains in `before-es7210-supply-range`. Official ES7210 Rev2.1 section 6 permits 1.6–3.6 V control supplies, including default ALDO1 1.8 V. The corrected gate has explicit off-range and reset tests; analog sample-rate/gain operation and PCM remain unverified.

AMOLED1.43 touch IRQ is TCA9554 EX0 to GPIO1. EX1 is RTC INT, and the optional reset resistor to EX1 is NC; pulling EX1 low does not reset touch. FT6146 physical identity is retained and unknown private identifiers reject access. AMOLED2.06 has direct separate display/touch reset lines and no TF socket or TCA9554.

The two 1.69 models and 2.06 model use published dimensions and explicit UNKNOWN internals where no exact solid CAD was found. AMOLED1.43 retains 689 native components; case-B has only 2D manufacturer case data and remains an unverified shell. Physical fit, RF/BLE functionality, audio PCM, analog charging/battery, complete interaction, and hardware acceptance are not established.

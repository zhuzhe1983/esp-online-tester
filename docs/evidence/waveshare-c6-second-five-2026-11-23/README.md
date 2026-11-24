# C6/C3 second-five evidence — 2026-11-23

This batch covers five official listings and ten exact SKUs. All runtime statuses remain `partial`. It contains seven mechanical models, five remote factory-image entries, five watchlist entries and ten independently identified knowledge records. Central catalog, browser/Worker and publication checks belong to the root integration.

## Unmodified factory execution

Every original merged image was fetched from a fixed official Git commit and retained outside Git. `factory-image-verification.json` records original byte count/SHA-256, E9 image chip ID, boot/application XOR checksum and appended digest validation, partition-table MD5 and selected flash capacity. The real C3 image chip ID is5; C6 is13. All images load at0 and their partition tables are at0x8000. The C3 factory's copied project-name string mentions S3; it is not used to change the CPU identity.

`native-runs.json` records exact commands, ROM and firmware hashes, timestamps, exit status, original console hashes and final image hashes. Each final run executes5 emulated seconds with the actual chip ROM and0 CPU exceptions. No original firmware bytes were changed, and no project diagnostic image is offered as a factory application.

| Board | Exact capacity | Final RGB colours | Visually observed result |
| --- | --- | --- | --- |
| `waveshare-c3-lcd147` | 4 MiB | 55 | Upright Page01 SD / Flash4 MB / IMU / Gyro interface. The declared physical172-column glass crops4 pixels from each edge of the180-column BSP output. |
| `waveshare-c6-lcd085` | 8 MiB | 2 | Green rectangle and black band; complete factory interaction is unverified. |
| `waveshare-c6-epaper154` | 16 MiB | 2 | Unreadable monochrome stripes. Digital refresh/retained ink work, but factory reset/window compatibility is unresolved. |
| `waveshare-c6-touch-amoled216` | 16 MiB | 4320 | Factory home icons and background, horizontally mirrored under the current BSP-reference presentation policy. |
| `waveshare-c6-touch-amoled164` | 16 MiB | 3323 | Upright Weather / Compass / Gallery / Settings home. |

No SD card was attached in these runs. An absent card or offline network is not interpreted as a nonexistent physical interface. Exit code0 and colourful output do not establish complete application, touch, audio, radio or hardware acceptance.

### C3 execution-path repair

The initial C3 board-facing test reached the phase-aware display endpoint, but the actual C3 SoC bus still dispatched only legacy SPI callbacks. The first factory capture therefore remained blank. The bus now passes the real C3 matrix/native pins and complete SPI phases to `uses_qspi_pins()` boards, and sends received bytes through the actual GDMA IN descriptor chain before transfer completion. This path preserves the legacy SPI callback for existing boards. Exact Espressif v5.5.4 GPIO/IO-MUX/SPI header bytes are included in the source archive.

The corrected path produced the original GUI. The fixed glass reference comes from the manufacturer's `ST7789.c:140` call to `mirror(true,false)`, rather than an image-dependent patch. Guest MADCTL remains effective; the MMIO regression writes asymmetric pixels with both MX values. The pre-dispatch and pre-reference captures are retained under explicit `c3-pre-phase-fix-*` and `c3-before-glass-reference-*` names. Their JSON file references point to the preserved original bytes.

### Sources that do not agree

- C3 published172×320 glass versus current180×320/X30 BSP: visible centre crop/X34 is explicit. The storefront `ST7798` spelling is not treated as a verified new controller.
- C6 AMOLED2.16 actual `main.cpp` uses constructor defaultsCS15/IRQ5, matching the schematic. Unused `user_config.h` CS5/IRQ15 is a source conflict, not a declared new hardware revision. DCDC1 supplies the panel and ALDO3 resets it; ALDO2 is amplifier control.
- C6 AMOLED2.16 current display BSP startsMADCTL30h, while its original factory image writesA0h. The model honours both guest bits and the recorded fixed BSP reference; physical scan calibration is not silently inferred from a pretty screenshot. Touch is an explicitly bounded raw CST9220 report subset, not a verified screen-coordinate calibration.
- C6 AMOLED1.64 overview saysFT6146, while its linked PDF and BSP useFT3168. The model implements only the sourced compatible report and readiness behaviour, without fabricating FT6146 silicon identity or version bytes.
- The e-paper V2 PDF gives R44/R45 power-on defaultsXEnd15h/YEnd127h despite a200×200 panel. The pinned factory partial initialization pulses physical RESET and writes5000 bytes without reprogramming those windows. The current visible stripe failure is retained; no application-specific reset bypass is added. Exact controller die remains UNKNOWN, despite reuse of an SSD16xx-compatible digital command model.

## Meaningful regressions

`rust-gates.json` records exact commands and log hashes. Thirteen Waveshare library tests pass across C3/C6, including the preceding six board regressions. Two new actual-C3-MMIO tests pass: source-correct expander-selected LCD writes, wrong MOSI rejection, effective guest MX, and DMA receive/short-descriptor error delivery. The four existing C3 SPI-DMA tests pass unchanged. Clippy with `-D warnings` passes for both chip libraries and the C3 integration test.

New board tests cover real I2C pins/addresses, distinct8-bitCH32 versusTCA expander protocols, actual reset/power rails, wrong SPI phase/pin and dual-chip-select collision rejection, QSPI lane checks, touch bounds/report acknowledgement/reset availability, retained e-paper optical image, and cardCMD0 responses on physical MISO. They do not claim physical colour, analog voltages, audioPCM, IMU interrupt timing, SHTC3, thermal waveforms or RF acceptance.

## Mechanical evidence

The original C3 STEP retains184 independent native topologies. The M variant adds two nine-pin rows at exact native through-hole centres, while header length, section and thickness remain explicit estimates. The original C6 AMOLED1.64 STEP contains264 topologies; the bare SKU omits only the two native header rows, while the M SKU retains all264. Its21.84×35.57 mm emissive rectangle remains distinct from the22.34×36.06 mm viewing aperture.

LCD0.85, ePaper1.54 and AMOLED2.16 use their exact C6 dimension photographs for their exterior envelopes and visible screen areas. Their hidden PCB is intentionally labelledUNKNOWN and is exposed by the inspection control. Connector/button/rear-label and optical-diffuser details remain reference-only. The C6-linked2.16 structure archive contains only S3-namedDXF/PDF; those bytes are retained without pretending they are a C6 solid assembly.

All seven models passed the actual production Three.js loader's byte hashes, geometry dimensions, framebuffer rays, control visibility/opposite-face blocking and shell-material isolation. Four-view engineering plots were visually inspected and hashed in `model-visual-review.json`. These CPU plots are not WebGL screenshots, decoded photograph registration, manufacturing material validation or physical-fit acceptance.

Native console logs retain exact bytes, including CRLF and trailing spaces. Local Git attributes prevent normalizing these immutable captures. Original source documents are gzip-retained with URL/UTC/length/SHA receipts in `docs/platform/research/waveshare-c6-second-five-sources/`; factory binaries remain remote-only.

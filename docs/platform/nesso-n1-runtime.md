# Arduino Nesso N1 DK001 runtime investigation

The `nesso-n1` board now runs the exact official UIFlow2 v2.5.3 image through the native interpreter and production WASM worker. Support is partial. Sources and actual retrieval receipts are in [sources.json](evidence/nesso-n1/sources.json). Third-party binaries and raw fetched originals are temporary ignored inputs, not published assets.

The Arduino schematic TPX00227 and M5Stack product table identify an ESP32-C6 QFN40, 16 MiB flash, ST7789 135×240 display, FT6336 touch, BMI270 IMU, two PI4IOE5V6408 expanders, BQ27220 fuel gauge and AW32001 charger. M5GFX `65c816a38386062d2c3dc47d41237a9083acbc75` and M5Unified `368ad60807161625aa65baad37a4a3af2a7eccea` supply the matching initialization and button scan. These current sources do not imply that every bundled firmware uses the same revision.

| Route | Source-backed wiring |
| --- | --- |
| Display | SPI SCK20 / MOSI21 / CS17 / DC16; 240×320 GRAM with visible x52..186, y40..279 |
| I²C | SDA10 / SCL8; FT6336 0x38, BMI270 0x68, expanders 0x43 / 0x44 |
| Expander 0 | P0 / P1 active-low keys; P5 LNA, P6 antenna switch, P7 LoRa reset |
| Expander 1 | P0 power-off pulse, P1 LCD reset, P2 Grove power, P5 USB VIN detect, P6 backlight, P7 status LED |
| Identification | G18 hard low, G8/G10 pulled high; do not substitute Unit C6L routing |
| LoRa | SX1262 SPI21/22/20, CS23, BUSY19, IRQ15; not yet modeled |

The official M5Burner project `1897200000000002638` declares sourceType OFFICIAL and exact device `ARDUINO_NESSO_N1`. Its UIFlow2 v2.5.3 image is an alternative official firmware, not a verified factory recovery image. SHA-256 `49c4a9a669a2bde5b95416fa19ab8e01fc6bdec6a81bb80f2a0fc07480e1444b`, 16,773,120 bytes: boot image at 0, partition table at 0x8000 and app at 0x10000. The existing factory audit found only a community factory-code entry; its uploader is not verified as the manufacturer, so it remains excluded from the official factory role.

PI4IOE5V6408 register behavior follows Diodes DS40583 Rev 3-5 (September 2022), retained via the manufacturer-linked M5Stack mirror. It has output=1 direction, separate high-impedance and pull controls, read-cleared reset/interrupt status and no burst-read support. It is not interchangeable with TCA9555.

Open upstream PR review: #262 adds the shared C6 board module and is already the ancestor `e5ac3605`; #310 is a separate C6 optimization, not adopted. No open PR found for the Nesso board or PI4IOE5V6408 implementation at this checkpoint. RF, charging, Grove electrical routing and physical-hardware validation remain outside this acceptance.


## Implementation and observed behavior

The QFN40 package and external-flash eFuse fields are selected only when attaching `nesso-n1`; other C6 profiles retain their previous identity. This is a board configuration, not a measured silicon-revision dump. The source header is IDF v5.5.4, matching the official binary's `v5.5.4-dirty` string. The read-only C6 GDMA FIFO status now reports up to 32 available bytes from a validated current SPI descriptor; it does not consume bytes or change descriptor ownership. SPI phases retain `buf_pos` until the descriptor is exhausted, then apply writeback/EOF. Malformed lengths, owner checks, unmapped buffers, zero-progress cycles, stop/reset and other peripheral selection have regression coverage. This is untimed readiness, not cycle-accurate prefetch or a FIFO occupancy model; zero-length head-descriptor prefetch is not implemented.

PI4IOE5V6408 uses its own register contract, not a TCA9555 alias. Display traffic is gated by SCK20/MOSI21/CS17 and the expander's reset output; backlight and high-Z states are respected. FT6336 single-contact input reaches 0x38 and shared IRQ3; the IMU register subset is available to the guest but the web pose API is not connected. No private SDK routine or firmware address is patched.

The official launcher shows CLOUD after boot. A short side B press selects USB; the green front A press opens USB READY at 115200. Both the physical 3D regions and keyboard A/B were exercised in the workbench. The launcher itself has no touch handler. In its unmodified MicroPython REPL, injected raw (42,73) reads as FT6336 `[1,128,42,0,73]`; M5's calibration returns (42,72). Its touch-detail state includes release/none transitions for several updates before `getCount()` returns zero; the harness preserves these semantics. Test commands are sent over the firmware's USB console, not used to fake button or display output.

UIFlow2 is still an official alternative with `simulationVerified:false` / trial. It is not promoted to factory firmware or full compatibility. Fuel gauge/charger are absent, leaving the real -1% battery indicator; initial I²C and PLL calibration diagnostics remain visible. LoRa RF, Wi-Fi networking, buzzer/audio, physical power key and web-driven IMU pose are not verified.

## Model and reproducibility

The existing manufacturer triangles, label texture and source photographs are unchanged. Front CAD component 5 carries `btnA`. Side KEY2 uses the original component 13 exterior flipper faces 1049..1058, separated from the shell for hit testing and an independent black material. The official side photograph and Arduino pinout page 2 identify this +X face, distinct from the power key on the opposite face. Sorted triangle-position/normal bytes are identical before and after separation; only record order/semantic grouping changes. The existing detached antenna, photo-derived glass/paper and UNKNOWN internal PCB limitations remain. The first metadata-only rebuild reproduced all seven original artifacts exactly. After the side-key material correction, the intentional reordered geometry has an exact triangle multiset match and the final offline plan rebuild matches the new assets; no calibration claim is added for switch travel or force.

Create an ignored `output/` directory first. Run `cargo run --release -p esp32c6 --example nesso_probe -- <official-bin> <c6-rom>` for the 9-second native input probe; its temporary PPM files are written under `output/`. Run `node tools/probe-nesso-official.mjs <official-https-url> <sha256>` for actual production worker boot, buttons, framebuffer and official REPL touch checks. No third-party firmware bytes are committed or published. Curated hashes, frames, source receipts and final checks are retained in `docs/platform/evidence/nesso-n1/`.

Negative setup/probe results: missing P4 CI fixture inputs initially caused ten `cosim` failures; pinned `tools/fetch-p4-firmware.sh` inputs fixed the environment and the full suite was rerun. Initial inspection relied on nonexistent MicroPython `Touch.isEnabled` / detail-object fields; the actual firmware uses tuple details and `getX/getY`. A one-pixel calibration difference and multi-update release state were confirmed rather than hidden. Browser recording initially rejected existing mixed-case `btnA`/`btnB` IDs; its button-ID validation now preserves case and still rejects invalid characters/extra fields.


## Final acceptance

Stable Rust 1.99.1: both warnings-denied Clippy commands passed, as did all 1,445 Rust tests across 132 groups (ignored included; only `external_*` skipped). Full CI commands are retained in [checks.json](evidence/nesso-n1/checks.json): 11 production WASM scenes, 100 platform checks, schema checks, VQ, JIT/profile and historical evidence checks. Final recording regression delivered 18 real guest events with identical final pixels; the Nesso workbench's own four keyboard events also replayed to an identical READY frame, with observed lateness retained and no bit-exact scheduling claim. No goldens changed.

The final mini player loaded the official remote image after trial confirmation. Its actual source-CAD side button selected USB, and its original front green button entered READY at 115200. Gold and purple transparent shells retained the separate black side-key surface and front green key. [Mini evidence](evidence/nesso-n1/browser-mini.json) includes screenshots and the actual 135×240 frame hash. A supplementary mini mouse-to-REPL touch check did not complete: an invalid test appearance reset left the guest paused and the supplementary check was stopped. It is not counted as touch-mouse acceptance; the production-worker FT6336 and official REPL checks remain the stated touch evidence.

Five alternating native hello pairs against fixed main `84e67571f0d1aecddf77d2432678fec81baabf72` preserved stdout, instruction counts and cycle counts. Median user CPU differences were S3 −4.000%, C3 +1.739%, C6 +1.274%; these cross-revision, hundredth-second measurements are a regression guard, not an attribution to this board or an optimization claim. No S3 core code changed, so Pocket was not required. [Raw samples and artifact hashes](evidence/nesso-n1/cpu.json), plus [the reusable helper](evidence/nesso-n1/cpu.py), retain the comparison.

Task-owned browser and local test service were closed after acceptance. Evidence excludes remote firmware bytes, raw private browsing data and unrelated process information. Screenshots contain only the isolated public-product simulator scene.

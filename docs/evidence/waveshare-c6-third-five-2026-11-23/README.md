# C6 third five listing groups

This batch contains five official listing groups, eight distinct sale SKUs, eleven revision-qualified product entries, seven board runtime profiles and six model assets. A sale SKU repeated for V1/V2 is not counted as another independent sale SKU. All runtime and model acceptance remains partial.

| Listing | Exact SKUs | Runtime distinction |
| --- | --- | --- |
| ESP32-C6-GEEK | 33675 | V1 separate SD bus; V2 SD bus shares LCD clock/MOSI |
| ESP32-C6-LCD-1.3 | 33643 / 33644 | Bare / presoldered headers; real ESP32-C6FH4, 4 MiB |
| ESP32-C6-Touch-AMOLED-1.8 | 33305 / 33366 | Battery included / excluded, each with explicit V1 SH8601+FT-compatible versus V2 CO5300+CST820 |
| ESP32-C6-Touch-LCD-1.83 | 32793 / 32794 | Battery included / excluded; ST7789P, shared LCD/touch reset, SDA7/SCL8 |
| ESP32-C6-Touch-AMOLED-1.32 | 32766 | Round CO5300+CST820; no TF, IMU, RTC or AXP2101 |

## Exact original factory observations

Original images were downloaded from pinned official repositories or the precise manufacturer ZIP member, with SHA-256, ESP image checksums/digests, C6 chip ID13, partition-table MD5, and flash bounds checked. The real C6 ROM was used. No private initialization bypass or firmware patch was inserted. `factory-image-verification.json`, `native-runs.json`, `native-stop-summaries.json` and original UART logs retain the distinction between layout verification, execution, and full application acceptance.

| Board | Emulated stop | Exceptions | Final colors | Observed result |
| --- | --- | --- | --- | --- |
| GEEK V2 | 5.000 s | 0 | 2541 | Real colorful output, but rotated/cropped; window/orientation unresolved |
| LCD1.3 | 3.221 s, software reset | 1 | 1 | BLE assertion; backlight off; full app not accepted |
| AMOLED1.8 V1 | 5.000 s | 0 | 1464 | Original home screen visible; interaction/audio/radio not accepted |
| AMOLED1.8 V2, old V1 image | 0.591 s, software reset | 1 | 2 | Old touch0x38 initialization fails; no V2 default assigned |
| TouchLCD1.83 | 5.000 s | 0 | 1589 | Original home screen visible; interaction/audio/radio not accepted |
| AMOLED1.32 | 5.000 s | 0 | 1 | 868624 digital pixels written but final snapshot single color; no home-screen claim |

GEEK V1 has source-backed digital routing tests but no separate original factory execution acceptance. Native SD runs used no card; card protocol is tested separately. Original pre-fix GEEK/LCD1.3 logs and frames remain with hashes in `prior-observations.json`. The GEEK correction accepts actual Arduino full-duplex MOSI writes while leaving unconnected MISO floating; it does not inject a display frame.

LCD1.3 factory is member `ESP32-C6-LCD-1.3_Demo/Firmwork/ESP32C6-1.3inch-Test.bin` of the official 97520767-byte ZIP (6092 entries, 356001148 total declared uncompressed bytes). Exact member length is2005152 bytes. Current single-BIN ZIP handling does not support this container; this batch retains it in the watchlist/knowledge and does not invent a direct binary URL. A separately reviewed bounded member reader can later attach the exact trial source.

## Verification performed

- Sixteen `cargo test -p esp32c6 waveshare --lib` tests pass. This includes wrong routing, distinct GEEK SD revisions, real LCD13 socket, missing-device NACKs, shared Touch183 reset, AM18 independent reset/power and separate SD bus, raw touch bounds, exact QSPI command phases, and valid full-duplex write/floating MISO.
- `cargo clippy -p esp32c6 --lib -- -D warnings` passes. Native release CLI build completed and the six above runs completed without host timeout; application assertions are still failures as recorded.
- Product, firmware, knowledge and watchlist schemas pass for all11 products. Original and gzip hashes of108 curated electrical sources/firmware receipts pass.
- The production Three.js model loader loads all6 models, checks exact dimensions, CPU rays, actual declared BOOT surfaces on LCD1.3, opposite-side occlusion, shell isolation and UNKNOWN interior visibility. This is a CPU/geometry check, not WebGL rendering or calibrated physical touch acceptance.
- All6 generated front/reverse/edge/oblique geometry plots were visually reviewed. Original manufacturer photographs were also inspected unchanged. No registered-photo overlay is claimed for these plans; exterior placeholder models omit unsourced openings/keys rather than pretending they are measured CAD.

## Mechanical and source boundaries

LCD1.3 retains all193 manufacturer STEP components and native colors; the presoldered variant adds14 illustrative header parts at the12 native hole centers, with section and length explicitly unknown. AMOLED1.32 retains all408 native components and distinguishes33.55 mm emissive,33.75 mm viewing,34.3 mm trim and36.4 mm cover diameters. Rigid transforms preserve native millimetres without deformation. Source archive identity and extracted member bytes are independently checked during the model build.

GEEK, AMOLED1.8 and TouchLCD1.83 have no matching solid assembly in their recorded complete pinned repository/resource search. They use published exterior dimensions and unchanged source photos, with an inspectable UNKNOWN internal board. GEEK dimensions versus resolution have a6.06% aspect conflict; LCD1.83 rounded physical dimensions differ by0.23%. The checker records these as documented source conflicts and `calibrationVerified:false`, not square-pixel or calibrated active-area acceptance. V1 AMOLED1.8 photos do not prove V2 mechanical fit.

The SH8601 implementation is a separately committed, source-named manufacturer-BSP RGB565 subset. Its QSPI RGB565 practice conflicts with the SH8601A PDF's24-bit-only QSPI statement; this remains explicit. CO5300 and SH8601 identities, reset defaults and accepted commands remain distinct. Private identifiers/OTP, analogue rails, optical gamma/TE/settling, battery circuits and RF/audio transfer are unverified.

Integration fragments are `tools/waveshare-batches/c6/<listing>.json`. They do not modify shared public catalogues. Full source bodies are in `docs/platform/research/waveshare-c6-third-five-sources/`; firmware binary bodies remain remote-only. Shared pinned Xiaozhi/UIFlow investigations are referenced per product; a source directory is not an accepted firmware image.

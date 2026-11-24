# Final five C6/C3/H2 listing families

This group closes the remaining five listing families in the assigned C6/C3/H2 catalogue scope. It contains 12 exact sale SKUs/products, 6 board profiles, eight newly built mechanical assets and two unchanged H2-Zero assets from the earlier source-only group. Runtime support remains explicitly partial. The independent H2 core and its image/clock/native evidence were committed separately as `5972cabf`.

| Listing family | Exact sale SKUs | Runtime profile and capacity | Mechanical evidence |
| --- | --- | --- | --- |
| ESP32-C6-Pico | 26866, 26845 | `waveshare-c6-pico`, 4 MiB/0 PSRAM | Published 21×57.3 mm XY; optional headers. Unmeasured structure visibly UNKNOWN |
| ESP32-C6-DEV-KIT | 25723, 25563 (N8); 30951, 30547 (N16) | `waveshare-c6-dev-kit-n8`, 8 MiB; `waveshare-c6-dev-kit-n16`, 16 MiB; external flash, 0 PSRAM | Original N8 217-component STEP retained unscaled; optional headers use native hole centres. N16 mechanical revision matching remains unverified |
| ESP32-C3-Zero | 25452, 25532 | `waveshare-c3-zero`, true C3FH4/4 MiB/0 PSRAM, RGB GPIO10 | Published 18×23.5 mm XY; optional headers. Unmeasured structure visibly UNKNOWN |
| ESP32-H2-Zero | 32261, 32260 | `waveshare-h2-zero`, true H2/4 MiB/0 PSRAM, independent SRAM subset | Existing source/photo model unchanged; published 18×23.5 mm XY and UNKNOWN layout/height |
| ESP32-H2-DEV-KIT-N4 | 26507, 26508 | `waveshare-h2-dev-kit-n4`, true H2/4 MiB/0 PSRAM, independent SRAM subset | Published 25.4×51.6 mm XY and 22.86 mm header row spacing; pitch/longitudinal placement explicitly photo-estimated |

Exact options, source URLs, original byte receipts, electrical identities and limitations are in the five per-listing fragments under `tools/waveshare-batches/c6/`. The 12 matching knowledge JSON files are included, not merely referenced by an index. Current category Position is only a listing-order proxy; firmware, schematic and CAD timestamps are not promoted to proven hardware release dates.

## Electrical decisions and real checks

C6 Pico's TCA9554PWR is at address `0x20`, on physical SDA22/SCL23. Its eight port bits are external expander outputs, not fake MCU GPIO aliases. Its INT is not connected to an MCU GPIO in the retained schematic. Tests exercise output/direction/input registers, pin binding and the absence of fabricated IRQ/MCU aliases. One schematic section title says H2, while the actual U2 part and current overview identify C6-MINI-1/4 MiB; the actual part/overview are used. The retained Espressif module datasheet describes FH4/FH8 variants, and the exact board's 4 MiB profile selects FH4.

C6 Dev Kit N8/N16 use WROOM-1 external flash identities. The runtime does not mistake the 8 MiB option for an FH8 integrated-flash die. Both capacities and package/eFuse boundaries are tested. The real C6 SRAM guest also executes through each new Pico/N8/N16 board factory and produces UART, BOOT and GPIO8 RMT output; the same program routed to an unrelated pad produces no LED update.

C3 Zero independently executes a real RV32 SRAM guest on the C3 bus, with RGB on GPIO10 and BOOT on GPIO9. GPIO8 does not update its RGB. Its current official related Basic Kit repository contains merged binaries under an S3 build target. The retained, downloaded representative is 4,194,304 bytes, SHA-256 `c71ff7820bb6c3631eaecc512d706a6664b73a2ee707093ce9aedcb544479df7`, with chip ID9 (S3), not C3 chip ID5. It is not assigned as C3 runnable firmware. See the exact source tree and `esp32-c3-zero-binary-identity-check.json` under the retained research directory.

Both H2 models now point to the independent H2 board implementation. They do not reuse C6 identity, ROM or MMIO. The separate [H2 native evidence](../waveshare-h2-sram-2026-11-23/README.md) documents actual SRAM/UART/BOOT/RMT execution, 29 fixed primary-source receipts, all 18 unchanged original example images and the real loader rejections. H2 full original apps, ROM/flash cache, radio and interrupt routing remain unsupported; the original image catalogue entry stays `reference` with `source:null`.

## Model review

`model-loader-check.json` uses the production Three.js loader, actual mesh buffers, original image hashes, native CAD bounds and constructed UNKNOWN labels. Its eight `modelSha256` values match the final model JSON files. Every four-view PNG in `model-review/` was opened and inspected after construction. The native Dev Kit shows the original USB shell, shield, board holes and full assembly; the optional header bodies/pin heights are separately labelled estimates. Photo-based boards show UNKNOWN on both PCB faces and an explicitly estimated open USB envelope; no unmeasured internal component placement is fabricated.

Native Dev Kit PCB bounds are approximately 25.3807×55.0207×1.6 mm versus the official drawing's rounded 25.4×55×1.6 mm. The source geometry is preserved without stretching. N16 product records explicitly reuse N8 geometry subject to an unverified mechanical/electrical revision match. Pico's drawing annotation 1.6 describes an XY edge offset, so it was not misread as a published PCB thickness. H2 Dev Kit's dimension image is the actual `Diagram_4` asset, while other similarly named images are a pinout/block diagram; its incorrect H2-Zero photo alt text is recorded as a source conflict.

These are source/geometry and CPU-ray checks, not WebGL rendering, browser pointer acceptance, physical fit or calibration. Public Worker/browser acceptance requires separate frontend checks. No unsourced 3D button/LED positions are registered on the UNKNOWN boards.

## Verification receipts

- `native-tests.log`: 28 C6 and 6 C3 meaningful board regressions passed, including all new basic profiles.
- `clippy.log`: C6/C3 library clippy with `-D warnings` passed.
- The separate H2 directory records eight native tests and all-target clippy passing, with two exact native Machine executions.
- `fragment-validation.json`: five listing fragments, 12 exact products and actual knowledge files pass current schemas; model/source paths and final model hashes match.
- `knowledge-validation.json`: all 12 records pass the production knowledge validator, including receipt timestamps and source references.
- `visual-review.json`: individually opened final geometry views, with SHA-256 and explicit scope.

The source pipelines are `prepare_final_basic_models.py --listing remaining`, `export_final_basic.py --listing remaining`, then `qualify_h2_runtime.py`. H2 qualification is applied only after its independent native evidence exists. No matching complete firmware was found for the final C6 basic boards in the bounded retained source scope; linked BLEAssist packages are Android APKs, not ESP firmware. The absence of a matching image is not presented as proof that no image exists anywhere.

# Waveshare 5-inch, 4.3B and 4-inch V4: exact profiles and bounded evidence

This directory covers three storefront families, eight sale SKUs and seven mechanical models. Product identity, manufacturer source evidence, digital behavior, mechanical approximation and physical validation are separate. All original images remain **trial**, `simulationVerified: false` and `hardwareValidated: false`.

## Identity and version boundaries

| Sale SKU | Catalog identity | Runtime selector | Mechanical qualification |
| --- | --- | --- | --- |
| 28117 | Touch-LCD-5, 800×480 | `waveshare-s3-touch-lcd5` | Native touch assembly, unversioned manufacturer CAD |
| 28151 | Touch-LCD-5B, 1024×600 | `waveshare-s3-touch-lcd5-1024` | Explicit resolution derivative; hidden panel BOM correspondence unverified |
| 30321 | LCD-5, 800×480, non-touch | `waveshare-s3-lcd5` | Remove only source CTP/glass/FPC component 0; remaining BOM correspondence unverified |
| 30320 | LCD-5B, 1024×600, non-touch | `waveshare-s3-lcd5-1024` | Same qualified non-touch derivative with its actual digital resolution |
| 27848 | Touch-LCD-4.3B | `waveshare-s3-touch-lcd43b` | Native bare-board assembly |
| 28141 | Touch-LCD-4.3B BOX | `waveshare-s3-touch-lcd43b` | Explicit bare-board reference; the case and its keycaps are not modeled |
| 28154 | Touch-LCD-4 V4.0 | `waveshare-s3-touch-lcd4-v4` | Historical native mechanical assembly does not prove the current V4 component layout |
| 30871 | LCD-4 V4.0, non-touch | `waveshare-s3-lcd4-v4` | Historical optical derivative; current external dimensions differ and physical fit is unqualified |

All eight profiles use 16 MiB Flash and 8 MiB PSRAM. Sales SKU, schematic revision and native CAD revision are not interchangeable. Manufacturer web text for the 5-inch family repeats 4.3B-like dimensions; the 5-inch dimension drawings and native geometry are retained with that conflict. No product launch date is inferred from firmware dates, repository commits or SKU numbers.

## CH422G digital behavior and actual CPU tests

The 5-inch and 4.3B boards follow their own schematics. CH422G uses its documented command/address interfaces rather than a fabricated byte-register expander. EX1 resets GT911, EX2 controls backlight, EX3 resets the LCD through **populated R20 = 100 Ω**, and active-low EX4 selects the SPI TF card. Earlier 4.3/7-inch boards retain their separate reset wiring qualification. PCF85063A at 0x51 advances from guest cycles on these new profiles; its presence is not silently added to the older variants.

The RGB data pads, in logical bit order, are `[14,38,18,17,10,39,0,45,48,47,21,1,2,42,41,40]`, with PCLK 7, HSYNC 46, VSYNC 3 and DE 5. I²C uses SDA 8 / SCL 9. SPI TF uses CLK 12 / MOSI 11 / MISO 13 and CH422 EX4. The implementation derives LCD routing from the **actual GPIO matrix callback** used by the S3 bus; implementing an otherwise uncalled board hook alone would not deliver frames.

`esp32s3/tests/waveshare_large_rgb.rs` executes real LX7 MMIO instructions and verifies four independent behaviors over all five selectors:

1. LCD_CAM/GDMA pixel transfer, both resolutions, first/middle/final pixel values, incorrect GPIO routing, backlight, populated EX3 reset and clearing of stale pixels.
2. GT911 coordinate limits, far-corner contacts, reset/address strapping, wrong I²C routes and controller absence on non-touch hardware.
3. PCF85063 leap-day calendar advancement and STOP behavior using guest time, plus absence on legacy board variants.
4. Actual SPI TF command transport, a 512-byte write/read, incorrect MISO routing, ejection and rejection of unsupported native SDMMC.

The host loads diagnostic programs and test data, but never paints the expected framebuffer. Large frames use linked DMA descriptors over a bounded reusable data block, within actual guest memory. Five previous tail-board CPU integration tests also pass. These counts are separate from unit tests and from manufacturer applications. Relevant test logs, strict Clippy output, source hashes and commands are retained here.

The 4-inch V4 runtime is an independent CH32V003 implementation. Its factory execution and board-level tests are recorded separately in `docs/evidence/waveshare-touch4-v4-runtime-2026-11-24`; the five runs below do not claim coverage of V4. Its source metadata distinguishes CH32 EX2 GT911 IRQ from GPIO16 **SYS_OUT**, and its only exposed ordinary button is BOOT. V4 code must not reuse the old TCA9554 electrical map.

## Unmodified manufacturer applications

Each run uses the original merged image, the pinned S3 ROM, 16 MiB Flash, 8 MiB PSRAM and a prepared 64 MiB FAT32 image. The stop condition is a **5,000 guest-ms / 1,200,000,000-cycle budget**, not successful completion of an application. Inputs, source archive/member receipts, runner hash, UART, board report, actual PNG framebuffer and compressed RGB565 bytes are retained.

| Selector | Observed frames | Last-frame colors | Reboots | TF commands | Interpretation |
| --- | ---: | ---: | ---: | ---: | --- |
| `waveshare-s3-touch-lcd5` | 193 | 2,053 | 0 | 0 | Actual 800×480 LVGL widget output |
| `waveshare-s3-touch-lcd5-1024` | 123 | 2,053 | 0 | 0 | Actual 1024×600 LVGL widget output |
| `waveshare-s3-touch-lcd43b` | 193 | 2,053 | 0 | 0 | Actual 800×480 LVGL widget output |
| `waveshare-s3-lcd5` | 0 | 1 | 0 | 0 | Black framebuffer; backlight/reset not enabled in the observed window |
| `waveshare-s3-lcd5-1024` | 0 | 1 | 0 | 0 | Black framebuffer; no full application acceptance |

The black outcomes are preserved. Neither forcing an enable pin nor bypassing LCD reset is used to manufacture a pass. The colored runs do not prove the TF filesystem, touch-driven widget navigation, CAN, RS485, wireless or full application lifecycle. Their UI data is the manufacturer's public LVGL demo fixture, not a user's personal data.

After those archive-member runs, the manufacturer repositories were inspected at fixed commits: `77252e5ebd2f1ff9b06e22d8693903c960652669` for the 5-inch family, `9e1e8e301af49825c53a3e31cdd446d66467d1eb` for 4.3B, and `390aaba10e80f1ef08506cb8d7db5e41cea09273` for 4-inch. The five corresponding current BIN files are **byte-identical** to the executed archive members; `pinned-original-byte-identity.json` binds both sources and hashes. This equivalence is not a second execution.

The 5-inch/4.3B README calls these integrated LVGL examples. Although filenames contain `FactoryOnly`, the catalog conservatively retains `role: example`. The exact 4-inch README explicitly identifies factory recovery; its touch/non-touch files receive `role: factory` and are default-first. Every image stays remote-only. Official UIFlow and Xiaozhi investigations did not establish an exact mapping for these particular models; 4B and 4.3C targets are not substituted.

## Geometry, optical mapping and controls

The manufacturer native assemblies retain millimetres, native transforms, solids, open surfaces and component coverage. The 5-inch source has 736 components, 4.3B has 698 and the 4-inch source has 709. Qualified non-touch derivatives omit only the explicitly documented optical component 0. Original CAD, drawing/photo receipts, rights notes and reproducible model plans live with each model.

The physical drawings identify visible optical windows, not independently calibrated active pixel areas. Live image planes preserve square digital pixels inside those windows: 109×65.4 mm for 800×480 5-inch, 109×63.8671875 mm for 1024×600, 90.6×54.36 mm for 4.3B and 70.18 mm square for 4-inch. The 4-inch center is `[0,-1.66]` in retained native XY coordinates. No manufacturer photo overlays the live framebuffer.

All seven final production Three.js models pass source-hash, dimension, screen-ray, visible BOOT and opposite-face occlusion checks. Their final four-view images and source registration overlays have been inspected. Three representative optical assemblies were additionally rendered expanded, checked for separation and exact restoration; the live screen moves with the optical group. There is no invented external case to recolor. BOOT verification uses the visible rear component face; its physical travel is not calibrated.

Source photos are construction/registration evidence, not independent metrology. Hidden BOM identity, connector mating depth, unversioned PCB revision correspondence and physical fit remain unverified. The non-touch 4-inch derivative is especially limited: its current stated 76.54×74.66 mm envelope differs from the historical inner LCD geometry. The BOX entry does not pretend its missing shell is present.

`acceptance.json` and `model-metadata-history.json` bind the final model hashes. A metadata-only correction made optical provenance and the control verification viewpoint explicit; unchanged geometry was not retessellated, and final loader/render checks were rerun. This is Node/CPU geometry evidence. Browser WebGL rendering, DOM interaction, iframe behavior and physical hardware have not been accepted in this session.

## Reproduction

Run from the repository root with the documented Rust and catalog dependencies:

```sh
CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test -p esp32s3 --test waveshare_large_rgb
CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test -p esp32s3 --test waveshare_tail_guest
cargo clippy -p esp32s3 --all-targets -- -D warnings
python tools/validate-platform-catalog.py
node --test tools/platform-knowledge-test.mjs tools/platform-watchlist-test.mjs tools/platform-manual-watchlist-test.mjs
python tools/waveshare-batches/root-s3-five/verify_large_models.py --output /tmp/waveshare-large-review
python tools/waveshare-batches/root-s3-five/verify_large_models.py --output /tmp/waveshare-large-expanded --expanded --models waveshare-esp32-s3-touch-lcd-5 waveshare-esp32-s3-touch-lcd-4-3b waveshare-esp32-s3-touch-lcd-4
```

Use `tools/rebuild-product-model.py plan <model-id>` and the repository model reproduction guide for retained-source rebuilding into an independent output directory. Manufacturer firmware is downloaded from its pinned catalog source and checked against its SHA-256; it is not included in this repository. Invoke `waveshare_source_attempt` with the exact selector, ROM, verified merged BIN, `16 8 5000`, output directory and optional prepared card path. The recorded image audits contain the chip and partition-layout checks.

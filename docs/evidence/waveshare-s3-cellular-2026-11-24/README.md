# Waveshare A7670E / SIM7670G: V2 digital subset and original-image observations

This contribution covers two exact sale identities and their current V2 electrical profiles. It does not emulate the cellular modem, radio network, USB modem, GNSS receiver, camera, battery gauge or analog supply. No hardware or browser WebGL acceptance is claimed.

| Exact part / sale SKU | Runtime board | Capacity | Default original source |
| --- | --- | --- | --- |
| ESP32-S3-A7670E-4G-EN / 26749 | `waveshare-s3-a7670e-4g-v2` | 16 MiB Flash, 8 MiB PSRAM | A7670E USB 4G CDC official example |
| ESP32-S3-SIM7670G-4G-EN / 26750 | `waveshare-s3-sim7670g-4g-v2` | 16 MiB Flash, 8 MiB PSRAM | SIM7670G USB 4G CDC 240422 official example |

The source pages distinguish the V2 examples as “After December 2025.” This identifies the documented resource revision; it does not establish the SKU release date. Old V1 source and schematic receipts remain available. No selectable V1 profile is inferred from that historical material.

## Electrical behavior and limits

The independent board module implements the single WS2812B at GPIO38, with the manufacturer GRB order and 800 kbit/s data framing. Software GPIO timing and the RMT path feed the same chain. BOOT is active-low GPIO0. The one-bit SDMMC slot uses CLK5, CMD4 and D0=6; it is provisioned with the platform's default 64 MiB FAT32 image. User card data is not replaced.

V1 battery-gauge wiring is SDA3/SCL2, while V2 uses SDA15/SCL16. Camera data GPIO7..14 is unchanged, but V1 XCLK34/VSYNC36/HREF35/PCLK37 becomes V2 XCLK39/VSYNC42/HREF41/PCLK46. MAX17048 protocol/measurements, camera frames, modem UART AT responses, USB CDC/PPP, radio and GNSS results are deliberately absent from this digital subset. The application cannot obtain a fabricated successful modem or battery response.

The old SD example calls GPIO46 `SD_CD_PIN`; V1 schematic labels the TF DAT3/CD contact, not an independent insertion switch. V2's SD_CS net is separate and its example removes that macro. GPIO46 is the V2 camera clock input. Consequently the board does not manufacture a GPIO46 insertion signal. It rejects unwired four-bit SD mode.

The V1 battery example's VCELL register read and multiplied value do not substantiate its percentage label. This source discrepancy is retained in knowledge rather than used as a state-of-charge model. The product's physical DIP and power switches are not ordinary GPIO pushbuttons; no animated or digital substitute for their analog supply effects is presented.

## Actual LX7 instruction checks

`esp32s3/tests/waveshare_cellular_guest.rs` contains **two actual LX7 tests**, each run for both exact profiles. S32I and L32I execute at real guest instruction addresses. Every register operation asserts PC advancement, avoiding an unnoticed exception that could make an apparent MMIO check pass.

1. Actual RMT register programming and GPIO matrix routing deliver GRB data on GPIO38. GPIO37 produces no LED update. A new machine starts with cleared chain state. No display is invented for these screenless boards.
2. Actual SDMMC/IDMAC register programming transfers a 512-byte pattern to a guest-visible memory card and reads it back. All 128 words and both byte counters are checked. Wrong CMD/D0 routing, unwired four-bit mode and ejection produce timeout results. The descriptor and data buffers use the chip's declared DRAM base, not an assumed address.

One additional unit test checks the exact chain order, card pin map/presence, no display, no fabricated I2C devices, BOOT identity, absence of a GPIO power alias, and rejected camera injection. These tests are simulator contracts, not observations from a physical board or proof of full official application compatibility.

The targeted unit test, both actual LX7 tests and targeted Clippy with warnings denied passed. Catalog schema validation, 13 catalog/selection/knowledge tests, and the four repository-mandated platform test files passed. The generic worker test reused the existing compiled platform WASM and ROM/demo fixtures; it does **not** establish a browser or WASM execution claim for these new 4G boards. Their original-image observations below use the newly built native runner.

The first attempt at the generic platform gate reported missing local fixture/ROM files. Linking the existing build artifacts into this isolated checkout resolved that setup issue; no test assertions or source behavior were weakened. No fixture binaries or links are committed.

## Original official example execution

The manufacturer Firmware pages explicitly describe precompiled examples, and warn not to interchange modem models. Neither these pages nor the inspected source archives identify an exact factory recovery image. Both defaults therefore remain `role: example`, `status: trial`, `simulationVerified: false`. A partition named `factory` is a boot-layout label, not evidence that an image is the manufacturer's shipping/recovery firmware.

The two exact original ZIP members pass ESP32-S3 image checksum/digest, partition-table MD5, app-chip and Flash-capacity checks. Complete original bytes are fetched for the observation but remain remote-only in the public catalog.

| Image | Original SHA-256 | Build observation |
| --- | --- | --- |
| A7670E USB 4G CDC | `1437291c3dfa9f3afa96987b97c8ba7af1f0952e7d0d87b6d407e130f97b2073` | ESP-IDF v5.1-dirty, application compiled November 2023 |
| SIM7670G USB 4G CDC 240422 | `d95d5742ddd43f2f7994ff4ef3c5fe70d5f9669738597944db4fdbfcbb82c459` | ESP-IDF v5.1.3-dirty, application compiled April 2024 |

Each exact image runs through the real ROM/bootloader on its 16 MiB Flash / 8 MiB PSRAM profile with a 64 MiB FAT32 card, bounded to 1,200,000,000 cycles (5,000 guest milliseconds). Both reach `usb_cdc_4g_module` application startup and a modem-initialization wait. Both retain zero reboots, zero SD commands, zero SD bytes transferred and zero RGB updates within this bound. The runner's stop label is `Halted` at the bound; it is not successful completion of a network workload.

Both original images have a 4 MiB Flash header. Their logs detect the configured 16 MiB and select the smaller image-header range. Neither image nor actual board capacity is edited to hide that discrepancy. The examples also log modem reset on GPIO13; that observation is not promoted into a V2 hardware reset connection, where GPIO13 is also camera data. The exact-model document link allows a clearly labeled trial, not a claim of V2 full compatibility.

Native receipts retain ROM, runner, image, initial card and artifact hashes. Original UART bytes are kept as `uart.original.gz`; the adjacent text normalizes CRLF/CR and trailing whitespace only. Interleaved/duplicated messages from the execution are preserved. No original binary, network credential or personal card content is published.

## Manufacturer geometry and visual review

Both exact resource pages link the same PCBA STEP archive. The STEP header is dated 2024-03-08, before V2. It contains **738 solids and four open shells**, all explicitly enumerated. Source component 4 is a modeled 18650 battery. Both exact storefronts exclude that battery from the sale kit, so only that component is omitted from display; the original source bytes remain unchanged. The other **741 native components**, including the battery holder, camera and all four open shells, remain at their original assembly coordinates and dimensions.

The sole assembly transform is the proper rigid map `(X,Y,Z) → (Y−55,16.14−X,Z)`. It changes presentation orientation only. The 110.0244 × 30.4644 mm native board outline is checked against the published 110 × 30.44 mm PCB, within 0.05 mm. The native 1.59841946 mm thickness is a CAD measurement, not a number printed on the photograph. The complete kit's 125 × 31.6 × 34 mm envelope is recorded separately and never used to rescale the PCB.

The selectable names and summaries explicitly state that this is a PCBA reference and the acrylic support is not modeled. The exact case, screws/standoffs, antennas, cables and current V2 placement are not reconstructed from unverified dimensions. The native camera is retained as a camera reference; the kit's OV5640 component listing does not prove the identity of that CAD housing or sensor. The A7670E kit also lists a speaker; the SIM7670G kit does not inherit it.

Native material colors are preserved after linear-to-sRGB conversion. Missing source colors use explicitly illustrative photo-informed materials; no silk, modem model label or hidden assembly is invented. The native RGB body is associated with chain index zero, but current V2 physical placement is not calibrated. The initial noncanonical metadata key was corrected to the production loader's `ledIndex`, with unchanged geometry hashes recorded; the public preparation tool now emits the canonical key directly.

Both final model hashes pass the actual production Three.js loader, finite geometry and dimension checks, independent LED-index wiring, camera inspection separation/reset and material isolation. Their real loaded meshes are reviewed from front, reverse, edge and oblique views. Each exact original dimension photograph has an independently registered PCB outline overlay. This validates the stated native-reference scope; it does not validate hidden heights, current V2 assembly, connector mating, acrylic fit, browser WebGL materials or 3D touch/button interaction. No unsupported 3D actuator or cable anchor is exposed.

The temporary large mesh dumps are hashed and deleted after review. `model-dump-receipts.json`, final model hashes, source manifests and the public build/review tools retain reproducibility without retaining a second bulky geometry representation.

## Reproduction and evidence

Source receipts and selected original V1/V2 schematics/examples/docs are in `docs/platform/research/waveshare-s3-cellular-sources/manifest.json`. Original model sources, photos, derived public SKU/package selections and per-file hashes are under each product asset directory. The raw storefront HTTP bodies are excluded because they contain anonymous session fields; the derived public selections identify their original receipt and do not pretend to be HTTP captures. Pinned upstream UIFlow and Xiaozhi tree investigations retain no exact accepted binary for these models.

Use caller-selected paths for source caches and runner inputs:

```sh
python tools/waveshare-batches/s3-tail/prepare_4g_models.py --sources INVENTORY --prefetch INDUSTRIAL_SOURCES
python tools/build-waveshare-models.py --output OUTPUT_ASSETS waveshare-esp32-s3-a7670e-4g waveshare-esp32-s3-sim7670g-4g
python tools/waveshare-batches/s3-tail/audit_4g_images.py --sources INVENTORY
cargo test -p esp32s3 --lib waveshare_s3_cellular
cargo test -p esp32s3 --test waveshare_cellular_guest
cargo clippy -p esp32s3 --lib --test waveshare_cellular_guest --example waveshare_source_attempt -- -D warnings
cargo build --release -p esp32s3 --example waveshare_source_attempt
python tools/waveshare-batches/s3-tail/run_4g_images.py --sources INVENTORY --binary NATIVE_RUNNER --rom S3_ROM --sd EMPTY_FAT32_IMG --evidence EVIDENCE_DIR
```

For rebuilding already submitted products, use the repository's retained-asset `tools/rebuild-product-model.py` workflow. Full repository CI/integrated WASM and hardware testing remain separate from these targeted source commits. Central catalogs and capability tables are regenerated by the parent batch integrator from the two fragments, not edited concurrently by this lane.

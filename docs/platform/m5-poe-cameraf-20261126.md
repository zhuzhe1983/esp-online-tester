# U138, U037 and K115: additional exact controller identities

This change adds three independent controller configurations with source-backed previews, hardware knowledge, official firmware investigations and nine exact-product manual watches. All three keep `runtime.board: null`, `status: unavailable` and no display framebuffer. The existing workbench/mini boundary remains **尚未完成适配**, with Run hidden. No Ethernet, camera or HDMI behavior, guest firmware execution or physical-hardware acceptance is claimed.

| SKU | Identity preserved | Preview scope |
| --- | --- | --- |
| U138 | Unit PoESP32; ESP32-WROOM-32U, 4 MB Flash; ESP32 MAC + IP101G/RMII; preloaded ESP-AT | Exact native manufacturer case assembly, two duplicate print components excluded; estimated metal RJ45 shield/socket, contacts, Grove insert and one photograph-visible fastener |
| U037 | Original M5Camera-F specification: ESP32, 4 MB Flash + 4 MB PSRAM, OV2640, 160° fish-eye lens, CP2104 | Explicit U038 donor case reuse, with an independently estimated fish-eye lens, original-SKU references and rear printed paper; not exact U037 CAD |
| K115 | Atom-PSRAM LCD Display Driver Kit; ESP32-PICO-V3-02, 8 MB Flash + 2 MB PSRAM, CH9102; GW1NR-9C FPGA and LT8618SX | Exact-SKU photographs and dimension-constrained kit approximation, open HDMI mouth, guide frame, distinct controller/driver UNKNOWN boards and cover groups; no exact kit CAD found |

U138 is neither W5500 PoE CAM nor ESP32-P4 PoE hardware. Its external-host Arduino examples send AT commands; they are not the module's internal firmware. K115 is not the C008 Atom Lite 4 MB/no-PSRAM configuration. It requires an external HDMI monitor; the product contains no LCD glass that could be assigned a fabricated framebuffer.

The U037 SKU redirect resolves to **M5Camera-F New**, whose documentation specifies 150°. The older direct document and storefront specify 160°, while the older page's photographs also include “NEW” branding. Both dated sources are retained; the catalog explicitly names the original 160° document identity and does not merge New-version firmware, pin maps or optical validation into it. The wider lens geometry is an exterior estimate, not proof of the optical field of view.

## Models and evidence

The M5_Hardware tree at `18de3bb3087ea617c38642ba3eedd10dc9f17cbf` contains 996 paths and is not truncated. It contains the exact U138 STL and U038 donor case, but no U037/K115 product directory or corresponding embedded KiCad model. Native triangle subsets retain every selected original STL triangle with one rigid, right-handed transform and no rescaling or hole repair. Distant print copies and mounting accessories are excluded explicitly. U037 donor correspondence and K115 hidden structures remain estimates.

U138 native case dimensions measure approximately **26.000025 × 72.000008 × 19.200001 mm**. The U037 donor case is **24 × 48 × 13.2 mm**; its separately estimated stem, rim and glass give an assembled solid depth near **19 mm**. K115's approximate assembled envelope is **24 × 64 × 29 mm**, matching the nominal specification; its official size PDF also gives 28.7 mm height, so the 0.3 mm difference is retained. Matching a chosen envelope is not independent manufacturing verification.

The first U138 native-carrier-only candidate was rejected: 12,598 of 52,010 effective diagram-label texels lacked a plastic carrier, and 578 branding-edge texels missed their selected native plane. The [negative receipt](evidence/m5-poe-cameraf-20261126/rejected-native-carrier-only.json) preserves the candidate hashes and measurements. Photo 2 shows a continuous diagram sticker and an exposed metal RJ45 shield. The final model uses a separately declared, photo-estimated metal shield below this source-CAD cutout and one continuous paper surface; it does not invent plastic or project the label onto internal walls. The branding uses corrected full-body registration and a conservative real-paper-edge mask. Native plastic geometry remains unchanged.

The final [alpha-support check](evidence/m5-poe-cameraf-20261126/alpha-support.json) tests **413,736 alpha≥128 texel centers** across all four printed surfaces against actual exported carrier triangles. Every effective texel has a carrier; unsupported count is zero. Native decals use their actual outer faces. U138 independent paper has 0.025–0.05 mm display clearance above its estimated shield/native plastic, and K115 paper has 0.025 mm above its cover. These offsets prevent display z-fighting; they are not manufacturing tolerances or measured adhesive thicknesses. Hidden shield dimensions remain uncertain.

The [actual-loader checks](evidence/m5-poe-cameraf-20261126/model-checks.json) verify all artifact hashes, native source-triangle fidelity, bounded UVs, fixed lens/paper/connector/PCB materials under original-like, gold and translucent-purple shell presets, repeated inspection reset and geometric sight lines to UNKNOWN. Each UNKNOWN marker's center and four inset corners are visible from a single oblique inspection direction. Grove cable attachment is disabled because physical pin-1 orientation and mating depth are uncalibrated. The source files, photo masks, transforms and estimates are retained in each model plan.

These are geometry/material checks with image decoding stubbed in the JS loader. Product-specific WebGL rendering, readable glyphs, same-view photograph overlays, physical connector fit and hidden-component collisions have not been accepted. Board placeholders and connector/metal-shield estimates are not manufacturer PCB or connector CAD. Manufacturer CAD retains M5Stack MIT; photographs and derived texture pixels remain M5Stack copyright and are not relicensed as project MIT.

## Firmware boundaries and checks

The [firmware survey](evidence/m5-poe-cameraf-20261126/firmware-survey.json) records factory, UIFlow and Xiaozhi findings separately. U138's exact page identifies ESP-AT as preloaded, so its first/default entry is a **factory reference**. The general ESP-AT upstream does not establish a pinned U138 recovery image. U037's fixed M5Stack-Camera tree contains M5CameraF bootloader/partition/app fragments and its document links an EasyLoader executable labelled New; original-160° recovery identity and a verified complete layout remain unresolved. K115's M5GFX library/header is an official development reference, not an identified factory image. Their reference defaults keep `role: project`.

The complete public M5Burner Atom Series and TimerCam category responses contain 18/18 and 15/15 projects respectively. No exact K115/Atom-PSRAM or U037 M5Camera-F image mapping was established. Generic “Unit Camera Factory Test” names do not prove U037 compatibility. The pinned UIFlow and Xiaozhi trees contain 6,924 and 2,126 paths, neither truncated, with no exact internal target-name match for these products. Host accessory scripts and Atom Lite targets are distinct roles. This is a bounded search, not a claim about every external fork.

All nine product-specific factory/UIFlow/Xiaozhi watches are manual, reference-only, with empty automatic `catalogTargets` and executable `assets`. No firmware body or EasyLoader executable was downloaded, mirrored or executed. Firmware links remain remote references; no borrowed factory identity or simulated/hardware verification is inherited.

[checks.json](evidence/m5-poe-cameraf-20261126/checks.json) records 50 actual upstream source byte/hash verifications, schema/knowledge checks, strict watch invariants and three fresh offline rebuilds whose model JSON and every artifact are byte-identical. The firmware policy/sync/target/catalog-selection tests pass **32/32**. The [source receipts](evidence/m5-poe-cameraf-20261126/source-receipts.json) retain original/final URLs, UTC acquisition times, media types, sizes and SHA-256; raw account/contributor API records are omitted.

Rebuild and verify from the repository root, with the existing model dependencies installed:

```sh
python3 tools/build-m5-poe-cameraf-models.py --output /tmp/m5-poe-cameraf-rebuild
node tools/verify-m5-poe-cameraf-models.mjs /tmp/m5-poe-cameraf-loader.json
python3 tools/verify-m5-poe-cameraf-label-support.py --output /tmp/m5-poe-cameraf-alpha.json
python3 tools/validate-platform-catalog.py
node tools/platform-knowledge-test.mjs
```

Use a fresh rebuild directory. This documented custom exporter is the reproduction route for these plans; it has no private queue, network access, firmware download or shared build target dependency.

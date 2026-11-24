# C6 Touch LCD 1.47 — two official mechanical assemblies

This fresh batch adds the exact **31203 / ESP32-C6-Touch-LCD-1.47** base assembly and **31201 / ESP32-C6-Touch-LCD-1.47-M** assembly with soldered headers. They use the shared runtime board `waveshare-c6-touch-lcd147`, but each model has its own official STEP and mechanical PDF. No central runtime or catalog registration is changed by this lane.

The current official repository is pinned to `baea85be7ac288b42664670a456f9c86df7b5a63`; its complete 1,115-entry tree has no mechanical entry. The retained canonical Resources page nevertheless links two exact 2D/3D RAR archives. Both were freshly retrieved. The original archives, exact extracted STEP/PDF members, hashes, retrieval receipts and reference photographs are retained with the corresponding model. The original DXF also remains inside each complete archive.

| Assembly | Native topology | Complete source bounds | Runtime mesh gzip |
| --- | --- | --- | --- |
| Base, no soldered headers | 238 solids + 3 open shells | 24.55 × 44.773315 × 10.60 mm | 4,081,539 bytes |
| -M, two soldered 1×11 headers | 240 solids + 3 open shells | 24.55 × 44.773315 × 15.60 mm | 4,146,169 bytes |

Every original component is retained in its supplied millimetre position. Small components with the same display material are grouped only for rendering; every native component is independently tessellated and checked. The complete assembly is slightly longer than the **44.50 mm case** because the native USB connector protrudes. The -M PDF's **10.60 mm** annotation ends at the standoffs, while its pin tips extend the complete CAD depth to **15.60 mm**. Those quantities are kept distinct.

## Screen registration

The native LCD and touch cover explicitly distinguish the active display from the larger viewing opening:

| Surface | Native source | Dimensions and position |
| --- | --- | --- |
| LCD active area | Component 0, face 46 | **17.39 × 32.35 mm**, R2.30 corners, centered at XY0 / Z1.85 |
| Touch viewing area | Component 1, face 40 | 17.75 × 32.93 mm, R2.35 corners, Z3.20 |
| Raised cover rim | Component 1 | Z3.25 |
| Cover outer XY | Component 1 and PDF | 22.05 × 42.00 mm |
| Surrounding case | Component 2 and PDF | 24.55 × 44.50 mm, R5.75 outer corners |

The 172 × 320 framebuffer uses the native **active area**, with the source's R2.30 corner mask. It is projected onto the viewing face at Z3.20 with a 0.008 mm rendering offset. This does not enlarge the framebuffer to the cover viewing area or change the CAD. The native cover component also contains flex; its full 4.35 mm component depth is not described as glass thickness. `optical-face-inspection.json` preserves the measured face bounds, areas and source circular-edge radii.

## Inputs, enclosure and source limits

Native component **7** is BOOT, bound to `boot` / **GPIO9**, active low. Component 8 is RESET and remains visible without a GPIO button binding. The retained overview repeatedly calls BOOT GPIO8; the actual schematic connects the BOOT net to GPIO9 with R10, a 10 kΩ pull-up. The disagreement remains in the source record.

The supplied case alone is marked as a recolorable shell. The production-loader check confirms that changing its appearance preserves PCB, cover, display and connector material identity, color and opacity. Native case material is restored before review plots are made. Native CAD colors remain approximate, with the cover, case and PCB darkened from the exact product photographs.

The source case has the reused library name `ESP32-S3-TOUCH-LCD-1_47-CASE`. It remains the supplied mechanical shape inside the explicitly named C6 archive; that label is not treated as evidence of an S3 controller or interchangeable complete circuit board. Neither archive certifies a shipping PCB revision.

The rear registration photo is the base board. Both models compare only common native board components against its unchanged pixels. The -M headers are excluded from that photo overlay and instead have their own STEP/PDF and -M package-photo evidence. No source screenshot replaces live framebuffer output, and there is no claimed optical text-to-USB calibration or physical fit test.

## Fresh verification and reproduction

The two models pass the actual production Three.js loader, native bounds checks, all nine interior screen rays, four rounded-corner exclusions, physical BOOT side-face picking and opposite-side occlusion. Inspection expansion returns every moved object to its original position. The existing C5 LCD 2.73 model passes as the no-shell compatibility case. Four views, both photo registrations, both dimension PDFs and the electrical schematic were directly inspected.

Each model's eighteen retained sources match their original and retained hashes. The new RAR reader verifies exact member bytes against the full retained archive, without extracting archive paths to disk. Eight checks cover four real STEP/PDF members and missing-member, decoded-size-limit, wrong-format and truncated-archive failures. This uses the system **libarchive** library; the builder's existing ZIP path is unchanged.

A fresh independent build is byte-identical for each `model.json`, `geometry.bin.gz` and `build-check.json`. `review-receipt.json` records mesh counts, exact source checks, maximum native bounding error and plot hashes. These are source and geometry checks, not browser/WebGL acceptance or new firmware execution results.

```sh
python tools/build-waveshare-models.py waveshare-esp32-c6-touch-lcd-1-47 waveshare-esp32-c6-touch-lcd-1-47-m --output output/c6-touch147-rebuild
python tools/check-waveshare-source-archives.py --output output/c6-touch147-rar-check.json
node tools/check-waveshare-model-loading.mjs waveshare-esp32-c6-touch-lcd-1-47 waveshare-esp32-c6-touch-lcd-1-47-m waveshare-esp32-c5-lcd-2-73 --output output/c6-touch147-loader.json --dump output/c6-touch147-meshes
python tools/review-waveshare-models.py --meshes output/c6-touch147-meshes --output output/c6-touch147-review
```

`integration.json` supplies both independent product/model mappings. No lost pre-reset archive, model or acceptance result is reused.

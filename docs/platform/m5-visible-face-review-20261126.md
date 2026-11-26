# Five M5Stack visible-face corrections · 2026-11-26

These are source-backed exterior corrections and explicit unknown internals. They do not change any controller runtime, factory-firmware mapping, connector electrical claim, source STL bytes or existing assembled source-CAD transforms. All five retain a stable `pcb-main` placeholder and a reachable `UNKNOWN` annotation.

| SKU | Result | Remaining limits |
| --- | --- | --- |
| K162 ToughC5 | A separate photo-estimated opaque rear infill closes the open manufacturer rear frame, matching exact-SKU rear photo-extra-9. The infill follows the rear inspection group and receives shell appearance. | Infill thickness 0.25 mm and registration ±0.6 mm are display estimates. Relief lettering, pads, installed screws and hidden PCB are not reconstructed. Original STL, selected triangles and rigid transforms remain unchanged. |
| A172 Stamp-AddOn C6 | Exact-SKU photo-extra-7 supplies independently masked mainboard backside and radio-substrate backside appearance at their respective heights. The old SDIO connector estimate moves to its photographed tab; the unsupported small upper through-hole is removed because the rear shows continuous soldermask. | Substrate outline/registration ±0.4 mm, thickness and connector envelope/height remain estimates. The known real through-hole stays open. Copper layers, internal radio geometry, exact connector CAD and mating are not verified. UNKNOWN remains on the mainboard layer under the radio module and is readable after inspection separation. |
| M141 Gateway H2 | The source front-print surface sits outside the UNKNOWN annotation depth, so the documented printed front remains visible. Separating the print exposes the unknown layer. | The 0.05 mm print/board separation is a rendering offset, not measured ink thickness. Opposite components remain coarse photo-derived envelopes. |
| K140 SwitchC6 | A newly located uncased assembly photograph, credited by the official shop to SK LABS / Dejan Petrovic, establishes green PCB material. The original UNKNOWN remains. | The oblique source is heavily occluded by relay, capacitors, terminals and a vertical radio daughterboard. It cannot become a flat complete-PCB texture. No exterior translucent-shell photograph is mapped onto a PCB. Full populated board geometry remains unknown. |
| C008 Atom-Lite | The original rear aperture stays open; UNKNOWN moves to the covered internal front face, clear of the rear hole and front slit. The overlapping solid USB proxy is replaced by the existing recessed visual sleeve at the same declared photo-estimated depth/anchor. | Sleeve/tongue/contact detail and mating remain generic approximations. No connector-specific CAD or electrical behavior is inferred. Original STL and binary triangles remain unchanged. |

## Retained evidence

Each directory contains a hash-bound `receipt.json`, separate source photograph/model comparison, original/gold/translucent-purple front/back/oblique CPU images, and a nine-view material contact sheet. K162, A172, M141 and C008 additionally retain the actual production inspection arrangement with readable UNKNOWN; M141's unobstructed evidence is `inspection-front.jpg`, because its print occludes part of the shallow right-hand oblique view.

- [K162](evidence/m5-visible-face-review-20261126/k162/receipt.json) · [source comparison](evidence/m5-visible-face-review-20261126/k162/source-comparison.jpg) · [materials](evidence/m5-visible-face-review-20261126/k162/materials.jpg) · [UNKNOWN](evidence/m5-visible-face-review-20261126/k162/inspection-oblique.jpg)
- [A172](evidence/m5-visible-face-review-20261126/a172/receipt.json) · [source comparison](evidence/m5-visible-face-review-20261126/a172/source-comparison.jpg) · [materials](evidence/m5-visible-face-review-20261126/a172/materials.jpg) · [UNKNOWN](evidence/m5-visible-face-review-20261126/a172/inspection-oblique.jpg)
- [M141](evidence/m5-visible-face-review-20261126/m141/receipt.json) · [source comparison](evidence/m5-visible-face-review-20261126/m141/source-comparison.jpg) · [materials](evidence/m5-visible-face-review-20261126/m141/materials.jpg) · [UNKNOWN](evidence/m5-visible-face-review-20261126/m141/inspection-front.jpg)
- [K140](evidence/m5-visible-face-review-20261126/k140/receipt.json) · [source comparison](evidence/m5-visible-face-review-20261126/k140/source-comparison.jpg) · [materials](evidence/m5-visible-face-review-20261126/k140/materials.jpg) · [source investigation](evidence/m5-visible-face-review-20261126/k140/source-investigation.json)
- [C008](evidence/m5-visible-face-review-20261126/c008/receipt.json) · [source comparison](evidence/m5-visible-face-review-20261126/c008/source-comparison.jpg) · [materials](evidence/m5-visible-face-review-20261126/c008/materials.jpg) · [UNKNOWN](evidence/m5-visible-face-review-20261126/c008/inspection-oblique.jpg)

CPU images use the actual production Three loader geometry, UVs, source textures and material isolation, exported to Cycles CPU. They are separate reference/render images, without calibrated photo pose, lens/lighting calibration or a 1:1 fit claim. JPEG copies preserve the original PNG render hashes, cameras, render scope and exact final model metadata hashes in each receipt. Black display texture is an empty framebuffer, not a static firmware photograph. No browser GPU, firmware execution or physical hardware acceptance is claimed in this correction set.

## Envelope measurements

These bounds come from the restored production loader, excluding UNKNOWN text and photo/print planes, while retaining estimated physical connectors/components. They are separate from source-CAD-only checks and do not establish manufacturing fit.

| SKU | Catalog dimensions, mm | Complete visible physical mesh extents, mm |
| --- | --- | --- |
| K162 | 75.9 × 58 × 42.5 | 75.9302 × 58 × 42.0450 |
| A172 | 18 × 27 × 4 | 17.9615 × 26.9500 × 4 |
| M141 | 54 × 54 × 13.1 | 54 × 54 × 13 |
| K140 | 58 × 46 × 27 | 57.9000 × 46 × 27 |
| C008 | 24 × 24 × 9.5 | 24 × 24.0300 × 10.3571 |

C008's retained photo-estimated female headers extend beyond the source housing envelope; this measurement does not silently treat the 9.5 mm housing depth as an assembled-component pass. K162's manufacturer source already differs from the nominal depth; no scaling is applied to hide that difference.

## Reproduction and checks

Run the source/hash synchronizer in check mode and the production geometric/material verifier:

```sh
python tools/sync-m5-visible-face-contracts.py --ids m5stack-k162,m5stack-a172,m5stack-m141,m5stack-k140,m5stack-c008
node tools/check-m5-visible-face-contracts.mjs --out /tmp/m5-visible-check.json
python tools/validate-platform-catalog.py
node --test tools/platform-model-inspection-test.mjs
```

The five-product verifier checks actual source hashes, shell-only appearance changes, five annotation visibility probes from reachable directions, eight inspection/reset cycles, opaque rear closure, unchanged C008 rear aperture, front print depth order and retained unknowns. Source appearance and purple appearance are checked separately. All five pass. Catalog schemas pass for 728 products, 10 accessories, 670 firmware entries and 537 watchlist entries; the seven general inspection tests pass.

Full offline rebuilds of K162, A172, M141 and K140 match every generated artifact hash and metadata contract. For the first three, use `python tools/build-m5-missing-models.py --output /new/scratch/directory <id>`; for K140, use `python tools/build-m5-missing-controller-models.py m5stack-k140 --output /new/scratch/directory`.

C008's general offline entry is `python tools/rebuild-product-model.py build m5stack-c008 --scratch /new/scratch/directory`. This run reports a sole exact-byte mismatch, `artifact:geometry.bin`; all metadata contracts and other source/assets match. With the pinned Trimesh 5.2.0, the regenerated file contains the exact same 2,926 oriented triangles and per-vertex normals in a different order. The production binary is retained. [The exact failure receipt](evidence/m5-visible-face-review-20261126/c008/exact-rebuild-failure.json) and C008 receipt preserve both binary hashes and the canonical multiset hash; this is not reported as exact-byte rebuild success.

To reproduce the canonical comparison, load each binary as little-endian float32 with shape `(-1,3,6)`, normalize signed zero, take the lexicographically smallest byte string among each triangle's three cyclic vertex rotations, then sort the resulting strings. Sorting never reverses winding or drops duplicate triangles; all six position/normal values participate. Both files produce canonical SHA-256 `2943d8061a84e7ef952e5fd3728028293073b84e074c3e31c4a6e6e4bd9d9a3c`.

CPU scripts use the unchanged tooling at commit `caa80ebe`: `export-m5-cpu-models.mjs --repo <repo> --ids <ids> --out <export>` and `render-m5-cpu-qa.py -- --repo <repo> --input <export> --out <render> --size 640 --samples 12 --back-oblique`. Repeat with `--appearance gold` and `--appearance purple`. Inspection uses exporter `--inspection 1`, size 720, and the retained [camera-only patch](evidence/m5-visible-face-review-20261126/render-inspection-camera.patch) to view from `(3,1,1)`; no part is hidden or independently repositioned for evidence. K140 has no fabricated inspection arrangement.

The broader existing edge/legacy-five/label-material test command was also attempted in the isolated sparse checkout. It is blocked by missing, unrelated C149/K007-v26/C126-family model assets (`ENOENT`/metadata 404), rather than a failure in these five loaded models. It must be rerun in a complete checkout before a broad release claim. No Cargo, network push or deployment was performed for this correction set.

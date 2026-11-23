# Expanded Seeed model evidence

This directory records the additional commercial SKUs and exact board/panel kit configurations beyond the original 13 models. Original acceptance records in `../seeed-models/` remain historical. A reused exterior CAD file does not transfer product, firmware, peripheral or hardware acceptance to a new SKU.

## Bare EE boards

EE02, EE03, EE04 and EE05 have no integrated display. Their model plans have `presentation.screen: null`; catalog checks require `display: null` and reject a mismatched screen. All four retain actual manufacturer KiCad layout files extracted from the official archive, plus official front and annotated reference photographs. Archive HTTP identity and extracted member identity are separate. EE05 contains one unrelated personal output-directory setting; only that setting is normalized, with original and retained hashes in its source manifest. Nine retained reference images contain no detected personal path or signed-URL metadata.

The substrate follows source Edge.Cuts, circular mounting and through-pad drills, and the 1.6 mm PCB thickness. The EE02/03/04 PCB outline measures 80.000 × 41.232385 mm; the EE05 outline measures 25.000 × 52.160 mm in the chosen front view. Official photo envelope dimensions are recorded separately, including the nominal 80 × 43 or 81 × 44 mm product envelopes; the source board is never scaled to those numbers. EE03 also contains five open internal Edge.Cuts segments with unspecified router/slot width. Their coordinates are retained and their visual omission is explicit.

Package bodies use manufacturer footprint XY/rotation and F.Fab/F.SilkS bounding envelopes. Most package heights and connector body simplifications are estimates. The XIAO module's internal PCB is marked UNKNOWN because its parent footprint is not its internal board layout. Copper tracks, vias and artwork are omitted. DNP parts remain unpopulated, and EE05 sample footprints outside the board are not relocated. Button caps bind to `key1`, `key2`, `key3` only where the retained pad net proves BUTTON1/2/3 identity; physical reset remains unbound to GPIO.

## Reconstruction and checks

Commands from the repository root:

```sh
python tools/derive-seeed-pcb-plans.py ee02 ee03 ee04 ee05
python tools/build-seeed-models.py seeed-xiao-epaper-ee02 seeed-xiao-epaper-ee03 seeed-xiao-epaper-ee04 seeed-xiao-epaper-ee05 --output /tmp/seeed-model-rebuild
python tools/build-seeed-models-test.py
python docs/platform/evidence/seeed-models/render-review.py seeed-xiao-epaper-ee02 seeed-xiao-epaper-ee03 seeed-xiao-epaper-ee04 seeed-xiao-epaper-ee05 --output docs/platform/evidence/seeed-expanded-models
python docs/platform/evidence/seeed-models/verify-models.py --catalog web/platform/catalog/products.json --review-dir docs/platform/evidence/seeed-expanded-models --output docs/platform/evidence/seeed-expanded-models/checks.json
```

The first four models contain 4,078,944 bytes of geometry altogether. `first-four-checks.json` verifies their local sources/artifacts at the first asset checkpoint, geometry, absent display planes, and key control IDs against the incoming catalog identified by its SHA-256. Each model has three actual CPU z-buffer views with nonclipping bounds and final model/image hashes. These are software geometry checks, not WebGL/browser screenshots, calibrated photo overlays or firmware/hardware tests.

Shared manufacturer sources use an explicit `sharedAssetProductId` plus a basename and SHA-256. The builder copies them into that fixed source owner's directory on an offline rebuild. It does not recursively resolve model plans, infer compatibility or follow arbitrary paths. Regression tests reject changed shared bytes, self/traversal/symlink escapes, and verify that an original HTTP/member hash is not confused with a normalized retained-body hash. Each product still has its own local binary for the unchanged browser loader.

## Exact panel choices and commercial family variants

The expansion adds all 25 named EE board/panel kit choices from the retained fixed-commit official Hub map. Fourteen exact panel SKUs have retained official photographs and mechanical-specification/drawing PDF pages. The source manifests retain each complete original HTTP byte count/SHA-256 separately from its smaller, explicitly derived PDF excerpt. Extracting selected PDF pages does not resample the manufacturer photographs/diagrams. The source 4.26-inch drawing distinguishes the 105.33 × 62.37 mm glass from the 24 mm FPC tail included in the table's 129.33 mm total length. The 7.3-inch thickness remains TBD in its source; 1.2 mm is explicitly estimated. Source active/glass XY sizes are retained, but active-area registration is centered as a debugging convention, not a claim that unequal border margins were calibrated.

`kit-checks.json` records 25 actually generated models, their final source/model/geometry hashes, 625 unobstructed synthetic framebuffer samples, and native screen aspect/control-ID consistency against the identified incoming catalog. Each has three CPU z-buffer views with numeric nonclipping bounds. The kits total 21,179,520 bytes of geometry. They are separated board-plus-panel inspection layouts; no enclosure, battery, antenna, FPC cable or mechanical attachment is invented.

The six new SenseCAP commercial variants retain exact-SKU product PDF excerpts and extracted original hero photographs, and cite the common manufacturer's CAD by a fixed shared source hash. W1-A and XiaoZhi retain clear Watcher shells. Indicator D1S/D1L/D1Pro/Meshtastic retain the common exterior, top button, display and stand; the unverified generic populated PCB/components are omitted and replaced with an explicit UNKNOWN envelope. Loose packaged sensors and unverified RF/connector hardware are not fabricated. The Meshtastic and XiaoZhi firmware identities remain independent of the ordinary family product.

The existing W1-B SKU 113991275 is corrected to white opaque shell/stand/door material based on the official W1-A/W1-B selection table. The material-only checkpoint preserves source CAD, component positions and geometry bytes. The final overlapping-open-surface correction is documented below; lens, screen and buttons retain independent materials. The old clear-family appearance remains part of historical evidence; the new material has separate review files in this directory.

The first-four and kit reports are historical asset checkpoints. The combined final reports below supersede their model hashes after inspection-metadata and source-surface corrections.


## Final combined verification

The final dataset contains 48 Seeed models: the original 13 plus all 35 confirmed additions. `checks.json` verifies all 105 unique retained source bodies, all declared per-product artifacts, valid source transforms, control-ID/native display consistency, four absent bare-board screens and 1,064 unobstructed front-plane samples. Total geometry is 121,033,296 bytes; the 35 additions account for 58,873,824 bytes. All 48 have three current CPU z-buffer views (144 actual views) with nonclipping bounds, final model/image hashes and explicit synthetic-screen labels. `expanded-front-contact.jpg` summarizes only the 35 additions; the individual PNGs retain the full views.

`loader-checks.json` records actual construction through the existing Three.js manufacturer loader, 384 closest-hit framebuffer rays, 109 visible-surface button rays, actual inspection expand/restore, and gold/translucent-purple material isolation. Static PCB/panel models have no recolorable case; lens, display, key and component materials remain independent of case appearance. This uses the real Three.js object/geometry/raycaster code under Node, with response fixtures containing the actual local-file bytes. It is not WebGL rendering, a DOM/browser screenshot, a calibrated photo overlay or a hardware/firmware acceptance result.

The actual loader uncovered invalid zero-offset inspection groups in the shared templates. Static components require no movement definition; those groups are now removed without changing geometry or assembly positions. E1004's estimated front/back groups also now cite their existing manufacturer-photo/specification source IDs. `inspection-contract-fix.json` records the metadata transition. The verifier now checks required nonzero offsets, group size and source identities in addition to part ownership.

The W1-B front artifacts were duplicate-depth rendering, not texture. Open source component 92 overlaps the actual shell component 21: 510 of 514 triangle centers project onto the shell, with 493 within the 0.12 mm export deflection and alternating positive/negative residuals. Its earlier protective-film name had no independent physical-layer proof. The original STEP is retained, while that overlapping open surface is explicitly omitted for all three Watcher identities. The remaining source solids are not moved or scaled. `watcher-front-overlap.json` records the before/after hashes and measurements; the final white shell is continuous in its actual review image. Each Watcher now has 8,281,440 geometry bytes.


## Watcher exploded assembly

W1-B, W1-A and XiaoZhi use five viewing layers from the front housing through the display and camera, populated PCB, battery and rear housing with stand. The Watcher STEP assembly at commit `0dbd7ac5a0bedb2de51fdb7bb2064a9001e9d273` places both USB-C ports, the microSD socket, Grove, the expansion header and the encoder within `01-PCBA-ASM_ASM`. Those parts and the wheel now share the PCB offset. The front housing carries its retained window, insert and door. Seeed’s internal photograph shows the speaker at the front inner wall and the separate battery pack over the PCB. [Internal photograph](https://wiki.seeedstudio.com/watcher_hardware_overview/#inside)

The split follows the ordered source-solid ranges in each product's `build-check.json`. The 65,800 affected triangles map to the original STEP solid bounds. The full set of part ranges reconstructs the exact 8,281,440-byte shipped geometry. Fresh exports of the old and new plans produce identical geometry to each other on the pinned local toolchain. Both fresh exports have 436 fewer triangles than the retained production geometry, so the update uses the verified triangle ranges in the retained binary. Chromium showed four adjacent axial clearances of 14.00–14.48 mm at 100% and identical assembled images at the same camera angle for all three targets. The mobile XiaoZhi embed also restored the original component transforms.


`reproduction.json` records byte-identical regeneration of all 29 PCB/kit plans and two complete fresh offline exports (EE05 bare and EE05 + 2.13-inch flexible panel), including the fixed shared-source copying and hashes. The other final exports have direct retained-source/artifact/load/geometry validation; a fresh independent full replay of every 48-product export is not claimed. `tools/build-seeed-models-test.py` additionally verifies changed shared-source rejection, original/retained hash separation and fixed-directory/self/traversal/symlink boundaries.

EE05 has no verified manufacturer total thickness or complete nominal product envelope in the consulted materials. Its 25 × 52.16 × 1.6 mm source PCB is recorded directly; the 7 mm preview package depth is explicitly estimated. No unavailable 25 × 50 × 7 mm official envelope is asserted.

Final source/body privacy review is recorded separately in `privacy-review.json`. Source photographs are never replaced with generated art or lossy re-encodings. Technical drawing author/company/copyright attribution is retained; unrelated CAD output paths and document metadata are minimized with original and retained identities kept distinct.

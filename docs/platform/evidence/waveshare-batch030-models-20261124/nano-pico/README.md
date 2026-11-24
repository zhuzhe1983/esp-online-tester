# Nano and Pico source-delivery model reconstruction

This directory verifies the new `dimension-photo-reconstruction` assets for
ESP32-S3-Nano, Nano-M, Pico and Pico-M. It does not recover historical model bytes
or reuse the old batch-04 image acceptance. The evidence below is bound to the
current model, plan, geometry, retained sources and newly rendered review images
by `visual-review.json`.

These receipts and the hash table below identify the original source-delivery
metadata before privacy normalization. Later removal of anonymous fields from
reference HTML changed source pins and model metadata hashes, while geometry,
photographs, textures and rendered metadata remained unchanged. See the
[original/normalized mapping](../../waveshare-batch030-import-20261124/source-normalization.json)
and [current source, loader and rendered-metadata equivalence checks](../../waveshare-batch030-import-20261124/checks.json).
Historical visual-review hashes and dates are unchanged.

Raw mesh dumps and review PNGs are archived in the exact batch030 delivery.
Their original hashes and recovery steps are retained in
[the import evidence](../../waveshare-batch030-import-20261124/README.md).
Generate new mesh dumps and review images under ignored `output/` directories.

## Sources and dimensions

Each asset directory retains the exact manufacturer image bytes, losslessly
compressed HTML, HTTP retrieval times and SHA-256 values in `source-manifest.json`.
The current store, overview, dimensional photographs and gallery photographs were
retrieved on 2026-11-24. The canonical Resources pages retain their actual
2026-11-23 retrieval times. Their parsed links contain no matching STEP, STL,
KiCad, Gerber or 3MF link; this finding covers those retained pages, not every
external location or linked software archive. The per-model `source-search.json`
receipts retain the inspected links and source hashes.

| Family | Published board XY | Header rows on -M | Published or source-supported geometry |
| --- | --- | --- | --- |
| Nano | 17.78 × 43.18 mm | 15 + 15 | Four radius 0.83 mm mounting holes; 15.24 × 40.64 mm mounting-hole spacing; 2.54 mm pitch explicitly stated in the official overview and store |
| Pico | 21 × 51 mm | 20 + 20 | 17.78 mm header-row spacing; 2.54 mm pitch; 1.00 mm pad drill; USB metal width 8.95 mm in the official dimension image |

The PCB thickness is **unknown**; 1.6 mm is only an illustrative modeling input.
Component heights, header bodies, pin thickness and length, USB depth and contact
geometry are unmeasured. The Nano pad drill is also an explicit estimate. The
back of each PCB visibly reads **UNKNOWN**. Photographic front detail does not
establish traces, internal structure, component internals or physical fit.

Original front photographs are projected onto the actual board outline and
individual modeled top faces. Both the board geometry and photographic surface
have through-holes. The front PCB texture excludes the raised component
footprints and USB envelope. Small passives that are not separately modeled remain
flat photographic detail. `photoRegistration` in each plan records the original
image size, manually selected board-boundary pixels and affine transformation.
The dimension-image review uses a separately declared transformation; no fitted
manufacturing tolerance or image-registration error metric is claimed.

Manufacturer photographs and diagrams retain their original rights. Public
availability is not an MIT licence; see each asset's `SOURCE-RIGHTS.txt`.

## Interaction scope

Nano and Nano-M have physical RESET geometry, no invented BOOT button, and empty
3D GPIO controls. GPIO0 remains an RGB channel. The runtime LED slots are 0 and 1
for RGB and SCK respectively. Pico and Pico-M expose the sourced BOOT cap as the
only mapped 3D button and RGB as LED slot 0. Physical RESET is visible on all four
models, but it is not mapped as a GPIO button; restarting uses the platform's
separate restart capability. This is not a claim that all physical buttons have
interactive 3D mappings.

## Source-delivery acceptance

`loader-checks.json` records the actual production Three.js loader results for
all four models: finite geometry, published board XY, sourced texture hashes,
photo UVs, LED indices, the Pico BOOT cap ray and opposite-face occlusion.
The declared 0.001 mm XY tolerance checks export consistency against the nominal
dimensions, not manufacturing accuracy. The largest measured XY difference is
below 0.000001 mm; no measured Z target is asserted.

`additional-geometry-checks.json` checks the same production geometry: visible
rear UNKNOWN labels, open USB envelopes, all Nano mounting holes, 30 Nano-M or
40 Pico-M pins centered in actual PCB and photograph holes, and the absence of
a fabricated RESET control.

All four new front/reverse/edge/oblique plots and all four manufacturer dimension
photo overlays in `reviews/` were inspected. The inspected result has the correct
board proportions, USB direction, sourced component arrangement, visible RESET,
row count and unobscured rear UNKNOWN. The four-view plots are geometry-only
views, not textured WebGL screenshots. The shared plot title's screen swatch
note is generic; these four boards have no display.

These results cover source traceability, deterministic geometry, CPU ray tests
and the displayed review images. They do not constitute browser/WebGL input,
browser text-readability, manufactured tolerance, cable-fit or real-hardware
acceptance. Browser acceptance is recorded separately for the final batch.

| Asset | Source-delivery model.json SHA-256 before privacy normalization |
| --- | --- |
| waveshare-esp32-s3-nano | `43f2a4f0c45357738eff5fb52918604b019f0d914447210e285a0d0e201da946` |
| waveshare-esp32-s3-nano-m | `94dd6195654eb89358f31932b674655947b49bb773aea1d8e2e55854c82eec63` |
| waveshare-esp32-s3-pico | `f4c6280e4c234e48bf1d6cd5c5b3dbcb682ef90af6e0c03025c3b31b58eedb2e` |
| waveshare-esp32-s3-pico-m | `f05cbd76b518e91987fca41b7403f4bc361689a6fc3451ac64df9cb6cddbe476` |

## Reproduction

Run from the repository root with the dependencies used by the shared model
tools installed. The recorded build uses Python, Pillow, NumPy and
`cadquery-ocp`; review plotting additionally uses Matplotlib. The loader uses
the repository's production Three.js and manufacturer model loader. Exact build
engine versions and generator hashes are in each asset's `build-check.json`;
the Node version is in `loader-checks.json`.

The retained sources are sufficient; no external session directory or live
network request is needed for this reconstruction. `reconstruct.py` verifies
both retained and original hashes, regenerates the plans, and leaves the source
retrieval times intact.

```sh
python3 docs/platform/evidence/waveshare-batch030-models-20261124/nano-pico/reconstruct.py
python3 tools/build-waveshare-models.py \
  waveshare-esp32-s3-nano waveshare-esp32-s3-nano-m \
  waveshare-esp32-s3-pico waveshare-esp32-s3-pico-m \
  --output web/platform/assets
node tools/check-waveshare-model-loading.mjs \
  waveshare-esp32-s3-nano waveshare-esp32-s3-nano-m \
  waveshare-esp32-s3-pico waveshare-esp32-s3-pico-m \
  --output docs/platform/evidence/waveshare-batch030-models-20261124/nano-pico/loader-checks.json \
  --dump output/waveshare-batch030/nano-pico/mesh-dumps
node docs/platform/evidence/waveshare-batch030-models-20261124/nano-pico/check-geometry.mjs
python3 tools/review-waveshare-models.py \
  --meshes output/waveshare-batch030/nano-pico/mesh-dumps \
  --output output/waveshare-batch030/nano-pico/reviews
```

The visual-review record is a manual observation of the listed hashes; rerunning
commands does not by itself create a new visual acceptance record. Reinspect
images and update that record if any source, model, plan or rendering changes.

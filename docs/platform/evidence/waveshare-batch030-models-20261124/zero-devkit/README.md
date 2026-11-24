# Batch 030: new Zero / DevKit model artifacts

These **11 newly generated models** replace missing authoring outputs with new,
source-verifiable artifacts. They are not presented as recovery of the original
model bytes. The earlier batch-04 images and checks remain historical evidence
for their own recorded hashes only. This folder binds the current plans, sources,
geometry, loader checks and new review images to the original source-delivery
model hashes. Later privacy normalization changed reference HTML and metadata
pins; the [original/normalized mapping](../../waveshare-batch030-import-20261124/source-normalization.json)
and [current source, loader and rendered-metadata equivalence checks](../../waveshare-batch030-import-20261124/checks.json)
bind the final metadata to the same unchanged geometry, photographs and textures.
Historical visual-review hashes and dates remain unchanged.

Raw review PNGs are archived in the exact batch030 delivery; their original
hashes and recovery steps are retained in
[the import evidence](../../waveshare-batch030-import-20261124/README.md).
The full native CAD component inspection is retained byte-identically as gzip.

## Source and geometry scope

| Models | Method | Geometry actually retained or reconstructed |
| --- | --- | --- |
| Zero-M | `official-cad-partial` | All 40 components of the existing manufacturer's FH4R2 Zero STEP, unscaled; two approximate nine-pin headers added at independently measured native hole centers. |
| DEV-KIT-N8R8-M | `official-cad` | All 420 native topology components from the official `ESP32-S3-DEV-KIT-N8R8_20260110.zip` assembly. Same-color components are grouped for drawing without dropping topology. |
| DEV-KIT-N8R8 | `official-cad` | The same native assembly with only header components 417 and 418 omitted, as supported by the exact bare-board photograph. All other 418 components remain. |
| Zero-N8R8 / N8R8-M | `dimension-photo-reconstruction` | Separate PICO-1-N8R8 photographic envelopes, not FH4R2 package CAD. The SiP is a roughly 45-degree square; 7.8 mm width and 1 mm height are explicit estimates. |
| DEV-KIT-N16R8 / M, N32R16V / M | `dimension-photo-reconstruction` | Each exact-SKU source and profile is retained independently. The shared coarse envelope follows the visible family layout; exact module CAD, hidden placement, silkscreen and stackup remain unknown. |
| DEV-KIT-N16R8-U / UM | `dimension-photo-reconstruction` | A genuinely open U-shaped board end and an approximate open RF socket. The photographed coax and whip stay in the source reference; cable length, routing and mating geometry are not fabricated. |

The existing `web/platform/assets/waveshare-esp32-s3-zero/` directory is unchanged.
`zero-protected-assets.json` records all 10 original file hashes, checked again
after generation and review.

The official DevKit archive SHA-256 is
`2edf965c20a7e3804b379e76ec2e432db10e8079f784f1db89f613527af99932`.
Its original STEP member SHA-256 is
`9bcedd033637555746d4ba873864da52edab18520b62c7f764d523d8fd3e75b4`.
The archive, member name, exact byte counts, actual acquisition timestamps,
uncompressed hashes and deterministic retained-gzip hashes are in each native
plan. `devkit-native-cad-inspection.json.gz` records the 420 source components.

The new exact Zero N8R8 bare and header photographs were fetched from URLs
observed in the official product page, with HTTP 200:

- Bare: `ESP32-S3-Zero-details-3-3.jpg`, 49,006 bytes,
  SHA-256 `25f42d85231df734fd121786223065e3abae64e4e987ea3525bd1d824214ab7c`.
- Header: `ESP32-S3-Zero-details-3-4.jpg`, 42,877 bytes,
  SHA-256 `fee080125932af8495993d600aff095affc477a32e20f7ec863b150e5fca723f`.

These are 420 × 360 source images. The board-plane registrations declare
unquantified reading uncertainty and occlusion; the M anchors include visible-edge
extrapolation where pins or USB obscure the board. They do not assert fine
metrology or a pixel-level error bound. Each source URL,
real `fetchedAt`, file, byte count, SHA-256 and rights note is retained in the
asset's `model-plan.json` and `source-manifest.json`. Public accessibility does
not establish a general redistribution licence for manufacturer sources.

## Measurements and explicit uncertainty

The Zero nominal PCB is 18 × 23.5 mm. Zero-M header centers come from the native
PCB circle edges in `zero-native-pcb-circles.json`, with a nominal 2.54 mm pitch
and 15.24 mm row spacing. This native center measurement is not transferred as
an N8R8 CAD claim. The N8R8 variants use the published family size and their own
photographs, with illustrative drill and header dimensions.

The DevKit drawing labels 25.4 × 63.3 mm, R1.0 mounting holes, 1.8 mm edge
offsets, and a 21.8 mm transverse hole-center distance. Its nominal mounting
centers are therefore X = ±10.9 mm and Y = ±29.85 mm. The unscaled native PCB
extents are slightly larger, approximately 25.4207 × 63.3107 mm; the source is
not distorted to conceal that difference. The 0.05 mm native comparison bound
tests source/export consistency, not manufacturing tolerance.

For photo models, unknown exact dimensions are `null`; `illustrative*` fields
are separate display estimates. In particular, 1.6 mm PCB thickness, component
heights, header body dimensions, hidden pin lengths, USB shell wall thickness,
module shields and the U-notch depth do not establish physical fit. The model
shows a visible reverse-face `UNKNOWN` label. There is no invented enclosure
and no recolorable PCB masquerading as a shell.

## Source-delivery checks and review artifacts

- `loader-checks.json`: all 11 actual production Three.js models loaded; finite
  geometry, declared dimensions, BOOT visibility, opposite-face input occlusion,
  LED index 0, visible UNKNOWN labels where declared and material isolation passed.
- `additional-geometry-checks.json`: all 11 passed applicable geometry checks.
  Photo-derived USB shells are genuinely open, estimated header pins cross
  actual PCB holes, and both U variants are open at (0, −29) mm while solid at
  (0, −25) mm. Native N8R8 headers are preserved source topology, not inferred
  procedural headers.
- `reviews/*-four-views.png`: new front, reverse, edge and oblique z-buffered
  geometry reviews from the production loader's actual vertices.
- `reviews/*-photo-registration.png`: nine new independent dimension-photo
  overlays. For the six photo DevKit variants, this family drawing checks only
  the shared planar extent; exact option details are checked separately.
- `reviews/*-exact-photo-overlay.png` and `exact-photo-registration.json`: eight
  new exact-option overlays, with the new model/plan/source hashes and explicit
  planar homographies. Projected component footprints are compared at the board
  plane, so raised surfaces can show parallax without implying a height fit.
- `review-registrations.json`: separate model/plan/photo-bound Zero-M dimension
  registration. This avoids the general review helper's legacy assumption that
  every `front` registration uses a file named `front-photo.jpg`; the original
  asset plan and retained photo remain unchanged.
- `visual-review.json`: actual inspected images, findings, current model hashes
  and the declared review boundary.
- `artifact-sha256.json`: complete file hashes for these 11 asset directories.
- `header-material-correction.json`: the two Zero header models now use the
  yellow plastic body visible in their exact official photographs. This is an
  uncalibrated color correction; geometry and dimensions are unchanged.
- `photo-registration-correction.json`: records the source-image size correction
  and the reselected, explicitly approximate N8R8-M board-edge registration.
  `zero-n8r8-sip-projection.json` retains the new bare-image package picks and
  inverse projection; it supports a rough diagonal footprint, not precise fit.

The standard generator ran with `cadquery-ocp 7.8.1.1.post1` and `numpy 2.3.5`,
as recorded by each current `build-check.json`. It validates source bytes,
retained encodings, archive-member identity, rigid transforms, native coverage
and source/export bounds before producing geometry.

## Reproduce without a private authoring cache

Each `model-plan.json` and all its inputs are self-contained in its asset
directory. The retained plan is the authoring specification; no private or
unpublished plan generator is required. Run from the repository root, with the
declared OCP and NumPy versions installed. `$REPO` means that root, and `$MESHES`
is a disposable output directory outside tracked source.

```bash
python3 tools/build-waveshare-models.py \
  waveshare-esp32-s3-zero-m \
  waveshare-esp32-s3-zero-n8r8 waveshare-esp32-s3-zero-n8r8-m \
  waveshare-esp32-s3-dev-kit-n8r8 waveshare-esp32-s3-dev-kit-n8r8-m \
  waveshare-esp32-s3-dev-kit-n16r8 waveshare-esp32-s3-dev-kit-n16r8-m \
  waveshare-esp32-s3-dev-kit-n32r16v waveshare-esp32-s3-dev-kit-n32r16v-m \
  waveshare-esp32-s3-dev-kit-n16r8-u waveshare-esp32-s3-dev-kit-n16r8-um \
  --output web/platform/assets
```

Pass the same 11 IDs to `node tools/check-waveshare-model-loading.mjs`, with
`--dump "$MESHES"` and `--output` pointing to this folder's `loader-checks.json`.
Then run:

```bash
node docs/platform/evidence/waveshare-batch030-models-20261124/zero-devkit/check-geometry.mjs
python3 tools/review-waveshare-models.py --meshes "$MESHES" \
  --output output/waveshare-batch030/zero-devkit/reviews \
  --registrations docs/platform/evidence/waveshare-batch030-models-20261124/zero-devkit/review-registrations.json
python3 docs/platform/evidence/waveshare-batch030-models-20261124/zero-devkit/review-exact-photos.py \
  --meshes "$MESHES" \
  --output output/waveshare-batch030/zero-devkit/reviews
```

The CPU loader and geometry review do not claim DOM text readability, WebGL
rendering, browser input acceptance, firmware execution, physical measurements
or hardware compatibility. Those gates are reported separately by the final
platform validation. No old firmware acceptance is inherited by a new model.

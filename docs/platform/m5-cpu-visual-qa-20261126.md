# M5Stack CPU image review of 141 production models

The production Three loaders were executed for every M5Stack catalog product:139 generic models plus the dedicated PaperMono and StopWatch models. Cycles CPU rendered423 original front/back/oblique images. Another21 products with substantial paper, labels or optical surfaces were rendered with the production gold metal and35%-opacity purple plastic presets,126 more images. The retained deliverable is48 JPEG contact sheets with separate manufacturer photographs and7 candidate768px PNGs, rather than every raw render.

These are actual model pixels, not ray-hit checks or generated illustrations. Screens use a blank black framebuffer; no static factory screenshot is substituted. Contact sheets are an image screening aid and do not claim browser GPU/DOM, physical hardware, calibrated lighting or pose-aligned photogrammetry acceptance.

All36 original contact pages were visually viewed.35 products have candidates recorded individually in `evidence/m5-cpu-visual-qa-20261126/visual-review.json`:missing manufacturer papers, exposed CAD `UNKNOWN` placeholders, and a few assembly/marking questions. The other106 are recorded as screened at320px with no additional obvious paper/placeholder candidate, not as blanket quality acceptance. Root review and repairs remain separate. One product, C147 Chain DualKey, has no retained reference photographs in the source model directory.

The initial21-model render and interrupted first all-model pass lacked the CPU approximation for coplanar paper. Those preliminary images are not the retained final result. The final export displaces only textured meshes with a negative Three `polygonOffset` by0.005mm along their actual transformed normals; original assets stay unchanged. This explicitly approximates GPU depth bias. Texture UV matrices, flipY, RGBA alpha and alphaTest clipping are retained. Basic/Standard color, emission, opacity, roughness, metalness and sidedness are translated. The actual loader audit found only MeshBasicMaterial/MeshStandardMaterial and no custom shader, unsupported maps or advanced material features for all141 models. Canvas lettering uses DejaVu Sans Mono as a documented font approximation; invisible hit planes, private user sticker-store overlays and non-mesh glows are omitted.

The three questioned blank backs—Basic v2.6, GRAY and Core2 v1.1—were independently re-exported and rendered with the depth-bias approximation. Their actual production materials have no file textures or custom shaders, so their missing labels are not caused by an untranscribed projected-texture shader. Other candidates and their actual texture files are in the per-SKU JSON. Atom-Lite's rear hole visibly exposes an `UNKNOWN` fragment even at768px. Plus2 and AtomU marking candidates remain subject to the stated canvas-font limitation and production geometry review.

PaperMono's existing `rear-nfc-albedo.webp` is explicitly documented in its source model as previously edited with ImageGen. The renderer faithfully consumes that existing model material; it is never treated as the reference photograph. `reference-photo-08.webp` is the separate retained genuine rear photograph used alongside it. This QA created no generated images or image edits.

Receipts contain source repository HEAD, per-model/source-texture hashes, baked geometry hashes, CPU camera/bounds, renderer version, appearance and image hashes. The full material/UV settings are retained as deterministic `material-receipt.json.gz`; large binary mesh exports and all raw renders stay reproducible and are not checked into git. This baseline was exported from source HEAD `a3749ac9ac343190a8aca5ea900605caae238a1b` before the subsequent model repairs.

Reproduce against the intended source checkout:

```sh
node tools/export-m5-cpu-models.mjs --repo /path/to/source --out /tmp/m5-cpu-export --all
/usr/bin/blender -b --factory-startup --threads 2 --python tools/render-m5-cpu-qa.py -- --repo /path/to/source --input /tmp/m5-cpu-export --out /tmp/m5-cpu-renders --size 320 --samples 12
python tools/compose-m5-cpu-qa.py --repo /path/to/source --renders /tmp/m5-cpu-renders --out /tmp/m5-cpu-contact --photo-order docs/platform/evidence/m5-cpu-visual-qa-20261126/photo-order.json --rows 4 --cell 320 --format jpg
```

Use `--ids` with a comma-separated product list to export/render a small subset. The renderer's `--appearance gold` or `--appearance purple` selects captured production material presets; a separate output directory prevents overwriting original images. Candidate images use `--size 768 --samples 24`. No source asset is written by these scripts.

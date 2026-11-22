# LilyGO expansion model source notice

The new preview plans distinguish manufacturer CAD, photo-derived coarse exteriors,
screen-only debugging panels, and unresolved placeholders. A preview is not a
firmware compatibility claim or a manufacturing drawing.

Official gallery photographs retained as `reference-*` are evidence, with their
original URL, retrieval timestamp, byte length and SHA-256 recorded in each model
plan. They retain the manufacturer's copyright. They are not AI-generated and
are not painted over the live framebuffer. Repository source-code licensing is
not silently applied to these photographs.

Manufacturer CAD sources retain their repository path, pinned commit, raw source
hash, native units and rigid viewing transform. Gzip archives are lossless and
their hashes are recorded separately from raw source hashes. Where available,
the same upstream repository's license is retained as `SOURCE-LICENSE.txt`.
Absence of such a file is recorded as an unknown license declaration; this work
does not relabel that manufacturer's CAD as MIT or claim ownership of it.

The photo derivation is deliberately limited: it measures a projected exterior
contour in a selected official image and retains the other official views for
comparison. A convex extrusion of an angled photograph is not photogrammetry.
Unless a plan records an explicit dimension calibration, absolute scale,
thickness, hidden geometry and the physical display location remain unknown.
The longest projected side uses 60 mm solely as a consistent debugging layout.
Internal PCB placeholders say `UNKNOWN`; no fictitious chips or traces are added.

The image review found and corrected accessory selection and dark-background
failures. `tools/lilygo-photo-outline-overrides.json` records those decisions;
all original photos remain unchanged. When a display location is uncalibrated,
the real framebuffer appears in a separately labeled `LIVE DEBUG` panel.

Rebuild emitted geometry offline with:

```sh
python3 tools/build-lilygo-expansion-models.py --output /tmp/lilygo-models
node tools/verify-lilygo-expansion-models.mjs
```

The verification loads every expansion asset with the production declarative
model loader, checks retained source hashes and native framebuffer aspect, and
rejects non-finite geometry. Passing it does not establish physical accuracy.

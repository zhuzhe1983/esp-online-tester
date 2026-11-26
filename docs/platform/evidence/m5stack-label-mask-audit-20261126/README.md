# M5Stack rear-label intersections and StampFly texture footprint

This five-product batch corrects two reproducible renderer/model defects without changing runtime compatibility:

| Product | Defect | Before | After |
| --- | --- | --- | --- |
| AtomS3R C126 | Rear paper alpha overlaps elevated female-header frames | 1,251 opaque source texels penetrate by 0.125 mm | 0 penetrating texels |
| Atom VoiceS3R C126-Echo | Same projection/geometry overlap | 2,185 texels | 0 texels |
| Atom Voice C008-C | Same projection/geometry overlap | 2,121 texels | 0 texels |
| Atom Matrix C008-B | Same projection/geometry overlap | 1,035 texels | 0 texels |
| StampFly K138 | Selected PCB triangles extend beyond the source-photo XY footprint | 14 of 143 triangles produce out-of-range UVs | 0 out-of-range UVs; complete native PCB retained |

The four masks exclude each model's independently represented header-housing footprint plus 0.05 mm display clearance. Paper size, placement, photo RGB bytes, source photos and geometry bytes stay unchanged. The rectangles are explicit in `model-plan.json`, use original photo pixels and preserve the pinned source hashes. This resolves overlap in the current model; the existing estimated header dimensions are not promoted to measured facts.

The texture renderer partitions crossing triangles at the original XY footprint. New vertices interpolate source position/normal attributes. Outside fragments retain ordinary material; decal bases remain complete. It preserves the photograph's scale and the documented back-face X mirror. Tests cover crossings, boundaries, disjoint triangles, sloping geometry, front/back UVs, both material modes and the actual K138 model.

`receipt.json` binds final metadata, plans, source and texture hashes. Surface checks evaluate every source texel with alpha ≥128 against the assembled physical triangle geometry. Magenta pixels in `*-before.png` identify penetration, and `*-after.png` contains the corrected alpha. These are geometric checks, not browser screenshots or manufacturing-fit acceptance.

Reproduce the mask bytes and geometry checks:

```sh
python tools/fix-m5-atom-label-masks.py
node tools/verify-m5-label-surfaces.mjs /tmp/m5-label-surfaces
python tools/raster-m5-label-support.py /tmp/m5-label-surfaces
node tools/platform-projected-texture-test.mjs
node tools/platform-manufacturer-model-test.mjs
node tools/platform-m5stack-label-material-test.mjs
```

The four PNGs also reproduce through their ordinary public model plans. The tested generic C126 rebuild matched the new texture and pinned inputs, but its pre-existing geometry bytes differed under this local dependency environment (`generic-rebuild-c126.json`). This batch keeps the original geometry. Some paper regions still span a model-space gap; those regions need source-photo/assembly review and were not filled with invented support. Firmware, physical fit and final browser appearance remain separate acceptance items.

Actual models were also checked with original-like, gold and translucent-purple shell presets: all non-shell part colors, opacity and texture identities stay unchanged. Cloud-browser Chrome could not create a WebGL context, so real 3D/oblique appearance remains unverified; these checks do not substitute rendered acceptance.

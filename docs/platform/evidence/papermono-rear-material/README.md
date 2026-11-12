# PaperMono rear NFC pad material

The original rear photo contains a bright reflection in the lower right of the black NFC pad. Previously the entire photo crop was rendered with `MeshBasicMaterial`, so that reflection remained fixed when the product rotated.

The upper printed label still uses the unchanged original `rear-label.webp` pixels. Its lower boundary is source row 670 of 1034. The lower 364 rows are replaced by a separate rounded, opaque surface with a diffuse texture, roughness 0.92 and metalness 0. The pad is independent of shell color and opacity. Geometry dimensions and the 1.7 mm corner radius are photo-derived presentation approximations; no new electronics geometry is claimed.

The new texture was edited with the built-in ImageGen tool from the original crop. It removes photographic glare and retains an approximate NFC marking. This edited marking, grain and reflectance are visual approximations, not a new manufacturer reference or measured optical data. The dense printed label above it was not generated or modified. The original manufacturer photo and original crop remain in the repository.

## Inputs and reproducibility

- Manufacturer photo `photo-08`: `f5ef27d8704514f83279e843f27bb1a7bfe3d8a7973b5b2054be95674d3bf525`; source and original acquisition receipt remain in `assets/m5stack-papermono/model.json`.
- Original `rear-label.webp`: `69adb7d0e726815500a60d964c4fe05858d421b0d7b072a1eb4280c7bd4977c3`.
- ImageGen output PNG: `ab30e7898e1168d584548e18c0427184b94a9bd435d912fd7f9fd98d2f634597`.
- Final `rear-nfc-albedo.webp`: `86b5d8aacd421d2d0cb693e4bdef4680ce49595294cf859f588b532f5af7b60b`; 620 × 364 pixels, optimized with `cwebp -q 92 -m 6 -resize 620 364`.
- `tools/build-papermono-model.py` verifies and copies the committed edited texture when rebuilding into a separate output directory. It does not regenerate a nondeterministic image. The source STL, exported geometry and original rear photo crop hashes remain unchanged.

Exact ImageGen edit prompt:

> Edit target: the supplied manufacturer rear-label texture for M5Stack PaperMono. Return ONLY the bottom black rectangular NFC pad as a clean orthographic albedo texture, tightly cropped to the black pad, landscape aspect ratio 620:364. Remove ALL baked photographic lighting: especially the large gray/white specular glare at the bottom right, edge reflections, and directional gradients. The entire pad must have an evenly dark near-black charcoal matte surface. Preserve the existing faint dark-gray NFC printed mark on the right half in its exact original proportions and placement, including the three radio arcs on each side, and preserve subtle surface grain. Do not increase contrast of the NFC marking, add markings, or invent other text. No upper white label, no surrounding gray frame, no border, no shadows, no perspective. This is a flat diffuse/albedo map for a 3D renderer; real lighting will be calculated separately. Output a high-fidelity corrected texture of this existing physical panel, not a redesigned product.

## Checks

Headless Chromium checks use the actual `papermono-model-review.html` renderer and fixed photo-review cameras. `checks.json` records the browser, selected texture, material parameters and two pad brightness samples. Captures cover front, back, button side and an oblique rear view with original, gold and transparent purple shell appearances. The upper label and NFC pad remain opaque and retain independent materials in all three appearances.

The sample rectangles are 28 × 28 pixels at (265, 590) and (520, 610) in the 800 × 800 back render. They avoid the NFC printing and assess the fixed lower-right glare. Mean RGB changed from left 11.13 / right 62.08 to left 36.00 / right 34.99. They are display RGB samples, not material reflectance measurements. An upper-label comparison found a maximum difference of one 8-bit channel step from UV sampling; its underlying photo pixels remain unchanged.

A scratch rebuild reproduced `model.json`, `geometry.bin`, `rear-label.webp` and the new albedo byte-for-byte. The required four platform test commands and catalog validation pass. These are appearance checks; firmware/NFC execution and real hardware optics are outside this change.

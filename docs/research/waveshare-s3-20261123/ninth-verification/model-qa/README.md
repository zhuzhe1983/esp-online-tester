# Final twelve-model geometry and source review

This receipt covers the five final S3 storefront families: 1.46B/base/C, 2.8C touch/non-touch, AMOLED 1.91 touch/non-touch with/without fitted headers, 1.85 touch/non-touch, and LCD 1.47. Hardware revision profiles may share the same sourced mechanical reference; that does not establish identical electrical routing.

## Checks actually performed

- Twelve assembled models passed the production Three.js loader, source-image hashes, finite geometry, declared dimensions, screen rays, physical button rays and opposite-face occlusion checks.
- Five representative optical assemblies were expanded and restored through the actual inspection API: 1.46C, Touch 2.8C, Touch AMOLED 1.91-M, Touch 1.85 and LCD 1.47.
- All 29 final plots were inspected: twelve front/reverse/edge/oblique sheets, twelve registered original-photograph overlays and five expanded sheets. The displayed screen is an explicit review swatch, not a firmware framebuffer.
- All 116 asset files listed by the twelve final model manifests and 68 retained source records were independently hash-checked. Compressed original STEP bytes were also checked after decompression. `source-hash-verification.json` records the results.
- `acceptance.json` binds the final model and plan hashes and the actual loader/review source hashes. `commands.json` records successful commands. Temporary mesh dumps were removed after recording their hashes; they can be regenerated from the retained models.

The two existing 2.8C assets were retained byte-for-byte. Their embedded photograph registrations remain in use. The other ten models use separate model/plan/photo hash-bound registrations in `photo-registrations.json`; no native mesh or original photograph is rescaled or edited by this review. A stale hash makes the review tool reject that registration.

## Observations and explicit limits

| Model group | Observed result and remaining boundary |
| --- | --- |
| 1.46 B/base/C | Common PCB outline and rear BOOT/PWR locations align with the labelled manufacturer photograph. Optical variants remain distinct. Native flex-tail bounds extend beyond the board-only annotation; the source geometry is retained. The C cover is a declared optical approximation. |
| 2.8C touch/non-touch | Native rear board, slots, connectors and optical-stack layout match the retained reference at the declared registration. Touch and non-touch outside outlines remain different. Expanded view preserves the optical stack and exposes the PCB without moving soldered connectors independently. |
| AMOLED 1.91 variants | The common PCB and BOOT outline align with the portrait rear photograph after the declared rotation. Fitted-header and bare-hole models remain distinct. The touch optical window is not promoted to a calibrated active area. The common mechanical reference does not prove V1/V2 electrical equivalence. |
| 1.85 variants | Common PCB and rear BOOT/PWR align with the manufacturer reference. Touch and non-touch glass/optical outlines remain distinct. The photographic registration has slightly unequal pixel scales and is not calibrated photogrammetry. |
| LCD 1.47 | PCB and BOOT align with the labelled rear photograph. The native source includes populated headers while the retail photograph shows bare holes; the model remains explicitly a manufacturer assembly reference, with fitted retail population unresolved. Original optical/FPC bounds are preserved alongside the separately sourced active area. |

LCD 1.47 required a substantive identity correction. The original rear-photograph item 4 identifies an ME6217C33M5G regulator, so the former source56 RGB assignment was rejected. The corrected native emitter is original XDE `LED1_ASM` / topology source2, independently mapped by global bounds and the original schematic's LED1 / WS2812B-0807 / GPIO38 net. See `../ninth-lcd147-led-xde.json` and `rejected-lcd147-led-binding.json`. Source56 remains ordinary native geometry. The real emitter is on the opposite PCB face within the optical stack; the rear-photo overlay therefore excludes it. The expanded sheet shows its source-preserved position. Acrylic light guiding and assembled glow are not simulated.

The first button-ray attempt also exposed incorrect control review normals. Those normals were corrected to the actual rear-facing switch surfaces; the visibility and opposite-face occlusion assertions were retained. All final loader checks pass with these corrections.

## Reproduce one model

Run from the repository root, replacing the model ID when needed. Use a fresh output directory so earlier evidence is preserved.

```sh
node tools/check-waveshare-model-loading.mjs waveshare-esp32-s3-lcd-1-47 --output /tmp/waveshare-review/loader.json --dump /tmp/waveshare-review/meshes
python tools/review-waveshare-models.py --meshes /tmp/waveshare-review/meshes --output /tmp/waveshare-review/plots --registrations docs/research/waveshare-s3-20261123/ninth-verification/model-qa/photo-registrations.json
node tools/check-waveshare-model-loading.mjs waveshare-esp32-s3-lcd-1-47 --expanded --output /tmp/waveshare-expanded/loader.json --dump /tmp/waveshare-expanded/meshes
python tools/review-waveshare-models.py --meshes /tmp/waveshare-expanded/meshes --output /tmp/waveshare-expanded/plots --registrations docs/research/waveshare-s3-20261123/ninth-verification/model-qa/photo-registrations.json
```

These are geometry/source checks under Node and an orthographic software renderer. They do not claim real browser/WebGL material rendering, DOM legibility, calibrated optical color, physical fit, connector mating, firmware UI acceptance or hardware testing. Native firmware runs and their failures are recorded separately in the surrounding verification directory.

Only transient workspace paths in retained commands and logs are normalized. Original photographs, measured numbers, final geometry and source hashes are unchanged.

# C5 LCD 2.73 — dimension and photo reconstruction

This batch adds `waveshare-esp32-c5-lcd-2-73` using the exact official dimensional image and front, rear and oblique product photographs. The source search includes the retained canonical overview and Resources pages, the official shop page and the complete, untruncated 2,326-entry repository tree at commit `418de818f4f40c5a04c4292dbe293bfaa1351068`. No matching mechanical file was found within those sources. The overview refers to a drawing in a resource package, but the retained Resources page and fixed tree expose only a schematic PDF; that reference remains unresolved. This is a bounded search result, not a claim that no CAD exists elsewhere.

## Geometry and uncertainty

SKU **35224** includes a separate lithium battery; **35225 / -EN** omits it. Both are non-touch. The model represents the published board/display assembly with the photographed standoff envelopes. It does not invent battery or cable placement.

| Feature | Source-supported geometry | Remaining uncertainty |
| --- | --- | --- |
| Front cover | 68 × 68 mm | Thickness and corner radius are estimates |
| LCD active area | **48.96 × 48.96 mm**, 320 × 320 pixels | Optical text-to-USB registration is uncalibrated |
| Mounting pattern | 62 × 62 mm pitch, Ø2.20 mm board holes, 3 mm edge offset | Standoff height, bore and threads are unmeasured |
| PCB outline | Nominal 68 × 68 mm, inferred from the dimensioned pitch and edges | Thickness and internal layout are unknown |
| Visible header | Two photographed rows, 32 holes in the visual model | The 2.54 mm pitch and 1.10 mm drill are estimates, without a fit claim |

Every Z dimension is unmeasured. The cover, display body, spacer and PCB use explicit rendering estimates of 0.8, 2.4, 0.8 and 1.6 mm. The model's approximately 9.6 mm depth is consequently **not a documented physical assembly depth**. Raised component envelopes, connector interiors and corner radii are likewise approximate. The internal PCB surface is marked **UNKNOWN** and becomes visible when the inspection view separates the optical stack.

The rear photograph remains unchanged. Its original pixels are projected with the declared uniform 6.5625 pixel/mm registration, clipped to the PCB and its holes, and excluded below raised envelopes. The engineering registration plot compares outlines with those same source pixels; it does not warp or replace the photograph. The live screen is an independent rectangular framebuffer plane. No stock display screenshot is used as runtime output.

## Physical inputs

The source rear photo identifies the actual side switches. The official schematic confirms KEY_L on GPIO2, KEY_R on GPIO3 and BOOT on GPIO28, each ground-closing with a 10 kΩ pull-up. The canonical pin table supplies their user-facing semantics:

| Physical key | Runtime control | GPIO |
| --- | --- | --- |
| KEY_L | `prev` | 2 |
| KEY_R | `next` | 3 |
| BOOT / ENTER | `boot` | 28 |

PWR remains visible and unbound. No RESET button is invented. Source timestamps do not certify a shipping PCB revision or a release date.

## Fresh verification

The actual production Three.js loader verifies all three declared input meshes exactly once, rays onto their visible side faces, opposite-side occlusion, all nine interior screen points, dimensional bounds, unchanged source-image hashes and photo UVs. Expanding the optical stack reveals the UNKNOWN internal surface; restoring it returns every moved object to its exact starting position. The checker now verifies declared button IDs generally, and the existing C5 Touch LCD 1.69 BOOT-only model passes the same check as a compatibility case.

Four views and rear-photo registration were visually inspected. Fourteen retained sources match their recorded original and retained hashes; the schematic also matches its Git blob SHA1 at the fixed commit. A fresh independent build is byte-identical for `model.json`, `geometry.bin.gz` and `build-check.json`. The runtime mesh is 7,973 bytes gzip-compressed. These are source and geometry checks, not browser/WebGL acceptance, a new firmware execution result or a hardware-fit test.

```sh
python tools/build-waveshare-models.py waveshare-esp32-c5-lcd-2-73 --output output/c5-lcd273-rebuild
node tools/check-waveshare-model-loading.mjs waveshare-esp32-c5-lcd-2-73 waveshare-esp32-c5-touch-lcd-1-69 --output output/c5-lcd273-loader.json --dump output/c5-lcd273-meshes
python tools/review-waveshare-models.py --meshes output/c5-lcd273-meshes --output output/c5-lcd273-review
```

`integration.json` supplies the independent central mapping. All source identities and build parameters are retained with the asset. No central catalog or runtime registration is changed by this lane, and no lost pre-reset acceptance result is reused.

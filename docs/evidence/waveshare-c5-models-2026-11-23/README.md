# C5 screen models — new source acquisition, 2026-11-23

This batch represents two exact board models with new, retained manufacturer sources. It does not reinstate a count from the workspace that was lost. Central product registrations are supplied separately in `integration.json`.

## Mechanical scope

| Model | Represented native topology | Physical optical window | Scope |
|---|---:|---|---|
| ESP32-C5-LCD-1.47, SKU34961 | All 310 components, including one open shell | 17.39 × 32.35 mm, R2.30, 172 × 320 pixels | Base board without soldered headers. PCB nominal 20.51 × 36.51 mm; source thickness 0.8 mm. The additional headers of SKU34960 (-M) are not represented. |
| ESP32-C5-Touch-LCD-3.5, SKU35417 | 810 of 812 components; every omitted source index recorded | 48.96 × 73.44 mm rectangle, 320 × 480 pixels | Internal board/display assembly. PCB 54.5 × 78 × 1.6 mm and cover 61 × 92.44 mm. Shipping black enclosure and battery are not reconstructed. |

The LCD1.47 physical dimensions have a 0.0108% aspect difference from the exact pixel ratio at the published precision. The model preserves the measured millimetres and records that small tolerance; it does not distort the CAD to force a pixel ratio. Its active native face has an area of 558.02552513749 mm², equal to the independently specified rounded rectangle. The production screen geometry uses normalized rectangle UVs and clips only the rounded corners.

The Touch3.5 optical window is rectangular. Its rounded *outer cover* is not a reason to round framebuffer corners. The screen overlay sits 0.008 mm above the corresponding native active face to avoid coplanar rendering. That offset is a visualization choice, not a laminate measurement.

## Exact official sources

- [LCD1.47 fixed STEP](https://raw.githubusercontent.com/waveshareteam/esp32-c5-lcd-1.47/acf81d83f8cc580fb8bc4f3f13e7332ab6b1332d/hardware/dimensions/esp32_c5_lcd_1_47_3d_drawing.stp): 13,557,728 bytes, SHA256 `055a3c80563102ee9722c87ff1ffebd18861744fd185508133ebfe4a7592b17f`. Git blob identity `81fff9cbc2dcea386ef9d161c6002a6152500c7e` was independently checked. CAD authoring timestamp 2026-06-22.
- [LCD1.47 fixed dimension PDF](https://raw.githubusercontent.com/waveshareteam/esp32-c5-lcd-1.47/acf81d83f8cc580fb8bc4f3f13e7332ab6b1332d/hardware/dimensions/esp32_c5_lcd_1_47_drawing.pdf): 61,766 bytes, SHA256 `1677e76c03aab50461c2c715278fdcac24b0818800148e5b67c4fd57d937b92d`.
- [Touch3.5 fixed STEP](https://raw.githubusercontent.com/waveshareteam/ESP32-C5-Touch-LCD-3.5/a39067a9b897f54c2b8dccb597542caa4e826349/hardware/mechanical/ESP32-C5-Touch-LCD-3.5-STP.stp): 28,756,600 bytes, SHA256 `24075274eb1ffe89043537a6e14acb9f84d0df09f3936d8280982c7426da147b`. Git blob identity `6950dd952da76a2bfae07859fc8a11b374aa2fc4` was independently checked. CAD authored 2026-11-05 and added to the pinned repository commit on 2026-11-23.
- [LCD1.47 canonical documentation](https://docs.waveshare.com/ESP32-C5-LCD-1.47) and [exact base shop listing](https://www.waveshare.com/esp32-c5-lcd-1.47.htm?sku=34961).
- [Touch3.5 canonical documentation](https://docs.waveshare.com/ESP32-C5-Touch-LCD-3.5) and [official shop option table](https://www.waveshare.com/esp32-c5-touch-lcd-3.5.htm).

The original sources, raw HTTP receipts, source dates, and exact unchanged photographs are retained in each model directory. The GitHub connector directory/tree responses are explicitly labelled connector JSON captures; their hashes are not presented as hashes of original HTTP response bytes. Source dates are not product release-date assertions. Shared mechanical names mentioning S3 or P4 remain native source names inside explicitly identified C5 assemblies.

## Camera packaging correction

The shop table explicitly distinguishes four packages: SKU35417 has a battery and no camera, SKU35416 (-EN) omits both, SKU35419 (-C) includes a camera and battery, and SKU35418 (-C-EN) includes a camera without battery. The dimension photograph is labelled -C and records a 63.67 × 95.11 mm housing, 13.50 mm body depth and 14.10 mm including feet. Those housing dimensions are stored as a separate reference; they are not silently assigned to this base internal model.

The initial complete CAD export exposed a rear lens cylinder with a long flex on component 801, `AS-BD108A93D13-50`. The supplied CAD therefore contains an optional camera-shaped attachment that cannot be represented as part of the explicitly no-camera base SKU. The final base omits index 801 and adjacent central foam index 806 (`EVA泡棉`), whose no-camera applicability is unverified. The original STEP is unchanged and complete. The geometry and photographs do not establish the camera's chip/BOM number. `negative-camera-scope-four-views.png` preserves the actual earlier review that revealed the scope mismatch; it is not the final model.

## Geometry and interaction checks

The rebuild preserves native millimetres and assembly transforms. All large parts remain separate. Small Touch3.5 components sharing the same source display color are concatenated into draw groups, with every selected source index and name retained. Each component is independently tessellated and bounds-checked before grouping. Duplicate selections, incomplete coverage, and source identity mismatches reject the build. Grouping drops no triangles.

The production Three.js loader checks are CPU geometry checks: finite coordinates, dimensions, normalized screen UVs, interior screen rays, clipped-corner misses, source-part BOOT side surfaces, opposite-face occlusion, and exact inspection expand/restore. BOOT is attached to the physically labelled side key on each board. RESET stays geometry only. The pinned [Touch3.5 BSP control header](https://raw.githubusercontent.com/waveshareteam/ESP32-C5-Touch-LCD-3.5/a39067a9b897f54c2b8dccb597542caa4e826349/examples/factory/components/esp32_c5_touch_lcd_3_5/include/bsp/esp32_c5_touch_lcd_3_5.h), retained with its original receipt here, identifies BOOT28 and PWR through EXIO6. Touch3.5 PWR remains unbound until its EXIO6 monitoring semantics are separately available; it is not treated as an ordinary GPIO button. LCD1.47's real WS2812 package is beneath the optical stack and is naturally hidden until that stack is separated.

The previous seven models also passed the production-loader checks after the new rounded-screen path was added. Independent tests compare old rectangle/circle geometry arrays exactly and check rounded-screen bounds, ray-interpolated UVs, clipped corners and area. Engineering front/rear/edge/oblique plots and unchanged-photo registrations were reviewed. Bare-PCB photo overlays intentionally exclude optical stacks and small merged groups that the photographed bare-board view cannot verify.

These checks do **not** claim a browser/WebGL render, physical fit, production firmware acceptance, or text orientation calibrated against a real unit. Native firmware validation is maintained by the separate runtime lane. Final measurements, hashes and plot receipts are in `review-receipt.json` and `loader-check.json`.

## Rebuild and review

```bash
python tools/build-waveshare-models.py \
  waveshare-esp32-c5-lcd-1-47 waveshare-esp32-c5-touch-lcd-3-5 \
  --output output/waveshare-c5-models/rebuilt
node tools/check-manufacturer-display-geometry.mjs
node tools/check-waveshare-model-loading.mjs \
  waveshare-esp32-c5-lcd-1-47 waveshare-esp32-c5-touch-lcd-3-5 \
  --output output/waveshare-c5-models/loader-check.json \
  --dump output/waveshare-c5-models/meshes
python tools/review-waveshare-models.py \
  --meshes output/waveshare-c5-models/meshes \
  --output output/waveshare-c5-models/review
```

Only the referenced compressed geometry and any actual runtime textures are needed by the viewer. Source CAD, HTML, PDF and reference photographs are retained for audit and rebuilding; this batch does not use them as live textures.

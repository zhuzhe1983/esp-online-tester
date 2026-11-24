# C5 Touch LCD 1.69 model — source and verification boundary

This batch adds one fresh model for `waveshare-esp32-c5-touch-lcd-1-69`, the documented bare board and display associated with SKU **35471**. The official package includes a separate lithium battery; **35356 / -EN** omits it. Neither battery placement nor an external speaker has been reconstructed.

The model uses official dimensions and unchanged front, rear, oblique and side photographs. It is a **dimension/photo reconstruction**, not manufacturer CAD. The complete official repository tree at commit `136f2a8c68b3808ce4a9dbea218e9c3a4928212f` contains 156 entries and is not truncated. None has the mechanical extensions recorded in `model-plan.json`; the inspected canonical Resources page also has no CAD link. That bounded search does not establish that CAD is unavailable elsewhere. The raw HTTP documents and the connector-derived tree capture retain distinct identities and capture timestamps.

## What is dimensioned, and what remains estimated

| Feature | Official evidence | Rendering decision |
| --- | --- | --- |
| Cover XY | Both dimension images: 41.13 × 33.13 mm in the landscape drawing | Rigid portrait arrangement, 33.13 × 41.13 mm |
| Active area | Both images: 32.63 × 27.97 mm | 27.97 × 32.63 mm, normalized 240 × 280 framebuffer UVs |
| PCB XY | Shop: 37.13 × 29.84 mm; docs: 37.12 × 29.83 mm | Shop nominal XY retained; both values and original images remain available |
| Side reference | Shop: 6.70 mm; docs: 6.40 mm | Explicitly chosen 6.55 mm midpoint for the front-to-rear reference planes; this is not a measured total assembly depth |
| Front optical stack | Shop: 3.80 mm; docs: 3.82 ± 0.2 mm | Approximate 3.80 mm stack |
| PCB thickness and package heights | Not dimensioned | Clearly marked photo-informed estimates; internal PCB face and stack are `UNKNOWN` |
| Rounded corners | Photographically visible, no matching overall radius annotation | Estimated cover R6.4, active area R4.3 and PCB R4.0 mm, each with an approximately ±0.8 mm visual uncertainty |

The shop image's **4 × R1.00** annotation is not repurposed as the overall PCB or display radius: the exact local feature has not been certified. The glass edge is simplified as a constant-depth rounded plate. Rear package envelopes have recorded pixel boxes and uncertain heights; the original photograph supplies their appearance. Small photographed passives do not imply a reconstructed package height, pinout or circuit revision.

The rear photograph uses one uniform scale, approximately 10 pixels/mm, registered to the documented cover outline. Image right maps to model −X, so the USB connector is on the model's left when looking at the front. **Text-to-USB optical alignment has not been physically calibrated.** The official front screenshot is reference material only and is never used as a runtime screen texture.

## Inputs and visibility

Only the photographed **BOOT** actuator is bound, using runtime control `boot` / GPIO28. RESET remains geometry, and PWR remains geometry because its signal goes through TCA EX4 rather than an ordinary GPIO key.

The inner PCB face carries `UNKNOWN`. In the assembled model it is hidden by the display. The inspection layout moves the display and optical stack together to expose that face; it is a debugging view, not a documented disassembly sequence. The production loader check verifies that the label is obscured while assembled, visible when expanded, and obscured again after restoration.

## Fresh verification

`loader-check.json` records the actual production Three.js loader, including dimensional bounds, nine interior display rays, four excluded rounded corners, visible BOOT side-face selection, opposite-side occlusion, finite UVs, unchanged image hashes and exact inspection restoration. The Node image stub does not decode pixels; the retained originals were separately opened for visual inspection.

The engineering four-view plot and rear-photo registration were visually reviewed. They show geometry and a deliberately plain screen swatch, not firmware execution or WebGL material acceptance. A review-only bug that printed hidden `UNKNOWN` annotations over the screen was corrected to respect the depth buffer. `check-waveshare-review-occlusion.py` checks a hidden label, an exposed label and an off-image label.

This batch does not claim browser interaction, hardware fit, a measured PCB thickness, or a new runtime acceptance. The separately implemented C5 runtime is not re-certified by this model change. No pre-reset model or lost historical test log is reused.

## Reproduce

```sh
python tools/build-waveshare-models.py waveshare-esp32-c5-touch-lcd-1-69 --output output/c5-touch169-rebuild
node tools/check-waveshare-model-loading.mjs waveshare-esp32-c5-touch-lcd-1-69 --output output/c5-touch169-loader.json --dump output/c5-touch169-meshes
python tools/check-waveshare-review-occlusion.py
python tools/review-waveshare-models.py --meshes output/c5-touch169-meshes --output output/c5-touch169-review
```

Source files, all model parameters and the generator identity are retained under `web/platform/assets/waveshare-esp32-c5-touch-lcd-1-69/`. `integration.json` supplies the independent central model mapping without changing the catalog in this lane.

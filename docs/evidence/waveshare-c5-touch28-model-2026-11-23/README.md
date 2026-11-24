# C5 Touch LCD 2.8 — official CAD model

This fresh batch adds `waveshare-esp32-c5-touch-lcd-2-8` from the exact official STEP at commit `0196dcb7e601d6faa1c33785a71f6b0c35b532e6`. The complete fixed tree contains 2,328 entries and is not truncated. Both the STEP and mechanical PDF were obtained from `hardware/mechanical/`; their original Git blob SHA1, byte lengths and SHA256 were checked. The current canonical Resources page still links a different, older directory path, so the source record preserves the verified fixed-tree path.

The source contains **728 components: 723 solids and five open shells**. Every component is retained. The builder combines small components with the same display material into 101 draw groups, while tessellating and checking each original component independently. No geometry is scaled, simplified into boxes, removed or moved from its native assembly position. The source STEP remains complete in a lossless gzip archive.

## Exact model scope

SKU **34957** is the board/display package with a separate lithium battery; **34958 / -EN** omits the battery. The CAD describes the bare board and display assembly, including the native sockets, module, flex and standoffs. It does not include a separate enclosure, battery, cable or external speaker. Those accessories are not synthesized.

| Feature | Verified dimension or source geometry |
| --- | --- |
| Complete native assembly | 50.54 × 73.06 × approximately 10.20 mm |
| PDF nominal PCB | 49.90 × 69.00 × 1.60 mm |
| Native PCB bounds | 49.913581 × 69.011087 × 1.600000 mm; preserved without rescaling |
| LCD active area | **43.20 × 57.60 mm**, rectangular, 240 × 320 pixels |
| Touch viewing area | 43.80 × 58.20 mm, distinct from the active area |
| Cover outer dimensions | 50.54 × 73.06 mm, R2 outer corners |
| Front cover reference | 0.70 mm; the full source component also includes folded flex |

The native assembly is already portrait. The LCD active face is component 1, face 32, centered at XY zero with Z = −0.60 mm. The front touch viewing face is component 0, face 11, at Z = 0.65 mm. The live framebuffer keeps the LCD's active dimensions and is projected onto that viewing face with a 0.008 mm rendering offset. The raised cover rim remains at Z = 0.70 mm. The folded flex explains why the complete touch component has a 7.00 mm depth; that number is not interpreted as glass thickness.

The STEP's module shield has a reused mechanical-library name, `ESP32-WROOM-32E_SHIELDING_CASE_`. It remains the supplied shape within this explicitly named C5 assembly. That label is not treated as evidence of a classic ESP32 circuit or a substitute for electrical documentation.

## Input and appearance boundary

The upper side switch, native component **9**, is **BOOT**, mapped to `boot` / GPIO28. Component 8 is RESET and component 712 is PWR; both remain visible source geometry without new runtime input bindings. PWR belongs to the CH32/power circuit and is not relabeled as an ordinary GPIO button.

Native material colors are retained except for the dark cover and black PCB, selected from the exact official photographs. The photographs are unchanged references. No stock screenshot is a runtime display texture. No optical text-to-USB calibration, physical fit test or certification of a shipping PCB revision is claimed.

## Fresh checks

The production Three.js loader verifies physical dimensions, all nine interior screen rays, BOOT's actual side face, opposite-side occlusion and exact inspection expansion/restoration. The source reader preserves open shells, and the independent native bounding checks cover every original component. `loader-check.json` and `review-receipt.json` record the actual results.

The engineering four-view plot and rear-photo registration were visually inspected. The latter compares native coordinates to the unmodified rear photo in its original landscape orientation. The affine registration rotates the comparison coordinates; it does not rotate or stretch the source photograph or CAD. These are geometry and source checks, not browser/WebGL acceptance, new firmware validation or hardware-fit certification.

## Reproduce

```sh
python tools/build-waveshare-models.py waveshare-esp32-c5-touch-lcd-2-8 --output output/c5-touch28-rebuild
node tools/check-waveshare-model-loading.mjs waveshare-esp32-c5-touch-lcd-2-8 --output output/c5-touch28-loader.json --dump output/c5-touch28-meshes
python tools/review-waveshare-models.py --meshes output/c5-touch28-meshes --output output/c5-touch28-review
```

All source identities and model parameters are retained under `web/platform/assets/waveshare-esp32-c5-touch-lcd-2-8/`. `integration.json` supplies the independent central mapping. No catalog or central runtime registration is modified by this lane, and no lost pre-reset acceptance result is reused.

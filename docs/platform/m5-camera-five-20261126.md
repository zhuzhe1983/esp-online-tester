# Five missing M5Stack camera controllers

The catalog adds five independent SKU records, source-backed model previews, hardware knowledge, exact firmware references and update investigations. Each keeps `runtime.board: null` / `status: unavailable`; the workbench and mini retain the red **尚未完成适配** boundary and hide Run. No GC0308 runtime or another product's firmware verification is borrowed.

| SKU | Correct product identity | Current documented camera | Model source |
| --- | --- | --- | --- |
| U121 | Unit PoE CAM | OV2640 2 MP | Explicitly shared U121-B-V11 exterior CAD; own photos/print and estimated OV2640 lens |
| U121-B | Unit PoE CAM-W | OV2640 2 MP | Same declared exterior reuse; own Wi-Fi-model references |
| U121-B-V11 | Unit PoE CAM-W v1.1 | OV3660 3 MP | Exact official original assembly STL |
| U174 | Unit CamS3 | OV2640 2 MP | Explicitly shared U174-B enclosure CAD; own gray finish, optics/markings |
| U174-B | Unit CamS3-5MP | PY260 5 MP | Exact official original assembly STL; separate 5MP source branch |

Timer Camera belongs to U082 / U082-X / U082-F. U121 names were corrected before modeling. Current U174-B documentation identifies PY260 and explicitly says ordinary Unit CamS3 programs are incompatible; this does not assert the camera fitted to every historical production batch. U121's store title says “W/O Wi-Fi” while the current hardware table also describes the ESP32 radio; the conflicting evidence is retained.

Both official STLs are pinned to M5_Hardware `18de3bb3087ea617c38642ba3eedd10dc9f17cbf`. Original STL faces are selected by stable ranges with one common rigid transform. Distant print copies and optional mounting accessories are omitted explicitly. Whole source layouts are never scaled to fit. The three reused enclosures have independent SKU dimensions and photos, but their correspondence remains an exterior approximation rather than a target PCB/CAD claim.

The physical native-plus-derived solid bounds are approximately 24.0417 × 64.0140 × 18 mm for PoE models and 24.0559 × 40.0559 × 11 mm for CamS3. Independent nominal body comparisons use a declared 0.1 mm display allowance, while source-to-export errors stay near floating-point precision. This allowance is not a manufacturing tolerance. Every model retains an UNKNOWN mainboard, open connector cavities, independent paper/metal surfaces and cover inspection groups. PCB, sockets, shield and SD cage stay together when covers separate.

CamS3 paper, RF-shield printing and SD-cage legends have separate source masks/carriers. A rejected flat 5MP badge intersected the carrier shoulder by up to 0.095 mm. Its final photo uses the native front/carrier faces, retaining source XY scale and RGB/alpha. All final independent-label opaque texels were checked for penetration; positive support gaps remain documented and were not filled with invented plastic.

U174's official UserDemo project and public v0.4 identity are fixed without inventing a publisher SHA-256. U174-B keeps its exact PY260 factory/source references and does not inherit that image. The official v2.2.0 PoE CAM Web Camera application is mapped only to the three PoE SKUs named in the manufacturer guide; its application role stays separate from their unpinned factory-image references. The full public camera category and exact documentation/source links were checked for UIFlow and Xiaozhi; no matching official versions were established. Third-party firmware bodies are not website assets.

The [evidence directory](evidence/m5-camera-five-20261126/receipt.json) binds final model/plan/geometry/source/texture hashes, complete assembled bounds, three shell-preset material checks, exact inspection resets and oblique geometric sight lines to UNKNOWN. All five public offline model rebuilds passed. Cloud Chrome cannot create a WebGL context, so rendered original/gold/translucent, side-angle photo overlays, readable glyph appearance and real hardware/firmware acceptance remain unverified.

Reproduce the checks:

```sh
python tools/validate-platform-catalog.py
node tools/platform-knowledge-test.mjs
node tools/verify-m5-camera-five.mjs /tmp/m5-camera-check.json
node tools/verify-m5-camera-label-surfaces.mjs /tmp/m5-camera-labels
python tools/raster-m5-label-support.py /tmp/m5-camera-labels
python tools/rebuild-product-model.py plan m5stack-u174-b
python tools/rebuild-product-model.py build m5stack-u174-b --scratch /tmp/m5-camera-rebuild
```

Replace the final product ID with each of the other four SKUs and use a fresh output directory. Manufacturer CAD retains its MIT license; original photographs and derived visible-print textures remain M5Stack copyright and are not relicensed as project MIT.

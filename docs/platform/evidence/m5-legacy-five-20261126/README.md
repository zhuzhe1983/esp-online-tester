# Five missing legacy M5Stack controller SKUs

The exact manufacturer SKU pages were fetched on 2026-11-26; redirects fix these identities. An ESP32 camera's image resolution is not treated as a physical LCD. These are five independent product IDs, not aliases of a newer revision.

| SKU | Physical identity | Model | Runtime boundary |
| --- | --- | --- | --- |
| K007-V26 | FIRE v2.6, ESP32-D0WDQ6, 16 MiB Flash / **8 MiB PSRAM**, MPU6886, 320×240 ILI9342C | Same-SKU photographs and 54×54×28.6 mm host envelope; authored exterior approximation | Existing Core host subset uses the documented identical display/key/SD/backlight pins. RGB/IMU/microphone/PMIC and official factory execution are not implied |
| K049-C | Paper **COMM Edition**, ESP32-D0WDQ6-V3, 16/8 MiB, IT8951E 540×960, GT911, 1150 mAh | Independent 67×118.6×10 mm exterior and alpha-masked rear paper from COMM photographs; no v1.1 CAD identity inherited | Unavailable: EPD, touch, rotary switch and power still require a dedicated board model |
| U017-PCBA | ESP32-CAM PSRAM **OV2640 / 150°**, 4/4 MiB, bare PCB | Photograph-corrected visible front, separate estimated lens, FFC and USB; unknown reverse marked UNKNOWN | Unavailable; no camera or Wi-Fi capture behavior invented |
| U038 | **M5Camera-X**, OV2640 / 65°, 4/4 MiB | Original fixed-commit case components 1+3; loose clips/print duplicates excluded; lens and internal board-space estimated | Unavailable; BME280/MPU6050/PDM footprints are explicitly **unpopulated**, not installed sensors |
| U082-F | **TimerCamera-F**, OV3660 / 120°, 4/8 MiB, BM8563 and 270 mAh battery | Original fixed-commit case components 0+3, with native relative placement; estimated optical lens | Unavailable; power button is not falsely mapped to the GPIO33 HOLD output; ordinary TimerCamera firmware identity is not inherited |

U038 documentation says 24×48×15 mm while its official store says 24×48×13 mm. Its native case is 13.2 mm deep; the estimated lens brings this preview to 14.9 mm. The disagreement is retained, with separate official and CAD-measurement facts, rather than stretching the model. U017-PCBA's page likewise reports SRAM as 520 KB in its description and 500 KB in its table; both statements remain recorded.

The two manufacturer STLs are pinned to `m5stack/M5_Hardware@18de3bb3087ea617c38642ba3eedd10dc9f17cbf`. Their selected case triangles are compared independently to the original binary STL, including the declared rigid transform and all six possible vertex orders. The independent check does not reuse the CAD loader's component decomposition and does not validate the estimated lenses, unknown boards or manufacturing dimensions.

The three other models are explicitly photograph-informed approximations. They retain three original photographs each. The FIRE model includes the host only; separately pictured charging stand, USB cable and LEGO pieces are omitted. Paper's manually recorded perspective transform and concave alpha polygon isolate actual white paper; green plastic, ports and holes are excluded without shrinking the printed image. U017-PCBA's board plane excludes the separate lens, USB and FFC footprints and keeps its unknown reverse identifiable. Estimated dimensions, missing ports/fasteners and untested grazing-angle/GPU appearance remain listed in each plan.

Every physical microSD slot is explicitly declared, so existing default-card policy applies. Camera connector signal inventories deliberately omit guessed wire colors and model-local pin1 order. No electrical connector routing is asserted.

Factory investigation is in `firmware-survey.json`. M5Camera-X's exact official directory contains bootloader, partition table, application and `flash.sh`; a local **metadata-only** manifest links the three original files at commit `7be1a13123ef8bb35143eb0b71542b64a3c6e4fb` and preserves offsets 0x1000/0x8000/0x10000. The catalog entry stays `trial`, with the board unavailable. The publisher supplied no SHA-256 for those segments; this batch downloaded no firmware body and asserts no header, partition-content, simulator or physical-flash acceptance. The other four retain source/recovery references, including the FishEye factory source for TimerCamera-F. The fetched Burner device inventory's generic Fire entry points at v2.7, and generic Paper does not establish COMM identity; neither was silently reused. No exact official Xiaozhi mapping was found in the inspected sources.

Sources, licenses and timestamps are in `source-receipts.json` and the product knowledge records. Native CAD retains its upstream MIT notice. Manufacturer photographs retain their separate copyright; they are not relabeled as project MIT assets.

Rebuild into an independent directory:

```sh
python tools/build-m5-legacy-five-models.py --output /tmp/m5-legacy-rebuild
python tools/verify-m5-legacy-five-models.py --output /tmp/m5-legacy-verification.json
node tools/platform-m5-legacy-five-test.mjs
python tools/validate-platform-catalog.py
node tools/platform-knowledge-test.mjs
node tools/check-evidence-privacy.mjs docs/platform/evidence/m5-legacy-five-20261126
```

`offline-verification.json` records fresh regeneration of every model and asset byte, independent source-CAD comparison and selected alpha assertions. `mesh-check.json` records actual Node/Three mesh loading, source asset hashes, three visible FIRE button ray hits and material isolation under gold and translucent purple. Image decoding is stubbed in that Node check. These checks do not certify GPU rendering, same-view photo alignment, readability of UNKNOWN in a real browser, official firmware execution or physical hardware. Final asset hashes, tool versions and remaining verification limits are in `verification.json`.

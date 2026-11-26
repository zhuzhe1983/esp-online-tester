# Five exact-SKU M5Stack photo-surface repairs

These changes repair source-confirmed visible surfaces on five existing photo-informed approximations. They preserve every original binary geometry file byte-for-byte. No manufacturing CAD, hidden PCB layout, source-certified connector fit, browser WebGL pass or physical validation is inferred. Existing runtime and factory-firmware receipts remain independent.

| SKU | Source-confirmed change | Unknown facts retained |
| --- | --- | --- |
| U017 M5Camera | Yellow front M5CAMERA title from officialphoto1; rear specification paper from officialphoto3. Independent paper planes follow their existing case/mount groups. | PCB interior, full optical-cover artwork, screws and connector seating stay approximate. No pixels from packaging, gray shell, lens or connectors become paper. |
| U017-PCBA150deg | Rear visible PCB photo plus printed RF shielding on its own existing metal carrier. Hollow rear Grove visual follows the visibleUSB/two-hole end. | UNKNOWN beneath RF is visible when the production module-inspection group moves; component heights, carrier notch and insertion are estimates. Blue source annotation is masked. |
| U007 ESP32 Camera | Rear board photograph rectified from the ruler-side photo. FrontUSB and rearGrove share the two-hole end, correcting the previous opposite-end socket placement. | This is the oldESP32 Camera, not UnitV2. UNKNOWN remains below the front camera package. Rear component heights and generous connector-footprint exclusion remain approximate. |
| U082 bare TimerCamera | Exact rear board appearance with mounting-hole alpha and bounded removal of photographic white backdrop. | UNKNOWN remains under the front camera. Rear components/socket remain flat source appearance, without a socket/electrical-identity claim. Prior coarse board outline is retained, so source side notches are not complete3D geometry. No enclosedX/F geometry is borrowed. |
| K115 Atom-PSRAM Display | Open-cap Atom-PSRAM PCB appearance from photo2; continuous rear support plate and visible holes constrained by photo4. | Dark Atom button cap from photo3 and HDMI mouth are retained. UNKNOWN stays on unmeasured board faces and is visible with the production inspection controller. Rear latch ribs, moldedM5 mark, screw depths and physicalHDMI are still approximate. |

The K115 dark rectangle at negativeY is its actual closed Atom button cap. The HDMI mouth is a separate positiveY connector. The earlier open-board photograph and the closed-cap photograph depict different inspection states; neither is an LCD. The physical product retains `display:null`, and the external FPGA-memory runtime observation is unchanged. The guide's former19×52mm large rectangular opening was a photo-derived ring approximation, contradicted by the solid web in the exact rear photograph. Only that raw ring is omitted from the active part list; its original bytes remain retained in `geometry.bin`. A separate shell-material plate has explicit estimated2mm thickness and photo-constrained slot/circle coordinates. No gray plastic photograph is pasted as a white paper surface.

Every asset folder has `surface-repairs.json`: source file/ID/hash, perspective quadrilateral, output resolution, alpha polygons/circles, source-pixel thresholds where used, target-mm placement, role and uncertainty. All new photograph crops retain M5Stack's source-photo copyright; no independent open photograph license is asserted. The new approximation geometry follows the project license. No image-generation system or new illustrative image was used.

The offline generator copies the pinned, unchanged prior approximation geometry and rebuilds the new visible surfaces/declarative placements. It does not pretend to re-export manufacturer CAD. Use a fresh output directory:

```sh
python tools/rebuild-m5-unit-photo-surfaces.py --output /tmp/m5-unit-photo-rebuild m5stack-u017 m5stack-u017-pcba m5stack-u007 m5stack-u082 m5stack-k115
```

For all five, final model/plan, new texture and original binary bytes reproduce identically. Coordinates/alpha and the earlier failed crop/placement observations are retained in `docs/platform/evidence/m5-unit-surface-repairs-20261126/<product-id>/`. Each SKU has three actually viewed CPU front/back/oblique contact images for original, gold and35%-opacity purple appearances. Bare-board colors intentionally do not change with a shell preset. U017-PCBA andK115 additionally retain actual expanded front/back oblique PNGs: known photos remain on the board/carrier, while UNKNOWN stays visible on unmeasured material. This viewing arrangement does not certify a real teardown trajectory.

Reproduce the independent CPU inspection with the production inspection controller:

```sh
node tools/export-m5-cpu-models.mjs --repo . --out /tmp/m5-unit-open --ids m5stack-u017-pcba,m5stack-k115 --inspection 1
/usr/bin/blender -b --factory-startup --threads 2 --python tools/render-m5-cpu-qa.py -- --repo . --input /tmp/m5-unit-open --out /tmp/m5-unit-open-images --size 640 --samples 16 --back-oblique
```

The existing photo-surface, model-inspection and manufacturer-model suites passed12 tests. Final production-loader exports succeeded for all five. Per-SKU evidence/privacy checks passed. No Cargo, WASM, Worker, catalog runtime, firmware identity or remote deployment was changed by these appearance repairs.

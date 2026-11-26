# Exact-SKU rear paper and carrier review

Each product directory binds its receipt and CPU images to the final model/texture hashes. These are production-loader meshes, UVs, masks and materials rendered by Blender Cycles CPU. They are not WebGL screenshots, calibrated photograph overlays, firmware output or manufacturing acceptance.

The paper remains a separate fixed-color material in original, gold and translucent-purple shell presets. The back, rear-oblique and near-side views review the paper edge, holes, support and transparency. The source-photo polygons, homographies, carrier coordinates, estimated display gap and original native geometry/transform hashes are public in each product's `model-plan.json`.

## Reproduce

Use a new independent directory for each command. No network is needed for the label rebuild:

```sh
python tools/build-m5-rear-labels.py --output /tmp/m5-paper-rebuilt m5stack-k001-v27
node --test tools/platform-m5-rear-labels-test.mjs
node tools/export-m5-cpu-models.mjs --repo . --out /tmp/m5-paper-export --ids m5stack-k001-v27
blender -b -t 2 --python tools/render-m5-cpu-qa.py -- --repo . --input /tmp/m5-paper-export --out /tmp/m5-paper-original --ids m5stack-k001-v27 --size 640 --samples 12 --appearance original --views rear
```

Repeat the renderer with `--appearance gold` and `--appearance purple`, each with a distinct output directory. The production loader substitutes only image decoding and canvas text in Node. Cycles reproduces the documented mesh/material subset; transparent sorting and lighting differ from browser WebGL. UNKNOWN canvas text uses DejaVu Sans Mono. The exporter records its 0.005 mm approximation of negative polygon-offset depth bias.

The label builder regenerates the original-photo textures and label configuration while verifying the fixed original native binary, part records (apart from explicitly declared appearance corrections) and source assembly transform. Byte comparison of the label outputs is separate from a complete source-CAD re-export.

The full generic source-CAD re-export was also attempted for original Basic, Basic v2.7 and Lite. Its byte/hash check remains failed because native vertex/triangle ordering, PNG compression and texture-method records differ. Every native semantic part's triangle positions and normals match after explicit 0.00001 mm / 0.00001 normal-component rounding and canonical ordering; decoded RGBA paper pixels match. This does not turn the failed binary/contract comparison into a pass. Reports retain both results. The original native binaries remain unchanged.

```sh
python tools/rebuild-product-model.py plan m5stack-k001-v27
python tools/rebuild-product-model.py build m5stack-k001-v27 --scratch /tmp/m5-full-rebuild
python tools/compare-native-model-triangles.py web/platform/assets/m5stack-k001-v27 /tmp/m5-full-rebuild/rebuilt/m5stack-k001-v27 --output /tmp/m5-native-comparison.json
```

K001-B uses the explicit family-source builder `tools/build-m5-classic-expansion-models.py`; its regenerated binary, paper texture and model contract match, while additional limitation text and historical metadata make the entire model JSON hash distinct. K005 has its separate kit-layout review under `../m5-faces-layout-review-20261126/`.

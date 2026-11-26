# Exact-SKU rear paper labels

K001-V26 Basic, K002 Gray and K010-V11 Core2 previously had no rear texture in the actual model loader. Each now has a separate `rear-paper` surface sourced from its own retained official rear photograph. The image, source hash, explicit perspective registration, alpha polygons and estimated registration uncertainty remain in `model-plan.json`. Only paper pixels are opaque; molded logos, body surfaces, feet, fasteners and connector gaps are excluded.

The native/procedural binary, existing part records and assembly transforms retain their original hashes. A 0.025 mm rendering gap avoids coplanar depth artifacts. Core2 uses the outer rear-frame plane; the photographed continuous paper bridges the source CAD's recessed center and openings. The production back-cover revision, paper backing and bending remain unverified. These are estimated visual surfaces, without calibrated 1:1 photograph or physical fit acceptance.

`rear-paper` remains fixed in color, opacity and map under original, gold and translucent shell appearances. It moves with its cover during inspection and resets exactly. The source photographs contain the product printing; the LCD remains the live framebuffer surface.

Reproduce into a new independent output directory:

```sh
python tools/build-m5-rear-labels.py --output /tmp/m5-rear-rebuild m5stack-k001-v26 m5stack-k002 m5stack-k010-v11
node --test tools/platform-m5-rear-labels-test.mjs
node tools/export-m5-cpu-models.mjs --repo . --out /tmp/m5-rear-export --ids m5stack-k001-v26,m5stack-k002,m5stack-k010-v11
blender -b -t 2 --python tools/render-m5-cpu-qa.py -- --repo . --input /tmp/m5-rear-export --out /tmp/m5-rear-original --ids m5stack-k001-v26,m5stack-k002,m5stack-k010-v11 --appearance original --size 640 --samples 12
```

Repeat rendering with `--appearance gold` and `--appearance purple` for the two enclosure presets. This independent Cycles CPU render uses the actual production Three loader's meshes, world transforms, textures and alpha tests. Its 0.005 mm approximation for negative GPU polygon offset is recorded by the exporter; this is not a browser GPU acceptance test.

Per-SKU evidence, exact source attribution and three rear closeups are under `docs/platform/evidence/m5-rear-paper-20261126/`. All three label-only rebuilds reproduce `model.json`, `rear-paper.png` and their rebuild receipt byte-for-byte. The loader tests pass 3/3. Basic and Gray additionally have zero opaque texel centers outside the unchanged flat carrier projection (152,515 and 152,250 checked at alpha >=128); that projection does not establish physical fit.

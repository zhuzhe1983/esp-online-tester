# Five sourced control-cap mappings

The production loader and `pickVisibleSurface` now resolve all 15 supported buttons on K049-B, K049, K049-C, K048 and K146. These five SKUs are an incremental control closure outside the separate 35+1 visual-model denominator. No factory firmware acceptance is added.

| SKU | Actual selectable surfaces | Visible change |
| --- | --- | --- |
| K049-B | One photo-estimated black dial partitioned into GPIO37/38/39 regions | New separate dial cap; original native geometry unchanged |
| K049 | Its own exact-SKU photos constrain the same exterior estimate; its documented family shell remains an approximation | New separate dial cap |
| K049-C | COMM exact-SKU photos constrain a separate black dial | New separate dial cap; COMM shell remains photo approximation |
| K048 | Black dial regions GPIO37/38/39; original CAD top user cap GPIO5 and side power cap GPIO27 | New separate dial only; native user/power surfaces partitioned |
| K146 | Original CAD rear circular cap GPIO21, selected only on exposed native triangles | No visible geometry/material change |

`manifest.json` links every old/new model hash, fixed geometry hash, source-photo receipt, control ray, public rebuild and new CPU image. `picking-and-equivalence.json` uses the actual production loader, full assembled geometry, real PNG alpha, production closest-hit selection and production pointer router. It verifies 15 positive picks, 15 opposite-side occlusions, 14 other non-button regions (including four physical reset locations), 30 press/release events and 15 original/gold/purple comparisons. Every retained original triangle, normal, UV, world transform and material matches the previous model. The added black caps are isolated from shell recoloring. Each of four new dial models adds 276 triangles; K146's entire assembly remains equivalent in all three materials.

The Paper cap is approximately diameter8 mm/depth2 mm at `[30.5,.55,.9]` mm. CoreInk is approximately diameter19 mm/depth2.2 mm at `[12,7.9,3.1]` mm. Dimensions and seating have ±1.5 mm XY and ±0.8 mm Z uncertainty. The three contiguous regions tile one physical dial disk: upper GPIO37, central GPIO38, lower GPIO39. They do not claim three separate buttons, a continuous encoder, tooth profile, hidden switch package or precise axle geometry. Exact source IDs and photo hashes remain in each plan/model.

K146's native circular actuator is centered at `[-16.75,-20.25]` mm. Its conservative selected dome is within radius4.8 mm and Z−3.69 mm. The source-positioned dock covers most of the cap in the default assembly: only the exposed upper native surface can be picked. The opposite side hits an actual occluder. This receipt does not claim the cap can be activated through the dock.

The native geometry is never regenerated during this metadata recipe. An unchanged K049-B plan rebuilt with the current general STL exporter produced different historical triangle ordering and float normal details. To preserve exact original bytes, each plan pins a small gzip of its pre-control metadata and the original native binary SHA. Half-open triangle ranges refer explicitly to that retained binary view, not to current trimesh source-face numbering. Original CAD, photographs, textures, geometry files and historical CPU receipts are unchanged. The three public builder entry points dispatch these five plans to `tools/rebuild-m5-control-models.py`; `public-rebuild.json` records all five exact model/geometry reproductions and three rejected tamper cases.

The five 320px,8-sample Cycles CPU views show the production meshes with a blank framebuffer; they are independent geometry/material proofs, not browser screenshots, calibrated photo overlays, physical measurements or guest output. Historical image hashes are preserved in the manifest. Four SKUs are explicitly `visible-geometry-delta`; only K146 is a metadata/native-partition equivalence. Existing exact-SKU runtime receipts are linked separately and are not rerun or extended by this visual task.

Reproduce from a checkout containing these plans and scripts:

```sh
python tools/rebuild-m5-control-models.py --source-root . --output /tmp/m5-control-rebuilt
python tools/check-m5-control-rebuild.py --source-root . --out /tmp/m5-control-public.json
node tools/check-m5-control-caps.mjs --source-root . --ids m5stack-k049-b,m5stack-k049,m5stack-k049-c,m5stack-k048,m5stack-k146 --out /tmp/m5-control-picking.json
node tools/export-m5-cpu-models.mjs --repo . --ids m5stack-k049-b,m5stack-k049,m5stack-k049-c,m5stack-k048,m5stack-k146 --out /tmp/m5-control-export
blender -b -t 2 --python tools/render-m5-cpu-qa.py -- --repo . --input /tmp/m5-control-export --out /tmp/m5-control-cpu --ids m5stack-k049-b,m5stack-k049,m5stack-k049-c,m5stack-k048 --size 320 --samples 8 --only-view oblique --oblique-eye 3,.4,1.5
blender -b -t 2 --python tools/render-m5-cpu-qa.py -- --repo . --input /tmp/m5-control-export --out /tmp/m5-control-cpu146 --ids m5stack-k146 --size 320 --samples 8 --only-view oblique --oblique-eye=-1.7,.5,-3
```

A final limitations-only correction on K049 and K048 follows their CPU render. The CPU source hashes and image bytes are preserved. `manifest.json.metadataOnlyTextAmendments` binds each rendered model to its final metadata hash and records exact actual-loader equivalence of the entire cap-inclusive assembly in original/gold/purple. The amended text reports already evidenced digital display/input/touch/SD-initialization scopes; it does not claim full FAT mounting. Three Paper knowledge notes now cite the final public Worker receipt as repository evidence, with official fetched sources unchanged.

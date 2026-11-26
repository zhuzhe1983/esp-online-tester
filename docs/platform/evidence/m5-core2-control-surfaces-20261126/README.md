# Core2 power control surfaces

Five products: K010, K010-V11, K010-AWS, K010-AWS-V13 and K010-V13.
The baseline production audit found one mapped-but-occluded power target and
four missing power mappings. All 20 supported buttons now pass the same actual
production loader and `pickVisibleSurface` audit, including the unchanged A/B/C
cover keys. The original failure receipt is retained.

Each exact SKU's retained official USB/Grove-side photograph identifies PWR
beside USB-C. The original native front-shell mesh has its molded cap on -X,
center Y approximately 13 mm. K010 and V11's retained separate component-2
insert instead lies at +X/-Y behind the opposite M5-logo wall. The original
K010 power mapping to that insert was removed. Native source geometry, material,
texture pixels, alpha, UVs and transforms are all retained.

Each new zero-opacity input plane covers only a central 2.4 by 4 mm flat area
of the real -X molded cap, 0.01 mm outside the CAD surface. Five actual nearest
ray samples per SKU first identify the original native front-shell surface,
then the new power input plane, with supporting-surface distance below 0.1 mm.
The region does not extend over the recessed upper lip or rounded edges.
Reverse-side rays remain blocked by the assembly; neighboring surface rays and
the historically misbound +X internal insert do not press power. The power hit
plane follows its existing front-enclosure inspection group; actual nearest rays also pass at inspection progress 0.5, 1 and the exact return to 0.

Per-product receipts bind both metadata hashes, complete unchanged artifact
manifests, original/gold/purple actual-loader visible mesh digests, and nearest
ray results. The visible digest includes geometry attributes/indices, normals,
UVs, world matrices, hierarchy paths, scalar/color/vector material parameters,
and texture source hashes/transforms/settings. Zero-opacity input planes are
excluded and separately asserted non-writing. Removing the listed added JSON
members, and explicitly restoring K010's old trailing `controlId`, reconstructs
each original metadata file byte-for-byte using the original Python encoding.
Existing historical CPU render receipts remain bound to their original hashes.

Reproduce from a checkout containing the metadata changes, with unmodified source
artifacts available in `SOURCE` and this contribution in `OVERLAY`:

```sh
node tools/verify-m5-core2-control-surfaces.mjs --source-root "$SOURCE" --overlay-root "$OVERLAY" --baseline 3bc45f158dc857a8aa968c2e7b795c1448831c62 --ids m5stack-k010,m5stack-k010-v11,m5stack-k010-aws,m5stack-k010-aws-v13,m5stack-k010-v13 --out /tmp/core2-control-equivalence.json
node tools/audit-m5-control-picking.mjs --source-root "$SOURCE" --overlay-root "$OVERLAY" --ids m5stack-k010,m5stack-k010-v11,m5stack-k010-aws,m5stack-k010-aws-v13,m5stack-k010-v13 --out /tmp/core2-control-picking.json
```

Limits: this is a conservative central interaction region, not a calibrated
whole-cap mechanical fit. Original photo alpha is decoded for production
picking; Node canvas UNKNOWN text is conservatively opaque. This receipt proves
actual-loader visible equivalence and nearest-surface input selection. It does
not claim a new CPU render, GPU/browser pointer pass, guest firmware execution
or physical product measurement.

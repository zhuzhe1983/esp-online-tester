# Four source-backed open-face appearance corrections

| Exact product | Corrected visible area | Retained unknown area |
| --- | --- | --- |
| C126-Ext | Yellow PROTO face, original photo UVs, 30 geometric through-holes | Hidden host-board envelope and its reverse |
| K129-V11 | Original-photo appearance on native Stamp front triangles and pre-existing approximate metal planes | Hidden host-board reverse; complete header/connector CAD remains unverified |
| K129 | Native black Stamp cover distinguished from the hidden host-board marker; no sibling photo texture | Hidden host-board reverse and unmodeled electronics |
| K125 | Separate exact-SKU white rear paper, explicit source-pixel aperture exclusions, carried by the rear shell during inspection | Both internal board faces |

Original source STL files, `geometry.bin`, manufacturer source receipts, controls
and connector anchors are unchanged. Source photograph hashes and every crop,
projection, corner, mask and placement are recorded in each `model-plan.json`
under `appearanceRebuild`. K129 uses its own oblique photograph only to establish
the black color category; its geometry is not fitted to the photograph.

The original C126-Ext generic rebuild was attempted before changes. Its current
environment's output differed in the native triangle binary and existing paper
PNG hashes; this negative result remains in that product's appearance
receipt. These corrections therefore preserve the retained native binary and
use the independent appearance-only rebuild:

```sh
python tools/rebuild-m5-appearance.py m5stack-c126-ext --output /new/scratch
node tools/verify-m5-open-face-models.mjs \
  m5stack-c126-ext m5stack-k129-v11 m5stack-k129 m5stack-k125
node tools/check-evidence-privacy.mjs docs/platform/evidence/m5-open-face-20261126
```

Appearance rebuilds reproduced all four metadata files byte-for-byte and K125's
new paper PNG byte-for-byte. The production Three loader checks retained hashes,
actual hole rays, UNKNOWN construction, fixed-material isolation and inspection
restoration. K125's paper covers the exposed rear center slot and moves with its
source rear shell. All 30 C126-Ext hole centers miss both the carrier and photo
meshes. The paper's four source hardware samples are alpha-zero or outside the
texture domain.

Each product has original, gold and translucent-purple Cycles CPU renders from
the actual production Three meshes, world transforms, UVs, texture alpha and
materials, at 450×450 with 12 samples. The evidence directory
`docs/platform/evidence/m5-open-face-20261126/<product-id>/` contains the prior
problem view, all nine current front/back/oblique images, a three-material sheet,
render receipts and source/build hashes. No custom shader or unsupported source
material was present in these exports. Printed paper, PROTO and Stamp surfaces
remain independent of shell appearance.

These are source-backed visual corrections, not populated PCB CAD, dimensional
fit certification, calibrated optical materials, browser GPU acceptance or
factory-firmware validation. Estimated carrier depth and placement retain their
stated 0.7–1 mm uncertainty. A source photograph used as appearance is not an
independent verification photograph. Original manufacturer image rights remain
with M5Stack; no new image license is asserted.

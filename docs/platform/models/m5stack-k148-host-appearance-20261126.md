# K148 PowerHub Kit shared-host appearance

K148 is PowerHub Kit. The [official PowerHub page](https://docs.m5stack.com/en/core/PowerHub)
explicitly includes one PowerHub host in both C148 and K148, with one additional
removable NP-F550 2000mAh battery in K148. Applicability of the shared host geometry
is an inference from that Includes table. The fixed page receipt is 160,671 bytes,
SHA-256 `d466583451e4bfa16eff6a047b764ab005ebc85e146185381ed8cc49f77f084c`,
with its original `2026-11-26T06:02:44Z` fetch identity preserved. Legacy `C124-K124`
photograph filenames are source provenance and do not determine the catalog SKU.

| Boundary | K148 handling |
| --- | --- |
| Product identity | Separate K148 catalog and knowledge record |
| Host geometry | Explicit shared `./assets/m5stack-c148/model.json`; no duplicate Kit CAD or wrapper |
| Rear print and pads | Same official PowerHub host rear photo, source ID `product-photo-c8fad993ac`; no Kit-specific marking inferred |
| Kit battery | Included in the official kit; explicitly uninstalled in this preview |
| Battery geometry | Optional source envelope omitted; battery identity, contacts and installation fit unverified |
| Hidden PCB | Supported inward-face UNKNOWN preserved and accessible with production inspection |
| Firmware reference | Existing explicit `firmwareTargetId` remains C148; no execution result inherited |

The shared corrections are in commits `418c98a5` and `16a62ab5`. The final model
SHA-256 is `7a054a3764457209a9a82e98ec979cc6f6467188f664c34ee4dce9cf1215e7c0`.
The native stream remains
`1a83db6a0d3bc223455d88b57fc42212f6b0235456245c0a2f37ea4adede7826`,
manufacturer STL remains
`ae845a13e4b9160249eece2351720bd76f951cc617f63bc765ac7f8a0b8a3ec9`,
and rear-paper texture is
`1ff14b1ec7a245b45724d8e6ff96617bef93c7be7e922e00b64d076466db1f08`.
The shared source reconstruction is retained in `tools/rebuild-powerhub-rear-appearance.py`.

| Independent K148 check | Result and scope |
| --- | --- |
| Source identity | Actual fixed official-page bytes, combined SKU heading and separate C148/K148 Includes entries checked |
| Production loader | K148 and C148 independently instantiated; all 77 meshes, positions, normals, UVs, indices, world matrices and texture hashes match |
| Rear paper | Nearest outward geometric ray hits K148 paper above EXT; actual CPU pixels retain its opaque white paper and colored print |
| Rear pads | Four independent dark pads have outward nearest-ray hits; non-shell materials and maps stay fixed through original, gold and translucent-purple |
| Opaque keyholes | Rear-facing UNKNOWN canvas removed; no partial white letters appear in the final rear views |
| Inspection | 25%, 50%, 100% move paper/pads with the declared rear housing; PCB stays in the electronics layer; exact restoration verified |
| Accessible UNKNOWN | Front-only marker stays on the actual PCB support; inspection=1, direction `[2,3,1]` has nearest marker hits at center/four inset corners and a full-assembly CPU image |
| CPU export | 77 meshes / 22,054 triangles; no unsupported material features; assembled geometry `913df9ec372374c784c4e9b97f5b90c06acba1e69a14c230f479998e153aa4a1` |
| Visible host envelope | 56.00 × 89.15 × 38.90 mm versus generic official 56 × 88 × 38.5 mm; exterior-estimate deviations 0 / +1.15 / +0.40 mm retained |

The Node texture decoder is a stub and Three Raycaster does not evaluate texture
alpha. Image appearance is checked independently using actual production-mesh CPU
renders with UV transforms, alpha, materials and world transforms preserved.
[Original](../evidence/m5stack-k148/original-contact.webp),
[gold](../evidence/m5stack-k148/gold-contact.webp) and
[purple](../evidence/m5stack-k148/purple-contact.webp) each show K148 front/back/oblique
views beside the separate retained official rear photograph. The CPU views were
rendered at 450 pixels, 12 samples, two CPU threads; contact-sheet cells are documented
350-pixel LANCZOS reductions. The [inspection view](../evidence/m5stack-k148/inspection-oblique.webp)
uses the complete production assembly at 650 pixels / 16 samples and its public
inspection control; no component was manually hidden.

The [earlier paper-only stage](../evidence/m5stack-k148/rear-paper-stage/review.json)
is retained as a negative result: its pads inherited shell color and rear-facing
UNKNOWN fragments leaked through keyholes. That stage is explicitly ineligible for
final acceptance. A second retained view-selection failure records a marker corner
occluded by `socket-bottom` from `[2,1,1]`; `[2,3,1]` clears all five sampled points
and has the independent readable image.

```sh
node tools/verify-powerhub-kit-appearance.mjs --source-html /path/to/PowerHub.html
node tools/export-m5-cpu-models.mjs --repo . --out /new/kit-review/export --ids m5stack-k148
blender -b -t 2 --python tools/render-m5-cpu-qa.py -- \
  --repo . --input /new/kit-review/export --out /new/kit-review/original \
  --ids m5stack-k148 --size 450 --samples 12 --appearance original
```

Repeat the render with `gold` and `purple` in matching output directories. The
optional `--source-html` input must match the fixed page hash; without it the verifier
still checks retained source records and every local model artifact. Exporting the
K148 catalog instance is required: renaming a C148 image is not an independent
Kit-entry check. For inspection, the exporter from `caa80ebe` uses `--inspection 1`
and the renderer from `e8877d05` uses `--oblique-eye 2,3,1`; exact immutable tool
hashes, extraction commands and complete reproduction commands are in
[review.json](../evidence/m5stack-k148/review.json). Then curate the views with:

```sh
python tools/compose-powerhub-kit-review.py --repo . \
  --renders-root /new/kit-review --out docs/platform/evidence/m5stack-k148 --inspection
```

This verifies the repaired rear appearance and K148 shared-host loading route.
Pad hidden dimensions, exterior connector details and side printing remain partial;
there is no pose-calibrated photograph overlay or complete product 1:1 certification.
The battery-installed Kit, populated PCB, manufacturing fit, browser GPU, iframe/share
behavior, electrical routing and firmware execution are outside this evidence.
Manufacturer CAD and manufacturer photographs retain distinct rights. Full raw render
hashes, camera parameters, source identities, material scope and retained failures are
in [images.json](../evidence/m5stack-k148/images.json),
[inspection-image.json](../evidence/m5stack-k148/inspection-image.json) and the review receipt.

# M5 supported-control picking audit

This evidence reads the production catalog, real production model loaders, original PNG alpha and production `pickVisibleSurface`. Dedicated PaperMono and StopWatch loaders are selected by exact product ID. Every accepted physical button has a real nearest surface ray; keyboard matrix keys are audited separately.

Batch 01 adds only interaction metadata for five products: BALA FIRE K014-B A/B/C and the exposed Stamp G0 area of K132, K132-V11, K132-Adv and K152. The four Stamp regions are narrow, photograph-aligned portions of each product's own BTN/G0 marking. They are not whole-cover or through-case hotspots. All 81 region gate samples per product land on its existing visible Stamp paper, with at most 0.025 mm clearance. Both outside-region negative rays reject G0.

All seven buttons and all 224 matrix keys pass actual nearest picking. Original, gold and purple assembled visible geometry/material/UV/texture signatures are unchanged before/after. Existing CPU image hashes remain attached to their original model hashes; the registry links them through visual equivalence. This does not assert that every earlier product had all three material renders.

`rebuild-m5-control-metadata.py` independently rebuilds the reviewed control fields from model plans and retained model bytes. Output is byte-identical to the submitted model JSON. It deliberately does not re-tessellate CAD or regenerate images.

Reproduce from the repository containing the final assets:

```sh
node tools/audit-m5-control-picking.mjs --source-root . --ids m5stack-k014-b,m5stack-k132,m5stack-k132-adv,m5stack-k132-v11,m5stack-k152 --visual-signature --negative-region-probes --region-surface-gates --out /tmp/m5-control-rays.json
python tools/rebuild-m5-control-metadata.py --source-root . --plan-root . --out /tmp/m5-control-rebuild --ids m5stack-k014-b,m5stack-k132,m5stack-k132-adv,m5stack-k132-v11,m5stack-k152
```

For an isolated metadata overlay, pass `--overlay-root CHECKOUT` to the audit and `--plan-root CHECKOUT` to the rebuild. Output directories must be independent of the source checkout.

The registry is control-only and contributes zero products to the visual-repair denominator. These receipts establish picking, source boundaries and visual equivalence; they do not establish browser event delivery, firmware execution, measured physical fit or mechanical travel.

Batch 02 handles Capsule K129/K129-V11 G0, VAMeter K136 encoder press, AtomS3R-Ext C126-Ext USER, and the already-correct StickS3 K150. Four products receive interaction-only changes. Eight of nine supported buttons pass nearest picking. K136 G0 remains **missing**, with source-limited spatial mapping explicitly retained in the registry; API support does not count as a 3D pass. The new regions have 69/69/81/69 physical-surface gate samples and at most 0.065mm clearance. K136 outside-region rays still hit the real encoder wheel with its encoder identity. K150 is a cleared center-ray false negative; its existing btnB presentation region is unchanged and excluded from the new-region gate acceptance.

Virtual touchscreen shortcut buttons are reported separately when catalog metadata explicitly identifies them. Acceptance requires the actual production LCD to be the nearest surface, a supported touch control and enabled runtime touch. This is not a physical-cap picking pass, and guest soft-button behavior depends on firmware touch-zone configuration.

Pass --inspection-missing to additionally check a mapped but occluded control at inspection progress1. Such hits are reported under inspectionControls and inspectionOnlyButtonPasses. They do not erase the assembled-state missing/occluded finding and are never counted as default physical-button passes. The model is restored to inspection0 before visual signatures are collected.

K136 G0 is now closed as **physical, inspection-only**, superseding the batch02 source-limited disposition. Independent source proof59f9f561 found the exact SKU open-base photograph and registered its native Stamp cover. Only a2.4x2.2mm invisible region is added over the existing -Z flat tongue at[3.4,-3.6,5.53]. Default assembly126 sampled rays remain blocked. Inspection1 reaches the real cap using a shallow oblique view; replaying that exact ray with the case restored hits base-shell. UNKNOWN geometry remains unchanged. The original/gold/purple visible signatures and independent metadata rebuild match. See k136-boot-registry.json and the independent m5-vameter-boot-source-20261126 proof.

The final read of stable Root28bd36532d3f732ddad100146b4bc6d6d3c58a7e covers all141 M5 products, with sourceStartCommit equal to sourceCommit. All204 supported button controls are classified:199 actual default-assembled physical hits,2 source-native internal BOOT caps available in inspection1 (U195 and K136), and3 K034 virtual touchscreen shortcuts with exact native pixel readback. Both internal-control successful rays are blocked when replayed with the original assembly restored. All266 matrix keys across5 keyboard products pass nearest picking. There are0 unclassified controls,0 source-limited controls and0 loader errors. The2 raw assembled findings are intentionally retained; they are not default-assembly passes.

See all-141-final.json for the lossless per-SKU archive manifest, all-141-coverage.json for the compact summary and full-coverage gzip reference, and final-validation.json for acceptance. Complete rays, source-model hashes and per-product dispositions are retained in the referenced gzip artifacts. Reproduce with:

```sh
node tools/audit-m5-control-picking.mjs --source-root . --inspection-missing --out /tmp/m5-controls-141.json
```

The final raw audit is retained as141 deterministic gzip product slices in all-141-final-products/. Each row in all-141-final.json records its compressed SHA-256/bytes and original uncompressed SHA-256/bytes. The manifest also retains the original prefix, separator and suffix, so every original byte can be reassembled, including all rays and negatives. The1,028,246-byte original audit retains SHA-256014723e2d2ddf53f5c5ed2904627120e9f3398c5847703ba565223d4ddcabb8a. The complete69,415-byte original coverage JSON is archived without edits as all-141-coverage.json.gz (9,125bytes), with both compressed and original hashes in the compact summary. The largest new retained evidence file is37,739bytes. Historical batch01/02 receipts and their hashes are unchanged.

Reassemble the exact originals outside the evidence folder:

```sh
python tools/retain-m5-control-audit.py unpack --input docs/platform/evidence/m5-control-picking-20261126 --out target/m5-controls-141-reassembled
```

The tool verifies every compressed and uncompressed artifact hash/size, then verifies the original complete-file hashes. Its pack mode reproduces the retained format from those original files in a separate output directory. Gzip headers use an empty filename and mtime0; no raw values are dropped or summarized away.

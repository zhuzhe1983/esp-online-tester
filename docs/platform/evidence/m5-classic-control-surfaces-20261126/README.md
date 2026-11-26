# Classic input surfaces

P2 is a visual-candidate follow-up: its three existing caps now route to the verified A37, B39 and C/Power35 inputs. The old CPU receipts retain their original model hash. `p2-assembled-equivalence.json` records the exact inverse metadata reconstruction and identical original/gold/purple production-loader visible digests.

`stick-five-contract.json` tracks **five additional control-only SKUs**, outside the 36 visual candidates. These eight controls pass in the default assembled view, not just inspection:

| SKU | Added routes | Surface basis |
|---|---|---|
| K016-P-SE | power | Own annotated SE photograph and shared original integral case actuator |
| K016-P | power | Own side photograph, native case and dimension drawing |
| K016-T | button-b, power | Exact old-SKU annotated photograph over its existing estimated case; footprint placement remains approximately ±3 mm |
| K016-T2 | btnB, power | Own annotated overview/demonstration and existing integral side surfaces; approximately ±2 mm photo correspondence |
| K126 | button-a, power | Existing front cap plus independently photographed silicone side POWER footprint, approximately ±2 mm |

The five power routes use the existing AXP192 PEK **board transport**, without a GPIO number. Only the implemented press/release short-PEK behavior is represented. Physical power-off/on, analog supply state and wake are not added. C148/K148 independent STM32 power controls are explicitly unsupported; their native user button remains GPIO11.

Each final loader receipt contains five nearest outward rays per control, opposite-shell blocking, outside-region negative rays, and inspection progress 0.5/1/0 with source-surface support. There are 40 default positive samples and 21 inspection samples. New planes have opacity zero and depthWrite false. All three assembled appearance digests include actual geometry, normals, indices, UVs, world transforms, material values, source texture hashes and UNKNOWN text; old and final values match exactly.

T2 `side-key-a/b` are hidden behind the enclosure and remain unmapped. Their full-triangle outward test is retained as a negative control. The input planes use the source-visible outer surfaces, and the power footprint avoids the open center of the native relief. The downloaded manufacturer overview, demonstration and drawing are retained byte-for-byte under `source-t2/`, with the original source receipts and hashes in `stick-five-source-basis.json`.

The original full generic builders were also exercised. SE, original T and TLite regenerate the final metadata and all runtime artifacts byte-for-byte. Plus and T2 retain existing generic tessellation/encoding reproducibility gaps, recorded in `stick-five-generic-rebuild-boundaries.json`; those failures are not reclassified as passes. The separate control-only replay uses a hash-fixed compressed prior metadata checkpoint plus only the added routing pointers, preserving every original native/source/PNG byte. It exactly reproduces all five final metadata files and artifacts without claiming to regenerate CAD. Its negative controls reject a changed checkpoint, geometry exemption, changed native bytes and an undeclared control.

```bash
node --max-old-space-size=256 tools/verify-m5-stick-control-surfaces.mjs \
  --source-root /path/to/final-tree --output-dir /tmp/evidence
python tools/rebuild-m5-stick-control-metadata.py m5stack-k016-t2 \
  --source-root /path/to/final-tree --output /tmp/t2-control-replay
```

`own-controls-audit.json` additionally verifies the already-correct OLED, AtomU, StackChan touch pads/screen and PowerHub/Kit native user button against the actual integrated Root assets. It does not replace guest execution or browser pointer acceptance.

# Current M5 asset integrity

`assets-current.json` audits the actual source bytes at the recorded start/end
revision, independently of historical image reports. All 141 product contracts
pass: 139 manufacturer-loader entries and two dedicated implementations. The
141 entries use 140 distinct model paths; shared model identity is explicit in
each product detail rather than treated as a missing asset.

Verified results:

- 1,413 artifact declarations and 1,405 unique files, 596,361,742 bytes, match
  every declared SHA-256. Local geometry, source artifacts, labels, textures and
  reference files exist. Forty-two gzip artifacts also have expanded hashes;
  binary part byte ranges stay inside the declared stride-6 payload.
- All 923 retained raster images, comprising 923 frames, pass Pillow verify
  and full decoding. Raw file hashes, dimensions, modes and alpha statistics
  are retained. This supersedes no historical 882-image receipt.
- All 489 read JSON documents reject duplicate keys. Every asset and catalog
  `sourceId`/`sourceIds` reference resolves. Resolution is explicitly recorded:
  local model, the product's own knowledge registry, a named reused parent,
  or a hash-bound immutable metadata snapshot. No global same-name lookup is
  used. Parent/snapshot registry paths, hashes, declared links and exact source
  records remain in the lossless model details.
- Static metadata retains 152 UNKNOWN declarations across 135 product entries.
  Generic PCB placeholders default to the production loader's UNKNOWN label;
  dedicated placeholders retain their explicit declarations. This counts
  declarations and does not certify rendered marker visibility or PCB detail.

The five retention qualifications are explicit: PaperMono, StopWatch, TriviaPOD
and C147 predate standalone `model-plan.json` and declare no such artifact.
Their metadata contracts remain present. TriviaPOD retains the exact decoded
promotional glTF in gzip, while the original JavaScript wrapper is represented
by its source receipt, not claimed as a retained local file. Gallery receipt
filenames in K128-Lite's source recheck describe external downloads and are not
invented as dependencies promised by the production manifest.

Every detailed group is gzip with `mtime=0`, below 50,000 bytes per file.
The summary binds each compressed SHA-256/byte count and its uncompressed
SHA-256/byte count. Decompressing restores sorted-key UTF-8 JSON with indent 2
and a final newline. Per-product details include current model/plan hashes,
every local dependency, expanded source bindings and source/UNKNOWN records.
Artifact and image groups retain every actual file hash and measured count.

```sh
python tools/audit-m5-current-assets.py --source-root "$SOURCE" --out /tmp/m5-assets/assets-current.json
```

No browser or Three loader is instantiated. This checks retained bytes,
decodability, metadata links and static declarations, not GPU upload, UV/photo
alignment, rendered color, guest firmware compatibility or physical hardware.
The source files stayed unchanged throughout the recorded successful scan.

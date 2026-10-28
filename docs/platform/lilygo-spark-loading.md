# LILYGO Spark loading and sharing

The Spark catalog is metadata, not a guarantee that every advertised URL still works or that the firmware's features are implemented by the simulator. The runtime checks the selected chip, flash size, image structure and SHA-256 before starting it. It detects a complete flash image or a supported application image automatically; changing boot mode cannot repair an HTTP 404 or a missing file.

## Single-BIN ZIP sources

Spark's [download script](https://github.com/Xinyuan-LilyGO/LILYGO-Spark/blob/main/scripts/download_firmware.mjs) creates a separate ZIP for each BIN. Its manifest hash identifies the **uncompressed BIN**. An audited source can declare:

```json
{
  "url": "https://example.org/firmware.bin.zip",
  "sha256": "<64 hexadecimal digits: SHA-256 of the extracted BIN>",
  "archive": {"format": "zip", "entry": "firmware.bin"}
}
```

The loader requires one exact, flat BIN name, validates ZIP structure and CRC, bounds downloads to 64 MiB and extracted data to the configured flash size, then checks SHA-256 and the normal boot layout. Stored/deflated ZIPs and streaming data descriptors are supported. Encrypted, multi-file, symlink and ZIP64 archives are rejected. Verified extracted bytes use an archive-specific cache key; scene links and history preserve the original ZIP URL, entry and payload hash. Hardware flashing still requires an extracted BIN or supported manifest.

As checked on 2026-10-28, the LILYGO OSS firmware endpoint returns the Bruce ZIP without an `Access-Control-Allow-Origin` header. Browser fetch therefore cannot read that endpoint directly. Under **远程地址 → LILYGO Spark**, download the original ZIP with the provided link and import it under the selected version; the workbench extracts and checks it automatically. Importing the matching BIN also works. This uses local verified cache and does not upload the file. It does not bypass CORS for another browser or turn a shared URL into a firmware attachment.

Bruce v1.10.3 for T-Embed CC1101 was checked: 4,203,216 uncompressed bytes, SHA-256 `c6e5c5df56ec30bbbfc826075f3deccf1f809343a6bec7bdf09e9a00fc74146d`. It is a complete flash image and selects normal ROM boot. Import, worker initialization and cached embed startup were checked; this is not evidence of complete Bruce peripheral/function support.

## Deleted example URLs

The catalog still points six T-Embed examples at `master/firmware/examples/`. Upstream [commit c4a522d](https://github.com/Xinyuan-LilyGO/T-Embed-CC1101/commit/c4a522d1f71e8bbe5c3a361ab475c98b562529d0) deleted those files. Each file was fetched from its parent, [68ef304](https://github.com/Xinyuan-LilyGO/T-Embed-CC1101/tree/68ef3042980e51711c0318efd6cc944999e688db/firmware/examples), and matched against the manifest's full hash. `lilygo-spark-sources.js` rewrites only that exact original URL/hash pair. A new digest cannot silently inherit the old mapping. The original firmware version and bytes are preserved.

The six receipts, including fetch time, size and full hash, are in [source-recoveries.json](evidence/lilygo-spark-fixes-2026-10-28/source-recoveries.json). `cc1101_send_irq.bin` was downloaded through the browser UI and loaded by the real worker. Loading success alone does not establish working RF transmission.

## Sharing the loaded firmware

When a firmware is ready, running or paused, UI share/iframe actions retain the runtime's actual remote URL, observed hash, archive descriptor and boot mode. Browsing another entry in the left selector does not replace that running source. A local-only image cannot be embedded in a URL. Configuration-only links carry `firmware: null` and `autostart=0`; an old link with `firmware: null&autostart=1` stays a preview and explains the missing source instead of choosing another firmware.

## T-Embed PCB placeholder

The manufacturer's shell CAD does not include a verified PCB assembly. Its `UNKNOWN` placeholder previously used an 84 × 32 mm rectangle, whose four corners protruded through the rounded shell. The same centered 84 × 32 × 1.2 mm placeholder now has a capsule outline. This is a case-constrained estimate, not a claim about the real board outline or mounting position. The shell CAD, controls, screen and binary geometry are unchanged; the placeholder remains visible through transparent finishes.

Rebuild with the LILYGO-specific generator:

```sh
python tools/build-lilygo-models.py lilygo-t-embed-cc1101 --output /tmp/lilygo-model-review
```

The rebuilt `geometry.bin` SHA-256 remains `8e659a98693e95480e8104fb4c3055ea72272c6016bc8b4772734f21c1555b98`. Geometry tests cover the outline, thickness and legacy box/disc shapes. Browser checks include front, back, side and oblique views, plus opaque and transparent finishes. Before/after images and browser receipts accompany this document under `evidence/lilygo-spark-fixes-2026-10-28/`.

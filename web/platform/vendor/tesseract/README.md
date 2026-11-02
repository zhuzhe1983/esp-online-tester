# Local image-processing dependencies

- Tesseract.js **6.0.1**: `tesseract.esm.min.js`, `worker.min.js`, Apache-2.0; copied from the npm package. Distributed dependency notices accompany both bundles.
- tesseract.js-core **6.1.2**: LSTM-only WASM bundles, SIMD and scalar fallback, Apache-2.0. The WASM bytes are embedded in each `.wasm.js`. Only OEM 1 is used; legacy OCR/detection is not enabled.
- English trained data: https://raw.githubusercontent.com/naptha/tessdata/gh-pages/4.0.0/eng.traineddata.gz (Apache-2.0; upstream https://github.com/tesseract-ocr/tessdata). The original compressed SHA-256 and the derived local file are pinned in `manifest.json`.
- Retrieved 2026-11-01 UTC. `manifest.json` records file size and SHA-256. No remote runtime dependency is used. URLs derive from `import.meta.url`, preserving repository subpath hosting.

The optional image-processing path imports these assets on demand and loads one core plus the English model (about 6.6 MiB before HTTP compression). Manual selection does not load them. Each job terminates its worker; images, answers and tokens are not persisted by this module. Missing or ambiguous words leave the manual dialog available.

The 2026-11-02 model packaging removes only legacy-engine components, which OEM 1 does not use. Compressed size drops from 10,923,060 to 2,949,352 bytes. The existing LSTM weights, three LSTM dictionaries, character set, recoder and version remain byte-identical; this does not substitute a different recognition model. `languageTransform.preservedComponents` records their sizes and SHA-256 hashes. `combine_tessdata` 5.5.3 independently extracted identical components from both files.

To reproduce, download the original URL above, then run `python3 tools/trim-ocr-traineddata.py original-eng.traineddata.gz web/platform/vendor/tesseract/eng.traineddata.gz` from the repository root. The script rejects a source with a different digest and writes deterministic gzip. This reduces cold download cost; slow networks can still exceed the 20-second component-loading deadline and return to manual input.

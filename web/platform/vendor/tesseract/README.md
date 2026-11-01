# Local image-processing dependencies

- Tesseract.js **6.0.1**: `tesseract.esm.min.js`, `worker.min.js`, Apache-2.0; copied from the npm package. Distributed dependency notices accompany both bundles.
- tesseract.js-core **6.1.2**: LSTM-only WASM bundles, SIMD and scalar fallback, Apache-2.0. The WASM bytes are embedded in each `.wasm.js`. Only OEM 1 is used; legacy OCR/detection is not enabled.
- English trained data: https://raw.githubusercontent.com/naptha/tessdata/gh-pages/4.0.0/eng.traineddata.gz (Apache-2.0; upstream https://github.com/tesseract-ocr/tessdata). Exact fetched bytes are pinned in `manifest.json`.
- Retrieved 2026-11-01 UTC. `manifest.json` records file size and SHA-256. No remote runtime dependency is used. URLs derive from `import.meta.url`, preserving repository subpath hosting.

The optional image-processing path imports these assets on demand and loads one core plus the English model (about 14 MiB before HTTP compression). Manual selection does not load them. Each job terminates its worker; images, answers and tokens are not persisted by this module. Missing or ambiguous words leave the manual dialog available.

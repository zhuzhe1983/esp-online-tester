# Waveshare ESP32-S3 manufacturer evidence · 2026-11-23

Sources are retained byte-for-byte inside deterministic gzip files. The source manifest records the original and resolved URLs, pinned Git revision, source byte SHA-256 and fetch time. The gzip hash is separate. No generated source text is represented as a downloaded file.

`firmware-manifest.json` describes original remote binaries inspected for ESP image checksums and partition layout. Original binaries remain remote-only. Structural integrity is not runtime acceptance. A filename date is not a product release date; a factory partition label alone does not prove factory-image provenance.

Raw storefront HTML is deliberately not archived because its response contains anonymous session values. Exact SKU and source receipt records are carried by the curated official inventory. Manufacturer source rights remain with their respective owners; retained excerpts are for hardware identification, reproducibility and review. No broader CAD or photograph licence is inferred.

LR ePaper 2.9 source: `docs/hardware-facts.zh-CN.md` and `WaveshareBoard.h` establish Rev1.1 wiring; shared GPIO38 supply gates the display/TF rail. The generic epaper_config.h defaults are not this board pinout. Its supplied HF/LF BINs are called reference binaries, not factory images. LR OLED source explicitly identifies its top-level Test BIN as original factory/recovery firmware, with upstream source revision unknown.

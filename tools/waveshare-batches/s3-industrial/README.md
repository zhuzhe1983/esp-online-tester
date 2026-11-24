# Isolated S3 industrial lane fragments

One JSON per official storefront listing. Each fragment contains exact SKU products, remote firmware metadata, non-promoting reference-only watch contracts, knowledge paths, exact board capabilities, model IDs and evidence references. They are integration inputs rather than replacements for the shared catalogs.

Group 1 is described by `docs/research/waveshare-s3-industrial-2026-11-24/batch-01-manifest.json`: five listings, twelve SKU profiles and seven mechanical models. The native runtime and actual guest test live in `esp32s3/src/board/waveshare_industrial.rs` and `esp32s3/tests/waveshare_industrial.rs`; bare Matrix reuses `waveshare_rgb_matrix` from shared commit `2cb056d4`.

No generic camera sensor, display panel, radio response, CAN/RS485 peer or external LED strip is attached merely because the ESP32-S3 CPU is shared. The remaining sixteen industrial/controller families are separate work and are not represented by this first manifest.

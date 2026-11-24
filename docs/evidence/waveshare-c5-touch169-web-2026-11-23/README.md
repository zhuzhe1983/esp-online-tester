# C5 Touch LCD 1.69: public runtime diagnostic acceptance

`result.json` records the real platform runtime → production Worker → production WASM path
executed in Node. The board has 16 MiB Flash and 8 MiB PSRAM; the initial generic C5 memory
policy incorrectly selected 4/0 and was rejected. `initial-memory-policy-failure.json`
preserves that failure. The subsequent result uses the corrected exact product policy.

The original project diagnostic tests all 8 MiB PSRAM, DMA display reads from PSRAM, the
source-wired TCA/QMI path, touch coordinates, and active-low GPIO28 BOOT. Six RGB565 native
frames, converted with the production RGB565-to-RGBA rounding, match the public output
byte for byte. Ready has 3360 colors; an idle interval changes zero pixels; touch changes
1024 pixels and BOOT changes 44928 pixels. Release restores the corresponding prior frame.
The full UART, RGBA gzip files, PNG previews, firmware hashes, and native comparisons are
retained in the capture archive. This board has no physical TF slot and is not given one by the runtime.

This verifies the explicitly labeled diagnostic example, not the untouched vendor factory
image. The original factory boot still stops before UART/display, and its reference catalog
entry remains unverified. Browser/WebGL operation, radio, codec audio, fuel gauge, and full
physical orientation are also not established by this Node execution.

## Raw capture retention

Raw UART, traces, frame buffers and PNGs referenced by the receipts are retained in
`waveshare-runtime-captures-20261123-batch3.zip` with their original directory paths.
See `docs/evidence/waveshare-integration-2026-11-23/capture-manifest.json` for byte sizes,
SHA-256 values, the owner-access recovery link and recorded privacy substitutions.
This development snapshot has not published the captures or application to GitHub.

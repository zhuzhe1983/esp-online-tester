# Final Seeed WASM and worker checks

[receipt.json](receipt.json) records eleven successful ABI/worker checks and a
separate successful run of fourteen named CI demo manifests plus JIT handoff.
The production module is 6,026,506 bytes, SHA-256
`1002b80d7336ee0e610eedad8b9688fc5389bb1bcaeadf48c02d279069181a24`.

These checks run the actual WASM module under Node, including real recording
replay, StopWatch, hardware-diagnostic observations, timing and BLE APIs.
They are separate from the complete platform contract suite, Seeed factory
probes, browser/WebGL acceptance and physical hardware verification.
The timing rates printed by the smoke harness are not quiet CPU benchmarks.

The JSON files retain input/output observations and artifact identities.
Gzip logs retain test numbers and results; only declared directory prefixes
and any MAC-shaped values are redacted. Original, published and archive
SHA-256 values are recorded. The raw private logs are not committed.

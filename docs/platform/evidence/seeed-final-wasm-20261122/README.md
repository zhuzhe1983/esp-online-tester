# Final shared WASM regression evidence

See [the complete runtime report](../../seeed-final-wasm-runtime.md).
`receipt.json` records 12 unchanged default-image ROM boots, 16 input actions,
26 actual framebuffer hashes and three `NO_FRAME` results. The preserved
negative observations are part of the result, not a test-harness exit-code
compatibility verdict.

`runs.json` is the complete startup/input plan. Files under `inputs/` contain
the exact button, touch and encoder events; the Indicator network fixture is
synthetic, with outbound networking and automatic provisioning disabled.
The receipt includes per-run reproduction commands and original artifact,
initial Flash, raw receipt, serial and framebuffer SHA-256 values.

Only five visually reviewed, identifier-free PNGs are published. They decode
to the exact original RGBA capture hashes. The TRMNL provisioning image is
omitted because it contains a generated device identifier and QR code. Raw
serial, network state, storage keys and local filesystem paths are not retained
here. Original raw-receipt hashes preserve the identities of the private
captures; selected guest markers and every failure remain explicit.

The frozen execution artifact is WASM SHA-256
`1002b80d7336ee0e610eedad8b9688fc5389bb1bcaeadf48c02d279069181a24`.
This is a shared WASM/worker regression. It does not validate later loader
cache changes, browser rendering/CORS, physical hardware, complete workflows
or host performance. Older evidence is preserved unchanged.

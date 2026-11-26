# Independent RMT negative observation

The unchanged source snapshots under `tools/` and `receipt.json` retain an independent Capsule K129 and NanoH2 C149 rerun. Both cold boot and public reset produced real red/green/red LEDs. Wrong-route runs reached the guest RMT completion and `OUTPUT COMPLETE`, consumed the key press, advanced the acknowledged guest clock by more than0.02 seconds, and published no nonempty LED event. Observed public LEDs remained`[]`; no engine/board defect was found.

Source snapshot SHA-256 values are in the receipt. From this directory, reproduce with `M5_COVERAGE_SOURCE_ROOT` pointing to the actual repository containing the production runtime and Worker:

```sh
M5_COVERAGE_SOURCE_ROOT=<repository> \
M5_COVERAGE_PRODUCT_IDS=m5stack-k129,m5stack-c149 \
ESP32SIM_WASM=<central020.wasm> ESP32SIM_ROM_DIR=<rom-directory> \
M5_COVERAGE_WORKER_RECEIPT=<receipt.json> \
node --test tools/platform-m5-coverage-worker-test.mjs
```

This corroborates the digital negative contract. It is not manufacturer firmware, physical RMT timing or physical device validation.

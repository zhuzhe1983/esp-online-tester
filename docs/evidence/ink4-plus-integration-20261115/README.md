# INK4 Plus and official Mosaico assets, 2026-11-15

Baseline is `65fa5671`. The integration adds the independent S31 INK4 Plus board, LCD/AXI DMA/RMT waveform delivery, physical I²C routes, SDMMC and auxiliary HP UARTs. Final core behavior is `56afb196` (both XOE and MODE must be high); `90145501` keeps semantic input IRQ timestamps on S31 board time. Public catalogs distinguish the simulated PSRAM capacity from unknown physical capacity. The unmodified official 2.0.68 and 2.0.79 images reach actual settings through touch. See the [board report](../../platform/ink4-plus-runtime.md) for electrical routes, version differences and missing hardware.

Mosaico uses the manufacturer's three original enclosure/button meshes with retained assembly transforms and original license designation; derived electronics remain separately labeled UNKNOWN. Its explicit default is the official Vibe 0.3.0 recovery from pinned browser-readable raw segments. Complete Hello and Xiaozhi sources retain their CORS limits and original download links. ZIP recovery now uses the same extracted-member cache key as ordinary loading. Source and real-browser checks are in the [Mosaico model report](../../platform/mosaico-model.md) and [download report](../../platform/evidence/mosaico-download-20261115/README.md).

## Correctness checks

[checks.json](checks.json) records exact scope and counts. Rust stable 1.100.0: 2,242 workspace tests pass in 175 groups, ignored tests included and 96 external-input tests filtered; no golden changes. After the final MODE gate correction, all four board tests, both Clippy variants with warnings denied and the original official firmware settings/SD smoke pass. The real production WASM also passes 15 manifest scenarios, recording/replay, timing/BLE ABI, network helper/relay, worker pacing/channel, 111,126 differential cases and 24 GPIO waveform cases. Profiling-feature compilation and native virtual-quantum checks pass. Final catalog/model/i18n checks are recorded separately after source refresh.

The [Mosaico worker regression](mosaico-regression.json) verifies the original complete Hello image, touch language choice and AI standby/wake on the integrated S31 core. It uses a caller-supplied local image; direct Vibe download is verified separately in a real browser. No private firmware or SD image is committed.

The first full-test attempt lacked the C61 ROM in this fresh worktree; all pinned ROM hashes were then checked and the full sweep rerun successfully. Source metadata checks caught missing archive filenames/index timestamps, and the i18n check caught untranslated new support text. The data and translations were corrected without weakening validators. Review found the missing MODE gate; its negative tests now cover low/disabled MODE and output enable, including shutdown during scanning.

## CPU comparison

This extends the existing EX239/EX259/EX260/EX265 integration regression contract, not an execution optimization. The workloads and code changes differ from the previous integration: baseline already includes Mosaico, while the candidate adds INK4-specific peripheral paths. The harness additionally accepts the repository's pinned S31 hello images. Both trees use the same stable compiler and release settings. Three alternating AB/BA pairs per workload use 30 guest seconds, identical inputs and captured output hashes. Task-owned browsers, compilers and other emulators are stopped during the measurements. No separate warmup; these short shared-host samples are not an isolated speedup claim or hardware timing qualification.

```sh
cargo build --release -p esp32sim --bins  # in both checkouts
python3 tools/compare-display-cpu.py \
  --baseline "$BASELINE_BIN" --candidate "$CANDIDATE_BIN" \
  --firmware "$FW_DIR" --roms "$ROM_DIR" --s31-firmware "$S31_FW_DIR" \
  --rounds 3 --output cpu.json
```

The [initial CPU receipt](cpu-initial.json) retains all 30 runs with identical work/output. Medians: S3 hello −2.73%, C3 hello −0.02%, C6 hello +2.20% (overlapping ranges), Pocket Tank −0.57%. S31 hello regressed **+4.02%** (4.572930 → 4.756879 s, 3/3 candidate runs slower, non-overlapping ranges). This negative result is retained; a follow-up confines inactive SD/LCD/RMT polling to one cold-state guard.

## Integration and privacy

The independent code, website and model changes merge without text conflicts. Pre-existing NFC edits in the primary working copy are backed up and restored separately; they do not enter this integration. Original source responses containing account fields are excluded; public release selections retain original and selected hashes. Captures retain only firmware/model views and relevant guest output. Machine inventories, account details and task session identifiers are not part of public receipts. `node tools/check-evidence-privacy.mjs` supplements manual review.

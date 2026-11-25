# C5 cold ABI recovery contract

After SRAM app-direct boot withdraws the validated-ROM profile, loading the ROM into the same VM must fail even if no instructions or cycles have elapsed. A new VM/chip epoch is required before selecting that cold profile again. The ABI error now says to create a fresh VM after SRAM app-direct. The guard and backend state rules are unchanged.

The native and raw-WASM regression cases now assert same-handle rejection, retained SRAM-mode PC/zero counters, and successful ROM setup on a fresh handle. Original corrupt-input rejection, complete merged Flash validation, bounded final-page restore and zero-instruction assertions remain. Both native tests that require caller-supplied ROM/factory inputs retain `#[ignore]` and use `external_*` names, so CI's `--include-ignored --skip external_` policy excludes them. The focused command explicitly includes them when pinned inputs are supplied.

[Checks and source hashes](checks.json) use base `f7a30105115fc98d1487cd972e7e5d069eae4eea`. All five selected native tests pass: three C5 cold ABI cases and two shared helper cases. The independent target directory and explicit native-only LTO=false override avoid a production-WASM claim. All 14 C5 source/protocol Node contracts and Worker channel checks pass; these use controlled ABI doubles and require no WASM binary. The updated raw-WASM script passes syntax checking only.

No WASM was rebuilt for this correction. The earlier 8,573,206-byte production artifact, SHA-256 `18931572bfab8c2decd82d5bf0109024da7f38bbe67f6aee44af8cf6b3f365ce`, tested the older backend/contract. Its [historical receipt](../c5-runtime-integration-2026-11-24/production-checks.json) remains unchanged and cannot validate these new same-handle rejection assertions. Run `tools/platform-c5-wasm-test.mjs` again with the final integrated production artifact.

This change establishes cold setup behavior only. It does not execute original C5 instructions, modify firmware, prove application/LCD/SD behavior, open a browser or qualify public factory support. No third-party input, compiled artifact or raw local log is committed.

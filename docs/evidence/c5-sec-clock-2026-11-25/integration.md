# C5 SEC and cold diagnostics integration

The unchanged original factory now executes the SEC clock-source RMW, then stops at PC4200255e reading Key Manager memory-power control60096168. The log records **27,039,922 scheduling steps/CPU insn_count,27,039,872 nominal cycles and3,711 UART bytes**. It has not entered app_main, produced LCD pixels, touched SD transport or initialized the ordinary watchdog. Native insn_count is not a measured retired counter.

SEC selection2 exposes the observed480MHz pre-DPA PLL input. It does not power or calibrate that PLL, grant a ready flag, or define a480MHz KM clock. TRM9.2 limits downstream CRYPTO_CLK to160MHz through separate DPA policy; that consumer remains unmodeled. Unknown selectors/reset/reserved controls stop explicitly. Strict exact-owner refusals yield ModelLimit; real negative guests preserve trap state, destination and source observation.

The combined focused run passes **491 C5 tests**, including all six pinned-input cases, three new SEC Bus/guest checks and the cold LCD273 fixture. Both SPI SD cold-observation tests pass. Workspace/all-target and release-WASM/jit-tests Clippy pass with warnings denied. Native focused checks use jobs1/LTO=false.

A separate real `tools/wasm-build.sh` run uses the repository default release profile. Its8,825,295-byte artifact has SHAc182ddfe1c4586de6773980b50cf90df5545be335df2c4b810a5d69738884197 and is used by the passing platform tests: all15 Node summaries,17 config groups,11 human-input checks,4 framebuffer lifecycle checks, plus the real worker/WASM hello boot/pause/resume/reset boundary. The first attempted platform run lacked this documented build artifact and returned404 before guest execution; that negative hash is preserved without changing a fixture or assertion.

Cold LCD273 diagnostics copy actual latches and existing SPI SD counters through the already existing ABI. Canonical decimal strings preserve full u64 values. Store loads do not prove a guest consumed MISO, and card presence/state do not prove FAT mount or persistence. No extra byte/tick work or physical validation is claimed. The independent read-only review found no scoped P1/P2; its hash and the exact source/check/input hashes are in `integration-checks.json`.

This checkpoint does not replace full default-LTO native CI, controlled CPU comparisons, real original-factory WASM/browser output or final product acceptance. The next actual missing boundary is Key Manager initialization, with RF/PHY still independently unsupported.

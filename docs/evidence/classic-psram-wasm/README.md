# Classic PSRAM native/WASM qualification

Current integrated base: upstream main `77e43c20a70054ac84d1724d757fa9ce12668151`.
Memory-miss implementation: `9823ed9a978c0fc798ffebc322662624566758f5`.
Current checks, final disassembly pair, archive manifest and model limits: [consolidated receipt](../classic-psram-followup/README.md).

The classic factory enables 4 MiB and leaves 0 absent. Unsupported sizes inside
the existing 32 MiB cap create a machine without PSRAM with one diagnostic;
requests above the cap return null. This preserves existing caller defaults.
The native setter stays strict (0/4 MiB). There is no new raw backing pointer.

The shared qualification lifecycle covers all 128 reversed pages, widths,
independent SPI readback, alias/remap, crossings and atomic faults, cache/mask
restore, 4/0/4/0 construction and zero-filled reboot with invalidated tables.
Actual WASM and native P5/P6 present/absent guests were compared with JIT off/on
and two boots per creation: 24/24 bounded comparisons in each production-settings
and jit-tests matrix. Serial result lines match, not scheduler quanta or whole
instruction totals. The [current gate log](../classic-psram-followup/README.md)
distinguishes fresh gates from this archived qualification.

Retained negative: the earlier extra JIT assertion failed after reboot while
production-settings qualification passed. Only 13/24 asserted comparisons ran;
unexecuted rows were not passes. A selected overlapping live region need not
be coverage's preferred owner. The integrated fix validates Hot fields against
the selected live region and clears owner hints on reset/reindex; epoch and
field guards remain. Native uses a different cache. The minimized regression
is in the standard JIT suite. No universal stale-code or hardware claim.

Reproduce with pinned Arduino2/3 recipes, ROMs, owned CARGO_TARGET_DIR and
`tools/psram-wasm-test.mjs`; build native psram-qualification and WASM with
psram-tests, then repeat with jit-tests. See that script's environment inputs.
`tools/psram-wasm-mutations.py` assertion-checks lifecycle/factory mutations on
both targets in an isolated checkout. `tools/psram-wasm-controls.mjs` compares
no-PSRAM controls. Build helpers require a local target symlink for out-of-tree
targets. Original full guest/control/negative JSON is preserved in the archive,
not repeated here or presented as a fresh final-head run.

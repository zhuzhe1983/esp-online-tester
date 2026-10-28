# Standalone ESP-IDF 4.4.7 PSRAM allocator fixture

Current integrated base: upstream main `77e43c20a70054ac84d1724d757fa9ce12668151`.
Memory-miss implementation: `9823ed9a978c0fc798ffebc322662624566758f5`.
Current checks, final disassembly pair, archive manifest and model limits: [consolidated receipt](../classic-psram-followup/README.md).

[Recipe, pinned source, config, licences and firmware hashes](../../fixtures/classic-psram-idf/README.md).
Committed firmware boots the ECO3 ROM, runs the genuine SDK memory test and heap
registration, then allocates/readbacks/frees 1 MiB across 33 pages. Host tests do
not write guest mappings or intercept functions. No-PSRAM startup rejects the ID.
This fixture and the real CLI allocator test run in CI with fetched ROMs.

Expected guest results: physical=4194304, heap=4194303, free=4192135;
allocation=1048576 at 3f80087c, errors=0, FNV-1a=678f9dc5, pages=33;
SPIRAM DMA rejected, ordinary DMA internal, exhaustion rejected, result=pass.
The one-byte heap distinction comes from the SDK's inclusive end calculation,
not a reduced backing or patched getter. No hardware or allocator emulation claim.

Run `python3 docs/fixtures/classic-psram-idf/validate.py`, then with ROMs,
`cargo +1.99.0 test --release -p esp32sim --test classic_psram_idf -- --include-ignored`.
Historical guest slot/shift mutation survivors reflect self-consistent addressing;
independent native mapping assertions kill those rules. The earlier normal-only
model failed stock dual-core Arduino low/high memory testing, and its old 40/40
command profile rejected ID; subsequent low/high and DCLK support address those
specific gaps. These negatives are not hidden or presented as passing qualification.

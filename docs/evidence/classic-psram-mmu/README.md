# Classic external RAM translation

Current integrated base: upstream main `77e43c20a70054ac84d1724d757fa9ce12668151`.
Memory-miss implementation: `9823ed9a978c0fc798ffebc322662624566758f5`.
Current checks, final disassembly pair, archive manifest and model limits: [consolidated receipt](../classic-psram-followup/README.md).

One bus-owned optional 4 MiB allocation backs both CPU and SPI access. The native
setter accepts only 0 and 4194304 bytes; unsupported sizes preserve the old state.
Classic has no fast_mem pointer or flat memory fallback. Default remains absent.

ESP-IDF v4.4.7 and v5.1.4
`components/esp_hw_support/port/esp32/cache_sram_mmu.c` defines PID 0/1 slot 1152,
128 entries and 32 KiB pages. Flash keeps its separate 64 KiB translator.
Each access validates every byte before any store, including noncontiguous and
invalid crossings. Absent/invalid/overflow data accesses fault Unmapped.
Instruction fetch is Prohibited; DMA remains internal-only. The loader uses the
same translation. PRO-only startup is allowed while APP is held; normal-mode
live divergent tables fault, a qualification restriction rather than silicon truth.

Run `cargo +1.99.0 test --release -p esp32 psram` and, in an isolated tree,
`python3 docs/evidence/classic-psram-mmu/mutate.py --repo TREE --out OUTPUT`.
Historical native slot/shift probes killed errors which self-consistent guest
readback did not. The historical bounds-removal probe included a host bounds
panic and is not an assertion-only kill. New miss-path probes in the consolidated
receipt require assertion-only failures. Older pre-LTO and linked pairs have
been replaced by the single final linked pair; no historical codegen comparison
is used as final-head evidence.

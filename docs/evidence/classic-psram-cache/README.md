# Classic PSRAM cache and shared backing

Current integrated base: upstream main `77e43c20a70054ac84d1724d757fa9ce12668151`.
Memory-miss implementation: `9823ed9a978c0fc798ffebc322662624566758f5`.
Current checks, final disassembly pair, archive manifest and model limits: [consolidated receipt](../classic-psram-followup/README.md).

CPU translation and SPI commands share one allocation. Functional write-through
means no dirty lines or delayed writeback; it is not a cache-latency model.
The cache gates require enabled owning caches, clear DRAM1/OPSDRAM masks,
32 KiB page mode, SPI0 CS1/QIO/read or write command readiness and device QPI.

Sources: ESP-IDF v4.4.7 `components/soc/esp32/include/soc/dport_reg.h:186-326,426-554`
and `components/esp_hw_support/port/esp32/spiram_psram.c:1090-1105`;
v5.1.4 `components/esp_psram/esp32/esp_psram_impl_quad.c`.
Normal mode qualifies matched live tables, not independent requester routing.
[Low/high mode](../classic-psram-lowhigh/README.md) uses owning halves.
The shared gate has no caller-core identity. Cache-disabled accesses raise
Prohibited rather than the hardware cache-IA interrupt. These limits are unchanged.

Run `cargo +1.99.0 test --release -p esp32 psram`. The original semantic harness
remains at `docs/evidence/classic-psram-cache/mutate.py`; use an isolated tree.
Historical checks/operands and old disassembly are archived, not final-head proof.

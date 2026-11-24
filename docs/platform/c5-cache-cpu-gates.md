# C5 cache CPU bus gates

`0x600c8004` is `CACHE_L1_CACHE_CTRL`. Bit0 shuts the CPU instruction bus, and bit1 shuts the CPU data bus. Both reset to0. C5 `cache_ll_l1_enable_bus()` and `cache_ll_l1_disable_bus()` explicitly map IBUS0 to bit0 and DBUS0 to bit1, with a shared external virtual address window. These are implemented as independent gates on actual CPU access to0x42000000–0x43ffffff, including PSRAM stores. Internal ROM/SRAM and peripheral registers remain accessible.

This is a functional access-control subset, not a cache-line timing model. A prohibited access returns an emulator bus fault; physical stall timing and cache access-error interrupts are not simulated. The separately modeled GDMA memory path is not the CPU I/D path and remains independent of these two core0 gates. MMU validity, device capacity and MSPI configuration are still required; opening a gate cannot create a mapping.

The read-only geometry fields are taken from the C5 header:4KiB/64-byte instruction cache fields and32KiB/32-byte shared cache fields. Stores cannot rewrite HRO geometry. The version register has its documented writable mask/default. Reserved instruction-bus fields are not treated as aliases for the real CPU buses.

Invalidate, clean/writeback, lock, freeze, prefetch/autoload, performance tracing, tag/data arrays and their completion/interrupt registers remain unmapped. No DONE flag is fabricated to pass firmware polling. The original data model still accesses coherent backing arrays without modeled cache lines; this change does not claim cached-data coherence or cache-maintenance behavior.

## Sources and versions

The [source manifest](../research/c5-cache-control-sources-2026-11-23/manifest.json) preserves six original headers with URL, actual retrieval time, size and SHA-256. Primary source is ESP-IDF v5.5.4 at `735507283d5b2f9fb363a1901172dbd9e847945d`. The manufacturer Touch1.69 factory apps identify v5.5.2; its exact commit `30aaf64524299d3bde422ca9a2848090d1bc5d0f` has byte-identical C5 `cache_reg.h` and `cache_ll.h`. The version comparison is recorded rather than assuming that a nearby chip or release has the same map.

## Execution boundary and regression

The unmodified rev100 ROM now executes the read/modify/write inside `Cache_Mask_All`. Both source-wired Waveshare LCD1.47 and Touch1.69 native probes, with their unchanged factory bytes placed at0 and exact physical capacities, then stop after6,831 instructions at fault PC0x4001df26 reading0x600b00e4. The ROM symbol is `pmu_enable_unhold_pads`; the next missing block is PMU immediate pad hold control. UART output and nonzero display pixels remain0. This is further ROM execution, not successful factory boot or public-loader acceptance.

The previous6,825-step cache boundary and factory evidence are retained as historical checkpoints. No ROM byte, PC, analog reset seed or guest success function is patched.

Tests exercise instruction/data gating independently, PSRAM store denial without modifying its die, internal SRAM access with both CPU cache buses closed, reset and invalid-MMU behavior, read-only geometry, rejection of unimplemented engines, and GDMA access while CPU buses are closed. The original Touch1.69 guest remains a separate full-memory/input regression; its raw frames, UART and instruction counts are compared with the preceding verified diagnostic. Exact commands and hashes are in the [validation receipt](../evidence/c5-cache-control-2026-11-23/checks.json).

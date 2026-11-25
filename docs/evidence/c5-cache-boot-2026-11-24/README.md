# C5 cache boot controls

The later [freeze mode0 extension](../c5-cache-freeze-2026-11-24/README.md)
supersedes this checkpoint's unsupported-freeze boundary.

This tranche adds private cache clock/reset controls and maintenance of the
existing coherent passthrough representation. It does not add resident cache
lines, latency, a prefetch engine or an MMU alias. The unchanged LCD273 factory
baseline reaches 275138 steps and faults at `ROM_Boot_Cache_Init` PC `0x4002c5be`,
reading PCR `0x60096104`; it has 101 UART bytes and no application/pixels.
That baseline is not a claim of execution after this module is wired.

The integration API is `CacheControls::read_pcr()` and
`write_pcr(value) -> bool` for `0x60096104`. Existing `read(off)`, `write(off,value)`
and `cpu_allowed(instruction)` remain. PCR stays accessible while the cache is
gated/reset; cache MMIO and external CPU instruction/data accesses are blocked.
Cache reset restores the cache register state, not MSPI MMU entries. The actual
ROM's `Cache_MMU_Init` writes all 512 invalid entries itself.

## Fixed sources and power conflict

The LCD273 factory's IDF v5.5.3 is
`2c211b236707889e8400c4dc5644dd5c4ee071e0`; the LCD147 control's v5.5.4 is
`735507283d5b2f9fb363a1901172dbd9e847945d`. Full-file comparisons confirm identical
cache register/struct/LL, ROM cache API header and PCR register bytes. Existing
public source archives are retained under
`docs/research/c5-cache-control-sources-2026-11-23/` and
`docs/research/c5-memory-sources-2026-11-23/`.

- [PCR header](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/register/soc/pcr_reg.h):
  `CACHE_CONF +0x104` names clock bit0, reset bit1, PU bit2 and PD bit3; reset is
  `5`. Both PU/PD descriptions say to write zero for opposite power effects.
- [C5 TRM v1.1](https://documentation.espressif.com/esp32-c5_technical_reference_manual_en.pdf),
  Register 9.61, printed page441, describes only bits0/1. Bits31:2 are reserved
  and the diagram reset is `1`. It does not resolve the header's power polarity.
- A public ESP-IDF code-index search for `cache_pu_en` finds only the generated
  PCR register and struct headers. This is an index result, not a proof about
  every historical revision or a substitute for silicon evidence.

The bounded contract reads the matching-firmware header default `5`, implements
clock/reset, and accepts only the unchanged power-bit pair `4`. Changing either
power bit rejects before any reset or state mutation. No power polarity is
inferred from a field name, and no new supported power mode is claimed.

## Maintenance semantics

[C5 cache registers](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/register/soc/cache_reg.h)
define mutually exclusive invalidate, clean, writeback and writeback-invalidate
commands in `SYNC_CTRL +0x94`, a RO done flag, and map/address/size staging fields.
The representation is explicitly `CoherentPassthrough`: every mapped read/store
already reaches its backing array synchronously, with no resident or dirty lines
and no pending fill/writeback queue. An accepted maintenance operation applies
to that actual empty set. It completes synchronously without manufacturing a
memory transfer, line fill, hit/miss count or completion of queued work.

The header's reset `INVALIDATE_ENA=1`, `SYNC_DONE=0` starts a reset invalidation.
It runs when clocked and released; the first accessible status is therefore a
completed operation. A reset held without a clock does not finish it. Software
sync commands require one operation and the explicitly supported flash-cache map
`16` from the [ROM API header](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/esp_rom/esp32c5/include/esp32c5/rom/cache.h).
Other maps or combined commands reject without a new completion.

The original ROM's `Cache_Enable_Cache(0)` clears the two bus masks;
`Cache_Resume_Autoload(0)` returns without MMIO. The [C5 LL](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/esp32c5/include/hal/cache_ll.h)
confirms software tracks cache enable/disable state; no extra enable register is
invented. Later Suspend/Disable polls preload/autoload done. These disabled
engines report their documented idle/reset done value and retain disabled
configuration fields. Any preload/autoload start request rejects transactionally.

Six module tests and two existing CPU cache-access tests pass. Standalone module
and focused integration-test Clippy pass with warnings denied and no allowance.
PCR bus routing and original ROM/native/Worker acceptance remain integration
work. Freeze, lock, tag/data inspection, tracing, access-error IRQs, internal
ROM-cache maintenance and active preload/autoload remain unsupported.

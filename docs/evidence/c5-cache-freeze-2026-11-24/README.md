# C5 functional freeze mode0

This follows cache module `70c6478a` (integrated as `8f2b6a2d`). The original
LCD273 image subsequently reaches 277440 steps and faults at `0x4002c6d0`, reading
cache `+0x28` in `Cache_Freeze_Enable(0)`. It still has 101 UART bytes and no
application/pixels. This follow-up's checks cover the private module and actual
CPU/backing-array behavior; full-ROM acceptance remains integration work.

The fixed IDF v5.5.3/v5.5.4 sources are byte-identical, as retained in the
[cache source receipt](../c5-cache-boot-2026-11-24/checks.json).
[cache_reg.h](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/register/soc/cache_reg.h)
names EN16, MODE17 and RO DONE18. However, its miss/stall descriptions conflict
with the ACK_BUSY/ACK_ERROR enum in
[ROM cache.h](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/esp_rom/esp32c5/include/esp32c5/rom/cache.h);
the latter's function comment also differs from that enum. The audited TRM
material supplies no resolving freeze description. Mode1 is unsupported; no
choice among these physical miss-response descriptions is presented as fact.

The [C5 LL](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/esp32c5/include/hal/cache_ll.h)
requests mode0. The unchanged ROM itself clears MODE17, sets EN16 and polls
DONE18 (`0x4002c6d0`–`0x4002c6ec`). Disable clears EN16 and polls DONE18 clear
(`0x4002c702`–`0x4002c71a`). These exact lifecycle accesses are primary evidence.

The functional state is Disabled → Quiescing → Frozen. Quiescence is established
only for the explicit coherent passthrough representation: all prior backing
accesses have returned synchronously, and there are no resident cache lines,
pending fills or writebacks. It then acknowledges DONE. Reading status cannot
create a completion, and software cannot set RO DONE. Disable and cache reset
clear the state; clock gating preserves it while blocking cache/CPU access.
There is no cache-line allocation while frozen. Preload/autoload starts remain
rejected, as do mode1, reserved freeze fields and unknown bits. Invalid writes
preserve the prior state. No cache-hit/miss, ACK_BUSY/ACK_ERROR or physical timing
accuracy is claimed by this bounded mode0 abstraction.

Eight module tests and three cache integration tests pass. The new integration
test executes a real mapped `addi`, freezes the cache, programs the backing
flash through real SPI1 commands, then executes the changed instruction. Actual
CPU register results change from 37 to 0; data reads agree, no fill engine starts,
and invalid writes, bus masks, disable and reboot preserve their contracts.
Module and focused-test Clippy pass with warnings denied. No firmware changes,
bus/shared edits, broad builds, benchmarks or publication were performed.

# C5 LCD2.73 manufacturer references reach real application output

Unchanged manufacturer V55303_sd_card now mounts the64MiB FAT32 card, writes
71 bytes, closes/reopens the file, reads71 bytes and reaches its length/error/
memcmp success branch and TF CARD TEST PASS. Unchanged02_lvgl_demo completes
ILI9488 and button-driver initialization, starts LVGL, requests backlight and
writes102135 nonzero framebuffer pixels with no native fault. These are separate
manufacturer references, not FactoryOnly; no firmware/SDK or guest-state patch.

The common SPI failure was an overly broad rejection of DMA_CONF19/20, the
slave-segment buffer-clear policies. C5 IDF5.5.3 and5.5.4 master_init set them
while actual segment-enable18 is clear. All four inactive combinations now
work; real slave/segment/QPI/subbyte/overflow/unknown-clock modes still reject.
Tests retain the serial deadline and actual board callback, RX data and IRQ
completion path. Missing DMA data supplies no FIFO fallback or fake done.
USR24 remains a transfer; UPDATE23 synchronization is a separate limitation.
Cold native snapshots confirmed the old guard was the only rejecting predicate.

The following display stop was LP_AON_GPIO_MUX600b1028. C5 TRM1.1 section8.6.2
and the same IDF headers define SEL[7:0] resetting0 for HP ownership. Only this
inactive-LP subset is supported: read0/fullword write0; all nonzero, reserved
and partial writes stop ModelLimit. Existing full-span CPU permissions and
older watchdog-reset arbitration remain ahead of access. No LP pad/clock/IRQ
engine is implied. Live GPRs show GPIO2/RTC2(PREV), correcting an earlier BOOT28
hypothesis; GPIO28 remains HP-only. Natural aligned subword reads expose0 via
the common bus; writes alone are restricted to fullword zero.

All583 focused C5 cases pass, including six available developer-input tests,
with0 failures/ignored/filtered. Sixteen transport cases (five new) and three
new LP mux cases pass; both required Clippy commands pass with warnings denied.
No goldens regenerated. The original FactoryOnly still has identical work/UART
and the same strict private MAC ModelLimit; original UI acceptance remains false.

The [receipt](result.json) keeps earlier genuine failures, exact source/input/log
hashes, actual UART success branches and source reviews. Native does not export
its card in this probe; FATFS cache limits per-byte MISO claims. The framebuffer
count has not yet been visually checked; button response, current WASM/worker,
browser persistence, final global/CPU checks and publication remain separate.
The LVGL benchmark program is used for display functionality, without a speed
claim. Exact reported dispatched steps include scheduler quantum overshoot and
are not retired instructions or physical timing measurements.

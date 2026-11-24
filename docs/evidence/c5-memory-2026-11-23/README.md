# C5 memory subset: current execution evidence

Parent: `f61aae75fe4733fde6368d0ea9f9b8f4ef48019a`.
The exact source, original guest, ELF/bin and fresh toolchain/ROM inputs are
identified in [results.json](results.json), [inputs.json](inputs.json) and the
[source manifest](../../research/c5-memory-sources-2026-11-23/manifest.json).

## Result

All **22 C5 tests** passed, including the supplied rev100 ROM test; all-target
Clippy passed with warnings denied. The original SRAM diagnostic initialized
physical pads and SPI/QPI itself, opened AXI, mapped 128 pages, wrote and checked
all 2,097,152 words of the configured 8 MiB, tested final byte/halfword addresses
and executed code through that mapping. It emitted `C5 PSRAM8 PASS`, with
27,300,072 retired instructions, zero exceptions and zero resets. The runner's
28,000,000 dispatch slots include its final WFI wait; they are not all retired
instructions. See [guest.json](guest.json) and [execution.json](execution.json).

The actual Espressif rev100 ROM stops at 6,575 steps, at a read of
`0x600b2804` from PC `0x400269b6`. UART is empty and the bootloader has not been
reached. Passing the test means that this observable incomplete boundary was
recorded, not that factory startup succeeded. No analog host seed, guest patch
or synthetic ROM function return was used.

## Runtime cost

Five alternating pairs of the same original SRAM loop each dispatched and
retired 100,000,000 instructions. Counter values and full output hashes matched.
Median process CPU time was **10.132414 s** for main and
**10.468657 s** for this change, a
**+3.32%** increase. Individual samples, run order and
binary hashes remain in [cpu-comparison.json](cpu-comparison.json). This is an
inactive-memory C5 interpreter cost measurement, not a PSRAM throughput or
repository-wide demo benchmark. The environment was not reserved exclusively.

For reproduction, build `esp32c5 --example memory_idle --release` in each tree
with `CARGO_BUILD_JOBS=1 CARGO_PROFILE_RELEASE_LTO=false`; copy the identical
committed `esp32c5/examples/memory_idle.rs` into the baseline tree first. Invoke
each example with `100000000` in the retained alternating order, measure
process CPU time and compare all stdout bytes. The physical IO_MUX correction
intentionally changes hardware semantics; this loop performs no pad MMIO and
does not establish electrical compatibility with the former shifted addresses.

## Reproduction and bounds

```sh
CARGO_BUILD_JOBS=1 CARGO_PROFILE_RELEASE_LTO=false C5_ROM_ELF=/path/to/esp32c5_rev100_rom.elf \
  cargo test --release -p esp32c5 -- --include-ignored --nocapture
cargo clippy -p esp32c5 --all-targets -- -D warnings
cargo run --release -p esp32c5 --example psram_probe -- firmware/c5-psram-probe/probe.bin
python3 esp32c5/tools/generate_mspi_registers.py
```

The original UART fixture was rebuilt after correcting GPIO28/27/5 physical
IO_MUX addresses; its previous evidence files are unchanged historical records.
This checkpoint adds no product, changes no declared product capacity, and has
no browser/WASM or factory UI acceptance. External-memory GDMA, analog tuning,
PLL sources and cycle-accurate cache/serial timing remain unimplemented.

Build/test logs normalize local checkout, compiler and cache locations only;
original and sanitized hashes are retained. No unrelated machine/process or
account inventory is committed.

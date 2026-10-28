# Classic ESP32 PSRAM P0 contract (design, not implementation)

Historical P0 capture base (not current integrated ancestry): esp32sim
142805d5399e0c3cf25995fe21ba524e75a2b3cb. Firmware inputs:
Arduino-ESP32 2.0.17 (5e19e086c43d0fa5e5a596497ff8f11a0a43f6c2) and
3.0.7 (3bfa3e0a56c80305eec90f10e8318af8d8091bab), esp32:esp32:esp32cam,
stock dual-core SDK, 80 MHz PSRAM, BOOT_INIT disabled. recipe.sh installs each
release in its own directory; it does not patch cores, archives or SDK config.

This document preserves the historical P0 negative and proposed design below,
not current implementation status. The subsequent [integrated qualification](../../evidence/classic-psram-followup/README.md)
supersedes the stage restrictions and the original WASM invalid-size proposal;
existing callers now retain absent-device compatibility for unsupported sizes.
The captured register values below remain the provenance for the SPI tests.

P0 was a fixture/design slice only. Both genuine startup probes rejected the P0
model and setup prints found=0, heap=0, free=0, allocation=unavailable. No PSRAM
success is claimed. The SDK returns flash ID 0x00164020 on the unsupported classic
CS1 path, not a correctly modeled absent-device 0xffffff. Keep that observed
baseline distinct from the later implemented absent-device contract.

## Shared memory and WASM boundary: P1/P2/P7

These are the single proposed interface decisions for the implementation lanes;
independent review and supervisor ratification precede any public API implementation.
No implementation in this commit changes the constructor, bus or WASM ABI.

1. Capacity/presence: reuse SocBus::set_psram_size(bytes) -> Result<(), String>.
   Classic supports exactly 0 or 4194304 bytes. Default classic machine remains
   absent (existing machine(mac, flash_size) signature unchanged). Reject all other
   sizes without changing backing, mappings or registers. Native fixture configuration
   opts in before boot; CLI --psram-mb accepts 0/4 for classic through that same setter.
   Other chips keep their existing size behavior. No camera setting implies PSRAM.
2. WASM: retain esp32sim_new(board, board_len, flash_mb, psram_mb), units MiB.
   For a classic board 0 means absent, 4 means present; a different size returns null
   with an error, never silently ignores the argument. Today classic ignores this
   argument; implementing its handling belongs to P7, not P0. No extra export,
   host pointer getter, board JSON field, or add-on ABI is introduced by P0.
3. Owner: one bus-owned, fixed-length PSRAM Vec<u8>, allocated only when opted in.
   SPI commands and CPU mapping use exactly that allocation. Do not place a second
   Vec in ClassicSpi, a camera buffer, or the WASM wrapper. Reuse shared SpiMem's
   PSRAM payload and dirty-memory distinction; classic adapter owns only registers
   and phase decoding. ID is 0d 5d 22 for this emulated 4 MiB device; absent ID is ff.
4. Addressing: data-only [0x3f800000,0x3fc00000), PID 0/1, 32 KiB pages, 128
   entries starting at SRAM table index 1152. PRO table 0x3ff11200; APP 0x3ff13200.
   Physical offset is page*32768 + (address & 32767), bounded against backing size.
   Do not reuse flash's 64 KiB shift, flash address windows, or S3 PSRAM-select bit.
   Keep both existing table arrays; don't add shadow MMU copies. Flash unaffected.
   First milestone qualifies the stock matched PRO/APP mappings only (the public
   driver maps both identically). Tests must exercise both table storage banks.
   Core-specific divergent APP mappings/PIDs are not qualified; P2 must explicitly
   reject divergent live mappings rather than claim dual-core MMU correctness.
   If stock firmware needs divergence, stop for a separately scoped CPU-context
   change, not a PRO-only silent alias. Startup PRO-only mapping is allowed before
   APP is released; the active dual-core qualification requires matched entries.
5. Access/faults: all native and WASM CPU reads/writes use the same translator.
   Absent/invalid/out-of-bounds/unmapped -> Fault::Unmapped; present mapped but
   cache disabled/masked or unsupported page mode -> Fault::Prohibited. Unsupported
   instruction fetch from external RAM -> Fault::Prohibited. Preserve existing
   alignment rules (no new host-only acceptance of unaligned accesses). Validate
   full width/page edges before access; crossing an invalid page never writes a
   partial value. No flat always-present fallback or host allocator interception.
6. Functional coherence: write-through, synchronous command writes visible to CPU
   loads and CPU writes visible to command reads; no cycle/cache-line fidelity.
   A cache invalidate synchronously invalidates translated/generated mappings, not
   PSRAM bytes. Disabled/masked access faults until re-enabled; retained bytes then
   return. Follow classic DPORT and SPI0 SRAM controls, not a per-tick cache poll.
7. Fast paths/lifetimes: initially keep classic's existing helper-based Bus access
   (it has no fast_mem implementation on this base). WASM JIT slow memory helpers
   must exercise that translator, never expose a flat PSRAM range. If P7 adds a
   FastMem implementation, use existing TlbEntry/FastMem layout with with_span;
   no ABI layout change. Every MMU/control/presence change invalidates cached
   translations before the next guest access; every published backing pointer must
   remain live/unmoved while generated code can access it. Reconfiguration is only
   at a stopped-machine boundary and clears translations before replacing storage.
8. Lifecycle: a new machine's PSRAM bytes are zero. Guest reboot preserves the host
   capacity choice and the allocation's identity, but zeroes its bytes and resets
   guest chip/QPI/reset/cache state and MMU mappings to invalid. This deterministic
   emulator power-cycle policy is not a physical DRAM-retention claim. Reboot must
   invalidate generated/translated pointers before reset; repeated create/delete
   cannot inherit another machine's bytes. Other chips retain existing semantics.
9. DMA: classic dma_range remains internal-DRAM-only for descriptor and payload.
   CPU-visible external RAM is not a new DMA capability. Camera staging/copy stays
   guest code; final PSRAM frame placement never enables classic direct PSRAM DMA.
10. Boundaries: P1 owns SPI command/phase adapter behavior; P2 owns backing and
    translation; P3 owns cache gating/coherence/reset integration; P7 qualifies
    constructor argument propagation and actual WASM interpreter/JIT parity.
    No lane changes shared ABI layouts or invents a second memory API independently.
    bus.rs/periph.rs integration needs one sequenced owner with camera lanes.

The reset, error and matched-table restrictions above are emulator design choices,
not measurements of hardware. They must appear as tests and limits on those cards.

## Register and ROM map for the captured negative

IDF v4.4.7 components/esp_hw_support/port/esp32/spiram_psram.c and IDF v5.1.4
components/esp_psram/esp32/esp_psram_impl_quad.c define the public probe. Actual
linked archives/configuration are captured by provenance.py, not inferred from
vanilla IDF tags. Register definitions: each installed SDK's esp32 soc/spi_reg.h;
MMU: IDF v5.1.4 components/esp_hw_support/port/esp32/cache_sram_mmu.c:22-30,56-125.

SPI1 base=0x3ff42000, SPI0 base=0x3ff43000. CMD+0x00 USR bit18;
ADDR+0x04; CTRL+0x08; USER+0x1c (command31/address30/dummy29/miso28/mosi27);
USER1+0x20 (address length bits26..31); USER2+0x24 (command length bits28..31,
opcode low16); MOSI_DLEN+0x28; MISO_DLEN+0x2c; PIN+0x34
(CS0 disable bit0, CS1 disable bit1); W0+0x80. Lengths are bits-minus-one.

Both actual probe windows select PIN=5 (CS0 off, CS1 on), then restore PIN=6.
EXIT_QMODE is actually MOSI data f5, USER=08008000, MOSI_DLEN=7, command disabled;
do not decode stale USER2 as an opcode. READ_ID: USER=f0000000, USER1=5c000001,
USER2=7000009f, ADDR=0, MISO_DLEN=3f, two words read for the eight-byte ID.
W0 returns 00164020 and W1=0, twice; KGD check fails. Those are observed emulator
reads, not evidence that the chip is present. P1 also needs source-derived address-
only ENTER_QMODE 35 and reset/re-entry tests; those are unreachable in this negative.

Linked cache_sram_mmu_set differs: A2=40084fec, A30=40085f98 (IRAM wrappers).
Both cache_sram_mmu_set_rom=400097f4. Neither mapping function is reached after
failed ID. Thus there is no successful runtime MMU-write trace in P0. Future
mapping expectation is source-derived, not a fabricated successful trace: map
PRO then APP via wrappers/ROM depending on cache-guard state, 128 sequential
32 KiB entries at the table bases above; cache mask/page-mode controls must agree.
P1/P3 must unblock the probe before P4/P5/P6 capture and qualify that path.

## Reproduction and bounds

Use arduino-cli 1.5.1, run bash recipe.sh A2 OUT then A30 OUT. Run provenance.py
OUT EVIDENCE/sdk. From repository root fetch ROM assets with
`tools/fetch-demo-assets.sh --no-linux`; build with
`CARGO_BUILD_JOBS=4 CARGO_TARGET_DIR=TARGET nice -n 10 cargo +1.99.0 build --release -p esp32sim --bin esp32sim`.
Then `python3 docs/fixtures/classic-psram/capture.py --inputs OUT --emulator
TARGET/release/esp32sim --rom web/wasm/fw/esp32_rev300_rom.elf --output EVIDENCE/capture`.
Capture uses stock ROM boot, max-insns=150000000, no reboot loop, no stubs; retains
serial, bounded probe-only MMIO trace and instruction/exception/interrupt summary.
The CLI budget includes idle fast-forward work, so retired instruction totals are
less than the budget. Wall time is not a benchmark. Repeat serial/stop pins before
acceptance. SDK builds embed build metadata; rebuilt binary hashes may differ.
Record new hashes instead of asserting cross-directory binary reproducibility.

No firmware is installed as a CI golden by P0. These are external fixture inputs,
not a passing PSRAM regression test. No camera/WiFi/JPEG/hardware claim.

## CPU comparison

PENDING (not measured; no emulator code change).

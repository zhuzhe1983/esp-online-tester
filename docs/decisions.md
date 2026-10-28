# Design decisions and gotchas

Things that cost real time to find out, recorded so they do not have to be found twice.

## Core
- **Window overflow size** is the distance to the next set WindowStart bit, not the CALL
  increment. `CALLn` writes `a[n*4]` and that write is what triggers the spill.
- **Zero-overhead loops**: the loop-back happens only on fall-through to LEND, not when a
  branch inside the body targets LEND.
- **ROM data lives at its physical address**: the mask-ROM ELF's writable sections must be
  loaded and the reset handler's copy table back-filled, otherwise ROM code reads garbage.
- **Interrupt lines must be up to date before the next instruction** — recompute on register
  writes that flag `irq_dirty`; the 32-cycle poll is only a backstop for sources that change
  on their own (timers, DMA).
- **`salt/saltu`, `s32nb`, `any4/all8` operand masking, MAC16 `mx/my` bit positions** were
  all missing until the IDF 5.5 toolchain's output was run through the objdump test.
  The decoder test over several real binaries is the cheapest correctness tool we have.
- **PIE encodings come from the TRM, not from guessing**: the "Instruction Word" tables are
  machine-readable after `pdftotext`; the generated table is cross-checked by assembling
  every mnemonic with the real assembler. 24-bit forms live in op0 = 4, 32-bit in 0xE/0xF.
- **Coprocessor numbering**: FPU is CP0, PIE is CP3; both are gated by CPENABLE so FreeRTOS's
  lazy context save works — do not execute PIE with CP3 disabled.

## SoC
- **Derived clocks need delivered-tick accounting**: compute ticks from the running cycle
  total and deliver the difference; per-quantum rounding otherwise drifts and timers fire late.
- **`esp_restart()` on IDF 5 does not write a reset bit** — it arms the RTC watchdog through
  ROM `wdt_hal` and spins. Model the watchdog (stages, feed, write-protect key) and the
  `SW_PROCPU_RST` bit; reboot from the ROM with the right cause instead of stopping.
- **JEDEC capacity must follow `--flash-mb`**, or IDF's flash probe refuses a 16 MB image.
- **Octal PSRAM** is a device on SPI1 chip-select 1 speaking 16-bit commands
  (0x4040 read MR, 0xC0C0 write MR, 0x8080/0x0000 sync write/read); vendor 0x0D, density 3
  = 8 MB. IDF verifies with a write/read of `0x5a6b7c8d` at address 0.
- **GDMA in-channel registers sit at channel×0xC0 + 0x00…**, out-channels at +0x60…;
  `IN_PERI_SEL` is +0x48. The camera is trigger 5 on the in side.
- **LCD_CAM `cam_start` is CTRL1 bit 29 and takes effect directly** — not gated by
  `CAM_UPDATE`; `cam_ll_start` sets update first, then start.
- **LCD_CAM RGB output**: timing registers hold value−1; pclk = src/(div_num + b/a)/(clkcnt_n+1),
  clk_sel 3 = PLL160M, 2 = PLL240M; the engine's 16-word async FIFO runs ahead of the pixel clock and
  the RGB driver relies on that when it restarts the DMA link mid-frame (skips FIFO depth + 1 pixels) —
  without the lookahead the picture drifts 17 px per restart; a one-byte misalignment byte-swaps colours.
- **Touch controllers must latch**: LVGL polls the GT911 every 30 ms; a UI click shorter than that must
  stay readable until the driver has seen it, or taps are lost.
- **`cycles * 1e9 / CPU_HZ` overflows u64 at 76.86 s** — keep wall/emulated time in f64.
- **The I2S sample rate must come from the clock registers, never a constant.** The model
  hard-coded 44.1 kHz; the panel's SID player programs 22.05 kHz, so its DMA was drained at
  twice the rate the tune was rendered and it played at double tempo. Nobody noticed while the
  emulator ran at 0.5–0.8× real time — the two errors roughly cancelled — and it surfaced the
  moment the JIT made real-time pacing exact. The derivation follows the silicon:
  `MCLK = src/(div_num + b/a)` with `b/a` recovered from `x/y/z/yn1` the way `i2s_ll_tx_set_mclk`
  encodes them, `BCK = MCLK/(bck_div+1)`, `fs = BCK/(slot_bits·slots)`. It also showed the Atech
  Arduino build runs at 44 101 Hz, not 44 100 (its driver picked 76/442 instead of 76/441), so the
  regression WAV was re-baselined: same sample stream, five more samples over five seconds.
- **WebSocket sends must not block the emulator thread** — a frozen browser tab stalled
  emulation until sends moved to per-client writer threads with bounded queues.

## Boards and firmware
- **Atech's own Pocket Synth build drives the ST7735 over hardware SPI2 and reads the encoder
  with PCNT**; our PlatformIO build of the same sketch bit-bangs SPI and uses a GPIO ISR (older
  SDK modules). Both paths are modelled; the display decoder accepts bytes from either.
- **The real board boots app1 (0x340000)**: its partition table is the Arduino 8 MB OTA layout and
  `otadata` selects the slot with the higher sequence — a 1 MB flash dump is not enough, and
  `pio run -t upload` alone would not change what runs (erase `otadata` at 0xE000 first).
- **ST7735 output is decoded from bit-banged GPIO**: a full redraw is ~4.8 M instructions per
  20 ms (100 % of core 1), the one place the Pocket Synth firmware cannot run at real time
  in an interpreter (~70 Minsn/s dual core vs 240 needed). The UI absorbs it.
- **Direct-audio/PIE firmware runs ~55–75 Minsn/s**; when a firmware spins (WiFi PHY
  calibration with no RF) it eats half the emulator — see networking-plan.md.
- **The generated Pocket Synth firmware's waveform button was a stub** (all names
  "TRIANGLE", oscillator ignoring `waveform`); the emulator was faithful. Now replaced by
  the SID engine in the firmware's `lib/sid` ([atech-firmware](https://github.com/joakimeriksson/atech-firmware)).
- **Chrome remote-control clicks are not user gestures**: WebAudio stays suspended in
  automated tests; verify audio by WAV capture, not by listening.

## WiFi and networking
- **Emulate the MAC, do not shim `esp_wifi`.** A shim (or the OpenCores Ethernet MAC IDF ships a
  driver for) needs a firmware config change, so the binary under test stops being the binary that
  runs on the board. Modelling the MAC registers keeps "unmodified firmware" literally true, and
  the blob's state machine then has to be walked to CONNECTED by frames a real AP would send —
  no register shortcuts it.
- **A pure-Rust NAT, not libslirp.** Terminating TCP/UDP in the emulator and relaying over ordinary
  host sockets — the way Contiki-NG's NAT64 does — is a few hundred lines, adds no C dependency and
  needs no root, tun device or entitlement. The cost is what user-mode NAT always costs: no
  multicast/mDNS, and inbound needs explicit forwarding.
- **`DSCR_RELOAD` (0x60033084 bit 0) must not rewind the RX pointer.** Software rewrites
  `BASE_RX_DSCR` every time it recycles descriptors; treating that as "restart here" made every
  second frame land in a descriptor the stack had moved past, so it was batch-recycled instead of
  indicated (`wDev_ProcessRxSucData` `a3=0xa` rather than `0x1`).
- **`BASE_RX_DSCR` (0x60033088) restarts the receive DMA, but only once it has stopped.** *Inferred, not
  documented hardware:* no public header names the register, so the rule comes from the register sequence
  of the IDF 5.1 blob under emulation (a base write immediately before the frame that follows an exhausted
  list) and from the behaviour it explains (below). A driver may hand the hardware one short list at a
  time instead of one long ring: Arduino-ESP32 arms four descriptors with the last one's `next` null, and
  writes the base of the next four. The frame that arrives after such a list finds no descriptor
  (`dw0 == 0`), which is the DMA's stopped state; the base write is the hand-over, so the DMA takes it as
  the new head and retries that frame. Rewinding a *running* DMA is still wrong, for the reason above. An
  IDF 5.1 application needs this: before it, 314 of 315 frames were dropped and the station never
  authenticated; after it, association, DHCP and MQTT run (receipt:
  [evidence/wifi-rx-handover](evidence/wifi-rx-handover/README.md)).
  **Assumption of the latch:** `WifiMac::rx_handover` is cleared by the next frame that goes in, so the
  model assumes a driver publishes the next list's base *after* the current list is exhausted (the
  Arduino-ESP32 3.x blob does: 1 drop in 570 frames). A driver that published the base while the current
  list still had two armed descriptors would lose the hand-over and stall at `next = 0`; holding the latch
  until it is consumed is the change that would cover it, and the bench for it does not exist yet.
  **Order against the unarmed counter (#276):** the hand-over is tried before the "no ring armed" check, so
  a frame that finds `rx_next == 0` with a latched base takes that base; only a frame that still has neither
  a head nor a latched base counts as `unarmed`, which keeps the two counters disjoint.
- **`rx_ctrl` word 0 must set filter-match bit 28**, plus bit 29 for unicast. With bit 29 alone the
  frame is dropped inside `wDev_ProcessRxSucData` before it is ever indicated — silicon shows
  `0x111b20ad` for a broadcast beacon.
- **The EAPOL MIC covers exactly the 802.1X frame**, not the whole 802.11 payload, which can carry
  trailing bytes; and **group-addressed downlink frames need CCMP key id 1** (the GTK) or the
  station drops them silently.
- **Trim IP payloads to the header's total-length field.** An 802.11 frame carries a 4-byte FCS;
  without trimming, the NAT hands those bytes to the peer as TCP payload and the guest's real
  request then arrives at a sequence number the connection has already passed.
- **Debugging aid that paid for itself**: rebuild the specimen firmware with
  `CONFIG_WPA_DEBUG_PRINT=y` and `CONFIG_LOG_MAXIMUM_LEVEL_DEBUG=y` and read the supplicant's own
  verdicts instead of guessing from the emulator side.

## Crypto accelerators
- **They are not optional.** mbedTLS and the WPA supplicant route everything through hardware, so a
  missing or subtly wrong accelerator does not raise an error — it hangs in a polling loop or
  returns a plausible wrong answer. WPA2 died at handshake message 3 without AES; TLS hung in the
  MPI driver without RSA; certificates failed to verify with a wrong SHA.
- **RSA `0x818` is an idle status, not the interrupt latch.** It reads 1 whenever the unit is done
  and stays 1; `0x81c` clears only the interrupt signal. Model it as a latch and every
  interrupt-driven `mbedtls_mpi_exp_mod` deadlocks — the ISR clears the flag, then the result path
  waits for it forever. `0x808` (QUERY_CLEAN) must read 1 or firmware spins before the first op.
- **Compute the arithmetic exactly, ignore Montgomery.** The silicon works in the Montgomery domain,
  which is why the driver also loads M' and R⁻¹; a model that computes `X*Y mod M` and `X^Y mod M`
  directly produces the same results the driver expects, including the failover case where it sets
  M = 2^n − 1, M' = 1, R⁻¹ = 1 to get a plain multiply.
- **mbedTLS hashes through GDMA and asks for SHA-384**, so the block interface alone is not enough
  and the 64-bit SHA-512 core is required. H_MEM words read back byte-swapped, 64-bit state stored
  high-half first, so the driver's plain `memcpy` yields digest order.
- **AES CTR (block mode 3) is used by the TLS record layer**; executing it as ECB produces traffic
  the server answers with a fatal alert rather than anything diagnosable.
- **Check the primitives against published vectors, not against the firmware.** RFC 3174/2202/3394,
  FIPS-197, 802.11i Annex H, and 2048-bit modexp vectors generated with Python — every one of these
  bugs above would otherwise have looked like "the network is broken".

## Performance
- **Benchmark interleaved, never sequentially.** `tools/bench.py` runs several binaries in turn
  for N rounds and reports best and median wall time. Background load on this machine drifts by
  10 % over minutes; quick A-then-B comparisons produced "wins" of that size that vanished under
  interleaving (the first ablation figures for the fetch cache and the version check were both
  inflated this way). It also checks the guest instruction counts agree.
- **A host syscall in the tick costs more than emulating the CPU.** The NAT polled its sockets every
  scheduling round — ~7.5 M `recvfrom` calls per emulated second — which put 69 % of run time in the
  kernel and only 26 % in the interpreter. Polling on an emulated-time cadence (500 µs, far below
  anything the guest's TCP stack notices) made WiFi workloads 3× faster.
- **Nothing per-instruction may hash.** `--stub` and `--trace-fn` looked their PC up in a `HashMap`
  on every instruction; SipHash alone was 16 % of run time. A 64-bit bloom bit (`1 << ((pc >> 2) & 63)`)
  in front of the map removes it, and the map stays the authority.
- **Software TLB + per-page write versions** (`SocBus::lookup`, `page_ver`). Loads, stores and
  fetches resolve through a 512-entry direct-mapped table of 64 KiB pages instead of walking the
  address ranges and the flash MMU; every write bumps a version counter for its page, and a
  decode-cache entry stores the version it was decoded under, so the cache is validated by one
  indexed load instead of re-fetching and comparing the bytes. Anything that writes guest memory
  behind the bus's back (image loaders, the SPI flash controller) reports it with `note_written`,
  and anything that re-points the MMU calls `invalidate_tlb`. Measured: +4 % on the SID player,
  +2 % on the detector, nothing on the (mostly idle) panel. Two variants that did **not** help:
  a raw base pointer in the entry instead of a `match` on the buffer, and a version-index in the
  cache entry instead of a lookup — both within noise, so the safe/simple forms stayed.
- **The basic-block interpreter** (`xtensa-lx7/src/block.rs`) is what finally reclaimed the
  per-instruction scaffolding: SID player 93 → 133 Minsn/s (1.42×), panel 77 → 104 (1.34×),
  detector 63 → 78 (1.24×), Atech 113 → 153, with every regression output bit-identical — the
  SID capture is sample-for-sample the same. The rules that keep it exact are in the module
  doc; the ones that were not obvious: a block must never run past a `CCOMPARE` match (bound
  the length by the distance), `rsr/wsr CCOUNT` must be block-first so time is exact, and a
  block cut by the scheduling quantum must *resume* rather than spawn a new block at the cut
  point, or the cache fills with fragments.
- **The JIT is the block interpreter with the loop unrolled into machine code** — same block
  boundaries, same exit rules, same page-version validation — which is what makes `--no-jit`
  an exact oracle: every regression output is bit-identical between the two. On top of the
  block interpreter it measured 1.23× (panel), 1.30× (detector), 1.37× (Atech), 1.46× (SID);
  the SID player now runs 193 Minsn/s, 2.1× real time. Decisions that mattered:
  a hand-written encoder for the ~90 instructions used, checked word-for-word against clang
  (`cargo test encodings_match_clang`) rather than a dependency; anything not inlined calls
  `exec_insn` for that one instruction and carries on natively, so coverage grew op by op
  with the oracle green at every step; the register window base is cached per block and
  reloaded after any helper that could rotate it; the window-overflow test is emitted once
  per frame count per block (windowstart only changes through helpers); shared exit tails
  and an offset-based `lend` compare cut code size 2.7× (the first version filled 48 MB and
  thrashed). `MAP_JIT` works for a plain cargo binary on macOS; `pthread_jit_write_protect_np`
  brackets every write.
- **IRAM and DRAM are one SRAM, so code and data share version pages.** With 4 KiB pages the
  app's `.dram0.data` (starting at `0x3FC9C300`) sat in the same page as the end of IRAM text
  (`0x4038c300`), and every global-variable write invalidated `_xt_context_save` — 1.9 M block
  rebuilds in 12 s. 256-byte pages cut that 9×. Any scheme that keys on address ranges must
  remember that `0x4037_0000 + x` and `0x3FC8_8000 + x − 0x8000` are the same byte.
- **MMU remaps must invalidate decodes.** Page versions only change on writes; re-pointing a
  cache-window pc at different flash bytes is not a write. `invalidate_tlb()` therefore bumps
  every flash and PSRAM page version — cheap, and it makes the decode and block caches correct
  across the bootloader → app handover without a separate epoch.
- **Device time is lazy but exact.** Cycles accumulate in the bus and the device models see them
  in one batch when a systimer/TIMG alarm is due (`Peripherals::cycles_until_timer`, conservative
  by one device tick), when a peripheral register is read or written (flushed first, so registers
  always show exact time and a write that arms an alarm re-plans immediately), or after
  `MAX_TICK_DEFER` = 256 cycles for everything without a computed deadline. On S3, interrupt lines are
  re-derived when a source may have changed: clocked-source transitions, register writes, DMA
  completion, board edges or host input. A quiet time flush alone does not require a scan.
  Read-driven source changes request a block exit before execution continues.
- **Timing granularity changes shift phases, not content.** Deferring device events by up to
  256 cycles moved the moment LVGL's 30 ms touch poll saw the play tap by 16 ms; the SID audio
  after that point is sample-identical. The Atech WAV, whose script is what the regression
  checks, is bit-identical. Lengthening the *idle* step from 512 to 2048 cycles (+3 %) did change
  the Atech WAV, so it was rejected: bit-identical regression output is the bar.
- **Cache the instruction-fetch *mapping*, never the bytes.** Superseded by the TLB above, but the
  rule stands: code rewritten in place must fetch fresh, and every MMU change must invalidate.
- **`lto = "fat"`, `codegen-units = 1`**: 11 % for a 7-second build. `-C target-cpu=native` gains
  nothing on Apple Silicon, where the default target already is the host.
- **Scheduling quantum 64**, not 32: half the device-tick overhead for ~9 % more throughput, and the
  Atech WAV regression stays bit-identical. 128 gains almost nothing and costs interrupt latency.
  This remains the native default. Since x8 the browser (wasm32) build defaults to 256 (M3 Chrome
  fluidbox −33 % wall time, different work); native keeps 64 because native Pocket Tank ran 5.38 %
  slower at 256 ([EX047](experiments.md#ex047)).
- **Free things, measured so nobody removes them for speed**: the three `ccompare` checks in
  `advance_ccount`, the per-instruction stub/probe/breakpoint/trace checks in `step_core`, and the
  decode-cache size (32 K, 64 K and 128 K entries all perform the same — it could shrink).
- **Generated code probes the TLB itself.** The `TlbEntry` layout is `#[repr(C)]` in the core
  crate and the bus hands the JIT its table and write-version pointers (`Bus::fast_mem`), so a
  load is index, two compares and one host load. Stores also bump the page version inline, but
  only when the bump touches a single page away from its first three bytes — the edge cases go
  through the helper, which keeps one implementation of the straddle rules. Worth 1.1–1.2× on
  everything except PIE-heavy code, whose memory traffic sits inside fallback instructions.
- **What is left** (exclusive time, SID): ~36 % generated code, ~29 % the dispatch between
  blocks (lookup, validation, bounds, prologue/epilogue), ~9 % slow-path memory, ~6 %
  fallbacks. Direct block chaining is next; register caching and inlining `call8`/`entry`/
  `retw` after that.
- **Profile the emulator, not just the guest.** `--profile` reports guest PCs and disables idle
  skipping, so an idle core shows up as a hot `waiti` — an artefact, not work. For emulator-side
  cost use `sample <pid>` (macOS) against a normal run, and confirm with an ablation build.
- **Check for leftover runs before benchmarking.** A background emulator from an earlier session at
  100 % CPU is indistinguishable from "the emulator got slower".

## Architecture (the 2026-09 refactor)
- **One table per chip, expanded to static calls.** The first `esp-periph` dispatch was a table of
  fn pointers and `dyn Device` calls with a runtime divider table for the clocks; it measured
  0.57 → 0.87× of baseline after three rounds of tuning, because `source_status`, tick delivery and
  the timer-deadline query each turned ~25 inlined field checks into ~50 indirect calls, and the
  clock divisions stopped folding to multiplies. `device_set!` keeps the table as the single source
  of truth and generates the read/write match, the source scan, tick delivery and the deadline
  query with direct calls — 0.98× of baseline, and a peripheral is still one line.
- **Nothing per-block in the cores for features they do not own.** A cost-model branch in the LX7
  block loop (`match &cpu.cost`) cost 4 % by itself; the same branch in the machine after
  `Core::run` is within noise. Hooks for analyses, cost models and observers live in
  `Machine`, next to the block boundary the core already returns to; the cores stay 1 IPC.
- **Observers pay only for what they ask.** `Wants` bits decide per hook whether the machine calls
  anyone. `INSN` enables per-instruction hooks; `TRAP_PC` bounds block runs to one instruction
  for exact fault-PC attribution. Ordinary `TRAP` supplies the run-entry PC and post-trap CPU
  state without fragmenting execution. The shipped `--irq-latency` and `--vcd` observers do not
  consume that PC and use ordinary `TRAP`. Context cycles still describe the scheduler's time;
  requesting an exact PC does not provide finer interrupt-latency timestamps. `NO_IDLE_SKIP`
  changes idle execution and can change emulated timing.
- **The C3's `Core::run` stops at `block_break`**, like the LX7's block interpreter, so the machine
  re-derives its interrupt line after the same instruction the old per-step loop did. Without it
  the shared run loop would have delayed C3 interrupts by up to a quantum.
- **A reset takes effect at the instruction that requests it.** The per-core loop checks
  `sw_reset` after every block, not only at the quantum boundary; the core no longer runs on
  through the ROM's `ret` after the reset store. Costs a bool per block; the C6 ROM's `Saved PC`
  is the observable.
- **The guest's cycle counter restarts at a chip reset; the emulator's instruction count does
  not.** `mcycle`/`mpccr` read `insn_count - cycle_base`, and a guest write moves the base rather
  than the count. The bootloader's log timestamps are cycles since reset on silicon (`I (23)`),
  and the old model printed 44 s after the first `esp_restart()` on both RISC-V chips; the goldens'
  instruction counts also stopped moving when firmware wrote the counter.
- **One interrupt controller, two register maps.** ESP-IDF on the C6 drives the PLIC at
  `0x20001000` while the ROM uses INTPRI at `0x600C5000`; both are views of the same enable/type/
  priority/threshold state in `esp32c6::periph::Intc`, and the software-interrupt latches live
  with it. The threshold is PLIC+0x90 — the handler raises it to the taken line's priority + 1
  before enabling nesting, and with it misplaced the systimer line re-entered until the stack had
  walked through all of SRAM.
- **A GP-SPI transfer reaches the board when it happens, after the GPIO edges before it.** The
  ST7789's D/C line is a GPIO the LCD driver sets right before each SPI transaction; delivering
  GPIO changes and SPI bytes only at the end of a scheduling round interleaved them wrongly. The
  C6 bus delivers both at the transfer (GPIO first), and the DMA data phase is fetched from the
  GDMA out-channel then, so ordering is exact and the display driver's DMA queue depth is moot.
- **Blob calibration that needs undocumented hardware is a stub, not a guess.** The C6 PHY's
  `bb_init` spins on handshake bits in a register block no header describes. The energy-scan
  recipe stubs that one function (`--stub bb_init=0`), keeps every other PHY step, and the MAC
  model is written not to depend on the PHY at all. The S3 panel's `esp_wifi_start=0` is the same
  policy. What the blob *polls over regi2c* is modelled (the RF block's SDM status reads 0x5b),
  because that is a documented bus with an observable value. Superseded for `bb_init` by #203
  ([EX220](experiments.md#ex220)): two fields of that block could be inferred from the blob's own
  code (TX-DC done, the tone readback), and with them modelled the calibration completes in 23.8 ms
  of device time, so the C6 recipes run it and the stub is gone (#248). The policy stands where no
  register behaviour can be inferred.
- **The RISC-V core stops at stub and probe addresses.** `Core::set_boundaries` existed for the
  Xtensa block interpreter; the RISC-V `run` now checks the same bloom per instruction. Before,
  `--stub` on the C3/C6 only worked when a periph write happened to end a run at the right pc.
- **Hardware-fixed register fields read their silicon values, not zero.** PCR's HRO clock-tree
  dividers and frequency query fields are what the clock code derives the CPU frequency from;
  a register-RAM zero produced a divider the HAL asserts on. The rule generalises: when a
  firmware *computes* from a field, the model must report the fixed value.

## WebAssembly build
- **The SoC crate compiled for `wasm32-unknown-unknown` without a single change** — `std::net`,
  threads and files all *compile* there and fail only when used. The hazards are the ones that
  panic at runtime: `Instant::now()` and `SystemTime::now()`. Those live behind `host.rs`; the
  worker passes `Date.now()` in with every run slice. Never call `Instant` on a path the wasm
  build can reach (real-time pacing is the worker's job there).
- **Never read the process environment on a path the wasm build can reach.** `std::env::var`
  answers "not present" on wasm32-unknown-unknown, but `std::env::vars()` aborts there since the
  std of Rust 1.9x (it used to yield nothing). `DebugFlags::from_env` iterated it at machine
  creation, so the day Pages built with that toolchain every demo died in `esp32sim_new` with
  "not supported on this platform". Nothing in the repository had changed. The env reading is now
  compiled out for wasm32, next to the `Instant::now()` rule above.
- **Reuse the WebSocket protocol, not the WebSocket.** Giving `WebServer` a queue mode meant zero
  changes to `Machine`'s push/poll code and a ~10-line change to the page. The one trap: the
  per-client "hello" snapshot (board name, console backlog) is built only for new socket
  clients, not in queue mode — the page kept the Atech layout for the panel until the wasm
  boot sent the board announcement itself.
- **No bindgen.** A dozen `extern "C"` functions plus `esp32sim_alloc`/`free` for buffers; the
  worker copies messages out of wasm memory (re-creating views after every call, since memory
  can grow). Kept the zero-dependency rule and the module at 4 MB unoptimised.
- **Sizes**: the block-cache tables are 4× smaller on wasm (`block.rs`), the arena would
  otherwise reserve 64 MB up front. The 480×480 panel at 50 frames/s moves ~23 MB/s through
  `postMessage` with transfer lists and is fine; copying instead of transferring is not.
- **Measured**: ~62 Minsn/s with the SID player, real time for all three boards in Chrome —
  more than the spike's 47 % of native predicted, because the block interpreter arrived after
  the spike. The JIT does not exist in wasm; a wasm-emitting backend is the next step for speed.

## Hardware differential testing
- **A device model must complete a command when the guest issues it, not on the next tick.**
  Firmware routinely writes a command register and reads the result a handful of instructions
  later; anything deferred to a scheduling-quantum boundary loses that race and returns stale
  data. Found on the C3, where a SPI flash ID read came back as zeros on one boot path and not
  the other — the kind of bug that looks like firmware flakiness until you diff against silicon.
- **What is board wiring must survive a chip reset**: flash JEDEC capacity, strapping pins, the
  MAC. Re-creating the peripheral set on reboot silently reverted all three to defaults, and the
  emulator started reporting a different flash size from the second boot onward.
- **Give the emulator flags to adopt a board's identity** (`--mac`, `--reset-cause`, `--strap`).
  Without them a comparison drowns in differences that are just "this board was reset over USB
  and yours was a cold power-on", and the real mismatches hide among them.
- **Normalise timestamps before diffing.** The emulator does not model flash read latency, so its
  log timestamps run ~10x faster through boot; every line differs until `s/[0-9]\+/t/` on the
  `I (nnn)` prefix, after which the comparison is exact and the residue is real.

## Process
- **Golden outputs are the regression bar, and they run in CI.** `cargo test --release --workspace
  -- --include-ignored` runs the committed demo firmware (Atech script1 with and without the JIT,
  the SID jukebox, the Touch-LCD-4B panel, hello_world on the S3 and the C3) and compares the
  console text, the audio's SHA-256 and the instruction count with `tests/golden/`. ~3 s for all
  of it. The tests are `#[ignore]`d only because they need the mask ROM ELFs; they never skip
  silently. Baseline on an Apple M5 Max (`tools/bench-goldens.sh`, best of 5, release, JIT):
  Atech script1 5 s → 1.45 s wall (214 Minsn/s), SID jukebox 6 s → 1.33 s (245), panel 7 s →
  2.30 s (172). A refactor phase is accepted when the goldens are unchanged and these are within
  the 10 % drift band; a speed change is accepted when the goldens are unchanged, full stop.
- Bring-up loop: run → first unknown register / unimplemented instruction (`--log-periph`,
  `Unimplemented(pc, raw)`) → model it → rerun. Keep the objdump test and the hardware
  differential test green after every core change.
- Reference emulator for behaviour questions: Espressif's QEMU (`~/.espressif/tools/qemu-xtensa`),
  never for code.

## Execution-boundary policies (September 2026)

- **S3 peripheral accesses are aligned words.** Both reads and writes of peripheral registers,
  including the MMU table, require aligned 32-bit accesses. Byte and halfword accesses raise
  `Fault::Prohibited`; unaligned words raise `Fault::Misaligned`, before device reads, writes,
  time flushing or mapping changes. Synthesizing a narrow write with read-modify-write can pop
  a FIFO or corrupt write-one-to-clear registers. Narrow reads are rejected consistently rather
  than silently invoking a full-word device read. This is an emulator policy for unsupported
  accesses, not a claim that silicon under every PMS configuration raises the same CPU fault.
- **Idle advances stop at known boundaries.** The former fixed 512-cycle skip could overshoot
  core timers, device deadlines, scripts and run limits. Both run entry points now bound each
  skip by those deadlines; active cores retain their scheduling quantum (64 natively, 256 on wasm32). This
  deliberately changes instruction totals in fixed-time workloads and makes cycle limits exact.
- **Read-driven interrupt changes are visible before the next block.** An MMIO read flushes
  pending device time. If a source changes, it requests a block exit and CPU-line refresh rather
  than waiting for a later periodic flush. Unchanged sources do not break polling blocks.

The accepted golden shifts are attributed to isolated changes in the
[correctness evidence](evidence/browser-correctness-2026-09-05/README.md): the operand correction
removes false register-window exceptions (hello and reboot counts, four reboot timestamps by
1 ms, and part of the panel count). Read-driven notification and bounded idle advances change
the Atech counts and the remainder of the panel count. Existing audio hashes are unchanged.
The native performance follow-up changes no goldens; exact-PC observers are now separately
opted into so ordinary trap analysis need not pay their fragmentation cost.


## BLE controller substitution

BLE currently substitutes the guest controller API behind opt-in `--ble`. The
S3/C3 VHCI adapters and the C6 native NimBLE adapter exchange H4 packets with
`esp_soc::ble::HciController`. The boxed controller has reset and cycle-based
advance/deadline methods. A separate peer owns scripts, ATT and the Battery
Service and exchanges packets over `ble::Link`; a replacement controller must
bridge that link, leaving scripts unchanged.

WiFi follows "emulate the MAC, do not shim" because its modeled registers and
DMA descriptors let the unchanged guest driver run. BLE has no equivalent
baseband model here. Substitution exercises the unchanged host stack and Arduino
sketches now, with guest tasks and allocators retaining their calling conventions.
It does not claim RF or connection-event timing, pairing, encryption sessions or
controller register fidelity. A fuller HCI controller can replace this one; a
future baseband model can remove the substitution. C6's native mbuf/NPL ABI is
restricted to IDF 5.5.x at enable time. The disabled path installs no hooks.

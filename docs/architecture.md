# Architecture

esp32sim emulates three ESP32 SoCs across both of Espressif's CPU architectures: the **ESP32-S3**
(two Xtensa LX7 cores), **ESP32-C3** (one RISC-V RV32IMC core, see
[esp32c3.md](esp32c3.md)) and **ESP32-C6** (one RISC-V RV32IMAC core, see
[esp32c6.md](esp32c6.md)). They share the SoC peripheral models wherever the IP is identical and
differ in their CPU crate, memory map and interrupt controller. This document describes the S3,
which has the most complete model; the C3 and C6 follow the same shape.

esp32sim is an instruction-level emulator of the ESP32-S3: it executes the real mask ROM, the
real second-stage bootloader and an unmodified application image on two emulated Xtensa LX7
cores, with the SoC peripherals modelled at the register level and the board (what hangs off
the pins) modelled as devices that interpret pin-level events. It is written in Rust, MIT
licensed, and contains no third-party emulator code (QEMU was consulted for instruction
*semantics* only).

```
cli/          esp32sim binary, every chip (`esp32sim-c3`, `esp32sim-c6`, `esp32sim-p4` are aliases): argument
              parsing, image loading, run loop, reports
              (`--chip esp32|s3|c3|c6|p4`; the setup that is chip-specific is one function per chip)
esp-soc/      Machine<S: Soc>, written once for every chip: the scheduler (256-instruction quanta
              on wasm32, 64 natively; idle skipping, per-core reset state), lazy device time,
              console capture, action scripts, function stubs/probes, tracing and watchpoints,
              the web UI protocol, real-time pacing, ROM/app image loading, reboot; the
              Soc/SocBus traits a chip implements; the BoardModel trait; elf/image/picture
              loaders; the web server
  wifi.rs     the virtual 802.11 access point (beacons, probe/auth/assoc, WPA2 four-way
              handshake) and StationLink, the station's side of the air every chip's MAC shares
  net.rs      the emulated subnet 10.0.2.0/24: ARP, DHCP, ICMP, DNS, SNTP
  nat.rs      user-mode NAT: guest TCP/UDP relayed over ordinary host sockets
esp32s3/      the SoC and boards
  soc.rs      the S3 as a Soc: two LX7 cores, core-1 reset/stall state, interrupt lines per core,
              app boot, reboot (what survives), console streams, audio, board
  bus.rs      SocBus: memory map, cache MMU, peripheral dispatch, DMA pumps (I2S, camera)
  periph.rs   the S3's device table (`device_set!`) and its S3-only models (interrupt matrix,
              WiFi MAC, PCNT, GP-SPI, LCD_CAM, EXTMEM, WDEV, regi2c); the rest come from esp-periph
              — formerly: register models (UART, USB-Serial/JTAG, systimer, TIMG, interrupt matrix, GPIO,
              RTC_CNTL + WDT, efuse, SPI0/1 flash + PSRAM, SHA, AES, RSA/MPI, RNG, regi2c,
              GDMA, I2S, RMT, LCD_CAM, WiFi MAC)
  i2c.rs      I2C master controller + bus devices (CH32V003, OV5640, ES8311/ES7210)
  board/      one file per board: Atech14, WaveshareCam, WaveshareLcd4b, WaveshareAmoled18V2 (BoardModel from esp-soc)
esp-periph/   the peripheral IP Espressif chips share, one file each (UART, USB-Serial/JTAG,
              systimer, TIMG, GPIO, RTC_CNTL, efuse, SYSTEM, SPI_MEM, GDMA, SHA/AES/RSA, I2S, RMT,
              I2C), the `Device` trait they implement, and `device_set!`: the one table per chip
              that generates dispatch, interrupt sources, clock ticks and timer deadlines
emu-core/     what every core shares: the Bus contract (with the TLB/page-version hooks a JIT
              needs), the Core trait a machine drives, Trap, ClockTree, and the AArch64 encoder
xtensa-lx7/   the core
  decode.rs   instruction decoder (24/16-bit base ISA, FPU, MAC16, booleans) -> Insn
  pie.rs      PIE SIMD (ee.*) decode/format/execute; pie_table.rs is generated from the TRM
  exec.rs     interpreter: windowed registers, loops, XEA2 exceptions/interrupts, CP enable
  block.rs    basic-block cache and block interpreter (the normal execution path)
  core.rs     the `emu_core::Core` implementation the machine drives
  jit/        native code generation for blocks: mod.rs compiler + helpers (encoder in emu-core)
  state.rs    Cpu: registers, special registers, user registers (ACCX/QACC/…), interrupt levels
  disasm.rs   objdump-compatible formatter (used by the differential decoder test)
web/          index.html: the landing page; run.html: board drawing, console, WebAudio, camera panel (no build step)
              emu.js + wasm/worker.js: the same page driving the WebAssembly build (docs/wasm.md)
wasm/         esp32sim-wasm: C ABI over Machine plus the guarded browser-JIT handoff
wasm-jit/     receipt-priced wasm emitter; first SRAM opcode slice, shared-memory sidecars
riscv-rv32/   RV32IMAC decoder (sampled objdump checks), interpreter, disassembler; RV32F
              (soft-float) and its lazy-FPU traps for the ESP32-P4, behind misa.F
esp32c3/      the C3 SoC: memory map, interrupt matrix, cache controller, RNG; peripheral models
              shared with esp32s3 where the IP is identical
esp32c6/      the C6 SoC: unified memory map and MMU, interrupt matrix + PLIC/INTPRI, L1 cache,
              PCR, the LP blocks (reset cause, software reset, RTC timer), ASSIST_DEBUG,
              the 802.15.4 and WiFi MACs
esp32p4/      the P4 SoC (rev < 3 and rev >= 3, two cores): memory map with L2MEM and the flash and PSRAM MMUs,
              interrupt matrices and per-core CLICs, HP/LP system blocks, PSRAM (esp32p4.md)
              and `coproc`, the P4 with an ESP32-C6 on SDIO in lock-step (`--coproc`, the browser demo)
tests/        golden-output regression tests and their fixtures (tests/README.md)
hw/           JTAG differential-test scripts against a real board, captured C3 and C6 consoles
              (hardware-testing.md)
examples/     firmware sources: hello_world (S3, C3, C6), waveshare-cam, wifi-station,
              c6-wifi-station, c6-radio-dashboard, ttgo-tdisplay, …
tools/        PIE table generator (TRM-derived), bench.py (interleaved A/B benchmark),
              wasm-build.sh (the WebAssembly module), fetch-demo-assets.sh
```

## CPU core (`xtensa-lx7`)

- **Decoder**: `decode(pc, bytes) -> Insn` with fields `op, r, s, t, imm, imm2, len, raw`.
  Earlier full-listing runs reported 977,544 instructions with zero mismatches over the
  Pocket Synth app, mask ROM, IDF 5.5 bootloader, `hello_world` and autopling image.
  CI checks sampled corpora; the optional full-listing test requires external inputs.
  PIE operand comparisons are excluded by [the decoder test](../xtensa-lx7/tests/objdump_diff.rs).
- **PIE**: all 217 `ee.*` encodings come from the TRM chapter-1 "Instruction Word" layouts
  (`tools/gen_pie_table.py` + `tools/pie_trm.json` → `pie_table.rs`), cross-checked against
  the ESP-IDF 5.5 assembler. Execution follows the TRM "Operation" pseudo-code; PIE is
  coprocessor 3, so `CPENABLE[3]` gates it and FreeRTOS's lazy save/restore works unchanged.
  The hot instructions of inference kernels (128-bit load/store with post-increment, signed
  8/16-bit multiply-accumulate into ACCX with and without the load, ACCX reset) are packed at
  decode: `pie::pack` puts their operands in the `r`/`s`/`t` a PIE `Insn` otherwise leaves zero, and
  `exec_packed` runs them without re-extracting fields, loading through `Bus::read_bulk` in one
  copy and computing the dot products over byte arrays. The table executor stays the reference;
  randomized tests hold the two to identical state, faults included.
- **Interpreter**: `exec_insn` executes one decoded instruction. Register windows are modelled
  with the 64-entry physical file and WindowBase/WindowStart, including overflow/underflow
  exceptions raised at the *instruction that would touch* the missing window (see
  decisions.md). Timing is 1 instruction = 1 cycle.
- **Basic-block interpreter** (`block.rs`, the normal path): a block is a straight-line run of
  up to 32 pre-decoded instructions ending at a control transfer or at anything that changes
  interrupt, timer or window state. The interrupt check, cache validation and CCOUNT/insn
  accounting happen once per block; window-overflow checks stay per instruction. Exactness is
  kept by bounding a block at the next CCOMPARE match, forcing CCOUNT/CCOMPARE/PS/INTENABLE
  accesses to start a block, ending a block when the bus reports an interrupt-line change, and
  comparing the actual `pc` with the fall-through address after every instruction. Blocks are
  validated by the write-versions of the pages they were decoded from (256-byte pages, see
  decisions.md). `step()` — one instruction per call, with a 16K-entry decode cache — remains
  for tracing, profiling, breakpoints and watchpoints.
- **JIT** (`jit/`, AArch64 on macOS/Linux): every block is also compiled to native code with the
  same exit rules, so the interpreter is the oracle (`--no-jit` must give identical output).
  ALU, shifts, moves, compares, all branches, and loads/stores through an inline probe of the
  bus's TLB (`Bus::fast_mem`) are native; misses and peripheral addresses call bus helpers; everything else (calls, returns, `entry`, special registers, FPU, PIE, MAC16) calls
  back into `exec_insn` for that one instruction and continues natively. Guest registers are
  addressed as `ar[(windowbase*4 + n) & 63]` from the window base cached at block entry;
  the window-overflow pre-check is emitted once per frame count per block. `jit/a64.rs` is a
  ~90-instruction encoder checked against clang's assembler; code lives in a `MAP_JIT` region.
- **Traps**: `Exception(cause)`, `Interrupt(n)`, `Unimplemented(pc, raw)`, `Simcall`. The
  machine counts them; `--stop-after-exceptions` and unimplemented instructions stop the run.

## SoC (`esp32s3`)

- **Memory map** (`bus.rs`): SRAM (IRAM `0x4037_0000`, DRAM `0x3FC8_8000` aliases of one
  buffer), mask ROM (`0x4000_0000` I / `0x3FF0_0000` D), RTC fast/slow RAM, the flash/PSRAM
  cache windows (`0x3C00_0000` D-bus, `0x4200_0000` I-bus) translated by the 512-entry MMU
  table at `0x600C_5000` (flash pages or PSRAM pages), peripherals `0x6000_0000–0x600D_0000`.
  Cache timing is not modelled; XIP from flash or PSRAM is a table lookup. A software TLB
  (512 entries of 64 KiB) caches resolved mappings for loads, stores and fetches, and a
  per-256-byte-page write version lets the CPU's block and decode caches skip re-fetching
  instruction bytes; MMU remaps bump the flash/PSRAM versions so decodes built through the old
  mapping cannot run.
- **Peripheral dispatch**: address bits 12–19 select a block. Each chip lists its devices once in
  a `device_set!` table (block, name, field, interrupt source numbers, optional offset range);
  the macro generates the read/write match, the source-status scan, tick delivery per clock
  domain and the timer-deadline query as static calls, so a device costs what a hand-written
  arm did. Unknown registers land in a generic register RAM and are logged on first touch with
  `--log-periph`. The three registers whose value depends on another device (interrupt status,
  the MAC's TSF timestamp, the FE's IQ-done bit) are handled in `DeviceSet::pre_access`.
- **Interrupts**: every source has a level computed by its model (`Peripherals::source_status`);
  the per-core interrupt matrix maps sources to the 32 Xtensa interrupt lines. Lines are
  recomputed after delivered device ticks or a change flags `irq_dirty`, then presented to
  the core before execution continues. The scheduler does not poll them every 32 cycles.
- **DMA**: GDMA out-channels feed I2S0/I2S1 (audio → `pcm` samples at the configured rate),
  in-channels are fed by the LCD_CAM camera engine (one frame per sensor period). Descriptor
  chains are walked in guest memory exactly as the driver builds them.
- **Reset**: `Machine::reboot()` re-creates the digital peripherals, keeps SRAM, RTC memories,
  efuses and the captured audio, sets `RESET_CAUSE`, and restarts both cores at the ROM reset
  vector — the path used by `esp_restart()` (RTC watchdog) and `SW_PROCPU_RST`.

## Observers

Analyses attach to a run without touching the scheduler (`esp-soc/src/observe.rs`). An
observer says what it wants — `INSN` (every instruction: the run single-steps), `BLOCK` (every
block the fast path ran, at full JIT speed), `TRAP` (trap notification), `TRAP_PC`
(exact fault-PC notification using one-instruction fragments), `IRQ` (a line appears at a core), `MMIO`,
`GPIO`, `ROUND` — and the machine only pays for hooks somebody asked for. The classic tools are
observers now (`--trace`, `--break`, `--watch`, `--regtrace`, `--profile`, `--regstat`), and so
are the full-speed analyses: `--profile-blocks` (time per function, no JIT penalty, no timing
change), `--coverage[-file]` (block starts per function), `--irq-latency` (raised → taken per
line), `--vcd` (GPIO edges and interrupt lines as a waveform). These two use ordinary `TRAP`
callbacks and retain block execution. `TRAP` receives the run-entry PC and post-trap CPU state;
`TRAP_PC` requests exact instruction attribution, at a throughput cost. Only `INSN` observers
force the single-step hooks, and only those that say `NO_IDLE_SKIP` change emulated timing.
`--profile` requests it. Breakpoints instead inspect sleeping PCs at scheduling boundaries
without disabling idle skipping (`IDLE_PC`).
A `CostModel` (`Machine::set_cost_model`) switches the
machine to a per-event path that records the conceptual fetch, CPU bus accesses, control event,
trap timing and next pc. The model may refuse any event it cannot price.

## Scheduling and time

Without a cost model, `Machine::run` interleaves the cores in quanta of 256 instructions in the browser (wasm32) build and 64 natively ([EX047](experiments.md#ex047)); `esp32sim_set_quantum` selects 64–4096 in multiples of 64. A core sitting in `waiti`
with nothing pending costs nothing. When both cores are idle, each advance is at most
512 cycles and is shortened to the earliest enabled-core wakeup, bus deadline, script event,
cycle limit or remaining instruction allowance. The S3 bus deadline also bounds idle steps to
its pending device-time flush. Device models see time lazily: cycles accumulate in the bus and are delivered in one
batch when a timer alarm is due, when a peripheral register is accessed (so registers always
read exact time), or after 256 cycles at most. Peripheral clocks (APB 80 MHz, systimer 16 MHz,
RTC slow 150 kHz) are derived from the 240 MHz cycle counter with delivered-tick accounting.
With `--web` the machine is paced to wall time (sleeping when ahead, resynchronising rather
than bursting if it falls > 0.5 s behind). Work that costs host syscalls — reading the NAT's
sockets — runs on its own emulated-time cadence rather than every round, because at 240 MHz a
per-round syscall costs more than the instructions it interleaves with.

**Virtual quanta.** When exactly one core is busy and every other core is idle, the per-quantum
cut serves nobody: no other core runs between the quanta, and nothing else can change until the
next device deadline. `run_unmodeled` then gives the busy core one budget of up to `vq_max` quanta
(`Machine::vq_max`, 1024 in the browser build, 1 natively) and afterwards closes the rounds it
spanned exactly as the loop would have closed them one by one. The run length is bounded so that
no interior boundary has work: no device flush (`SocBus::next_deadline`), script event, page push,
timer wake-up of an idle core, cycle or instruction limit. A device register is reached only at
exact device time: while such a run is active the bus is armed (`SocBus::set_defer`), the executor
asks `Bus::defer_access` before a word access (a 16-register scan covers PIE and MAC16 operands),
and a hit stops in front of the instruction; the machine closes the completed rounds and finishes
the current quantum the ordinary way, where the access executes. A run that a register cuts short
inside two quanta doubles a skip counter, so firmware that polls registers every few instructions
is left alone. Results are bit-identical with `vq_max = 1`: the TinyDraw and pocket-tank batteries
keep their pinned instruction totals and console hashes, and the Atech, SID and panel goldens keep
their WAV hashes (`ESP32SIM_VQ_NATIVE=1 ESP32SIM_VQ=1000 --no-jit`; the AArch64 JIT's helpers have
no deferral guard, which is why the native default is off). `SocBus::vq_violations` counts any
register access that escaped deferral; it must stay zero for the claim to hold. The S3 bus also
lengthens its device-tick backstop from 256 to 32,768 cycles while no cadence-driven device is
active (I2S, LCD_CAM, GDMA, WiFi, AES/SHA DMA, SPI2, RMT, GPIO changes, the RTC watchdog, USB SOF
interrupts), which is what lets a run reach the next timer deadline.

With a cost model, each core has a next-ready timestamp in the one shared device timeline. The
machine runs the ready core with the lowest timestamp, breaking ties by core index, and advances
devices to exact core, timer and script frontiers. Architectural and bus effects occur at the
event's start. A refused event keeps those effects and adds no modeled cycles or device time.
Synthetic app boot (`--boot app`) is unsupported because the model has no configuration snapshot;
modeled runs must boot from the reset vector (`--boot rom`). Modeled execution also refuses a
function stub when it reaches one, so firmware that needs one (the S3 panel's `esp_wifi_start=0`)
cannot run with a cost model yet. If every core is idle and no device reports a deadline, modeled execution
stops with `Halted` instead of waiting for host input.

## Networking

Nothing about the network is faked at the API level: the firmware runs Espressif's own closed
`libpp`/`libnet80211` against a modelled MAC, and what comes out the other end is 802.11 frames.
Six layers turn those into packets on the host's network:

```
esp_wifi + libpp/libnet80211        unmodified blob, drives the MAC registers
  WifiMac (each chip)               TX queues, RX descriptor ring, interrupt events, TSF (S3/C3);
                                    the S3's in periph.rs, the C3's and C6's in their wifi.rs
  StationLink (esp-soc wifi.rs)     shared by the chips: RX pacing, network step, Ethernet relay
  VirtualAp (esp-soc wifi.rs)       beacons, probe/auth/assoc responses, WPA2 four-way handshake,
                                    802.11 <-> Ethernet conversion, CCMP framing
  VirtualNet (net.rs)               10.0.2.0/24: ARP, DHCP, ICMP echo, DNS, SNTP from the host clock
  Nat (nat.rs)                      everything past the gateway -> host sockets
```

- **The air**: `StationLink::next_rx` (`esp-soc/src/wifi.rs`) picks one frame at a time —
  spaced ~400 µs apart, and only once the driver has recycled the previous descriptor or 50 ms
  have passed; a frame that then finds no usable descriptor is dropped and counted, and the gap
  and the 50 ms start again from it, on every chip — and
  each chip's `wifi_air_step()` in its `bus.rs` writes it into the RX ring with the chip's
  descriptor and header layout, then raises the MAC's RX interrupt. Management frames are delivered ahead of beacons so a
  response never waits behind a beacon the ring may drop.
- **Encryption**: the four-way handshake is real (PMK, PTK, MIC, AES-key-wrapped GTK), but data
  frames carry plaintext *framed* as CCMP — protected bit, 8-byte CCMP header with the right key
  id, 8 bytes of MIC space — which is exactly what firmware sees when silicon encrypts in place.
- **Off the subnet**: `Nat` terminates each guest flow. A SYN starts `TcpStream::connect` on a
  worker thread and the SYN/ACK waits for it; guest payload is written to the socket; socket
  reads come back as segments the emulator sequences, acknowledges, retransmits and closes.
  UDP is a bound socket per flow with an idle reaper. Name lookups go to the host's own resolver.
- **Crypto accelerators**: mbedTLS drives AES (block + GDMA, ECB/CBC/CTR/OFB), SHA (block + GDMA,
  SHA-1 through SHA-512) and the RSA/MPI unit; the WPA supplicant unwraps the group key on AES.
  All three are modelled at the register level, so a TLS session exercises the same peripherals
  it would on silicon — see peripherals.md and networking-plan.md.

## Boards

`BoardModel` (esp-soc/src/board.rs) receives GPIO output edges, decoded RMT frames, owns the I2C devices
and the camera source, and exposes optional display/ring/camera state for the UI. The SoC
never knows what board it is on; `--board` selects the implementation. See boards.md.

## Web UI

`web.rs` serves `web/run.html` and one WebSocket per tab. The machine pushes state 50 times
per emulated second (console text, display frames, audio, ring colours, statistics) and polls
inputs (buttons, encoder, serial lines, camera pictures). Protocol in web-ui.md. The same
`WebServer` has a **queue mode** with no sockets: the WebAssembly build's worker drains the
outbox after every run slice and feeds inputs in, so the page code is identical for both.

## Provenance and verification

- Instruction semantics: Xtensa ISA reference + ESP32-S3 TRM; PIE from the TRM only.
- Peripherals: ESP32-S3 TRM register maps and the ESP-IDF `hal/*_ll.h` drivers as the
  "what does software expect" reference.
- Ground truth: `hw/difftest*.sh` single-step a real ESP32-S3 over USB-JTAG (openocd + gdb)
  and compare PC/registers with the emulator running the same flash image — zero divergence
  over the ROM reset path and the bootloader.

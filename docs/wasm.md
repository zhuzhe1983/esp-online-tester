# Running the emulator in the browser (WebAssembly)

The whole emulator — both Xtensa cores, the SoC, the boards, the virtual WiFi and subnet —
compiles to a single WebAssembly module and runs inside the page, in a Web Worker. Nothing is
uploaded anywhere: firmware is read from the visitor's disk (or fetched from files you host next
to the page) and executed in the tab.

## Build and try it

```sh
tools/wasm-build.sh                      # -> web/wasm/esp32sim.wasm (needs the wasm32-unknown-unknown target)
tools/fetch-demo-assets.sh               # mask ROM ELFs, xterm.js, the Linux image (--no-linux skips its 16 MB)
python3 -m http.server -d web 8790       # any static server; file:// will not do (workers, fetch)
open 'http://127.0.0.1:8790/run.html?wasm&fw=hello'
```

Every demo in `web/wasm/fw/demos.json` boots from the mask ROM, which is not committed: without the
fetch the page stops at `esp32s3_rev0_rom.elf: 404`. xterm.js is only for the Terminal tab.

`?wasm` switches `web/run.html` from its WebSocket transport to the worker; the page gains a
firmware panel: board, flash/PSRAM size, an optional WiFi spec, function stubs, and file inputs
for the mask-ROM ELF, `bootloader.bin`, `partition-table.bin`, the app image, its ELF (symbols
— needed for stubs) and a script. The ROM input may stay empty: the page then fetches
`wasm/fw/<chip>_rom.elf` for the chip the board implies, and refuses to boot without one (a ROM boot
from empty memory would run silently). Picking a board sets the flash and PSRAM sizes the demos use.
**Boot** starts it, and pressing it again replaces the machine without reloading the page, so the
chosen files stay; the rest of the page — console tabs,
display, touch, buttons, knob, audio, camera — is the same UI the native emulator serves.

For your own demos, `?wasm&fw=<name>` loads `web/wasm/fw/<name>.json` and boots it without
clicking (format in `web/wasm/fw/README.md`). Everything in that directory except the manifests
and `public/` (our own demo firmware) is git-ignored: the mask ROM is Espressif's and other firmware
is whoever built it; host them only where you may.

## On GitHub Pages

`.github/workflows/pages.yml` builds the module on every push to `main`, runs
`tools/fetch-demo-assets.sh` (the mask-ROM ELFs from the Apache-2.0 `espressif/esp-rom-elfs` release,
xterm.js, the Linux image), and publishes `web/` — so the page at
**https://mikroverk.github.io/esp32sim/** is the emulator, with the demos in
`web/wasm/fw/demos.json` — hello_world, the Touch-LCD-4B energy panel with its SID player, the Atech
Pocket Synth, and the C3 and C6 demos — one click away and the file inputs for anyone's own firmware. It
also runs `tools/fetch-pocket-tank.sh` for the **pocket-tank** demo ([mediacutlet/pocket-tank](https://github.com/mediacutlet/pocket-tank),
MIT): it checks the committed bootloader, partition table and app in `web/wasm/fw/public/` (the bytes of
the project's browser installer, whose host answers GitHub's runners with something else) and fetches the
7.56 MB model from the repository, all pinned by SHA-256. The workflow also fetches the **Linux-on-esp32-S3** flash image
(GPL-3.0, [svermigo/Linux-on-esp32-S3](https://github.com/svermigo/Linux-on-esp32-S3), release 0.7,
pinned by commit and SHA-256 in `pages.yml`) so the `linux` demos boot it — `linux-term` opens on the
xterm.js terminal tab (manifest `terminal: true`), where `vi`, `top` and colours render properly; the image is never
committed here — the source for everything in it is that repository. On a `github.io` host the page starts in wasm mode without
`?wasm`. Firmware committed under `web/wasm/fw/public/` is ours, except pocket-tank's three MIT parts next to its license; the panel is a
separate build with placeholder `secrets.h` values (checked with `strings` against the real ones).

**Demo data without a rebuild.** The panel firmware has a `demo` data partition (0x610000,
64 KB); when it holds a JSON document the firmware renders that — prices for today and
tomorrow, hourly kWh, tile states, header power, a fixed clock — and never starts WiFi. The
manifest writes `public/energydata.json` there (`flash_at`), so changing the demo is editing a
JSON file; real boards have the partition erased and behave as before. Natively:
`--flash-at 0x610000=web/wasm/fw/public/energydata.json`.

## What it is

`tools/wasm-test.mjs` runs the built module under Node through the same manifests the page
uses and fails on a panic or a missing console line; CI runs it after the goldens.

Every chip is in the one module: `esp32sim_new` takes a board name, and `esp32c3` or `esp32c6`
builds the RISC-V machine instead of the Xtensa one; `esp32` or a classic board name such as
`ttgo-tdisplay` builds the classic ESP32. The C3 has no `WebServer` of its own — it is
console-only — so the wasm layer turns its console into the same `{"t":"serial"}` messages the
S3 sends, and `esp32sim_cpu_hz` tells the worker which clock to pace against (240 MHz vs 160).

```
web/run.html     the UI, unchanged; `link` is either a WebSocket or the worker
web/emu.js       page side: firmware panel, manifest loading, window.EmuLink
web/wasm/worker.js   owns the wasm instance, paces it to wall time, relays the UI protocol
wasm/            the crate: a C ABI over the S3, C3 and C6 machines (esp32sim_new / load / wifi / stub /
                 boot / run / out_* / in_*); no bindgen, no dependencies
```

Inside the module the machine talks to the page through the same `WebServer` the native build
uses, in **queue mode**: every `send_text`/`send_binary` lands in an outbox the worker drains
after each run slice (`docs/web-ui.md` is the protocol on both sides). The worker keeps the
machine at wall time: it computes the cycle count the clock has earned, runs in ≤2 M-cycle
slices, yields every 25 ms so frames and audio flow, and resynchronises instead of bursting if
the tab falls half a second behind. `Date.now()` is passed in for the emulated SNTP server.

## What works, measured (M-series Mac, Chrome)

| firmware | in the tab | notes |
| --- | --- | --- |
| IDF hello_world | real time | ROM → bootloader → app, `esp_restart` reboots through the ROM |
| **Linux 6.11 on the ESP32-S3** (svermigo/Linux-on-esp32-S3) | real time, ~4× under Node | the esp-hosted network adapter on core 0, Linux XIP from flash on core 1, cramfs root, jffs2 overlays; login and a shell typed on the page console (UART0). Needs the `wifi` spec (PHY calibration) and a stub on `nimble_port_init` — no Bluetooth baseband is modelled, so BLE provisioning is skipped |
| Waveshare Touch-LCD-4B energy panel + SID player | **real time**, ~62 Minsn/s | LVGL at 60 fps, touch, the tune plays through WebAudio |
| Atech 14-port synth | real time | ST7735 and WS2812 decoded, buttons/knob, scripted scenario |
| ESP32-C3 hello_world | real time | the other chip: one RV32IMC core, console only — pick board `esp32c3` |
| ESP32-C6 hello_world | real time | the newest chip: one RV32IMAC core, console only — pick board `esp32c6` |
| LilyGO TTGO T-Display spot prices (MicroPython) | real time, ~3× under Node | the classic ESP32: stock MicroPython v1.29.0 booted by the ROM from the combined image, the app from a LittleFS image in the vfs partition, the ST7789 135×240 over VSPI; demo data, no Wi-Fi — board `ttgo-tdisplay` |
| ESP32-C6 802.15.4 energy scanner | real time | the Waveshare ESP32-C6-LCD-1.47: LVGL spectrum on the ST7789 over SPI2+GDMA, WS2812, energy detect from the MAC model's moving 2.4 GHz picture; BOOT on the page — board `waveshare-c6-lcd147` |
| ESP32-P4 Scintilla synth (`p4-lcd35-synth`) | about a quarter of real time (one uncontrolled measurement: Chrome on an M-series Mac, 180–190 Minsn/s counted, 24–26 %) | the Waveshare ESP32-P4-WIFI6-Touch-LCD-3.5: LVGL on the ST7796 over SPI2 and the AXI-GDMA, FT5x06 touch, the ES8311's audio from I2S1; no C6, so BLE MIDI is off — board `waveshare-p4-lcd35` |
| ESP32-C6 Contiki-NG | real time | Contiki-NG as an unmodified IDF app: its own scheduler and 802.15.4 stack over the emulated MAC |
| …two of them on one medium | real time | a manifest with a `nodes` array boots a network through `esp32sim_net_*` instead of `esp32sim_new`: several motes, one medium, no simulator behind them |
| …an RPL/IPv6 network | real time | rpl-udp server (the DAG root) and client: the client joins, then UDP request and reply every 10 s over 6LoWPAN, RPL Lite, CSMA and hardware ACKs |

The browser's Xtensa block scheduler can compile hot integer/branch/memory blocks into
additional WASM modules. After 32 executions, an eligible block is installed in the emulator's
exported function table. Subsequent calls stay inside WASM; JavaScript compiles and retires
modules but does not dispatch each block. Calls have generated WASM paths; window overflow
and illegal calls retain the existing exception handling. A supported prefix ending in a
return can also be compiled: the return uses the existing interpreter helper, preserving
its window and exception semantics. Other unsupported operations keep their block interpreted.
Unaligned, unmapped, read-only and peripheral accesses use the ordinary bus helper.

Compiled WASM blocks can survive decoded-arena turnover. Reuse requires matching every decoded
instruction, the block length and the fast-memory contract. At each arena flush, retention is
limited to the two most recent decoder generations, 16,384 blocks and 64 MiB of emitted WASM
per core. New code can grow beyond those retention limits between flushes; engine-generated
machine code and other browser allocations are additional. The host reports peak live emitted
bytes and module counts to make this tradeoff measurable.

Whole-block calls use a separate path when entry checks prove there can be no register-window
collision and no active loop end within the block. That path omits per-instruction entry,
budget, overflow and loop-end tests. Generated code loads only the block's operand registers
and computes register-window collision state once per entry. Budget cuts, resumptions and
states that fail either guard keep their checked path.

Once a block is hot, the emitter also tries to form a *region*: the block plus the blocks
reachable from it over statically known edges (fallthrough, conditional-branch target, `J`,
and the backedge of a hardware loop set up inside the region), compiled as one function.
Formation starts with at most 8 chunks, 64 instructions and 4 code pages; splitting at
hardware-loop ends can add chunk boundaries without adding instructions. Guest registers stay in WASM locals
across the internal edges; continuing inside the region checks the remaining instruction
allowance and whether a helper or code-page store requires an exit. Anything that
could make a block boundary observable leaves the region instead: interpreter helpers and
stores into one of the region's own code pages make the next chunk head exit, calls, returns
and computed jumps end a chunk, and entry checks cover page versions, probe boundaries, an active hardware loop,
and window and coprocessor state. A region's continuation
exits carry the next PC and are never mid-block cuts, so budget cuts inside a chunk still go
through the chunk's own block module. A dispatch at any chunk head of a live region enters the
region at that chunk, and a head already inside a live region does not get a region of its
own. `ENTRY` may head a region; the window proof is redone after the rotation. The profile
build (`jit-profile`) reports regions formed, entries, generated-entry rejects and instructions retired
per core.

The PIE (coprocessor 3) instructions the TinyDraw tile kernels use are emitted on WASM SIMD:
aligned 128-bit load and store with post-increment, lane compares, the bitwise q-register
operations, 32-bit lane insert and zeroing. So is the dot product of on-device inference
(pocket-tank's 4-bit matmul): signed 8- and 16-bit multiply-accumulate into ACCX, with and
without its load, the ACCX reset, and the RUR of ACCX_0/ACCX_1 that follows each dot product.
The products are widening vector multiplies with 64-bit lane sums, and ACCX is updated with
the interpreter's 40-bit saturation. Q registers stay in CPU memory as `v128` values; generated
functions have one `v128` and one `i64` scratch local. The CP3-disabled check is proved once per
body next to the FP one. Other PIE instructions keep the interpreter, and a block containing
any of them stays interpreted.

Compiled execution uses the same instruction-count timing as the default block interpreter.
Timer budgets, interrupts, loop ends, code-page versions and observer boundaries still bound
execution. This does **not** extend the receipt-based cycle model or establish cycle accuracy.
The earlier `esp32sim_jit_prepare/commit` experiment remains available for its synthetic test;
the page no longer uses it to run firmware.

Custom WASM hosts must provide `host_jit_compile` and `host_jit_release` from
`web/wasm/jit.mjs`, alongside `host_log`. The module exports `__indirect_function_table`.
Append `&jit=0` to the page URL, or configure `esp32sim_set_jit(emu, 0)` before
running, to compare with the interpreter;
`esp32sim_block_jit_insns(emu)` counts retired instructions through compiled blocks, including
memory helpers. `createJitHost` exposes compilation, failure, release and compile-time counters.
`ESP32SIM_NO_WASM_JIT=1 node tools/wasm-test.mjs ...` exercises the interpreter on the same build.

`tools/wasm-jit-test.sh` builds a separate test-enabled module and runs generated-code
comparisons with the interpreter under Node, including budget/resume, timers, register windows,
loops, memory faults and code invalidation. CI runs it alongside the firmware smoke tests.
It does not overwrite `web/wasm/esp32sim.wasm`.

An optional `jit-profile` feature adds statistical block profiling to the WASM build. Its
host must supply `env.host_profile_now`, a monotonic millisecond clock (for example,
`() => performance.now()`). Call `esp32sim_profile_report(emu)` to emit per-core TSV through
`host_log`: sampled PCs, compiled/interpreted status, instruction counts, elapsed samples,
unsupported operations and block operations. Blocks are sampled with probability 1/4096
using a pseudorandom sequence. PC rows describe the first sampled decoded shape; profiles
of self-modifying code may combine multiple shapes at the same PC. Counts include trap
iterations and compiled helper execution, so they are estimates rather than exact opcode
retirement counts. Elapsed samples include lookup and dispatch but exclude the outer SoC
scheduler. Clock-call overhead and quantization can dominate these short intervals; do not
convert their sum into a wall-time breakdown. Use uninstrumented runs to measure speed.
Normal builds contain neither the sampling code nor the clock import.

## Limits

- **No NAT.** The browser has no sockets. With a `wifi=` spec the firmware still associates,
  gets a DHCP lease, resolves names and syncs time against the emulated subnet, but connections
  past the gateway are refused (the `--net none` behaviour). The [host Ethernet relay](#host-ethernet-relay)
  below lets a page carry the guest's frames to a host helper instead.
- **No file outputs**: `--wav`, `--tft-png`, register traces — the page is the output.
- **Emulator log lines** (`[emu] …`) that the native build prints to stderr do not exist here,
  except the ones the wasm glue forwards (stubs, resets, load errors) to the console tab and the
  browser console.
- **Memory**: flash + PSRAM + SRAM + ROM plus the block caches; the panel configuration takes
  ~45 MB of wasm memory. The block tables are sized smaller than natively (`block.rs`).
- Audio needs one click on **enable audio** — browsers will not start WebAudio otherwise.

## A network of motes

A manifest with a `nodes` array boots a *network* rather than a single machine: the page calls
`esp32sim_net_new` and, per node, `esp32sim_net_add` (its MAC, board, position and power-on
offset) and `esp32sim_net_load`, then `esp32sim_net_run` advances the whole network to a point in
network time while `esp32sim_net_console_take` and `esp32sim_net_stat` report what each mote did.

```json
"nodes": [ { "mac": "02:00:00:00:00:01", "start_ms": 0,    "x": 0, "y": 0 },
           { "mac": "02:00:00:00:00:02", "start_ms": 1300, "x": 2, "y": 0 } ]
```

The medium and the lock-step are in the module (`esp32c6::net`), not in the worker, for the same
reason the Cooja front end keeps them in Rust: `Machine::run_until_cycle` stops at the instruction
that starts a transmission and `SocBus::radio_receive` puts a frame on the air at its first
preamble byte, so the exactness is already there and a native test can hold it to it
(`esp32c6/tests/net.rs`). The worker only paces network time to the wall clock and relays.

A node may carry its own `files` and `symbols`: an entry of a kind the node names replaces the
shared one, so a root and a client — two images — share one
bootloader and partition table and differ only in `app`.

`start_ms` is not cosmetic. Two identical images booted at the same instant are deterministic to
the cycle, so their application timers never drift apart: every broadcast is sent while the other
mote is transmitting, and nothing is ever heard. Real motes are staggered by their power-on; here
it has to be said out loud.

The browser build script defaults to LLVM inline threshold 4000, selected by
[EX154](experiments.md#ex154) on Rust 1.98.1. Set `RUSTFLAGS` explicitly to override
it; `RUSTFLAGS='' tools/wasm-build.sh` uses the compiler's default threshold.
The script leaves the Rust-produced WASM unchanged unless `WASM_OPT=1` is set,
which requests Binaryen `wasm-opt -O3` and requires that tool to be installed.
The measured configuration used no Binaryen postprocessing. `CARGO_ENCODED_RUSTFLAGS`,
when set, takes precedence over `RUSTFLAGS` under Cargo's usual rules.

## Host Ethernet relay

S3, C3 and C6 provide an optional Ethernet transport behind their virtual AP.
Configure the AP with `esp32sim_wifi`, then call `esp32sim_ethernet_relay(e, 1)`
before boot. Association and WPA2 still run in the emulator; the host supplies
the network behind the AP, including DHCP. This mode bypasses the built-in
`VirtualNet`. With relay disabled, the existing virtual subnet remains the default.
Native Rust hosts use `SocBus::set_ethernet_relay`, `take_ethernet_frames` and
`receive_ethernet_frame` for the same operations.

| Export | Contract |
| --- | --- |
| `esp32sim_ethernet_relay(e, enabled)` | Nonzero selects relay, zero restores the built-in network; returns 0 on success, 1 on error. |
| `esp32sim_ethernet_take(e)` | Drain TX in FIFO order; returns the number of frames in the new batch. |
| `esp32sim_ethernet_ptr(e, index)` | Pointer to a drained frame; null for an invalid index. |
| `esp32sim_ethernet_len(e, index)` | Frame length; zero for an invalid index. |
| `esp32sim_ethernet_receive(e, ptr, len)` | Copy an inbound frame; returns 0 if queued, 1 if rejected. Requires an AP and relay mode. |

Frames contain Ethernet headers and payloads without FCS, from 14 through 1518
bytes. Each direction holds at most 64 queued frames in relay mode. Excess TX
frames are dropped and counted in the WiFi link's `tx_dropped` (`StationLink`); a full RX queue rejects injection. Delivery can drop frames
when the guest has no free receive descriptor. Drain after each run slice and
copy bytes before the next `ethernet_take`, which replaces the entire batch.
Recreate JavaScript memory views after calls that may grow WASM memory.

Changing modes clears pending Ethernet queues; selecting the same mode leaves
them intact. Configure transport before starting guest connections. Switching
modes does not migrate DHCP leases or TCP connections. Chip reset clears pending
frames but preserves the AP, network object and relay selection. A drained WASM
batch remains readable until the next `ethernet_take` or emulator deletion.

The [Arduino relay check](evidence/ethernet-c3-2026-10-02/relay.mjs) shows the ABI
with a host DHCP responder.

Camera frames use `esp32sim_in_bin` type 3: `[3, width u16le, height u16le, RGBA...]`.
The entire message must fit 8 MiB, with nonzero dimensions and exact pixel length.
The receiver copies the message before returning and applies it at the next run
boundary. Sensor SCCB settings select output format and geometry. A board without
a camera emits `{"t":"camera-error","message":"no camera on this board"}`.

### C3 full BLE observation

Call `esp32sim_ble_full(emu)` before boot (0 = enabled, 1 = wrong chip or already
booted). This runs the guest link controller without HCI substitution. After each
run slice, call `esp32sim_ble_take(emu)` until it returns zero. Each nonzero return
is the UTF-8 byte length at `esp32sim_ble_ptr(emu)`, valid until the next take or
emulator deletion. `[ble-air]` contains modeled half-microsecond time, channel,
PDU type, AdvA, decoded name/service AD fields and the raw PDU. `[ble-config]`
contains configured scan-response data, not a transmitted packet. The bounded
1024-entry queue reports lost observations as `[ble-observer] dropped=N`.
Queued observations and the dropped count survive guest reboot. Invalid enable
calls log a reason through `host_log`. Use `esp32sim_ble_scan(emu, enabled)` for active scanning,
`esp32sim_ble_command(emu, ptr, len)` for UTF-8 `connect`, `disconnect`,
`central-stop` or `read-uuid SERVICE CHARACTERISTIC`. `central-stop` silences
a connected central. These exports return 0 on
success or 1 for invalid commands/state/chip. Reads use the guest ATT server.
The central models one unencrypted 1M CSA#1 connection with a 30 ms interval,
2 s supervision timeout and ATT MTU 23. See [EX213](evidence/ble-c3-connection/README.md)
for model limits and the production-module reconnect test.

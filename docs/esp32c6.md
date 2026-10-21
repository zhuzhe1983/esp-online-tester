# ESP32-C6 (RISC-V) — draft

The third chip in the workspace, and the second RISC-V one: a single RV32IMAC core at 160 MHz
with 512 KB of unified HP SRAM, on the board it was brought up against — the Waveshare
ESP32-C6-LCD-1.47 (ESP32-C6FH4, 4 MB embedded flash, ST7789 172×320 LCD, WS2812, TF slot).
Unmodified ESP-IDF firmware boots from the real mask ROM, through the real 2nd-stage bootloader,
into FreeRTOS; with `--board waveshare-c6-lcd147` the board's display and LED are there too, and
an IEEE 802.15.4 energy scan gets answers from the emulated MAC.

```sh
cargo build --release
H=examples/hello_world-c6/build
./target/release/esp32sim-c6 --boot rom --flash-mb 4 \
    --bootloader $H/bootloader/bootloader.bin --ptable $H/partition_table/partition-table.bin \
    --app $H/hello_world.bin --elf $H/hello_world.elf --max-seconds 27
```

prints the ROM banner, the bootloader's partition table, the IDF startup log, `Hello world!`, the
countdown, and reboots through the ROM on `esp_restart()` — three complete boots in 27 s:

```
ESP-ROM:esp32c6-20220919
Build:Sep 19 2022
rst:0x1 (POWERON),boot:0x6e (SPI_FAST_FLASH_BOOT)
I (5) boot: ESP-IDF v5.5.4 2nd stage bootloader
I (5) boot: chip revision: v0.1
I (5) boot: efuse block revision: v0.3
I (33) main_task: Calling app_main()
Hello world!
This is esp32c6 chip with 1 CPU core(s), WiFi/BLE, 802.15.4 (Zigbee/Thread), silicon revision v0.1, 2MB external flash
Minimum free heap size: 473128 bytes
```

The mask ROM ELF is picked up from `~/.espressif/tools/esp-rom-elfs/*/esp32c6_rev0_rom.elf`
(shipped with ESP-IDF); override with `--rom`. Flags mirror the C3 binary — see `--help`;
`esp32sim --chip c6` is the same thing.

## Verified against real silicon

The Waveshare board (chip rev v0.1, efuse block v0.3, MAC `dc:1e:d5:6e:8c:dc`; its efuse summary
is in `hw/c6-efuse.txt`) was flashed with the same three binaries the emulator runs, and 27 s of
its USB-Serial/JTAG console captured (`hw/c6-hello-world-real.txt`). Comparing that with the
emulator, log timestamps normalised:

| | |
| --- | --- |
| **203 of 204 lines identical** over three complete boot cycles | |
| the only difference | the ROM's `Saved PC:` line on the **first** boot |

To reproduce, tell the emulator the board's identity and boot conditions:

```sh
./target/release/esp32sim-c6 --boot rom --flash-mb 4 \
    --mac dc:1e:d5:6e:8c:dc --reset-cause 0x15 --strap 0x6e \
    --bootloader $H/bootloader/bootloader.bin --ptable $H/partition_table/partition-table.bin \
    --app $H/hello_world.bin --elf $H/hello_world.elf --max-seconds 27
```

`Saved PC` is the PC the ASSIST_DEBUG block recorded when the reset hit. After `esp_restart()`
the emulator prints the same value as silicon (`0x4001975a`, the instruction after the ROM's
`software_reset_cpu` store), because a reset now takes effect at the instruction that requests
it. The first boot's value is where the chip was when esptool reset it out of download mode,
which a cold start cannot know; that line is the one missing.

The bootloader's own timestamps are a few milliseconds earlier than the board's (`I (5)` vs
`I (23)`): the ROM's flash probing takes real time on silicon that the model does not spend.

### What the bring-up found

Four of these were invisible without the board's console to compare against; the fifth came
from the trace.

- **The cycle counter restarts at a chip reset.** `mcycle`/`mpccr` fed the emulator's own
  monotonic instruction count, so after `esp_restart()` the bootloader's log timestamps read
  `I (44117)` instead of `I (23)` — on the **C3 as well**, hidden in its golden by timestamp
  normalisation. The guest now sees cycles since reset; the emulator's count stays monotonic.
  (A guest write to the counter also used to move the emulator's instruction count: the C3
  golden's count was inflated by a billion phantom instructions over two resets.)
- **A reset takes effect at the instruction that requests it.** The scheduler used to notice the
  reset at the end of the 64-instruction quantum, so the core ran on through the ROM's `ret`
  and back into the app. Harmless for the console, wrong for `Saved PC`.
- **PCR's hardware-fixed clock-tree fields must read their silicon values.** SOC_ROOT→HP_ROOT is
  a fixed ÷3 on the PLL path and PCR_SYSCLK_CONF says so in a read-only field; a register RAM
  read it as 0, the app derived a CPU divider of 2 from it, and `clk_ll_cpu_set_hs_divider`
  asserted. The PLL and XTAL frequencies the query register reports are fixed the same way.
- **The ROM's `SPI_init` waits for SPI0's AXI FIFOs to report empty** (a C6-only status
  register), and the bootloader's clock switch waits for `BBPLL_CAL_DONE` on the analog I2C
  master. Each is a "done" bit that must be set, or the boot stalls silently at 100% CPU.
- **The interrupt threshold is PLIC+0x90, not INTPRI's offset.** With it in the wrong place the
  interrupt handler could not mask its own level before enabling nesting, the systimer line
  re-entered 3 867 times and walked the stack down through all of SRAM into the code. The
  symptom was an illegal instruction in `rtos_int_enter`; the cause was 500 KB away.

### Reading a board yourself

```sh
python -m esptool --port /dev/cu.usbmodem101 chip_id          # identify before touching anything
python -m esptool --port /dev/cu.usbmodem101 --baud 921600 \
    read_flash 0 0x400000 backup.bin                           # back up first (the factory demo)
cd examples/hello_world-c6/build && python -m esptool --port /dev/cu.usbmodem101 \
    --baud 921600 --chip esp32c6 write_flash '@flash_args'
python -m espefuse --port /dev/cu.usbmodem101 summary          # the revision fields above
```

## A network of motes

`esp32c6::net::Network` owns several `Machine<C6>` on one medium, so a whole 802.15.4 network runs
in one process — or one browser tab — with no simulator behind it. The exactness is the same as
the `--cooja` path's, because it is the same primitives: `Machine::run_until_cycle` stops at the
instruction that starts a transmission, and `SocBus::radio_receive` places a frame on the air at
its first preamble byte. The network only owns the clock and the medium: every node advances to
the end of a common slice (100 µs by default, a transmission cutting a node's slice short), and
the frames a slice produced are handed to every other node in range with `started_ago` set from
how far that receiver has run past the transmission. A receiver whose radio is not listening at
that moment refuses the frame, which is what makes two simultaneous transmissions collide.

Two things a network needs that a single mote does not:

- **Different MACs.** Contiki derives its link-layer address from the efuses and drops a frame
  that appears to come from itself.
- **Staggered power-on** (`start_ns`). Two identical images booted at the same instant are
  deterministic to the cycle, so their application timers never drift apart: both broadcast at
  exactly the same nanosecond, each is transmitting while the other's frame arrives, and neither
  ever hears anything. Real motes are staggered by their power supplies; here it is explicit.
  This is visible in csim too — two `esp32sim-c6 --cooja` nodes running the same image transmit
  at 5.065 s, 10.065 s, 15.065 s together and never receive.

`esp32c6/tests/net.rs` (`external_two_motes_exchange_nullnet_broadcasts`, needing `CONTIKI_C6_DIR`) holds it to that with the Contiki-NG nullnet image: two motes, staggered,
each hears all five of the other's broadcasts in 30 s, and the payload reaches Contiki's nullnet
layer rather than only the PHY.

With two different images it is a real IPv6 network: `contiki-ng-esp32` built with
`-DCONTIKI_UDP_SERVER=ON` is the rpl-udp server and makes itself the DAG root; the ordinary build
is the client. On one medium the client joins the DAG in about 25 s and then exchanges a UDP
request and reply with the root every 10 s — 6LoWPAN, RPL Lite, CSMA and the hardware
acknowledgements, end to end. `web/wasm/fw/c6-rpl-net.json` is that pair; under Node it runs at
40x real time.

## In the browser

The C6 is in the WebAssembly build too — pick board `esp32c6` on the page, or open it directly:

    https://mikroverk.github.io/esp32sim/run.html?fw=c6-hello
    https://mikroverk.github.io/esp32sim/run.html?fw=c6-energy-scan
    https://mikroverk.github.io/esp32sim/run.html?fw=c6-contiki
    https://mikroverk.github.io/esp32sim/run.html?fw=c6-contiki-net
    https://mikroverk.github.io/esp32sim/run.html?fw=c6-rpl-net

The first is console-only; the second is the energy scanner on the Waveshare board at real time,
with the panel, the WS2812 and a BOOT button on the page (its firmware is published under
`web/wasm/fw/public/`; no ELF ships). See [wasm.md](wasm.md).

## What is modelled

| | |
| --- | --- |
| CPU | RV32IMAC, machine mode: the C3 core plus the A extension (`lr.w`/`sc.w`, the nine AMOs), `misa` says IMAC |
| Interrupts | the interrupt matrix (77 sources → 31 lines) with its two front-ends: the PLIC at `0x20001000` that ESP-IDF drives and INTPRI at `0x600C5000` that the ROM uses, one state; the four `FROM_CPU` software interrupts |
| Memory | 512 KB HP SRAM (one address space for code and data), 320 KB mask ROM, 16 KB LP SRAM, a 16 MB flash window through the 256-entry MMU programmed via SPI0's item index/content registers, page size from `MMU_POWER_CTRL` |
| WiFi | an unmodified ESP-IDF station scans, joins the virtual access point (open or WPA2-PSK), takes a lease and talks to the virtual network or, with `--net nat`, the host's (`--wifi ssid=esp32sim,psk=esp32sim-pass`, as on the S3). The MAC model is `esp32c6/src/wifi.rs` (register map and differences from the S3 in [wifi-c6-plan.md](wifi-c6-plan.md)); the descriptors move in `bus.rs`; the access point, network, pacing and relay are the chips' shared `StationLink` (`esp-soc/src/wifi.rs`). The `wifi_station_c6` golden runs the committed `examples/c6-wifi-station` build in CI; the browser build runs it too (a manifest's `wifi`, without NAT) |
| SDIO slave | HINF, SLC and SLCHOST (`esp32c6/src/sdio.rs`) as IDF's sdio_slave driver programs them, and the I/O card a host sees: CMD5/3/7/52/53, the CCCR, FBR and CIS, function 1's registers and FIFO onto the SLC DMA links, the interrupts both ways (source 64). A host reaches it through `SocBus::sdio_command`, `sdio_read_data` and `sdio_write_data`; the P4's co-processor is one ([esp32p4.md](esp32p4.md), "The co-processor"), running ESP-Hosted's slave |
| Peripherals | UART0/1, USB-Serial/JTAG, systimer, TIMG0/1, GPIO, efuse, SPI0/1 flash controller, SHA/AES/RSA, L1 cache controller, PCR, the LP blocks (LP_CLKRST reset cause, LP_AON store registers and software reset, LP_TIMER, LP_WDT registers), the analog I2C master (regi2c, with the RF block's status the PHY polls), ASSIST_DEBUG's saved PC, hardware RNG; for the board: RMT (the S3 transmitter on the C6's register map), GDMA (three channels, the S3 model behind the C6's layout), GP-SPI2 with its DMA data phase; the 802.15.4 MAC: energy detect, TX and RX of whole frames with the timing of the air, its two timers (below) |

Peripheral models are **shared with the C3 and S3 through `esp-periph`** where the IP is the same:
the register-offset comparison of the IDF headers shows UART (the registers the model uses; the
C6's `REG_UPDATE` handshake was added), systimer, timer groups, GPIO, USB-Serial/JTAG, the SPI
flash controller, GP-SPI and efuse all match. RMT and GDMA are the same IP behind a different
register map, so `RmtC6` and `GdmaC6` translate offsets onto the shared models. `esp32c6/src/periph.rs`
is the `device_set!` table plus the chip-specific blocks.

The decoder is checked against `riscv32-esp-elf-objdump` the same way: 126,384 instructions from
the C6 mask ROM and hello_world, 0 mismatches (`riscv-rv32/tests/corpus/c6-*.dis` carry a
sample of each). Neither binary contains an atomic instruction — IDF's hello_world does not need
them — so the A extension is pinned by an assembled program in `riscv-rv32/tests/semantics.rs`;
the LVGL firmware for this board has 33 of them.

```sh
OD=~/.espressif/tools/riscv32-esp-elf/*/riscv32-esp-elf/bin/riscv32-esp-elf-objdump
$OD -d ~/.espressif/tools/esp-rom-elfs/*/esp32c6_rev0_rom.elf > /tmp/rom.dis
$OD -d examples/hello_world-c6/build/hello_world.elf > /tmp/app.dis
RISCV_DIS_FILES=/tmp/rom.dis:/tmp/app.dis cargo test -p riscv-rv32 --release -- --ignored external_
```

LEDC models low-speed timers, static duty latching and GPIO-matrix frequency/duty observation through `--pwm PIN`. Hardware fades and sampled PWM edges are not modelled.

## The 802.15.4 MAC (`esp32c6/src/radio.rs`)

The `IEEE802154` block at `0x600A3000`, with the register layout of IDF's
`soc/ieee802154_struct.h`, modelled as far as `esp_ieee802154` (IDF 5.5) drives it. What the
driver does — read from `esp_ieee802154_dev.c` and confirmed with register traces of Contiki-NG's
radio driver on this model:

- **transmit**: `STOP`, `TIMER0_STOP`, `TIMER1_STOP`, read and clear `EVENT_STATUS` (the previous
  operation is torn down), the PIB if it changed (`CHANNEL` = `3 + 5·(ch−11)`, `TXPOWER`,
  `ED_CFG`, `CONF`), `DMA_TX_ADDR` = the frame buffer — `buf[0]` the PSDU length *including* the
  2-byte FCS, `buf[1..]` the MAC frame without it. For a frame with the AR bit the driver also
  points `DMA_RX_ADDR` at a free buffer: that is where the acknowledgement lands. Then `TX_START`
  (or `CCA_TX_START`). The ISR expects `TX_SFD_DONE` and `TX_DONE`; for an AR frame with
  `CONF.auto_ack_rx` it then enters `RX_ACK`, arms TIMER0 for 200 ms with a NO_ACK callback and
  waits for `ACK_RX_DONE`, on which it reads the ACK from the RX buffer — the usual
  `[len incl. FCS][frame][RSSI][LQI]`, so `[5][02 xx seq][RSSI][LQI]` — and reports
  `transmit_done(frame, ack, ack_info)`. Hardware has its own `ACK_TIMEOUT` register (0x5c,
  units of 16 µs, `TX_ABORT` reason `RX_ACK_TIMEOUT`); the driver never programs it, so the
  200 ms timer is what fires without an ACK. When it is programmed and fires, an acknowledgement
  still on the air is discarded rather than delivered: a 3-byte ACK's `RX_DONE` lands 352 µs
  after its first preamble byte, so one handed over more than 512 µs after `TX_DONE` would
  otherwise complete with the state already back to `Idle` and be reported as a received frame.
- **receive**: the same teardown, `DMA_RX_ADDR` = a free 129-byte buffer, `RX_START`. A frame
  that passes the address filter raises `RX_SFD_DONE` and then `RX_DONE`; one that does not is
  dropped by hardware (`RX_ABORT` reason `FILTER_FAIL`, an event the driver leaves disabled). A
  frame with the AR bit (frame version 0/1, `CONF.auto_ack_tx`) is acknowledged by hardware: the
  driver moves to `TX_ACK` on `RX_DONE` and only hands the frame up and re-arms RX on
  `ACK_TX_DONE`. Before a TX the driver reads `RX_STATUS.rx_state`; `> 1` means a frame is
  coming in and the TX is refused.
- **stop** mid-frame raises `RX_ABORT` (reason `RX_STOP`); abort events fire only for the
  reasons the driver enabled in `RX_ABORT_EVENT_EN` / `TX_ABORT_EVENT_EN`.
- **CCA**: `CCA_TX_START` transmits unless the channel is busy, which here means a frame is
  being received: `TX_ABORT` reason `CCA_BUSY`, `TX_ERR_CCA_BUSY` to the caller.

The model: `TX_START` asks the bus for the frame (the device cannot see SRAM), stamps the start
for the host, and raises `TX_SFD_DONE` after 5 bytes and `TX_DONE` after `6 + len + 2` bytes of
air (32 µs each); an AR frame then waits for its ACK (`RxAck`), taking only an ACK frame with
its sequence number. A frame offered while listening goes through the filter — a destination
PAN of ours or 0xffff and a destination address of ours or the short broadcast, per multipan
entry the driver enabled; no destination only for a coordinator on its PAN or a beacon; `CONF.
promiscuous` takes everything — raises `RX_SFD_DONE` after 5 bytes and `RX_DONE` after
`6 + len + 2` bytes, counted from the frame's first preamble byte, exactly as the transmitter
counts them; the buffer is written by the bus as the driver reads it; an AR frame is acknowledged from
there: the ACK (`02 <version> seq`, pending bit 0) starts 192 µs (12 symbols) after the frame's
end and takes 11 bytes of air, 352 µs, then `ACK_TX_DONE`. The ACK leaves through the same host
path as a data frame — the round is cut at its first cycle — so a lock-stepped medium sees it
in time for the sender's ACK wait. The two radio timers count microseconds to their thresholds.
ED keeps the synthetic scene from before. **Not modelled**: enhanced ACKs (frame version 2),
security, the frame-pending table, CCA against energy that is not a frame. `--debug ieee802154`
narrates commands, events and DMA; `--log-periph` reports the first touch of an offset the
model does not interpret.

Why the filter and the ACK matter to the driver: without them, a frame with the AR bit that was
*not* addressed to the C6 (another node's unicast) still raised `RX_DONE`, the driver went to
`TX_ACK` for an `ACK_TX_DONE` that never came, and from then on refused every transmit before
touching hardware (`TX_ERR_ABORT`) and never re-armed RX — the way stage 1 died in an RPL
network.

## Cooja-NG lock-step: `--cooja`

`esp32sim-c6 --cooja` makes the C6 an *external mote* of Cooja-NG (csim,
`docs/design/external-nodes-plan.md` §4): csim owns the clock and drives the guest over NDJSON on
stdin/stdout — `hello` once, `step {t, in}` per slice, `stop`; one `done {t, wake, out}` per
message back. Times are nanoseconds; ours is the bus cycle counter at 6.25 ns per cycle, exact
integer arithmetic both ways, monotonic across a chip reset.

```sh
# what csim runs (the config's "firmware" string); csim writes hello/step/stop on its stdin
B=~/work/esp32/esp32-contiki/build-nullnet
esp32sim-c6 --cooja --boot rom --flash-mb 2 \
    --bootloader $B/bootloader/bootloader.bin --ptable $B/partition_table/partition-table.bin \
    --app $B/esp32-blink.bin --elf $B/esp32-blink.elf
```

Exactness is the point, not "within a slice":

- a `step` runs the guest to the first cycle at or after `t` (`Machine::run_until_cycle`), not to
  the end of a 64-instruction quantum;
- the round is cut at the instruction that writes `TX_START`: the `tx` event carries that time,
  and the reply goes out before the slice ends (`SocBus::take_host_event`), with `done.t` = that
  time so csim's clock for the node stops there. csim delivers the frame at its own clock, the
  end of the slice — the same lateness an emulated mote's radio has — so the busy-slice `wake`
  (`--cooja-slice-us`, default 100 µs) bounds it. The rest of the slice runs when csim steps again;
- an `rx` inside a slice is injected at its own time: the guest runs to `rx.t` and the frame goes
  to the radio as csim reports it, at its start — `rx.t` is the frame's first preamble byte, so
  `RX_SFD_DONE` follows 5 bytes (160 µs) later, `RX_DONE` at the end of the whole on-air frame,
  and the acknowledgement 192 µs after that, which is when the sender's CC2420 expects it
  (`--cooja-rx-timing end` takes `t` as the frame's end instead) — and the run continues.
  IDF timestamps a received frame by reading `esp_timer` in its `RX_SFD_DONE` handler, so that
  cycle is the timestamp the driver records;
- a guest in `wfi` costs nothing: time jumps to the bus's next device deadline (systimer, TIMG,
  the radio's countdowns, a running RMT channel) or the slice end, whichever is first, so the
  FreeRTOS idle loop is free and `wake` is the deadline;
- nothing reads a host clock or host randomness — the MAC comes from the node id (`--mac` to
  override), the RNG is the model's xorshift — so the same NDJSON in gives the same NDJSON out,
  byte for byte (`cli/tests/cooja.rs` checks it twice over).

Console lines go out as `log` events (UART0 by default, `--console` picks), stamped within 20 µs
of the newline. Radio state changes are `radio` events. Serial input (`serial` events) reaches the
USB-Serial/JTAG console. The end-of-run figures go to stderr.

Measured with the esp32-contiki nullnet probe (ROM → bootloader → FreeRTOS → Contiki-NG, a
broadcast every 5 s, one injected): 10.3 s of simulation in ~0.1 s wall with 10 ms steps, and
12 s in 0.33 s with a driver that follows `wake` (17825 exchanges, ~19 µs each) — the guest is
idle 99% of the time, and an idle second costs one exchange per systimer tick it wakes for. A
busy second (boot) runs at ~160 M instructions/s. Inside csim (`configs/test-ext-esp32c6-nullnet.json`
there: a Sky node, the C6 and a sniffer exchanging nullnet broadcasts for 20 s) the C6's
broadcasts reach the Sky and the Sky's reach Contiki on the C6.

## The board: `--board waveshare-c6-lcd147`

The Waveshare ESP32-C6-LCD-1.47 wires an ST7789 172×320 panel to SPI2 (MOSI 6, SCLK 7, CS 14,
D/C 15, RST 21, backlight 22 on LEDC), a WS2812 to GPIO 8 over the RMT, a BOOT button on GPIO 9
and a TF card to the same SPI bus (MISO 5, CS 4; not modelled). `esp32c6/src/board.rs` is the
board: a `DcsPanel` (esp-soc `devices`) with the ST7789's RAM, fed the SPI bytes with the D/C GPIO level (commands and parameters are
separate transactions, and the driver sets D/C right before each one, so the bus delivers GPIO
edges to the board before the bytes that follow them), the LED decoded from the RMT frame, the
button as `press boot 150` in a script.

The firmware that lives on this board — the owner's IEEE 802.15.4 energy scanner
(`examples/waveshare-c6-lcd147/`, an LVGL spectrum of channels 11–26) — runs end to end:

```sh
ENERGY_SCAN_DIR=~/work/esp32/energy_scan examples/waveshare-c6-lcd147/run.sh --max-seconds 8 --tft-png lcd.png
```

- Colours are the real module's, checked against the board: Waveshare's driver sets RAMCTRL
  (`0xB0: 0x00, 0xE8`) so pixels arrive low byte first, which is how LVGL without
  `LV_COLOR_16_SWAP` writes them, and the glass exchanges red and blue unless MADCTL's BGR bit
  is set. Firmware that looks right here looks right there: BGR set, RGB565 as it is in memory.
  (Until 2026-09-21 the model read high byte first and had the BGR rule the other way round, so
  firmware tuned to it showed green as blue on the board, and the scanner's red bars were
  yellow-green here.)
- The ST7789 goes through `esp_lcd`'s SPI panel IO: every transfer is a GDMA out-channel
  descriptor chain (the driver enables DMA for the bus), so the C6's GDMA layout and the SPI's
  DMA data phase both had to exist before the first pixel arrived. LVGL flushes ~500 frames a
  second here.
- The WS2812 driver (`led_strip` over RMT) blocks on the transmit-done interrupt; the RMT
  frames are decoded to bits by the shared model.
- `esp_ieee802154_energy_detect` programs a duration, issues `ED_START` and takes the `ED_DONE`
  interrupt; the model completes the scan after the programmed symbols with a synthetic 2.4 GHz
  picture — a −93 dBm floor with the three non-overlapping WiFi channels (802.15.4 channels
  11–14, 16–19, 21–24) sitting on it. The picture moves: every 2.5 s a network changes level or
  a burst lands on a channel and fades, and levels drift 1 dB per 100 ms toward their targets,
  all from one xorshift so a run is reproducible (`Ieee802154::set_channel_dbm` sets a level
  outright). ~260 scans in 8 s, 16 bars on the screen.
- The PHY blob's init runs in full, no stub. `bb_init`, the baseband calibration (TX DC,
  power-detector tones, RX DCO, the RX gain tables), once spun forever on handshakes in the
  undocumented block at `0x600A0000` and was stubbed with `--stub bb_init=0`. #203 ([EX220](experiments.md#ex220),
  `42c5a547`) modelled the two fields it waits on, inferred from the blob — TX-DC done at `+0x418`
  and the tone readback at `+0x814` (`esp32c6/src/wifi.rs`, `ModemBb`) — and since then it completes:
  3.81 M cycles, 23.8 ms of device time, 3.1 M of them `ets_delay_us`, returning 0 (#248).
  Everything before it (`rf_init`, the parameter tables the PLL-tracking timer later dereferences)
  runs; stubbing `register_chipv7_phy` as a whole crashed that timer. The 802.15.4 MAC model does
  not depend on the PHY. The blob's own `wait_i2c_sdm_stable` polls the RF block over regi2c for
  0x5b, which the analog master answers.
- The app writes its PHY calibration into NVS on first boot ("Saving new calibration data"),
  through the shared flash model.

`--stub` and `--trace-fn` are exact on the RISC-V chips now: the core stops at the addresses the
machine wants to see, as the Xtensa block interpreter always did.

## The board: `--board xiao-esp32c6`

The Seeed Studio XIAO ESP32-C6 is a small board around the same chip: an ESP32-C6FH4 with 4 MB of
flash and no PSRAM, a user LED on GPIO 15, the BOOT button on GPIO 9 and the RF switch on GPIO 3
(power, low active) and GPIO 14 (the antenna). `esp32c6/src/board/xiao.rs` is the board model.

The LED is **low active**: the anode goes to 3V3 through a resistor, the cathode to GPIO 15, so a
low pin is a lit LED. The model reads the GPIO drive masks (`ENABLE` and `OUT`) with every output
write instead of the edges, because an edge says what the pin did and the masks say what the pin
is doing now: a released pin is high, whatever the last edge was. A firmware that blinks the pin
the usual way lights the LED here exactly as it does on the board.

```sh
cd examples/xiao-esp32c6 && idf.py set-target esp32c6 build && cd ../..
examples/xiao-esp32c6/run.sh --max-seconds 10
```

The blink firmware in `examples/xiao-esp32c6/` prints one line per half second and the LED
blinks with it:

```
LED on, BOOT button released
```

The BOOT button is pulled up on the board, so it reads high while it is released; a script
presses it with `press boot 150` and the browser's BOOT button drives GPIO 9 low. The RF switch
is tracked and reported (`[emu] xiao-esp32c6: ... rf switch powered, antenna onboard ceramic`)
but nothing is behind it: the radio is the C6's own 802.15.4 and WiFi MAC, which the emulator
models on the chip, not on the board.

The pin map is the Seeed one (D0–D10 and A0–A2 on GPIO 0, 1, 2, 21, 22, 23, 16, 17, 19, 20, 18;
the back pads MTMS/MTDI/MTCK/MTDO on GPIO 4, 5, 6, 7; `EN` is the chip's reset, not a GPIO).
Nothing is wired to those pins on this board, so they are left alone: no ADC, no I2C, no TWAI,
no display.

## Not there yet

- **TF card, backlight rendering.** The SD slot on SPI2 has nothing behind it. LEDC frequency and duty
  are modelled, but the panel is shown regardless of backlight duty.
- **The PHY's baseband calibration** (above): a stub, not a model.
- **`--boot app`** maps the image through the unified MMU and jumps to it, but the system
  registers the bootloader would have set up are not preset; ROM boot is the tested path.
- **Watchdogs.** The LP_WDT and the TIMG watchdogs are register RAM: they never fire.
- **WiFi** beyond one station: legacy rates only, no power save, no TSF.
- **BLE, the LP core** — nothing of that radio or the second core is modelled. The
  802.15.4 MAC sends, receives, acknowledges and filters (above); enhanced ACKs and security are not there.
- **Peripherals on demand**: GDMA, I2C, SPI2, RMT, ADC, TWAI, PARL_IO. Each shows up as an
  unknown register with `--log-periph` the moment a firmware wants it. The registers hello_world
  still touches without a model are PMU, IO_MUX, HP_SYSTEM, APB_SARADC and a few LP blocks —
  all of them configuration the firmware only writes and reads back.

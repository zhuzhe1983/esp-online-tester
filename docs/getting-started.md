# Getting started

How to build esp32sim and run firmware on each chip. For every flag, see the
[CLI reference](cli.md). The short version, from a release tarball with no Rust toolchain, is
[quick-start.md](quick-start.md).

- [The chips at a glance](#the-chips-at-a-glance)
- [Build and get the ROMs](#build-and-get-the-roms)
- [ESP32-S3](#esp32-s3)
- [ESP32-C3](#esp32-c3)
- [ESP32-C6](#esp32-c6)
- [Classic ESP32](#classic-esp32)
- [Scripts: host actions at emulated time](#scripts-host-actions-at-emulated-time)
- [WiFi](#wifi)
- [In the browser (WebAssembly)](#in-the-browser-webassembly)
- [Live board UI](#live-board-ui)
- [Debugging](#debugging)
- [Real time on the ESP32-S3](#real-time-on-the-esp32-s3)

## The chips at a glance

| | ESP32-S3 | ESP32-C3 | ESP32-C6 |
| --- | --- | --- | --- |
| CPU | 2 x Xtensa LX7 @ 240 MHz | 1 x RISC-V RV32IMC @ 160 MHz | 1 x RISC-V RV32IMAC @ 160 MHz |
| Command | `esp32sim` | `esp32sim-c3` | `esp32sim-c6` |
| Core crate | `xtensa-lx7` | `riscv-rv32` | `riscv-rv32` (+ the A extension) |
| Boots | ROM, bootloader, FreeRTOS, app | ROM, bootloader, FreeRTOS, app | ROM, bootloader, FreeRTOS, app |
| Boards / displays | ST7735, ST7701S 480x480 + touch, WS2812, camera, audio | none, console only | ST7789 172x320 over SPI+DMA, WS2812, an 802.15.4 energy-detect stand-in |
| WiFi | virtual AP, WPA2, DHCP/DNS/NTP, NAT to the real network, HTTPS | the same virtual AP and network: scan, WPA2, DHCP, NAT (one station) | the same virtual AP and network: scan, WPA2, DHCP, NAT (one station, legacy rates) |
| Speed | block interpreter and AArch64 JIT, about 150 to 240 Minsn/s | plain interpreter (no block cache or JIT yet), still well above real time on hello_world | same interpreter |
| In the browser | yes (WebAssembly) | yes (WebAssembly) | yes (WebAssembly) |
| Status | mature | draft, see [esp32c3.md](esp32c3.md) | draft, see [esp32c6.md](esp32c6.md) |

The classic ESP32 is described in [esp32.md](esp32.md). How each chip is checked against silicon is
in [hardware-testing.md](hardware-testing.md).

Most of the SoC is shared. The C3 reuses the S3's UART, USB-Serial/JTAG, systimer, timer groups,
GPIO, SPI flash controller, GDMA and SHA/AES/RSA models unchanged, and adds its own memory map,
cache controller and interrupt matrix. The C6 reuses the same models again, with its own memory
map, PLIC front-end, L1 cache, PCR and always-on LP blocks. Only the CPU crates are genuinely
separate. The crate layout is in [architecture.md](architecture.md).

## Build and get the ROMs

```sh
cargo build --release
```

This builds `esp32sim`, which runs every chip (`--chip s3|c3|c6|esp32`), and the alias binaries
`esp32sim-c3` and `esp32sim-c6`.

Every run boots from the chip's mask ROM ELF, which is Espressif's and not committed. If you have
ESP-IDF installed, the S3's ROM is picked up from
`~/.espressif/tools/esp-rom-elfs/*/esp32s3_rev0_rom.elf`. Without ESP-IDF:

```sh
tools/fetch-demo-assets.sh --no-linux      # the S3, C3 and C6 ROMs into web/wasm/fw/
```

Then pass `--rom web/wasm/fw/esp32s3_rev0_rom.elf` (or the C3/C6 file), or set
`ESP32SIM_ROM_DIR=$PWD/web/wasm/fw`, which the tests also use. Give the tests an absolute path:
`cargo test` runs each test from its own crate's directory.

## ESP32-S3

### The demo firmware

```sh
FW=web/wasm/fw/public                       # the demo images the goldens and the web page run
./target/release/esp32sim --boot rom \
    --bootloader $FW/atech-bootloader.bin --ptable $FW/atech-ptable.bin --app $FW/atech-firmware.bin \
    --script $FW/atech-script1.txt --wav out.wav --tft-png tft.png --max-seconds 5
```

This is the Pocket Synth firmware of the Atech 14-port kit. It is its own project,
[atech-firmware](https://github.com/joakimeriksson/atech-firmware): a PlatformIO build of a SID synth
and a cRSID player for real `.sid` tunes. This repository carries the images it produces, in
`web/wasm/fw/public/`.

`--boot app` skips the ROM and bootloader and loads the app image directly.

### Your own ESP-IDF project

For example `examples/hello_world`, the IDF 5.5 get-started example built with
`idf.py set-target esp32s3 && idf.py build`:

```sh
H=examples/hello_world/build
./target/release/esp32sim --board none --boot rom --console uart0 \
    --bootloader $H/bootloader/bootloader.bin --ptable $H/partition_table/partition-table.bin \
    --app $H/hello_world.bin --elf $H/hello_world.elf --elf $H/bootloader/bootloader.elf --max-seconds 26
```

It prints the ROM banner, the bootloader and app logs on UART0, `Hello world!` and the countdown.
Then it reboots through the RTC watchdog exactly as silicon does (`rst:0xc (RTC_SW_CPU_RST)`),
about 30 times faster than real time.

Chip resets (software, RTC watchdog) restart the machine from the ROM with the right reset cause.
`--no-reboot` stops at the first reset instead.

### Boards

The SoC model emits pin-level events (GPIO edges, RMT symbol streams, I2S samples), and a board
model interprets them. Pick one with `--board`:

| `--board` | Board |
| --- | --- |
| `atech14` (default) | The Atech 14-port kit: ST7735 display, WS2812 ring, encoder and buttons |
| `none` | A bare module: any ESP32-S3 firmware, console only |
| `waveshare-lcd4b` | Waveshare ESP32-S3-Touch-LCD-4B: ST7701S 480x480 over the LCD_CAM RGB bus, GT911 touch, TCA9554, codecs. Runs the esp32-screen LVGL panel with touch |
| `waveshare-amoled18-v2` | Waveshare ESP32-S3-Touch-AMOLED-1.8 V2: CO5300 368x448 over QSPI, CST820 touch |
| `waveshare-cam` | Waveshare ESP32-S3-CAM-OV5640: CH32V003 IO expander, OV5640 over SCCB and on the LCD_CAM DVP port, ES8311/ES7210 codecs on I2C0, speaker on I2S1 |

The `waveshare-cam` board runs the `waveshare-autopling` firmware end to end (IDF 5.5, 16 MB flash,
8 MB octal PSRAM: `--flash-mb 16 --psram-mb 8`). Camera frames come from `--cam-image` or, in the
browser, from a picture upload or the webcam. An esp-dl pedestrian detector runs on the emulated
PIE SIMD unit, and a detection plays a sound on the ES8311. See `examples/waveshare-cam/`.

To add a board, implement the `BoardModel` trait (`gpio_changes`, `rmt_frame`, `i2c_devices`,
`camera_frame` and so on). See [boards.md](boards.md).

## ESP32-C3

`esp32sim-c3` runs unmodified ESP-IDF firmware on the emulated RISC-V core, from the real mask ROM
through the real bootloader into FreeRTOS. It takes the same flags as `esp32sim` (`--trace`,
`--break`, `--watch`, `--peek`, `--disasm`, `--log-periph`) and prints RISC-V mnemonics with symbols.

```sh
H=examples/hello_world-c3/build
./target/release/esp32sim-c3 --boot rom --flash-mb 4 \
    --bootloader $H/bootloader/bootloader.bin --ptable $H/partition_table/partition-table.bin \
    --app $H/hello_world.bin --elf $H/hello_world.elf --max-seconds 26
```

It prints the ROM banner, the bootloader log, `Hello world!` and the reboot, three times over.

- `--mac`, `--reset-cause` and `--strap` let a run adopt a real board's identity, so that a
  comparison with that board is meaningful.
- WiFi station mode supports open and WPA2 virtual access points, with DHCP and NAT.
- There are no boards, and `--boot app` is not supported.

See [esp32c3.md](esp32c3.md) for what works, what does not, and the five emulator bugs the
hardware found.

## ESP32-C6

`esp32sim-c6` is the same front end on the ESP32-C6: RV32IMAC, the PLIC, a unified memory map.

```sh
H=examples/hello_world-c6/build
./target/release/esp32sim-c6 --boot rom --flash-mb 4 \
    --bootloader $H/bootloader/bootloader.bin --ptable $H/partition_table/partition-table.bin \
    --app $H/hello_world.bin --elf $H/hello_world.elf --max-seconds 27
```

- `--board waveshare-c6-lcd147` adds the Waveshare ESP32-C6-LCD-1.47: the ST7789 over SPI2 and the
  GDMA, the WS2812 and the BOOT button.
- The C6's 802.15.4 MAC answers energy scans, so the board's LVGL spectrum-scanner firmware runs end
  to end (`examples/waveshare-c6-lcd147/`). The MAC also sends and receives frames with the timing
  of the air.
- `--cooja` makes the C6 an external mote of Cooja-NG, driven in exact lock-step over NDJSON. An
  unmodified Contiki-NG-on-IDF image exchanges broadcasts with emulated MSP430 nodes.
- `--wifi` attaches the same virtual access point as on the S3 (see [WiFi](#wifi)).

See [esp32c6.md](esp32c6.md).

## Classic ESP32

The classic ESP32 boots its ECO3 mask ROM on dual LX6 cores, with GPIO, UART and timers:
`--chip esp32`. `--board ttgo-tdisplay` adds the LilyGO TTGO T-Display's ST7789 and buttons. See
[esp32.md](esp32.md) for how to run it.

## Scripts: host actions at emulated time

`--script FILE` performs actions at given emulated times:

```
1.5  press btn1 150                       # GPIO17 low for 150 ms
2.2  knob cw 2                            # two quadrature detents on CLK5/DT4
3.4  press btn2 120
4.0  serial {"action":"set_note","value":"9"}   # into USB-CDC RX
5.0  stop
```

## WiFi

WiFi works on the ESP32-S3, C3, C6 and the classic ESP32.

`--wifi ssid=NAME[,psk=PASS,chan=N,bssid=..]` attaches a virtual access point. The unmodified
Espressif WiFi blob associates with it: scan, authentication, association and, with a passphrase,
the WPA2-PSK four-way handshake. Behind it is a virtual network with DHCP, ARP, ICMP, DNS, and SNTP
off the host clock.

`--net nat` (the default) relays the guest's TCP and UDP over ordinary host sockets, so firmware
reaches the real network. `--net none` refuses it. `--hostfwd tcp:127.0.0.1:8080-:80` publishes a
guest TCP server on the host. No firmware changes, no root, no tun device.

```sh
./target/release/esp32sim --board waveshare-lcd4b --boot rom --flash-mb 16 --psram-mb 8 \
    --bootloader $P/bootloader/bootloader.bin --ptable $P/partition_table/partition-table.bin \
    --app $P/energy_panel.bin --console usb --wifi "ssid=home,psk=secret" --max-seconds 45
```

This runs the esp32-screen energy panel: it joins, takes a lease, syncs its clock, fetches two days
of electricity prices over HTTPS and polls a real Home Assistant on the LAN.

The ESP32-C6 joins the same access point and network through its own MAC model
([wifi-c6-plan.md](wifi-c6-plan.md)): one station, legacy rates, no power save.
`examples/c6-wifi-station` is the specimen: scan, join, lease, five pings, and the state on the
board's screen:

```sh
B=examples/c6-wifi-station/build-emu
./target/release/esp32sim-c6 --boot rom --flash-mb 4 --board waveshare-c6-lcd147 --console usb \
    --bootloader $B/bootloader/bootloader.bin --ptable $B/partition_table/partition-table.bin \
    --app $B/c6_wifi_station.bin --elf $B/c6_wifi_station.elf \
    --wifi ssid=esp32sim,psk=esp32sim-pass --max-seconds 14
```

The ESP32-C3 joins the same access point too, through a MAC model derived from the S3's. The same
station example, built for each chip without the display, is committed under `web/wasm/fw/public/`
as the input of the WiFi goldens (`wifi_station_{s3,c3,c6}` in `cli/tests/goldens.rs`), so it runs
without a local build:

```sh
P=web/wasm/fw/public
./target/release/esp32sim-c3 --boot rom --flash-mb 4 --console usb \
    --bootloader $P/c3-wifi-bootloader.bin --ptable $P/c3-wifi-ptable.bin --app $P/c3-wifi_station.bin \
    --wifi ssid=esp32sim,psk=esp32sim-pass --max-seconds 14
```

`examples/c6-radio-dashboard` runs WiFi and 802.15.4 energy scans in one firmware, a page each.

More:

- [networking-howto.md](networking-howto.md): flags, debugging and limits.
- [wifi-plan.md](wifi-plan.md) and [networking-plan.md](networking-plan.md): how the MAC model and
  the packet path work.
- [architecture.md, Networking](architecture.md#networking): the layers every chip shares.

## In the browser (WebAssembly)

The live site is <https://mikroverk.github.io/esp32sim/>. To build and serve it yourself:

```sh
tools/wasm-build.sh                  # the module -> web/wasm/esp32sim.wasm
tools/fetch-demo-assets.sh           # mask ROMs, xterm.js and the Linux image (--no-linux skips its 16 MB)
python3 -m http.server -d web 8790   # then open http://127.0.0.1:8790/?wasm&fw=hello
```

The demo firmware is committed. The mask ROMs, xterm.js and the Linux image are not, and every demo
boots from the ROM, so run the fetch once. It gets the same files the Pages site serves.

The emulator runs inside the page, in a Web Worker. Pick a board, load the firmware from disk (or
use `?wasm&fw=<name>` for a hosted manifest), and press Boot. The ROM ELF input is optional: left
empty, the page fetches the site's copy of the chip's mask ROM (`wasm/fw/<chip>_rom.elf`, which the
fetch above puts there). Booting again replaces the machine and keeps the chosen files.

- hello_world, the Touch-LCD-4B panel with its SID player, the Atech board, the ESP32-C3 and the
  ESP32-C6 all run at real time in Chrome.
- There is no NAT, because the browser has no sockets.
- The S3 has a WebAssembly JIT of its own (hot blocks and regions, PIE on WASM SIMD).

See [wasm.md](wasm.md).

## Live board UI

The native emulator can also serve the board UI:

```sh
./target/release/esp32sim --boot rom --bootloader B.bin --ptable P.bin --app A.bin --web 8766
# open http://127.0.0.1:8766/
```

`--web PORT` runs the emulator in real time and serves `web/run.html`. On the Atech kit it shows:

- the 14-port board: knob and LED ring, buttons, a speaker VU meter, and the ST7735 in its physical
  orientation plus a readable copy;
- USB-CDC and UART0 consoles, and an action box for the SDK JSON protocol;
- audio through WebAudio.

Click the buttons, turn the knob with the mouse wheel, by dragging or with the left and right arrow keys, and click the cap to push it. The protocol
is described in [web-ui.md](web-ui.md).

## Debugging

| Flag | Does |
| --- | --- |
| `--trace [--trace-from N]` | trace instructions |
| `--break ADDR` | stop at an address |
| `--watch ADDR` | stop when a word changes |
| `--peek ADDR[,N]` | print memory at the end of the run |
| `--profile` | PC histogram |
| `--log-periph` | log the first touch of every unknown register |
| `--stop-after-exceptions N` | stop after N exceptions |
| `--gram-png` | dump the raw ST7735 GRAM |
| `--no-jit` | interpret instead of running native code; must give identical results |

Environment variables: `ESP_EMU_DEBUG`, `ESP_EMU_DEBUG_SPI`, `ESP_EMU_DEBUG_USB`,
`ESP_EMU_DEBUG_WIFI[_FRAMES]`, `ESP_EMU_DEBUG_NET`, `ESP_EMU_DEBUG_AES`, `ESP_EMU_DEBUG_SHA`,
`ESP_EMU_DEBUG_RSA`.

## Real time on the ESP32-S3

Blocks of instructions are decoded once and executed from a cache, then compiled to native AArch64
code. Idle cores (`waiti`) are skipped, device time is delivered lazily against computed timer
deadlines, and interrupt lines are recomputed only when a source changes. See
[speed-plan.md](speed-plan.md) for the measurements behind each step.

The Pocket Synth firmware runs at real time with margin while idle or playing notes. The one place
it cannot keep up is a full display redraw, where core 1 runs 100 % busy bit-banging SPI: 4.8 M
instructions per 20 ms, so 240 Minsn/s needed against about 70 Minsn/s achieved on an M-series
MacBook. Each redraw therefore costs about 0.3 s of lag:

- the UI's adaptive audio buffer absorbs it (it grows on underrun, up to 400 ms), and the pacer
  recovers afterwards;
- if the emulator ever falls more than 0.5 s behind, it resynchronises instead of bursting;
- the header shows `real time`, `⚠ N s behind` and the resync count.

`ESP_EMU_RT_LOG=1` prints every 20 ms window that took more than 40 ms of wall time, with both
cores' instruction counts and PCs. `hw/wsdrive.py [port] [seconds]` drives the UI protocol without
a browser (button presses and knob turns) and reports push gaps, lag and audio delivered. Use it to
measure changes to the scheduler.

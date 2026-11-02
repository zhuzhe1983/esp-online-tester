# Quick start

Your own firmware running in the emulator in three steps: install, get the chip's mask ROM, run.
Then the few flags that answer "what did it do", and where to go when it does not work. Every flag
is in the [CLI reference](cli.md); each chip has its own page in [getting-started.md](getting-started.md).

Nothing here needs a Rust toolchain or a checkout of this repository.

- [1. Install](#1-install)
- [2. Get the mask ROM](#2-get-the-mask-rom)
- [3. Run your build](#3-run-your-build)
- [In the browser instead](#in-the-browser-instead)
- [Read what happened](#read-what-happened)
- [Drive it: scripts, WiFi, the board in a browser](#drive-it-scripts-wifi-the-board-in-a-browser)
- [When it does not work](#when-it-does-not-work)

## 1. Install

Download the tarball for your machine from the
[releases page](https://github.com/mikroverk/esp32sim/releases) (macOS Apple silicon, Linux x86_64,
Linux arm64) and unpack it:

```sh
tar xzf esp32sim-*.tar.gz
cd esp32sim-*/
./esp32sim --version
```

It holds `esp32sim` (every chip: `--chip esp32|s3|c3|c6|p4`, default S3), the aliases `esp32sim-c3`,
`esp32sim-c6` and `esp32sim-p4`, this page, the CLI reference, the getting-started guide and `tools/fetch-rom-elfs.sh`.
Put the directory on your `PATH` or call the binaries by path; the examples below assume the current
directory.

On macOS the first start of a downloaded binary may be refused by Gatekeeper ("cannot be opened
because the developer cannot be verified"): allow it under System Settings → Privacy & Security, or
`xattr -d com.apple.quarantine esp32sim esp32sim-c3 esp32sim-c6 esp32sim-p4`.

Building from source works as well: `cargo build --release` in a checkout puts the same four
binaries in `target/release/`.

## 2. Get the mask ROM

Every run boots the chip's own mask ROM, exactly as the silicon does. The ROM is Espressif's
(Apache-2.0, [espressif/esp-rom-elfs](https://github.com/espressif/esp-rom-elfs)) and is not in
the tarball. One command fetches all five, pinned by SHA-256:

```sh
tools/fetch-rom-elfs.sh roms
```

Then either pass `--rom roms/<chip>_rom.elf` on each run, or set the directory once:

```sh
export ESP32SIM_ROM_DIR=$PWD/roms
```

| Chip | ROM file |
| --- | --- |
| ESP32-S3 | `esp32s3_rev0_rom.elf` |
| ESP32-C3 | `esp32c3_rev3_rom.elf` |
| ESP32-C6 | `esp32c6_rev0_rom.elf` |
| ESP32 (classic) | `esp32_rev300_rom.elf` |

If ESP-IDF is installed, the ROMs are found on their own under `~/.espressif/tools/esp-rom-elfs/`.
Without a ROM the emulator stops at once with `no <chip> mask ROM ELF found`.

## 3. Run your build

Build the firmware as always: ESP-IDF, PlatformIO or Arduino, unmodified. A build directory holds
the three images the emulator takes, and the ELF for symbols:

```
build/bootloader/bootloader.bin
build/partition_table/partition-table.bin
build/<app>.bin
build/<app>.elf
```

ESP-IDF, for the ESP32-S3 (`idf.py set-target esp32s3 build`):

```sh
H=path/to/your/project/build
./esp32sim --board none --boot rom --console uart0 \
    --bootloader $H/bootloader/bootloader.bin --ptable $H/partition_table/partition-table.bin \
    --app $H/my_app.bin --elf $H/my_app.elf --max-seconds 10 --no-dump
```

The ROM banner, the bootloader log and the app's output appear on the console as they would on
UART0, then the run stops after 10 emulated seconds and prints its report. `--board none` is a bare
module: any firmware, console only; the boards are listed under `--board` in
[the CLI reference](cli.md#images-and-boot).

The other chips take the same flags. The RISC-V chips default to a 4 MB flash and UART0:

```sh
./esp32sim-c3 --boot rom --bootloader $H/bootloader/bootloader.bin \
    --ptable $H/partition_table/partition-table.bin --app $H/my_app.bin --elf $H/my_app.elf --max-seconds 10
./esp32sim-c6 --boot rom --bootloader $H/bootloader/bootloader.bin \
    --ptable $H/partition_table/partition-table.bin --app $H/my_app.bin --elf $H/my_app.elf --max-seconds 10
./esp32sim --chip esp32 --board esp32dev --boot rom --bootloader $H/bootloader/bootloader.bin \
    --ptable $H/partition_table/partition-table.bin --app $H/my_app.bin --elf $H/my_app.elf --max-seconds 10
```

Flash and PSRAM sizes follow the module: `--flash-mb 16 --psram-mb 8` for a 16 MB / 8 MB S3 module
(the S3 default is 8 / 2). PlatformIO puts the same three files under `.pio/build/<env>/`; an
Arduino export (`Sketch → Export compiled binary`) gives `<sketch>.ino.bootloader.bin`,
`<sketch>.ino.partitions.bin` and `<sketch>.ino.bin`, which go to the same three flags. A whole flash
dump (`esptool read_flash`) goes to `--flash-image F` instead of the three images.

Always give a limit (`--max-seconds S` or `--max-insns N`): the run then returns on its own, with
the same result every time, down to the instruction count.

## In the browser instead

The same emulator runs in a browser tab at **<https://mikroverk.github.io/esp32sim/>**: press
*Configure a machine*, pick the board, then your `bootloader.bin`, `partition-table.bin` and app.
Leave *ROM ELF* empty and the site's copy of the chip's mask ROM is used. Nothing is uploaded; the
files are read by the page in your browser. Boot again after changing a file and the machine is
replaced; the chosen files stay.

The browser has no sockets, so WiFi reaches a virtual access point but not the network, and there
is no native JIT: hello_world and the display demos run at real time, heavy workloads slower.

## Read what happened

The console is what the firmware printed. The rest comes from flags that leave files or lines in
the end-of-run report:

| Want | Flag |
| --- | --- |
| the display, as the firmware drew it | `--tft-png frame.png` |
| the audio the firmware played | `--wav out.wav` |
| GPIO edges and interrupt lines as a waveform | `--vcd gpio.vcd` (open it in GTKWave or PulseView) |
| a PWM pin's frequency and duty | `--pwm 4` |
| an ADC pad's last 12-bit sample | `--adc 1` |
| memory at the end of the run | `--peek 0x3fc90000,64` |
| which functions ran, and for how long | `--profile-blocks` (needs `--elf`) |
| every call to a function, with arguments | `--trace-fn my_function` |
| what a register block was asked to do | `--debug spi,i2c,wifi` (device names or prefixes) |
| the first touch of every register the model lacks | `--log-periph` |

The report after the run lists the stop reason, the instruction count per core, exceptions,
interrupts per source, and the board's own line (LEDs, display frames, audio frames). `--no-dump`
leaves out the register dump.

## Drive it: scripts, WiFi, the board in a browser

**Scripts** act on the board at emulated times, one line each, so a test is reproducible:

```
1.5  press btn1 150          # a button for 150 ms (or a GPIO number)
2.0  gpio 17 0
3.0  touch 450 240 1         # touch panel press at (450, 240); `touch x y 0` releases
4.0  serial hello            # into the USB-Serial/JTAG console
4.2  uart0 root              # a line into UART0
6.0  stop
```

```sh
./esp32sim ... --script actions.txt --max-seconds 8
```

**WiFi**: `--wifi ssid=home,psk=secret` attaches a virtual access point the unmodified Espressif
WiFi library joins (scan, WPA2, DHCP, DNS, NTP), and `--net nat` (the default) lets the firmware
reach the real network through the host's sockets, HTTPS included. No root, no tun device. See
[networking-howto.md](networking-howto.md).

**The board in a browser**: `--web 8766` runs in real time and serves the board at
`http://127.0.0.1:8766/`: the display, buttons, knob and touch, the consoles with keyboard input,
audio. Nothing else changes.

## When it does not work

- **`no esp32s3 mask ROM ELF found`**: step 2. Pass `--rom` or set `ESP32SIM_ROM_DIR`.
- **`not an ESP image (magic 0xE9 missing)`**: a flag got the wrong file. `--bootloader`,
  `--ptable` and `--app` take the three `.bin` images; a whole flash dump goes to `--flash-image`.
- **The console stops after the bootloader, or the app reboots at once**: the module sizes. Set
  `--flash-mb` and `--psram-mb` to what the firmware was built for; `--console both` also shows the
  USB-Serial/JTAG console, where IDF prints when `CONFIG_ESP_CONSOLE_USB_SERIAL_JTAG` is set.
- **A reboot loop with `rst:0x7 (TG0WDT_SYS_RST)` or a panic**: real behaviour of the firmware on
  this model, most often a peripheral the emulator does not have yet. `--log-periph` names the first
  register of each block the firmware touched that the model lacks; `--debug <block>` shows what a
  modelled block was asked to do. [peripherals.md](peripherals.md) lists what each chip has.
- **A function that must not run** (a driver for a missing device, a calibration): `--stub name=0`
  returns the value at its entry; needs `--elf` for the name.
- **A result that looks wrong**: run again with `--no-jit`. The interpreter and the native code must
  agree to the instruction; if they do not, that is an emulator bug worth an issue with the
  firmware attached.
- **Something else**: `--trace-fn`, `--break PC`, `--watch ADDR` and `--trace` are in the
  [debugging table](cli.md#debugging).

Issues and questions: <https://github.com/mikroverk/esp32sim/issues>. A firmware image plus the
command line is the most useful report; nothing in the emulator phones home.

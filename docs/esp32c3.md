# ESP32-C3 (RISC-V) — draft

A second chip in the same workspace: one RV32IMC core at 160 MHz instead of two Xtensa LX7s.
Unmodified ESP-IDF firmware boots from the real mask ROM, through the real 2nd-stage bootloader,
into FreeRTOS.

```sh
cargo build --release
H=examples/hello_world-c3/build
./target/release/esp32sim-c3 --boot rom --flash-mb 4 \
    --bootloader $H/bootloader/bootloader.bin --ptable $H/partition_table/partition-table.bin \
    --app $H/hello_world.bin --elf $H/hello_world.elf --max-seconds 26
```

prints the ROM banner, the bootloader's partition table, the IDF startup log, `Hello world!`, the
countdown, and then reboots through the ROM on `esp_restart()` — the same end-to-end path the S3
`hello_world` test covers:

```
ESP-ROM:esp32c3-api1-20210207
rst:0x1 (POWERON),boot:0xf (SPI_FAST_FLASH_BOOT)
I (4) boot: ESP-IDF v5.5.4 2nd stage bootloader
I (4) boot: chip revision: v0.4
I (28) main_task: Calling app_main()
Hello world!
This is esp32c3 chip with 1 CPU core(s), WiFi/BLE, silicon revision v0.4, 2MB external flash
```

The mask ROM ELF is picked up from `~/.espressif/tools/esp-rom-elfs/*/esp32c3_rev3_rom.elf`
(shipped with ESP-IDF); override with `--rom`. Flags mirror the S3 binary — see `--help`;
`--trace`, `--break`, `--watch`, `--peek`, `--disasm` and `--log-periph` all work and print
RISC-V mnemonics with symbols.


## Verified against real silicon

An ESP32-C3 module (QFN32, rev v0.4, 4 MB embedded XMC flash, MAC `3c:84:27:b6:a7:1c`) was
flashed with the same three binaries the emulator runs, and 26 s of its console captured
(`hw/c3-hello-world-real.txt`). Comparing that with the emulator, timestamps normalised:

| | |
| --- | --- |
| **205 of 208 lines identical** over three complete boot cycles | |
| the only difference | the ROM's `Saved PC:0x...` line on a non-power-on reset |

To reproduce, tell the emulator the board's identity and boot conditions:

```sh
./target/release/esp32sim-c3 --boot rom --flash-mb 4 \
    --mac 3c:84:27:b6:a7:1c --reset-cause 0x15 --strap 0xd \
    --bootloader $H/bootloader/bootloader.bin --ptable $H/partition_table/partition-table.bin \
    --app $H/hello_world.bin --elf $H/hello_world.elf --max-seconds 26
```

`--mac`, `--reset-cause` and `--strap` exist for exactly this: without them the emulator does a
cold power-on with its own MAC, which is correct behaviour but not the same boot the board had
after esptool reset it over USB.

`Saved PC` is printed by the ROM for a non-power-on reset, from a PC stashed in RTC memory by the
previous reset. The emulator does not stash it on the C3 (the C6 does, from ASSIST_DEBUG), so
the line is absent. The bootloader's timestamps after `esp_restart()` match silicon since the
C6 bring-up found the cycle counter carrying across resets. Everything else — the ROM
banner, the bootloader's partition table and segment map, the IDF startup log, the heap regions,
`Minimum free heap size: 331296 bytes` to the byte, the countdown and the reboot — matches.

### Five bugs the hardware found

Each was invisible without a board to compare against; four came from that one 26-second capture.

- **A SPI flash command must execute when it is issued, not at the end of the scheduling
  quantum.** Firmware kicks a command and reads the result registers a few instructions later,
  well inside one quantum, so the deferred model handed back zeros. On the power-on path the
  timing happened to work; a chip-reset boot failed with `E memspi: no response` in
  `esp_flash` init. (The S3 avoids this by accident: its lazy-tick work made every peripheral
  access flush pending device work.)
- **`SpiMem` misroutes a flash command to the S3's octal PSRAM on CS1.** The C3 has no PSRAM;
  the model now carries `has_psram` and the C3 clears it.
- **The efuse block revision was v1.0; the silicon reads v1.3.** `BLK_VERSION_MINOR` is BLK1
  bit 120, which the model never set. The bootloader prints it on every boot.
- **A chip reset reported POWERON.** `Machine::reboot()` kept the cause in its own field but
  never wrote `RTC_CNTL_RESET_STATE_REG` (0x38), where the ROM reads it. Real silicon says
  `rst:0xc (RTC_SW_CPU_RST)` after `esp_restart()`.
- **Flash capacity and strapping were lost across a reset.** Both are board wiring, not chip
  state, so `reboot()` now preserves the JEDEC capacity and `gpio.strap`; without that the
  emulator re-detected its default 8 MB from the second boot onward.

### Reading a board yourself

```sh
python -m esptool --port /dev/cu.usbmodem1101 chip_id          # identify before touching anything
python -m esptool --port /dev/cu.usbmodem1101 --baud 921600 \
    read_flash 0 0x400000 backup.bin                           # back up first
cd examples/hello_world-c3/build && python -m esptool --port /dev/cu.usbmodem1101 \
    --baud 921600 --chip esp32c3 write_flash '@flash_args'
python -m espefuse --port /dev/cu.usbmodem1101 summary          # the revision fields above
```

## In the browser

The C3 is in the WebAssembly build too — pick board `esp32c3` on the page, or open it directly:

    https://mikroverk.github.io/esp32sim/run.html?fw=c3-hello

Console-only, real time, from the same mask ROM and binaries. See [wasm.md](wasm.md).

## What is modelled

| | |
| --- | --- |
| CPU | RV32IMC, machine mode, `mstatus`/`mtvec`/`mepc`/`mcause`/`mtval`, vectored traps, WFI |
| Interrupts | the C3 interrupt matrix: 62 sources → 31 lines, per-line priority and threshold, level and edge, plus the four `FROM_CPU` software interrupts |
| Memory | 400 KB SRAM (SRAM1 dual-mapped IRAM/DRAM), mask ROM, RTC slow RAM, 8 MB flash cache windows through a 128-entry MMU |
| Peripherals | UART0/1, USB-Serial/JTAG, systimer, TIMG0/1, GPIO, RTC_CNTL with RTC watchdog reset stages, efuse, SPI0/1 flash controller, GDMA, GP-SPI2 master (MOSI only, data from GDMA, as esp-hal's `SpiDma::write_buffer` drives it), SHA/AES/RSA, cache controller, hardware RNG |

Peripheral models are **shared with the S3 through the `esp-periph` crate** wherever the IP is
identical (which is most of it — same UART, same systimer, same timer groups, same USB-Serial/JTAG,
same SPI flash controller). Only the address map, the cache controller and the interrupt matrix
are C3-specific; `esp32c3/src/periph.rs` is its `device_set!` table plus those three.

The RV32IMC decoder is checked against `riscv32-esp-elf-objdump` the same way the Xtensa one is:
161,388 instructions from the C3 mask ROM and a real app, 0 mismatches.

```sh
OD=~/.espressif/tools/riscv32-esp-elf/*/riscv32-esp-elf/bin/riscv32-esp-elf-objdump
$OD -d ~/.espressif/tools/esp-rom-elfs/*/esp32c3_rev3_rom.elf > /tmp/rom.dis
$OD -d examples/hello_world-c3/build/hello_world.elf > /tmp/app.dis
RISCV_DIS_FILES=/tmp/rom.dis:/tmp/app.dis cargo test -p riscv-rv32 --release
```

LEDC models low-speed timers, static duty latching and GPIO-matrix frequency/duty observation through `--pwm PIN`. Hardware fades and sampled PWM edges are not modelled.

## Wi-Fi station

`--wifi ssid=esp32sim` attaches the shared virtual AP and network. Add
`,psk=esp32sim-pass` for WPA2-PSK. Native runs use the existing NAT by default;
`--net none` keeps only the virtual subnet. The browser supports the same AP and
subnet, with optional host Ethernet transport through `SocBus` and the WASM ABI.

The C3 adapter models the MAC at `0x60033000`, 12-bit DMA descriptor lengths,
48-byte RX metadata, analog register handshakes and AES through the C3 GDMA
register layout. It reuses `esp-soc` for association, WPA2, DHCP, DNS, SNTP and NAT.
The radio and calibration results are idealized; this is one station on an open
or WPA2 AP, without Bluetooth, WPA3, roaming or RF simulation.

Arduino-ESP32 3.3.8 connects and receives an HTTP body through NAT with both open
and WPA2 configurations. See the [validation receipt](evidence/ethernet-c3-2026-10-02/README.md)
for firmware hashes, commands and limits. No physical C3 Wi-Fi comparison was run. In CI the
`wifi_station_c3` golden runs the committed build of `examples/c6-wifi-station` for the C3
(`web/wasm/fw/public/c3-wifi-*`): scan, WPA2, DHCP and five pings, with the console, the WiFi
counts and the interrupt count pinned.

## Not there yet

- **Watchdogs.** The [timer-group watchdogs](../esp-periph/src/timg.rs) are register RAM and never fire. The [RTC watchdog](../esp-periph/src/rtc_cntl.rs) supports reset stages, feed and write protection at the C3 register offsets; its interrupt stage sets raw status but does not interrupt the CPU.
- **`--boot app`** (skipping the ROM and bootloader). The C3 has one 128-entry MMU table shared by
  the data and instruction buses, and software keeps their page ranges disjoint; a direct app boot
  needs the bootloader's split, which is not modelled. The flag fails with that message.
- **Bluetooth** is not modelled.
- **No board yet.** The C3 target is a bare module (`NoBoard`): console only. `BoardModel` lives
  in `esp-soc` now, so a C3 board is an `impl BoardModel` plus its devices; nothing else changes.
- (superseded) `BoardModel` is an S3 concept
  today; a C3 board would need it lifted out.
- **Peripherals on demand**: I2C, RMT, ADC, TWAI. Each shows up as an unknown
  register with `--log-periph` the moment a firmware wants it.
- **`Saved PC`** on a non-power-on reset: the ROM reads a PC the previous reset stashed in RTC
  memory, which the emulator does not write.
- **Speed work** — the C3 bus does a plain address-range walk per access; it has none of the S3's
  software TLB, block interpreter or JIT. It still runs ~200–300 Minsn/s (well above the C3's
  160 MHz) because the workload is light, but a busy firmware would want the same treatment.

## Gotchas found bringing it up

These cost real time; they are the reason the boot log looks the way it does.

- **Interrupt source numbers come from the `INTERRUPT_CORE0_*_MAP_REG` order, not from
  `soc/interrupts.h`.** That enum omits the NMI entries, so its indices are shifted by up to 7 and
  every source lands on the wrong line — the systimer silently never fires. The register order in
  `interrupt_core0_reg.h` is the hardware's.
- **A line whose priority equals the threshold fires.** IDF enables interrupts by setting the
  threshold to 1 and allocates handlers at priority 1, so modelling the comparison as `pri > thresh`
  masks everything, forever, with no error.
- **FreeRTOS yields through a software interrupt.** `xPortStartScheduler` raises
  `SYSTEM_CPU_INTR_FROM_CPU_0` (SYSTEM + 0x28) and expects never to return. Without that register
  the yield falls through, `vTaskStartScheduler` returns, and the app lands in a `j .` trap loop
  having printed a *complete and healthy* startup log — a very quiet failure.
- **The ROM's `unpackloop` will zero its own data if you let it.** The reset path copies RAM
  initialisers out of ROM using a table of `(dst_start, dst_end, rom_src)` entries between
  `_data_start` and `_data_end`. The ELF carries the RAM copies but not the ROM-side originals, so
  the copy overwrites them with zeroes; `load_rom` back-fills the ROM side first. Symptom without
  it: a store through a null `rom_spiflash_legacy_data` about 350 instructions in.
- **The efuse wafer version lives in BLK1 bits 114 (minor low), 183 (minor high) and 184 (major)** —
  a different layout from the S3's. Get it wrong and the bootloader stops with
  `chip revision check failed. Required >= v0.3, found v0.0.` We report v0.4, which is what current
  C3 silicon reads back.
- **`ets_delay_us` busy-waits on CSR 0x802**, Espressif's user-mode cycle counter, not on a timer.
  Return 0 from it and the ROM hangs before the bootloader ever runs.
- **EXTMEM register offsets differ from the S3's**: ICACHE sync-done is bit 1 at 0x028 (the S3 has
  DCACHE there), freeze is at 0x0CC, cache state at 0x0B0. Each wrong one is a separate silent hang
  in a `Cache_*` poll loop.
- **The hardware RNG is at APB_CTRL + 0xB0**, and the bootloader's `bootloader_fill_random` spins
  on it before it will load the app.
- **The ROM mirrors its console to both UART0 and USB-Serial/JTAG**, so `--console both` prints
  everything twice; the default is `uart0`.

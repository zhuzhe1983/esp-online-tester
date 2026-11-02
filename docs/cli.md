# Command line

```
esp32sim [--chip esp32|s3|c3|c6|p4] --boot rom --bootloader B.bin --ptable P.bin --app A.bin [--elf X.elf ...] [options]
esp32sim --flash-image flash.bin --boot rom ...
```

One binary for every chip; `esp32sim-c3` is `esp32sim --chip c3`, `esp32sim-c6` is
`esp32sim --chip c6` and `esp32sim-p4` is `esp32sim --chip p4`. The RISC-V chips' defaults differ
where the chip does (`--boot rom`, `--console uart0`, `--flash-mb 4`) and they refuse the flags
their chip has nothing behind: the C3 board, camera, PSRAM and register presets; the C6, which has
a board and WiFi, only camera, PSRAM and register presets. The P4 boots from its ROM on UART0 by
default too, with 16 MB of flash ([esp32p4.md](esp32p4.md)).

## Images and boot
| Flag | Meaning |
| --- | --- |
| `--boot rom\|app` | `rom`: start at the mask ROM reset vector (real boot chain). `app`: load the app image segments and jump to its entry |
| `--bootloader F`, `--ptable F`, `--app F` | written to flash at 0x0 / 0x8000 / 0x10000 |
| `--flash-image F` | whole flash dump written at 0 |
| `--chip esp32\|s3\|c3\|c6\|p4` | which chip (default s3; `esp32` is the classic ESP32) |
| `--rom F` | mask ROM ELF (default: the chip's in `ESP32SIM_ROM_DIR`, then `~/.espressif/tools/esp-rom-elfs/*/`) |
| `--mac xx:xx:xx:xx:xx:xx` | the station MAC the efuses report |
| `--cam-stream PATH\|-`, `--cam-size WIDTHxHEIGHT` | live RGB24 camera source and its dimensions; requires `waveshare-cam` |
| `--serial TEXT` | bytes into the USB-Serial/JTAG console before the run |
| `--elf F` (repeatable) | symbols for logs/profiles (app ELF, bootloader ELF) |
| `--flash-mb N`, `--psram-mb N` | flash size (JEDEC follows it) and PSRAM size. S3 defaults: 8 / 2, or 16 / 8 for StopWatch; octal MR2 density follows 4, 8, 16, 32 and 64 MiB. Classic ESP32 defaults: 4 / 0; `--psram-mb 4` enables quad PSRAM, `0` disables it; other classic sizes are rejected |
| `--flash-octal` | octal SPI flash (ESP32-S3-WROOM-2): Macronix octal commands, JEDEC C2 80 xx, efuse FLASH_TYPE octal; S3 only. An `--efuse-regs` dump is loaded after it: a dump that includes RD_REPEAT_DATA3 (efuse +0x3c) replaces that word, FLASH_TYPE (bit 9) with it |
| `--sd IMAGE`, `--sd-rw IMAGE` | (P4) an SD card in the reference board's socket (CLK 43, CMD 44, D0-D3 39-42), holding the image: a high capacity card of the image's size, rounded down to 512 KiB. `--sd` is copy-on-write: what the firmware writes stays in memory and the file is never modified. `--sd-rw` writes through to the file. See [esp32p4.md](esp32p4.md), "SD card" |
| `--sd-prg-us N`, `--sd-serial HEX` | (P4) the card's programming time after a write or an erase, in microseconds of emulated time (default 0: instant), and its CID serial number (default: derived from the image size) |
| `--sd-timing` | (P4) commands and data take their time on the SD bus at the slot's card clock and width: CMD_DONE, DATA_OVER and the auto-stop come when the bits are through (default: at once); with `--coproc` the SDIO link's too. See [esp32p4.md](esp32p4.md), "The host" |
| `--usb-host F` | (P4) a USB host on the high-speed USB-OTG port, running the commands in F (below) from power-up; the run then goes on to `--max-seconds`. See [esp32p4.md](esp32p4.md), "USB" |
| `--coproc c6:KEY=VALUE,...` | (P4) an ESP32-C6 co-processor on SDIO slot 1, run in lock-step with the P4 (below); `--wifi` and `--net` then go to the C6's radio |
| `--board atech14\|waveshare-cam\|waveshare-lcd4b\|waveshare-amoled18-v2\|stopwatch\|none` | board model (default atech14); on the C6: `waveshare-c6-lcd147` or `none`; on the P4: `waveshare-p4-lcd35` (rev ≥ 3) or `none`; on the classic ESP32: `ttgo-tdisplay` (alias `tdisplay`), `ulanzi-tc001` (alias `tc001`), `esp32dev-loopback` or `none`/`esp32dev` |
| `--strap HEX`, `--reset-cause HEX`, `--efuse-regs F`, `--regs-init F` | reproduce a real chip's boot state (used by the differential tests) |
| `--no-reboot` | stop at the first chip reset instead of rebooting from ROM |
| `--flash-at OFFSET=FILE` (repeatable) | write a file into flash at a hex offset — a data partition's contents (the panel's `demo` partition takes `energydata.json`) |
| `--stub SYMBOL[=value]` (repeatable) | return `value` (default 0) immediately when execution reaches the function's entry; numeric function addresses require a `0x` prefix; accepts decimal, `0x` hex, `true` (1) or `false` (0); rejects invalid values |
| `--ble full` | experimental C3 register-level BLE controller; advertising, scanning and one virtual central; excludes `--ble` |
| `--ble-scan` | enable a virtual active scanner in C3 full mode; sends SCAN_REQ and observes guest-configured SCAN_RSP |
| `--ble-observe` | passive full-mode PDU log: half-microsecond timestamp, channel, type, AdvA, decoded AD and raw PDU |
| `--ble` | opt-in virtual BLE controller on S3/C3/C6; requires the matching application `--elf` |
| `--ble-nearby SPEC` (repeatable) | a Bluetooth LE device around the station, heard but not connectable: the guest's scans report it (ADV_SCAN_IND, plus SCAN_RSP for an active scan) next to the virtual peer — `name=Thermo,rssi=-60` (`name` 1 to 26 bytes, a Complete Local Name after the Flags; `adv` / `rsp` the advertising data / scan response as hex, at most 31 bytes each, `adv` instead of `name`; `addr` aa:bb:cc:dd:ee:ff, default a random static address derived from the data, a random one not with the reserved top bits 10; `type` random (default) or public; `rssi` -127 to 20 dBm or 127 for not available, default -60; a `name` cannot hold `,`, use `adv` for one that does); reported as scannable and not connectable (ADV_SCAN_IND); up to 32, each on an address of its own, needs `--ble` |
| `--wifi SPEC` | attach a virtual access point the WiFi blob hears, plus a virtual network (DHCP/ARP/ICMP/DNS/SNTP; station 10.0.2.15, gateway 10.0.2.2) — for example `ssid=demo,chan=6,psk=demo-password,bssid=02:00:00:00:00:01`. `password` and `pass` alias `psk`; unknown keys and invalid values are rejected. Open and WPA2-PSK networks both join end to end, on S3, C3, C6 and the classic ESP32 (docs/wifi-plan.md, docs/esp32c3.md, docs/wifi-c6-plan.md). `--hostfwd` publishes a guest TCP port on the host |
| `--wifi-nearby SPEC` (repeatable) | a network around the station, heard but not joinable: a scan lists it with its channel, signal and security, joining it fails as out of reach — `ssid=Home,chan=1,rssi=-60,secure=1` (`chan` 1 through 14, default 1; `rssi` in dBm, default -60; `secure` 0 or 1, default 1; `bssid` optional, derived from the SSID otherwise); up to 32, each on a BSSID of its own, needs `--wifi`; one with the access point's SSID or BSSID is left out with a log line |
| `--net nat\|none[,epoch=S]` | what the virtual network does with traffic it is not itself answering: `nat` (default; `user` is an alias) forwards TCP and UDP to the host's own network through ordinary sockets, `none` refuses it. `epoch=S` sets the network's clock at power-on (seconds since 1970, at most 4294967295): its SNTP answers are that plus emulated time, so a run that asks for the time is reproducible; without it the host's clock at the first answer fixes it. `epoch=` in the `--wifi` spec does the same |
| `--hostfwd tcp:HOST:HOSTPORT-GUEST:GUESTPORT` | (repeatable) listen on the host and deliver accepted TCP connections as SYNs to the station. Empty HOST binds 127.0.0.1; empty GUEST is 10.0.2.15. Example: `tcp:127.0.0.1:8080-:80`. Requires `--wifi` and `--net nat`. UDP is not forwarded |
| `--trace-fn PREFIX` (repeatable) | log every call to functions whose name starts with PREFIX, with args and caller; append `$` for an exact name |
| `--regstat FILE` | write per-register access statistics (count, pc, symbol) at exit — for reverse-engineering |

## Running
| Flag | Meaning |
| --- | --- |
| `--version`, `-V` | print `esp32sim <version>` and exit |
| `--help`, `-h` | print the usage line and exit |
| `--max-seconds S` | stop after this much emulated time, including all reboots; also applies to `--cooja` |
| `--max-insns N` | cap scheduler work across all reboots (details below); unavailable with `--cooja`, which uses `--max-seconds` |
| `--i2s-tone PORT:HZ:AMPLITUDE` | continuous RX sine on I2S controller 0 or 1; amplitude 0..1, for example `0:1000:0.5`; repeat for multiple controllers |
| `--script F` | host actions at emulated times (below) |
| `--console usb\|uart0\|both\|all\|none`, `--console-prefix` | which consoles to print |
| `--realtime` | pace to wall time without the UI |
| `--web PORT [--web-dir DIR]` | browser UI (implies real time) |
| `--cam-image F`, `--cam-fps N` | camera source for boards with a camera |
| `--cooja` (C6) | run as a Cooja-NG external mote: the lock-step NDJSON protocol on stdin/stdout, the guest console as `log` events, the 802.15.4 frames as `tx`/`rx`. Honors `--no-reboot` and `--max-seconds` (see [esp32c6.md](esp32c6.md), "Cooja-NG lock-step") |
| `--cooja-slice-us N` | how long a busy guest runs before asking csim to step it again (default 100; `hello.args.slice_us` overrides). A transmission reaches csim's medium at the end of the slice it started in, so this bounds how late it is |
| `--cooja-rx-timing start\|end` | what an `rx` at `t` is: the frame's start (default — csim hands a frame-consuming mote the frame when it starts: `t` is the first preamble byte, the SFD five byte times later, RX_DONE after the whole PPDU, the ACK 192 µs after that) or its end, complete at `t` |
| `--cooja-verbose` | narrate the exchange on stderr |

The legacy name `--max-insns` counts scheduler work, not total retired instructions. The
default scheduler charges one unit per core 0 instruction or idle cycle advanced; another
core does not add to that count. It checks the cap between scheduling rounds, so it may finish
the current round past the requested number. With an approximate cost model, it counts
scheduled events. Use `--max-seconds` for a predictable duration. The final execution report
separately lists the actual instruction count for each core.

## Outputs
| Flag | Meaning |
| --- | --- |
| `--wav F` | audio captured from I2S (whichever controller played) |
| `--tft-png F`, `--gram-png F` | display frame (visible, scaled) / raw GRAM |
| `--no-dump` | skip the register dump at exit |
| `--adc PIN` | (repeatable) last completed ADC conversion on that GPIO: 12-bit raw count and generation |
| `--adc-raw PIN=COUNT` | (repeatable) a post-attenuation 12-bit count (0..4095) on that pad, before the first instruction |

### PWM observation

`--pwm PIN` reports the GPIO-matrix PWM frequency in Hz and duty percentage when the run stops.
Repeat it to observe several pins. PIN is an unsigned 8-bit decimal number.
LEDC is supported on S3, C3, C6 and the classic ESP32; MCPWM on S3 and C6.
A running timer with 0% or 100% duty still has a snapshot. "No supported running PWM" means
unrouted, disabled, paused or unsupported configuration; it does not imply a low GPIO level.
A paused hardware timer can hold its last level. This reports register-derived steady state,
not sampled edges. LEDC fades and MCPWM dead time, carrier and extra generator actions are unsupported.

### ADC observation

`--adc PIN` reports the last completed conversion on that pad when the run stops: the 12-bit
raw count and the generation (zero until the firmware samples, then one step per completed
conversion). Repeat it to observe several pads. PIN is an unsigned 8-bit decimal number.
`--adc-raw PIN=COUNT` puts a post-attenuation count on the pad before the first instruction;
unsupported pins and counts above 4095 are refused. Voltage scripts (`adc`, `adcwave`) still
set the transfer-curve input; a script on the same pin replaces the raw count at its timestamp.
Pads, units and the voltage curve are in [peripherals.md](peripherals.md#host-adc-inputs).
"no ADC pad" means that GPIO is not an ADC input on this chip.

## Debugging
| Flag | Meaning |
| --- | --- |
| `--trace`, `--trace-from N` | per-instruction trace (from instruction N) |
| `--break PC` (repeatable) | stop at PC |
| `--watch ADDR` | stop when a word changes |
| `--peek ADDR,N`, `--disasm ADDR,N` | dump memory / disassemble at exit |
| `--profile` | top PCs by instruction count (single-steps, and keeps idle cores stepping: an idle core shows as a hot `waiti`) |
| `--profile-blocks` | time per function from the block path — full speed, no timing change; needs `--elf` for names |
| `--coverage`, `--coverage-file F` | block starts reached, per function; with a file, one `addr symbol` line each |
| `--irq-latency` | cycles from an interrupt line appearing at a core to the core taking it, per line; retains block execution |
| `--vcd F` | GPIO edges and interrupt lines as a VCD waveform (1 ps units); retains block execution |
| `--debug AREAS` | what the model prints: device names or prefixes (`spi`, `usb`, `i2c`, `wifi`, `gdma`, `sha`, `rsa`, `lcd_cam`, `sdmmc`, `usb_otg` for the P4's USB-OTG core and the host's link; `usb` also takes USB-Serial/JTAG), `net`, `wifi-frames`, `aes`, `rom`, `mmio`, `rt`; also `ESP_EMU_DEBUG=a,b` |
| `--log-periph` | log the first access to every unknown peripheral register |
| `--no-jit` | run blocks through the interpreter instead of native code (aarch64 hosts compile blocks to machine code by default); the two must produce identical results, so this is the oracle when something looks wrong |
| `--stop-after-exceptions N` | stop after N exceptions |
| `--regtrace F`, `--regtrace-from-pc PC`, `--regtrace-max N` | register trace file for `hw/compare.py` |

Environment: `ESP_EMU_DEBUG=wifi,spi,net` is `--debug` for every run (the older
`ESP_EMU_DEBUG_SPI`, `ESP_EMU_DEBUG_NET`, `ESP_EMU_LOG_ALL`, `ESP_EMU_RT_LOG`... still work as aliases).
`XTENSA_DIS_FILES=a.dis:b.dis` feeds the decoder equivalence test.

## Action scripts

One action per line, `<seconds> <cmd> [args]`; buttons/encoder are active low.

```
1.5  press btn1 150        # press for 150 ms (btn1, btn2, knob/sw, or a GPIO number)
2.0  release 16
2.5  gpio 17 0
3.0  knob cw 3             # 3 detents clockwise (ccw for the other way)
4.0  serial {"action":"set_note","value":"5"}
4.2  uart0 root              # a line into UART0's receive FIFO (also `uart1`): a login on a Linux console
4.5  touch 450 30 1        # touch panel press at (450,30); `touch x y 0` releases
5.5  stop
```

`hw/wsdrive.py [port] [seconds]` drives the same inputs over the UI's WebSocket and reports
real-time keep-up (push gaps, lag, audio delivered); `hw/wsaudio.py [port] [seconds]` listens to the
UI's audio stream and reports sample counts/peak (how to check sound without listening).

### ADC inputs

Scripts accept `adc <gpio> <volts>` for a constant voltage and
`adcwave <gpio> <file> <rate_hz>` for voltage samples separated by whitespace or
commas. The waveform starts at the action's emulated time. A later input replaces
the previous source. S3 GPIO1–10 select ADC1 and GPIO11–20 select ADC2; C3 GPIO0–4
select ADC1 and GPIO5 selects ADC2; C6 GPIO0–6 select ADC1.

```
0 adc 1 0.5
0 waituart0 10 ADC READY
0 uart0 A
1 adc 1 1.0
1 uart0 C
2 adcwave 1 /tmp/adc-volts.txt 1000
```

`waituart0 <timeout_s> <text>` holds later actions until UART0 prints the text or
the timeout expires. Raw counts and completed-conversion observations are exposed
through the [host ADC API](peripherals.md#host-adc-inputs). `--adc PIN` prints the last completed
sample at exit; `--adc-raw PIN=COUNT` injects a 12-bit count before the run.

## USB host (P4)

`--usb-host FILE` plugs a host into the P4's high-speed USB-OTG port and runs FILE's commands in
order, one a line (`#` starts a comment), each taking the emulated time it takes on the bus while
the firmware runs, within the run's `--max-insns` and `--max-seconds` and its reboot policy
(`--no-reboot`, an app-mode Restart). The first command that fails ends the file, unless its line
starts with `?`: then the failure is logged and the file goes on. A failure that leaves no device
on the port drops the interface and the volume, as Linux's disconnect does. The run then goes on
to its time limit.
What each command did goes to stderr as `[usb-host] t=<seconds> ...`, and the end-of-run report
adds a `[usb]` line with the core's and the host's counts. A time is at most a day (86400000 ms),
a block count at most 65536 and a file at most 1 GiB; a larger value is a usage error.

| Command | What the host does |
| --- | --- |
| `attach [timeout_ms]` | VBUS on, wait for the device to connect (10 s), debounce 100 ms, port reset, GET_DESCRIPTOR, SET_ADDRESS, the configuration and strings, SET_CONFIGURATION |
| `detach`, `reset` | VBUS off; reset and enumerate again |
| `port-reset` | Linux's `usb_reset_device`: reset, GET_DESCRIPTOR(64), reset, SET_ADDRESS to the same address, the descriptors read and compared, SET_CONFIGURATION again |
| `suspend`, `resume`, `idle MS` | stop SOFs; drive resume for 20 ms; let time pass |
| `rate MBPS` | cap the host's bulk payload per microframe (a slower host) |
| `msc` | open the bulk-only mass-storage interface: GET MAX LUN, INQUIRY |
| `recovery linux`, `recovery bot` | how a transport error or a timed-out command is recovered: Linux v6.12 usb-storage's port reset, its class reset only if that fails (the default), or BOT 1.0's reset recovery |
| `timeout MS` | how long a command may take before the host aborts it (30000, sd's SD_TIMEOUT) |
| `inject vbus-off MS` | VBUS goes MS from now, whatever the host is doing then; a transfer in flight ends disconnected |
| `ready INTERVAL_MS TIMEOUT_MS` | TEST UNIT READY until it passes, REQUEST SENSE after each failure |
| `poll INTERVAL_MS MS` | TEST UNIT READY every interval for a while, as a host polls a removable drive; a NOT READY seen here or by `ready` drops the mounted volume |
| `capacity`, `mode-sense`, `prevent`, `allow`, `sync`, `eject` | READ CAPACITY(10), MODE SENSE(6), PREVENT ALLOW MEDIUM REMOVAL, SYNCHRONIZE CACHE(10) (a refusal is logged, not an error), START STOP UNIT with LoEj |
| `read LBA COUNT`, `write LBA COUNT BYTE`, `verify LBA COUNT BYTE` | READ(10) (its hash), WRITE(10) of one byte value (hex), READ(10) and compare |
| `mount`, `ls` | mount the FAT32 volume on the LUN (an MBR's partition or from LBA 0); list its root directory |
| `fat-write NAME BYTES CHUNK_KIB [GAP_MS] [FAT_EVERY]` | write an 8.3 file of test bytes into the root directory in a vfat-like order: the directory entry, the data in chunks, the FAT sectors (both copies) after every FAT_EVERY chunks (1), then the entry's size and cluster and FSInfo. Chunk i is due GAP_MS·i after the start, as a fixed-rate source produces them; the log gives the rate overall and over the second half, the worst lateness against that schedule and whether the host kept up (never a whole chunk behind) |
| `fat-read NAME` | read a file back; one this file wrote is compared with what it wrote |
| `uart0 TEXT` | a line into the firmware's UART0 (its console) |

`examples/usb_msc-p4/host.txt` writes a file, ejects the drive, polls it while the
example's application owns the card (MEDIUM NOT PRESENT) and takes it again.
`examples/usb_msc-p4/faults.txt` meets faults: a command that times out mid-transfer,
a port reset after an eject, and VBUS lost while a file is being written.

## Co-processor (P4)

`--coproc c6:KEY=VALUE,...` puts an ESP32-C6 on the P4's SDIO slot 1, wired as ESP-Hosted wires it
on the reference board (CLK 18, CMD 19, D0-D3 14-17, the C6's EN on GPIO 54), and runs the two
chips in lock-step on one clock ([esp32p4.md](esp32p4.md), "The co-processor"). The C6 boots
from its mask ROM at power-up; `--wifi` attaches the virtual access point to its radio and `--net`
the network behind it. Native only: the browser build has no co-processor.

```sh
esp32sim-p4 --flash-mb 2 --bootloader p4-bootloader.bin --ptable p4-ptable.bin --app wifi_station.bin \
    --coproc c6:bootloader=c6-bootloader.bin,ptable=c6-ptable.bin,app=network_adapter.bin,otadata=ota_data_initial.bin,stub=bb_init=0,elf=network_adapter.elf \
    --wifi ssid=esp32sim,psk=esp32sim-pass --max-seconds 6
```

| Key | Meaning |
| --- | --- |
| `bootloader=F`, `ptable=F`, `app=F` | the C6's images, at 0x0, 0x8000 and 0x10000 (`app` is required) |
| `otadata=F[@OFFSET]` | the OTA data partition's image, at 0xD000 unless an offset follows (hex) |
| `elf=F` (repeatable), `stub=SYMBOL[=value]` (repeatable) | the C6's symbols and function stubs, as `--elf` and `--stub` for a C6 run (a C6 radio run needs `stub=bb_init=0`) |
| `rom=F` | the C6's mask ROM ELF (default: `esp32c6_rev0_rom.elf` found as for `--rom`) |
| `flash-mb=N`, `mac=xx:..` | the C6's flash size (4) and efuse MAC (40:4c:ca:00:11:22) |
| `en=N`, `en=!N` | the P4 GPIO the C6's EN follows (54): driven low holds the C6 in reset, driven high or not driven (the pull-up) lets it run; `!` inverts the line |
| `slice-us=N` | the lock-step slice (10): how late, at most, the slave's SDIO interrupt reaches the P4 |
| `console=F` | write the C6's console to F instead of stdout |
| `fault=KIND@WHEN` (repeatable) | inject a fault on the SDIO link at a point the run fixes: `read-crc@N[:K]`, `read-timeout@N` (the Nth FIFO read fails after the slave gave its data, or at the block holding byte K), `reg-read-crc@N` (the Nth register-window read), `write-crc@N`, `write-nostatus@N` (the Nth FIFO write), `cmd-timeout@N`, `resp-crc@N` (the Nth command), `wedge@T` (the C6's CPU stops, its SDIO hardware answers on), `dead-bus@T`, `drop-irq@T` (one DAT1 edge unlatched), `spurious-irq@T+D`, `token-stall@T+D` (TOKEN1 shows no free buffer); T and D as `3.5s`, `200ms`, `1500us`. See [esp32p4.md](esp32p4.md), "Faults" |

Both consoles go to stdout a line at a time, in the order the slices produced them: the P4's as
they are (`[p4] ` in front with `--console-prefix`), the C6's UART0 with `[c6] ` in front. The
end-of-run report adds the C6's lines (`[c6] stop:`, its SDIO counts, its WiFi and network counts)
and `[cosim]` (slices, EN changes, P4 resets; when there are any, commands that found the C6
ahead of them, EN pulses too short to reset it, and each fault with the time it fired). `--debug sdio` prints the C6 slave's commands and DMA,
`--debug sdmmc` the P4 host's commands. `--max-seconds` is rounded up to a whole slice;
`--max-insns` does not apply.

## Virtual BLE

`--ble --elf firmware.elf` substitutes the controller lifecycle and legacy VHCI
functions on S3 and C3, or the native NimBLE transport on C6. The guest host and sketch remain unchanged.
An emulator-created FreeRTOS task delivers controller packets through guest
callbacks. S3 uses the windowed Xtensa ABI; C3 uses RV32IMC. Unstripped controller,
VHCI, task and BSS symbols are required on S3/C3. C6 requires native transport,
NPL and mbuf allocator symbols; its board image needs `--flash-mb 8`.
C6 is **ESP-IDF 5.5.x only**. Enable reads `esp_app_desc.idf_ver` from the application
ELF and refuses missing, malformed or other versions before installing hooks.
Its native transport uses that version's `os_mbuf` layout and NPL initialization.
S3/C3 accept builds exposing the required VHCI/lifecycle symbols, controller BSS
and function sizes; there is no IDF version gate. Arduino 3.3.8 with IDF 5.5.4 is
validated. IDF 4.4 Bluedroid is unsupported and fails with a missing-symbol error.
Other IDF releases have not been validated.

S3 VHCI transmit packets may also reside in mapped PSRAM, as used by
StopWatch's ESP-IDF 6.1 build with external NimBLE allocation. Every page must
map to installed PSRAM; flash, invalid mappings and out-of-range packets are
rejected. The controller task trampoline and its receive buffer still require
internal controller BSS.

`[ble]` output reports advertising data and discovered GATT handles. Script commands
use seconds like the other script actions. For example:

```text
0.5 ble connect
0.6 ble discover
1.0 ble read 0x0010
1.1 ble write 0x0010 68656c6c6f
```

Use handles reported by discovery. `ble subscribe CCC_HANDLE` writes notification
enable to a discovered client configuration descriptor. Commands are validated when
CLI arguments are parsed, before loading the ROM, and require `--ble`. `connect` waits for guest advertising;
ATT commands wait for the connection and run in order, with one request outstanding.
At exit, the CLI reports the number of queued or in-flight commands still pending.
Writes are limited to 20 bytes. A scanning guest sees a virtual
peripheral named `esp32sim` advertising the Battery Service.

This models one unencrypted LE link, legacy advertisements and ATT MTU 23.
It does not model RF, pairing, physical connection timing or the controller's
registers. Unsupported HCI commands return Unknown Command. The controller reports LL
Privacy and keeps a 10-entry resolving list with an ESP32-S3 controller's statuses, so a host
stack can set up controller privacy; it resolves no addresses and makes no resolvable private
address. The callback task
and its storage stay allocated until reboot. C6 initializes the guest NPL support
and a heap-backed mbuf pool; it uses no controller BSS reservation. Cost-model execution
does not support function substitutions; reported cycle time is not radio timing.

`--ble full` selects the experimental C3 link-controller register model and excludes
`--ble` HCI substitution. It runs the original guest controller without ELF hooks.
It implements initialization, clock capture, timer IRQs and legacy advertising on
channels 37–39. Completion IRQs let the guest schedule its next event. `--ble-observe`
logs to stderr emitted ADV_IND, ADV_NONCONN_IND or ADV_SCAN_IND packets as `[ble-air]` lines;
`hus` is modeled time in half-microseconds. `[ble-config]` reports configured
SCAN_RSP data separately; a passive observer cannot elicit that response. The queue
holds the latest 1024 observations and reports dropped entries when polled.
`--ble-scan` enables active SCAN_REQ/SCAN_RSP exchanges through guest RX descriptors.
Use script commands `ble connect`, `ble read-uuid SERVICE CHARACTERISTIC`,
`ble central-stop`, and `ble disconnect`. The last sends LL_TERMINATE_IND;
`central-stop` silences the central so the guest supervision timer can expire.
A `read-uuid` sent between `central-stop` and that timeout is queued on the
silent link and lost; send it after the next `connect`.
For example, pass this file with `--ble full --script connection.txt`:

```text
0.5 ble connect
0.7 ble read-uuid 4fafc201-1fb5-459e-8fcc-c5c9c331914b beb5483e-36e1-4688-b7f5-ea07361b26a8
3.1 ble central-stop
```

The model supports one unencrypted 1M CSA#1 link, a fixed 30 ms interval and
2 s supervision timeout, default ATT MTU 23 and no L2CAP fragmentation. It replies
to LL_VERSION_IND even without an ATT request. Pairing, CSA#2, PHY updates and
optional LL procedures are unsupported. Register meanings and packet timing
remain inferred/model choices. [EX213 evidence](evidence/ble-c3-connection/README.md)
separates the historical probe-b field checks from unvalidated RF timing.

## Live camera input

For classic ESP32 use `--chip esp32 --board ai-thinker-cam --cam-image image.bmp`
(BMP or PPM). This is a still source for OV2640 raw RGB565/YUV422 capture,
with fixed approximate 10 FPS. A baseline JPEG file (`--cam-image photo.jpg`) is
delivered verbatim to `PIXFORMAT_JPEG` firmware when its size equals the configured
frame size; a malformed or non-baseline one is a usage error, and one of another
size yields no frame (one `[camera] JPEG output` notice). Adjustable classic
frame cadence is unsupported. The public-driver fixture and
complete command are in [esp32.md](esp32.md#the-camera---board-ai-thinker-cam).
The live stream, crop and adjustable FPS description below applies to the S3
`waveshare-cam`, not the classic model.

On `--board waveshare-cam`, `--cam-stream PATH --cam-size WIDTHxHEIGHT` reads
consecutive RGB24 frames with no headers or row padding. Use `-` for stdin or a FIFO
path for a producer in another process. Streaming enables real-time pacing, keeps only
the latest complete input, and retains the last image at EOF. A truncated frame reports
an error and leaves the previous image intact. This is a live source only: regular
files are read as fast as possible, so the frames observed by firmware are not
deterministic. Use `--cam-image` for deterministic still input. FIFO opening runs
on the reader thread and never blocks emulator startup or shutdown. Input dimensions
share the browser limit: an equivalent RGBA message must fit 8 MiB. This limit also applies to `--cam-image`. `--console` still selects firmware console output.

For a macOS webcam, choose a video device with
`ffmpeg -f avfoundation -list_devices true -i ''`, then replace `0` below with its index:

```sh
ffmpeg -f avfoundation -framerate 10 -video_size 640x480 -i '0:none' \
  -an -pix_fmt rgb24 -f rawvideo - |
  target/release/esp32sim --board waveshare-cam --boot rom \
    --rom "$ROM" --bootloader "$BOOTLOADER" --ptable "$PTABLE" --app "$APP" \
    --cam-stream - --cam-size 640x480 --no-dump
```

The firmware programs the OV5640 through SCCB. The model crops its sensor-array
window, scales to its output dimensions with nearest-neighbour sampling, and emits
RGB565 MSB first, YUYV422, or grayscale Y8. `--cam-image` still loads BMP or PPM;
a live frame replaces that image through the same board hook. Without a host image,
the configured sensor supplies black frames so firmware can initialize. `--cam-fps` controls
emulated capture cadence independently of host updates. JPEG encoding and RAW Bayer
are unsupported and produce no frame. Optical black pixels, ISP offsets, binning,
mirror/flip, exposure and sensor clock calibration are not modeled.

Camera crop/scale does not model ISP offsets (0x3810–0x3813), mirror/flip
(0x3820/0x3821), or binning. Typical driver settings can differ from the sensor
by about 2.5% horizontally and 1.6% vertically; mirror/flip is not applied.

On classic ESP32, `touchpad <gpio> <0|1>` drives a capacitive pad separately
from the panel's `touch <x> <y> <0|1>` action. For example, `0 touchpad 4 1`
applies a touch at time zero. `adc` and `adcwave` also support classic ADC pads.

## I2S microphone input

Standard I2S RX supports 16/24/32-bit mono or stereo on S3 controllers 0/1 and
C3/C6 controller 0. S3 controller 0 also accepts PDM-to-PCM RX at 16 bits.
`--i2s-tone 0:1000:0.5` supplies a continuous 1 kHz sine at half full scale;
omit it for silence. The source survives software resets.

Rust hosts use `SocBus::i2s_input(port)` to access `PcmInput`. `push(&[[left, right]])`
queues signed 16-bit stereo frames at the firmware's programmed sample rate.
For mono input, duplicate each sample into both lanes. `push` returns the number
accepted, up to a queue limit of 65,536 frames; retry the remaining frames later.
`clear()` removes queued data and disables the tone. Empty queues read as zero.
Queued input pauses while RX or its DMA channel is stopped, including during reboot.
Each 16-bit input sample is the top 16 bits of the guest's sample, low bits zero.
32-bit RX writes 4 bytes per sample. 24-bit RX writes 3 bytes per sample, the size
ESP-IDF allocates, unless the firmware sets RX_24_FILL_EN (RX_CONF bit 16), which
pads each sample to 4 bytes. An unprogrammed RX_EOF_NUM uses the hardware reset value 0x40.

Input attaches to a controller at the PCM/DMA boundary. It does not simulate GPIO
wiring, serial bit timing, or PDM filter response. Raw PDM, external slave clocks,
more than two slots, companding, and reversed bit/byte order are unsupported.
C3/C6 and S3 controller 1 have no hardware PDM-to-PCM converter, so their raw PDM
mode does not consume this PCM source. Existing I2S TX capture is unchanged.

### Independent source clocks

Rust hosts attach the shared source bank with `SocBus::attach_pcm_sources()` and
append `PcmSource` values to its `inputs` slot vector. Each source names `PcmPins::I2s`
with BCLK/WS/data pins, or `PcmPins::Pdm` with clock/data pins, and a rate from
8 to 96 kHz. Pushes retain the newest two seconds. Call `attach_pcm_sources()`
before each push or slot edit; it synchronizes the bank's clock (on S3 it also
flushes deferred ticks). `SocBus::pcm_sources()` is read-only: it returns the
attached bank as of its last synchronization, or None, and changes nothing.
Queues advance during RX/DMA stalls and survive resets. Matrix routes select the
lowest matching slot; a matched source that underruns produces silence. A receiver
whose route matches no slot keeps its per-controller input, so `--i2s-tone` and
`i2s_input` pushes keep working with a bank attached. `tone()` is rejected on a
routed source; push generated frames instead.

Routing matches the RX BCK/WS output signals only. In ESP-IDF full-duplex standard
mode the later-initialised channel becomes the slave and the BCK/WS pins usually carry
the TX output signals (IDF v5.5.5 `esp_driver_i2s/i2s_std.c`, lines 214–237 and
292–297). Such a duplex codec setup matches no routed source and reads its
per-controller input instead. This
models matrix selection and PCM samples, not IO_MUX, pad enable, serial edges or
PDM filtering.

For ADC audio, attach `AnalogSource::Stream(stream.clone())` through `analog_set`.
Create `AnalogStream::new(rate, bias_volts, cycles)` and push finite `f32` volts
with `push(samples, cycles, CPU_HZ)`. For exact 12-bit counts, create
`AnalogStream::new_raw(rate, bias_count, cycles)`, attach it as
`AnalogSource::RawStream`, and call `push_raw(samples, cycles, CPU_HZ)`.
Stream rates are 1 Hz to 96 kHz, so slow sensor streams work too.
Invalid samples leave queue and time intact. Push between emulator runs at the
bus's current cycle count. Conversion uses `AnalogInputs.cpu_hz`. Buffers retain
two seconds, drop oldest samples on overflow, and hold the last value on
underrun. Both forms preserve main's completed-conversion observation counters.

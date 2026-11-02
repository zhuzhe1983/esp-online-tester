# EX259: CCOUNT counts again from a CPU reset (#220)

Base: upstream/main `2e36224d`; the instruction trace and the classic runs below compare main
`dd4796a0` with this branch merged onto it. The final source is the commit containing this
receipt.
Inputs, hashes and the exact commands are in [inputs.json](inputs.json); the checks that ran
and their results in [checks.json](checks.json).

`Cpu::reset` (`xtensa-lx7/src/state.rs`), which `Machine::reboot` runs for each core, left
CCOUNT where the previous boot had taken it. ESP-IDF's early log timestamp is CCOUNT divided by
the CPU's ticks per millisecond (IDF v5.5.4 `components/log/src/os/log_timestamp.c:28-37`,
`esp_log_early_timestamp`), so after `esp_restart()` the S3's second boot printed
`I (24844) boot: ...` where the first printed `I (7)`, and the classic ESP32 of #220 the same.

`Cpu::reset` now sets CCOUNT to 0 (and recomputes the browser build's cached next-comparator
event). The comparators keep their values and count from the new CCOUNT. Nothing else changes.

0 is a modelling choice, not an architectural reset value: the Xtensa ISA Reference Manual
(2010) says "CCOUNT and CCOMPARE[0..NCCOMPARE-1] are undefined after processor reset"
(4.4.6.2, p. 111; Table 5-175, p. 232). What backs it is silicon (below) and the absence of any
other writer before the first early log: neither mask ROM writes CCOUNT (no `wsr.ccount` or
`xsr.ccount` in the disassembly of `esp32s3_rev0_rom.elf` or `esp32_rev300_rom.elf`), and
ESP-IDF v5.5.4 writes it only to rescale it on a CPU clock switch
(`components/esp_system/port/soc/esp32s3/clk.c:139`, `esp32/clk.c:197`), which only makes sense
for a count that runs from reset.

## Silicon

The golden's own images (`web/wasm/fw/public/hello-bootloader.bin`, `hello-ptable.bin`,
`hello_world.bin`: ESP-IDF v5.5.4 `examples/get-started/hello_world`, which counts down and
calls `esp_restart()`), flashed to an ESP32-S3 board (chip revision v0.2) and read over its
USB-Serial/JTAG port from a reset by the host through two restarts:
[s3-hello-reboot.txt](s3-hello-reboot.txt).

| Boot | Reset | `boot: ESP-IDF v5.5.4 2nd stage bootloader` | `cpu_start: Multicore app` | `main_task: Calling app_main()` |
| --- | --- | ---: | ---: | ---: |
| 1 | `0x15 (USB_UART_CHIP_RESET)` | 24 | 145 | 261 |
| 2 | `0xc (RTC_SW_CPU_RST)`, from `esp_restart()` | 24 | 145 | 261 |
| 3 | `0xc (RTC_SW_CPU_RST)` | 24 | 145 | 261 |

Every early-log timestamp of the three boots is within 1 ms of the same line in the others: the
count starts again at each reset, a software CPU reset like a chip reset. The application's
`ELF file SHA256: 92ff5a529...` is the golden's.

## Emulator

`hello_world_s3_reboot` (`cli/tests/goldens.rs`), the same images for 12 emulated seconds:

| | Second boot, first bootloader line | Second boot against the first |
| --- | --- | --- |
| main | `I (24844) boot: ESP-IDF v5.5.4 2nd stage bootloader` | each of the 48 timestamps 24837 ms (6 lines) or 24838 ms (42) later |
| this change | `I (7) boot: ESP-IDF v5.5.4 2nd stage bootloader` | the 48 lines from the bootloader to `Calling app_main()` identical |

The golden is regenerated: `hello-s3-reboot.console.txt` (the second boot's 48 timestamped
lines, 47 `I` and one `W`; no other text changed) and `hello-s3-reboot.insns`
(10306747 -> 10260321). The instruction difference is confined to the second boot:

| `--max-seconds` | main, core 0 + core 1 | this change | difference |
| ---: | ---: | ---: | ---: |
| 11 | 4114972 + 2388186 | 4114972 + 2388186 | 0 |
| 11.5 | 6749066 + 3422631 | 6716238 + 3409033 | -32828 - 13598 |
| 12 | 6818916 + 3487831 | 6786088 + 3474233 | -32828 - 13598 |
| 21 | 8129802 + 4661431 | 8096974 + 4647833 | -32828 - 13598 |

It appears with the reset (`[emu] chip reset at t=11.005s: cause 0xc (RTC_SW_CPU_RESET)`)
and stays constant afterwards, consistent with the second boot formatting one- and two-digit
timestamps instead of five-digit ones. Every other golden is byte-identical: the C3, C6 and P4
reboot goldens never had the effect (RISC-V), and no other Xtensa golden reboots.

The classic gets the same fix. Its hello (`hello_world_classic`'s images, 30 emulated seconds,
two `SW_CPU_RESET`s), bootloader / `cpu_start: Multicore app` / `Calling app_main()`:

| Boot | main | this change |
| --- | --- | --- |
| 1, `POWERON_RESET` | 4 / 30 / 42 | 4 / 30 / 42 |
| 2, `SW_CPU_RESET` | 44176 / 44202 / 17371 | 4 / 29 / 42 |
| 3, `SW_CPU_RESET` | 6115 / 6141 / 6154 | 4 / 29 / 42 |

On main the second boot's `app_main` is lower than its bootloader because IDF rescales CCOUNT
when it switches the CPU clock (`esp32/clk.c:197`). The exception and interrupt counts are main's
(27408 / 6016); the instructions are 37417 + 14800 fewer. No classic golden covers a reboot.

The absolute values differ from silicon (7 ms against 24 ms to the bootloader's first line);
that is the emulator's boot timing, not part of this change.

## Tests

`xtensa-lx7/src/core.rs`, `reset_restarts_ccount_and_the_comparators_count_from_it`: a reset
from CCOUNT `0x12345678` reads 0, and CCOMPARE0 left at 100 is the next wake 100 cycles later
and matches at the 100th. Without the change it fails.

## Limits

- Probed on one ESP32-S3, for `USB_UART_CHIP_RESET` and `RTC_SW_CPU_RST`. Watchdog and
  `SW_SYS` resets and core 1's release from reset take the same `Cpu::reset` here and were not
  probed. The classic is not probed on silicon; the numbers above are the emulator's.
- CCOMPARE keeps its value over the reset, as before; firmware writes it before it enables the
  timer interrupt. Its reset value on silicon was not probed.
- The classic ESP32's deep-sleep wake (#240) goes through the same reset and now also starts
  CCOUNT again; no classic golden pins its early timestamps.

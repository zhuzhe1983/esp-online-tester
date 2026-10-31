# ESP32-S31 upstream contribution receipt

Status for [EX260](../../experiments.md#ex260) and docs in
[`docs/esp32s31.md`](../../esp32s31.md).

## Acceptance (upstream)

| Item | Status |
| --- | --- |
| `esp_soc::Machine<S31>` / `Soc` / `SocBus` | Compiles; SRAM load + interpreter steps; UART0 console drain; CLIC/matrix IRQ presentation |
| IDF v6.1 `hello_world` (`--preview set-target esp32s31`) | **Prints.** Real mask ROM (`esp32s31_rev0_rom.elf` 20251218, esp-rom-elfs 20260528) → SPI_FAST_FLASH_BOOT (strap 0x10) → 2nd-stage bootloader → app → `Hello world!`, full 10 s countdown, `esp_restart`, second boot. Golden: `esp32s31/tests/hello_golden.rs` (ignored by default; needs `S31_ROM_ELF`) |
| Software reset cause after `esp_restart` | **Latched.** `poll_chip_reset` sets `RST_SW_CPU` (0x0c); SYSTIMER `next_deadline` lets idle skip FreeRTOS delays so the golden reaches restart. Second ROM banner: `rst:0xc (SW_CPU_RESET)` |
| Flash size vs image header | **Aligned.** Hello path: `BoardInputs::hello_w25q16` + 2 MiB NOR (JEDEC `0xef4015`) matches the image header (no size warning) |
| Flash-MMU `--boot app` | **Ready.** `SocBus::boot_app` programs 32 KiB Flash MMU + AXI, maps flash / copies SRAM; example `s31_hello app FW_DIR` |

## Oracle / license

Espressif **esp-emulator 0.51.0** (MIT) was used as an **observable oracle only**
(register/behaviour observations that informed EXT_ILL defaults and ROM probes).
This tree is **not** a source port of that emulator.

## Bringup vs Machine

- `esp32s31::Machine` — review path for mikroverk/esp32sim PR integration.
- `esp32s31::Bringup` — temporary fixture runner for hermetic / `external_*` probes.

## hello_world golden (IDF v6.1, commit fff9895c82d7)

`tools/fetch-s31-firmware.sh` builds the unmodified IDF `examples/get-started/hello_world` with
`tools/s31-hello/sdkconfig.defaults` (reproducible build, fixed project version `s31-hello-1`) and
checks SHA-256; two clean builds gave identical bytes:

| File | Bytes | SHA-256 |
| --- | --- | --- |
| s31-hello-bootloader.bin (0x2000) | 22160 | d60f6fac20b132ffe97194e3f160cb575b0fc52cea744e0a14c06658973d0b68 |
| s31-hello-ptable.bin (0x8000) | 3072 | 7f00b6c042a89b15b0cac534f82ed988caf29278ff5700b0c511eb1b5bb7c820 |
| s31-hello_world.bin (0x10000) | 156848 | 6b965db02941e8db4895d27c2ad8eea1733e22ddf9a82ccd4d6f77ae62c6335c |

    tools/fetch-s31-firmware.sh
    S31_ROM_ELF=…/esp32s31_rev0_rom.elf cargo test -p esp32s31 --release --test hello_golden -- --ignored

Console excerpt (UART0, unmodified firmware output, no injected text):

    I (63) main_task: Started on CPU0
    I (63) main_task: Calling app_main()
    Hello world!
    This is esp32s31 chip with 2 CPU core(s), WiFi/BLE, 802.15.4 (Zigbee/Thread), silicon revision v0.0, 2MB external flash
    Minimum free heap size: 478208 bytes
    Restarting in 10 seconds...
    Restarting in 9 seconds...

Modeled for this (from IDF v6.1 `soc/esp32s31` headers and HAL call order): RTC timer and RTC
calibration advance with CPU time; PMU `SLP_WAKEUP_CNTL2` (0x158) storage; HP_SYSTEM
`CPU_INT_FROM_CPU_0..3` latches as interrupt sources 65..68; matrix `PASS_LEVEL` 0 is plain
routing (only 1, S-level, is not delivered); SYSTIMER `PERIOD_MODE` set after a one-shot
`COMP_LOAD` starts the period (IDF `port_systick.c` order); `esp_restart` reboot keeps NOR flash
and reloads the ROM. Hello golden uses `BoardInputs::hello_w25q16()` (JEDEC 0xef4015, 2 MiB NOR) to match the
image header; `devkit_w25q128()` remains available for 16 MiB bring-up profiles.

Remaining gaps (later PR): deeper esp-periph reuse for shared blocks (SYSTIMER/UART/…);
Metalio product image is not an acceptance target.

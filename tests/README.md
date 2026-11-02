# Tests

Eight layers, base to top. `cargo test --workspace` runs the hermetic ones (no toolchain, no
download, no hardware); the goldens and the ABI tests need the mask ROM ELFs and run with
`--include-ignored` (CI fetches the ELFs). Nothing skips silently: a test either has a
checked-in oracle or is `#[ignore]`d with the reason in its name.

| layer | where | what it pins |
| --- | --- | --- |
| decoders and encoder | `xtensa-lx7/tests/objdump_diff.rs`, `riscv-rv32/tests/objdump_diff.rs`, `emu-core/src/jit_a64.rs` | checked-in sampled objdump corpora (app + mask ROM, `tests/corpus/`) decode to objdump's text, with PIE comparisons excluded by the Xtensa test; the RISC-V P4 corpora (mask ROM, ESP-IDF's P4 libraries, every F form from `rv32f.S`) decode as RV32IMAFC, objdump's `.insn` words decode as illegal, and the C3/C6 decoders reject every F instruction in them; every AArch64 encoding in the fixture matches what clang assembled (`emu-core/tests/fixtures`) |
| core semantics | `xtensa-lx7/tests/semantics.rs`, `riscv-rv32/tests/semantics.rs` | on a flat RAM: register-window overflow at the instruction that touches the wrapped window, zero-overhead loops, a CCOMPARE interrupt landing on the same instruction through `step`, the block interpreter and the JIT, INTLEVEL masking, `waiti`; RV32 vectored interrupts, `mret`, `ecall`/`ebreak`; RV32F on the P4 core (rounding modes, flags, NaNs, conversions) and its lazy-FPU traps (`mstatus.FS`, `mtval`, EXT_ILL), F illegal on the C3/C6 cores; the P4 core's CLIC mode (hardware vectoring through `mtvt`, nesting by level and `mret` restoring it, CLINT mode taking nothing), its Zicntr counters, `mhartid`, and a trap breaking its LR reservation; the binary32 soft-float (`riscv-rv32/tests/softfloat.rs`) against the host in round-to-nearest-even and a round-to-odd reference in every mode; and the oracle property — the three execution paths leave identical state |
| peripherals | `esp-periph/tests/devices.rs` | systimer one-shot and periodic alarms and their deadlines, TIMG prescaler/alarm/autoreload, GPIO edge and level interrupts, USB SOF cadence per chip clock, the I2S rate from the clock registers; the `device_set!` table from outside: ranges, `delta`, `alias`, the generic fallback, source numbering, tick delivery per clock domain, the deadline minimum, `--debug` fan-out |
| audio source clocks | `cli/tests/pcm_sources.rs`, `cli/tests/adc_stream.rs` | pin routing, independent source clocks, DMA stalls, resets and exact ADC raw counts at all attenuations; no local firmware required |
| microphone input | `cli/tests/i2s_rx.rs`, `esp-periph/src/i2s/rx.rs` | three-chip MMIO to guest DMA, C3 interrupt layout, descriptor ownership/EOF, stereo packing, clocks, reset persistence and unsupported modes; no firmware or home-directory inputs |
| machine | `esp32s3/tests/machine.rs`, `esp-soc/tests/parsers.rs`, `esp32p4/tests/harts.rs`, `esp32p4/tests/revision.rs`, `cli/tests/p4_revision.rs` | idle cores skip time, core 1 runs when SYSTEM releases it, what a chip reset keeps, scripts against the board's pin names and encoder, console backlogs and masks, observers on both paths; the ELF/app-image/picture loaders never panic on random, truncated or hostile input; the P4's CLIC take rule and cross-hart reservations through `Machine`, and an image built for P4 rev ≥ 3 refused by the loaders and the CLI before it runs |
| whole runs | `cli/tests/goldens.rs`, `wasm/tests/abi.rs` | the goldens below; the wasm C ABI driven natively for the S3, C3 and C6, checking the web protocol's `board`, `serial`, `stat`, frame, audio and ring messages |
| the wasm build itself | `tools/wasm-test.mjs` | the real `esp32sim.wasm` under Node, driven through the page's firmware manifests (`web/wasm/fw/*.json`): boot, run, drain the outbox, expect the board message and the console line, no panic — the only layer that sees a wasm-only abort (a std that panics where it used to return nothing took every demo down once) |

GPIO routed-drive checks live in `cli/tests/routed_waveform.rs` and
`esp-soc/tests/routed_waveform.rs`: four chips, classic APB/AHB aliases,
route/mux/enable inversion, opt-out, reset and invalid/high pins. The shared
`tests/gpio_waveform.rs` RGB/pulse fixture now consumes this routed callback on
native and WASM while retaining its exact widths and scheduling assertions.
These are generated instruction fixtures and register tests, not library firmware
or hardware calibration. [Receipt](../docs/evidence/routed-gpio-waveform/README.md).

GPIO feedback and release checks live in `tests/gpio_feedback.rs`, included by all three
chip crates, and `esp-periph/tests/gpio_release.rs`. C6 `spi_routes.rs` covers physical
SPI selection. They use register writes and board models without firmware fixtures;
[EX217](../docs/evidence/gpio-input-feedback/README.md) records their mutation checks.

S3 PHY completion is covered without firmware by `esp32s3/tests/wifi_phy.rs`:
no AP, idle/associated AP, AP removal and register writeback must not gate FE
completion or create a network/link. `cli/tests/wifi_phy.rs` additionally boots an
unchanged public Robot Face starter; its `external_` tests are ignored until
`ROBOT_FACE_FW_DIR` names bootloader.bin, partitions.bin, firmware.bin and
esp32s3_rev0_rom.elf. They pin no-AP progress within 960,000,000 cycles, offline
timeout at 20 nominal seconds, matching-AP connection and mismatched-AP wait.
No firmware binary is added here; these are developer-input tests, not CI goldens.

The **golden-output tests** (`cli/tests/goldens.rs`) are the
regression bar for everything else: they run the committed demo firmware from the mask ROM and
compare the guest console, the captured audio (SHA-256) and the instruction count against the
files in `tests/golden/`. Bit-identical is the requirement — a timing change that shifts one
audio sample is a failure, not noise (see `docs/decisions.md`, "Performance").

They need the mask ROM ELFs, which ship with ESP-IDF (`~/.espressif/tools/esp-rom-elfs/`) or
can be pointed at with `ESP32SIM_ROM_DIR`, so they are `#[ignore]`d by default and never skip
silently: without a ROM they fail naming the file and both places. Every test and the CLI find
them the same way (`esp_soc::rom`: `ESP32SIM_ROM_DIR` first, then the newest ESP-IDF copy); three
external tests still take the older names their evidence receipts use (`ADC_ROM_DIR`, `HOOKS_ROM`,
`CLASSIC_I2C_ROM`) ahead of that. Without ESP-IDF,
`tools/fetch-demo-assets.sh --no-linux` fetches them into `web/wasm/fw/`, where `tools/wasm-test.mjs`
finds them by itself; for the cargo tests set `ESP32SIM_ROM_DIR=$PWD/web/wasm/fw`
(absolute: `cargo test` runs each test from its own crate's directory). A set directory that lacks
the file falls through to ESP-IDF's copy, so for evidence runs record which one was used.

```sh
cargo test --release --workspace -- --include-ignored --skip external_      # ~15 s for the whole set
UPDATE_GOLDENS=1 cargo test --release --workspace -- --include-ignored --skip external_   # after an intentional change
tools/wasm-build.sh && node tools/wasm-test.mjs hello c3-hello c6-hello c6-energy-scan c6-contiki c6-contiki-net c6-rpl-net panel   # the wasm module, as the page runs it
```

Classic PSRAM stock Arduino-ESP32 2.0.17 qualification is a developer-input test,
not a CI golden. Build `docs/fixtures/classic-psram-arduino2/recipe.sh OUTPUT`, then
set `ARDUINO2_FIRMWARE_DIR=OUTPUT/fw-A2` and `ESP32SIM_ROM_DIR` and run
`cargo test --release -p esp32sim --test classic_psram_arduino2 -- --ignored`.
It checks genuine dual-core low/high startup/test/heap-add, exact Arduino getter
and heap accounting, two volatile 1 MiB write/read/hash passes with free recovery,
repeat boot and the identical firmware with no device. No firmware binary is
committed for this external test; its receipt explains the SDK size accounting.

The same P6 external assertions also qualify stock Arduino-ESP32 3.1.1 on classic
ESP32-CAM. Build `docs/fixtures/classic-psram-arduino311/recipe.sh OUTPUT`, then set
`ARDUINO3_FIRMWARE_DIR=OUTPUT/fw-p6` and `ESP32SIM_ROM_DIR` and run
`cargo test --release -p esp32sim --test classic_psram_arduino3 -- --ignored`.
This is a separate 5.3-family binary SDK profile, not all Arduino 3.x or vanilla
IDF v5.3.2 equivalence; no new CI golden or firmware binary is added.

`tools/psram-wasm-test.mjs` compares the retained P5/P6 public Arduino fixtures
through the native C ABI and real WASM, with JIT off/on, same-machine reboot and
successive emulator creation. It requires both Arduino firmware directories, ROMs,
and the `psram-qualification` example (`--features psram-tests`). Build the WASM
module with `psram-tests` for production JIT settings, and separately with
`jit-tests` for extra internal validation. Both matrices pass, including classic
same-machine reboot and repeated construction; the earlier hot-region ownership
assertion is resolved by the separately retained JIT fix/regression.
See [the current integrated qualification receipt](../docs/evidence/classic-psram-followup/README.md)
and [the retained WASM negative and its resolution](../docs/evidence/classic-psram-wasm/README.md).

The classic CLI PSRAM test (`classic_psram_option`) boots the committed IDF
fixture through the executable with `--psram-mb 4`, explicit `0`, and the absent
default; unsupported sizes are rejected without a panic. Only the ECO3 ROM is
needed for the boot test. Unit tests replay normal and delay-clock SPI phases.

The external `classic_psram_micropython` test boots the unmodified official
MicroPython v1.29.0 SPIRAM image, waits for its REPL, then runs `gc.mem_free()`
with 4 MiB and with none. Set `MICROPYTHON_SPIRAM_IMAGE` and `ESP32SIM_ROM_DIR`;
[download, hash and results](../docs/evidence/classic-psram-dclk/README.md).
This downloaded image stays local, not a new committed firmware or CI golden.

Tests named `external_*` need inputs only a developer machine has (full objdump listings via
`XTENSA_DIS_FILES`/`RISCV_DIS_FILES`/`RISCV_P4_DIS_FILES`, Apple's clang for the encoder fixture) and fail loudly
without them; run them by name. Their hermetic counterparts (`decoder_matches_corpus`,
`encodings_match_fixture`) run in the default suite against checked-in oracles.

Use `--release`: the debug build runs the same scenarios ~30x slower. On a mismatch the actual
output is left next to the golden as `*.actual` for diffing.

| golden | what it covers |
| --- | --- |
| `atech-script1.*` | Pocket Synth: buttons, encoder, serial command, ST7735 over bit-banged SPI, WS2812 via RMT, SID voice on I2S/GDMA; re-run with `--no-jit` (the JIT's oracle) |
| `atech-sid.*` | the cRSID C64 jukebox: a 6502 + SID inside the emulated S3 |
| `panel-sid.*` | Touch-LCD-4B energy panel: PSRAM, LCD_CAM RGB frames, GT911 touch and TCA9554 over I2C, ES8311 on I2S, a demo partition via `--flash-at`; console, audio, instructions, stop interrupt totals and per-source counts. EX216 intentionally adds I2C transfer time. |
| `hello-classic.*` | classic ECO3 ROM → bootloader → app; console and interrupt totals/per-source counts at the selected CPU frequency, reproducible IDF 5.5.4 fixture; `classic_reset_clock_and_flash_size` additionally checks 25 emulated seconds and 16 MB JEDEC retention through two resets |
| `esp32/tests/camera_fixture.rs` | public esp32-camera on the AI-Thinker classic ESP32: normal ROM boot, OV2640 initialization, two 160 × 120 RGB565 frames, guest SHA-256 against the independent host-source oracle; reproducible IDF 5.4.3 inputs and recipe in `examples/classic-camera`. Requires the fetched classic ROM; no PSRAM or JPEG. |
| `esp32/tests/camera_lifecycle.rs` | same public driver/configuration, two initial frames, live host-image replacement, deinit/reinit and a second full sequence after same-machine reboot; pins per-phase hashes and reset IRQ state. `esp32/tests/fixtures/classic-camera/lifecycle.bin` shares the first fixture's bootloader and partition table; [recipe](../examples/classic-camera/lifecycle.md). |
| `cli/tests/classic_camera.rs` | real `--chip esp32 --board ai-thinker-cam --cam-image` run, normal ROM boot and two non-zero RGB565 frame hashes; same committed fixture and independent image oracle as the library test. Requires the fetched classic ROM. `classic_camera_jpeg_file_reaches_framebuffer_verbatim` runs `--cam-image vga-a.jpg` with `jpeg.bin` and pins the first two VGA frames' length and hash; a truncated JPEG is a usage error. |
| `esp32/tests/camera_jpeg.rs` | public esp32-camera with `PIXFORMAT_JPEG`, 20 MHz XCLK, quality 12: host JPEG reaches `fb->buf`/`fb->len` byte for byte across two host changes at VGA and QVGA, size-mismatch and malformed refusals, a raw RGB565 phase in between and JPEG after reinit; one DRAM framebuffer (`jpeg.bin`) and two PSRAM framebuffers with 4 MiB PSRAM (`jpeg-psram.bin`). Independent `make-jpeg.py` inputs; [recipe](../examples/classic-camera/jpeg.md). Requires the fetched classic ROM. |
| `classic_psram_idf` | standalone unicore IDF 4.4.7 ROM boot, authentic PSRAM memory test and heap registration, 1 MiB/33-page bytewise readback/hash, DMA/exhaustion controls, repeated creation and absent device; committed firmware and recipe in `docs/fixtures/classic-psram-idf`, requires ECO3 ROM only |
| `hello-s3.*` | stock ESP-IDF hello_world on UART0, ROM → bootloader → app_main |
| `hello-c3.*` | the same on the ESP32-C3 (RISC-V), with the MAC/reset cause/straps of the real module in `hw/c3-hello-world-real.txt`, through `esp32sim-c3` and `esp32sim --chip c3` |
| `hello-c6.*` | the same on the ESP32-C6 (RISC-V, RV32IMAC), with the identity of the Waveshare ESP32-C6-LCD-1.47 in `hw/c6-hello-world-real.txt`; the reboot variant also pins the ROM's `Saved PC` |
| `hello-p4.*`, `hello-p4-reboot.*`, `hello-p4-smp.*` | the ESP32-P4 rev < 3 hello_world of `examples/hello_world-p4` from the ECO0 mask ROM: into FreeRTOS on the CLIC and `app_main`, through `esp32sim-p4` and `esp32sim --chip p4`; the reboot variant follows `esp_restart()` back through the ROM (`rst:0xc`, `Core0 Saved PC`); the SMP variant runs both cores |
| `sdcard-p4.*` | IDF's sdmmc example on the P4 (`examples/sd_card-p4`) with a 64 MiB FAT32 card image the test builds (`tests/sd_image.rs`, MBR and volume laid out by hand so the bytes never change): the SDMMC host, the card's identification at 4 bits, FatFs mounting it through the IDMAC, `hello.txt` written, renamed and read back; console and instruction count, then the files left on the image. `sd_card_p4_unformatted_and_absent` checks the same firmware on a blank card and an empty socket |
| `sdcard-sectors-p4.*` | the same example with raw sector transfers (`examples/sd_card-p4-sectors`): 64 and 128 sectors with CMD25/CMD18, the auto-stop and the driver refilling its DMA descriptors past 16 KiB, a misaligned buffer split into CMD17s, a read past the end, on a card that programs for 2 ms (`--sd-prg-us 2000`); console and instruction count, the timing and read-back results checked |
| `usbmsc-p4.*` | IDF's USB mass-storage example on the P4 (`examples/usb_msc-p4`), the 64 MiB card in its socket and a host on its high-speed USB-OTG port (`--usb-host examples/usb_msc-p4/host.txt`): the DWC2 core under TinyUSB in DMA mode, enumeration, SCSI over bulk-only transport, raw blocks, a FAT32 file written in 64 KiB chunks paced at 4.1 MB/s and read back, an eject that hands the card to the application (TEST UNIT READY: MEDIUM NOT PRESENT), the application writing README.MD from its console (`--script`) and exposing the card again, the capacity and write protection read again, suspend, resume and unplug; console, the host's log with its emulated times and instruction count, then the files and blocks on the image |
| `usbmsc-p4-faults.*` | The same example and card with a host meeting faults (`--usb-host examples/usb_msc-p4/faults.txt`): a WRITE(10) aborted mid data stage by a 5 ms timeout and recovered with Linux usb-storage's port reset, a port reset after an eject (tud_mount_cb takes the card back from the application), and VBUS lost mid-write (tud_umount_cb gives it to the application; the file keeps size 0, its clusters allocated); console, the host's log and instruction count, the hand-overs the example logs, then the cut file on the image |
| `hosted-p4.*` | the P4 with an ESP32-C6 co-processor (`examples/esp-hosted-p4`, `--coproc`): IDF's WiFi station example on the P4, ESP-Hosted 2.12.13's unmodified slave on the C6, the two in lock-step; the P4 resets the C6 on GPIO 54, brings the SDIO transport up, receives the INIT event and takes a DHCP lease through the C6's radio. Both consoles, the P4's instruction count, the C6's run and its SDIO, WiFi and network counts. `cli/tests/cosim.rs` holds the lock-step to the same run however it is driven |
| `hosted-rawtp-rx.*`, `hosted-rawtp-tx.*` | ESP-Hosted's raw throughput test on the same pair, 10 s with the SD bus timed: the C6 sending (about 40 MB, PKT_LEN wrapping at 2^20 dozens of times) and the P4 sending; the per-second rate reports on both consoles, the instruction count and the link's counts. `cli/tests/cosim.rs` also runs a fault test per `--coproc fault=` hook on the hosted pair |
| `energy-scan-c6.*` (`external_`) | the 802.15.4 energy scanner on the Waveshare board — its owner's firmware, `ENERGY_SCAN_DIR` points at the build: PHY up, no panic, ≥50 scans, the board's report, console and count |
| `ble-advertiser-c3.*` | C3 original-controller advertising: console, BLE configuration, every observer PDU, exception and interrupt totals, per-source interrupt counts, and accounted instructions. The latter equal modeled cycles; timing regressions are checked by the interrupt report and directed tests. |
| `ble-server-c3-*.console.txt`, `ble-server-c3-*.report.txt` | C3 original-controller scan, UUID read, timeout and normal disconnect/reconnect: guest console, exception/interrupt totals and per-source counts. Three scenarios cover empty-first and read-first timeout, and normal termination. |
| `wifi-station-{s3,c3,c6}.*` | WiFi on all three chips: `examples/c6-wifi-station`, committed as `web/wasm/fw/public/*-wifi*` (rebuild in the example's README): the unmodified library scans, finds the virtual access point, joins through the WPA2 four-way handshake, takes a lease and gets five of five gateway pings. Pinned: the station's own lines (the same on every chip), the console, the end-of-run WiFi/network counts with the exception and interrupt counts, and the instruction count. On the C3 and C6 the instruction count is the cycle count, so a timing change in the WiFi model shows in the interrupt count |
| `crypto-tls-{c3,c6}.*` | Unstubbed WPA2/DHCP then TLS 1.2 ECDHE-PSK P-256 with SHA DMA, C6 ECC and a checked 256-byte exchange in each direction. Public reproducible Arduino-ESP32 3.3.11 inputs in `examples/crypto-tls`; pins console, stop exception/interrupt totals, per-source counts and Wi-Fi/network reports. Memory-queue TLS peers; no external endpoint or cycle-derived golden. |
| `cooja-echo.ndjson` (`cli/tests/cooja.rs`) | the Cooja-NG lock-step peer against a 32-instruction RV32 echo program: a hand-written NDJSON session with frames injected mid-slice; the echoes stamped at the `TX_START` cycle one air time after the frame went in, byte-identical twice over — no ROM, no toolchain |
| `cooja-ack.ndjson` (`cli/tests/cooja.rs`) | the stage-2 radio under a 55-instruction RV32 program with an address and auto-ACK: an AR frame to it is acknowledged by hardware exactly 192 µs after its end and echoed with AR on `ACK_TX_DONE`; the injected ACK is taken; without one the hardware timeout aborts; a frame to another address is filtered; a broadcast is echoed |
| `cooja-nullnet-c6.ndjson` (`external_`) | the same peer around Contiki-NG on ESP-IDF (`CONTIKI_C6_DIR` = esp32-contiki's `build-nullnet`): the periodic broadcasts as `tx` events at their `TX_START`, an injected broadcast reaching the driver after its air time and Contiki's nullnet callback, two sessions byte-identical |
| `atech-script1` with observers | the same run with `--profile-blocks --coverage --irq-latency --vcd` attached must be byte-identical and produce every report |

CI ([workflow](../.github/workflows/ci.yml)) gets the ROM ELFs with `tools/fetch-rom-elfs.sh`, which pins
the espressif/esp-rom-elfs release and each ELF's SHA-256 (the same script fills `web/wasm/fw/`), and runs the hermetic and golden tests on every push and pull request. It excludes tests named
`external_*`; it does not rerun hardware comparisons or full external objdump listings.

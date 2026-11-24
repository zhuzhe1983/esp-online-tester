# Waveshare ESP32-C5-Touch-LCD-2.8 runtime

`waveshare-c5-touch28` is an independent C5 board, using the manufacturer repository at `0196dcb7e601d6faa1c33785a71f6b0c35b532e6`. The schematic identifies an ESP32-C5-WROOM-1-N32R8 module: **32 MiB Flash and 8 MiB PSRAM**. The current factory image and project intentionally use a **16 MiB Flash layout** within that physical capacity. This is not a reason to reduce the board to16F or to omit its PSRAM. The [source manifest](../research/waveshare-c5-touch28-sources-2026-11-23/manifest.json) retains32 source files with original bytes, retrieval metadata and hashes.

## Physical wiring

| Function | Source-wired connection | Implemented boundary |
| --- | --- | --- |
| ST7789V2 LCD | SPI2 CLK6, MOSI7, CS10, D/C9;240×320 | RGB565 DCS address window, MADCTL, sleep, display enable, reset; source BSP uses INVON |
| LCD reset | CH32V003 EXIO1 | A low level resets the controller; distinct from touch reset |
| Backlight | CH32V003 PWM command05h | Commanded duty0–255 changes visible output; electrical PWM frequency and physical luminance are not calibrated |
| CST3530 touch | I2C58h, SDA0/SCL1, IRQ5, EXIO0 reset | One digital contact; exact four-byte big-endian report selector and end-read ACK |
| CH32V003 | I2C24h, SDA0/SCL1 | Eight-bit direction/output/input and PWM commands only |
| QMI8658C | I2C6Bh, SDA0/SCL1; SA0 grounded | Shared register/sample model; motion input supported; actual sensor orientation and GPIO4 interrupt timing unverified |
| PCF85063A | I2C51h, SDA0/SCL1; INT to EXIO2 | Shared virtual-clock RTC subset, including real STOP forwarding; alarm output not routed |
| ES8311 | I2C18h, SDA0/SCL1; CSB grounded | Control registers only; no I2S PCM output/input claim |
| microSD | CLK6, MOSI7, MISO8, CS23 | Real SPI SD transport via public `SocBus::attach_sd_card`; wrong MISO leaves input floating and simultaneous LCD/card selection is rejected |
| BOOT | Active-low GPIO28 | Named input `boot` (`button` alias); no fake GPIO for the independent power circuit |

The virtual board is supplied from nominal USB/3.3V. It does not simulate the charger, battery ADC, power latch, SHTC3, USB or radio. Only actual devices above are installed at their single schematic addresses. The resource page links QMI8658C, so the C variant is explicit; SA0 grounding selects6Bh regardless of a library's HIGH/LOW macro naming.

The CH32V003 is an independently programmed MCU. This model interprets its documented **board firmware protocol**, not the MCU instruction set. The pinned component1.0.1 defines direction02h (`1=output`), output03h, input04h and PWM05h. It identifies its default configuration as directionFFh/output00h, and explicitly writes those values on initialization. The model adopts that configuration; the diagnostic also sets its directions/output explicitly. PWM remains unconfigured/dark until a guest writes05h. Unconnected digital inputs use a declared virtual low value, not a measured electrical level. ADC06h, RTC07h, unknown commands, extra data bytes and direction/output wire reads are refused. The official driver returns cached direction/output values rather than reading those registers over I2C; wire readback is not invented.

This eight-bit protocol is not an alias for the sixteen-bit CH32V006 present on C5 Touch3.5/LCD2.73. The C5 Touch2.8 `docs/ch32v003.md` also assigns SHTC3 to EXIO4/5, while the schematic's signal table places the SHTC3 bus on GPIO0/1. That conflict is retained; the unimplemented SHTC3 is not wired speculatively.

## Touch and reset semantics

The bundled current IDF driver and Arduino driver both write `D0 07 00 00`, read9 bytes, and send `D0 00 02 AB`. Count is byte3 low nibble; X is byte4 plus byte7 low nibble; Y is byte5 plus byte7 high nibble; byte8 high nibble is the contact event. The model implements these exact transactions. Unspecified header/pressure fields are zero, unknown identity/calibration/sleep commands NACK, and there is no fabricated chip ID or calibration success. It supplies one contact, with coordinate bounds240×320 and the source default portrait/no-mirror touch mapping.

IRQ5 represents pending host contact events. The report snapshots the contact; its explicit ACK clears that event. An ACK for an older report cannot consume a newer press/release. Holding the reset line low suppresses address acknowledgement and IRQ, and a reset clears the prior report selector. This digital event contract does not claim the physical controller's scan rate, gesture recognizer, debounce or pulse duration. The panel and touch reset nets are independently tested through actual C5 I2C register transactions.

## Executed original diagnostic

The project diagnostic is **not manufacturer firmware**. [Its source](../../firmware/c5-touch28-probe/probe.c), linker script, startup, build command, ELF and4400-byte app image are retained. Its app SHA-256 is `52f7d72b7361e2a781c3cab63db39d08fe6771383537cbc218bbede1f78bda24`. It executes in SRAM and programs real modeled C5 MMIO; it does not patch the ROM or call a host success function.

The native runner creates the exact32F8P board and executes99,500,000 guest instructions, with0 exceptions and0 resets. The guest configures the MSPI pads/protocol/MMU, writes and reads all8,388,608 PSRAM bytes, and then submits its153,600-byte framebuffer through GDMA from mapped PSRAM. SPI runs at24MHz and I2C at400kHz from the explicit48MHz source. The panel displays3,840 distinct RGB565 colors. The quiet control changes0 pixels; touch at(80,110) changes1,024 pixels; BOOT changes53,248 pixels. Each release restores every original framebuffer byte.

The same guest selects the real SD CS23, performs CMD0/CMD8/ACMD41, saves LBA32, writes and verifies a512-byte test pattern, restores the saved bytes and reads them back. The runner checks three block reads, two block writes, and compares all8MiB of the initially nonzero test image afterward. Complete execution preserves every byte. It does not mount FAT/exFAT, and an interrupted guest is not a transactional storage guarantee. The test is separate from the platform's64MiB prepared-card default.

Reproduce after fetching the compiler/packager described in the C5 memory source receipt:

```sh
RISCV_GCC=/path/to/riscv32-esp-elf-gcc ESPTOOL=/path/to/esptool sh firmware/c5-touch28-probe/build.sh
CARGO_BUILD_JOBS=1 CARGO_PROFILE_RELEASE_LTO=false cargo run -p esp32c5 --release --example touch28_probe -- firmware/c5-touch28-probe/probe.bin /path/to/output
```

The [input/artifact receipt](../evidence/waveshare-c5-touch28-2026-11-23/inputs-and-hashes.json) records guest, ELF, source, framebuffer and full SD image hashes. PNGs and compressed raw framebuffers are retained. The [checks](../evidence/waveshare-c5-touch28-2026-11-23/checks.json) show45 C5 tests and all-target release Clippy with warnings denied. Tests include wrong board addresses/routes, separate reset nets, PWM blanking, unsupported ADC/identity rejection, real IRQ/report acknowledgement and external-device state across C5 I2C reset.

## Manufacturer firmware remains a separate result

The fixed `ESP32-C5-Touch-LCD-2.8_FactoryOnly_20261024.bin` is12,839,935 bytes, SHA-256 `e577a5451b58630acabdc322829fbeb3d52f4c1dc4248e1c6f06a7e8f34d24b3`. The manufacturer's firmware README explicitly calls it the current factory desktop and specifies loading the merged image at0. Bootloader is at2000h, partition table8000h, factory application20000h; application metadata identifies ESP-IDFv5.5.4. Both C5 chip23/revision100–199 image checksums and appended SHA digests validate. The full layout, source URL and download timestamp are retained in the [factory receipt](../evidence/waveshare-c5-touch28-2026-11-23/factory-validation.json); the third-party binary is not mirrored into the repository.

With unchanged official rev100 ROM, raw native placement at0 and physical32F8P, execution reaches6,831 instructions and stops at PC4001df26 reading PMU600b00e4 (`pmu_enable_unhold_pads`). There is no UART output or display content. Probe process exit0 means the boundary was measured; it is not a factory boot success. Public C5 firmware loading remains constrained to SRAM app-direct, so this factory record remains a reference pending the real ROM/flash path. The independent project diagnostic is an explicit selectable option rather than a factory replacement.

The precise Xiaozhi v2.6.0 release asset is also recorded as linked/unexecuted in the [central integration patch](../research/waveshare-c5-touch28-central-patch.json). Its published digest is not described as a freshly downloaded archive hash. No official UIFlow package for this exact C5 SKU was found in the inspected manufacturer resource/firmware entries; that limited search is not a universal absence claim.

# ESP32-C61 native runtime evidence

The independent `esp32c61` crate models ESP32-C61 revision 1.0 (image chip ID 20), not an ESP32-C6 alias. The original CoreMatrix UIFlow image executes from the Espressif mask ROM through the bootloader, PSRAM initialization, FreeRTOS, MicroPython and USB Serial/JTAG REPL. No firmware bytes, Python modules or function stubs are injected to make startup succeed.

## Reproducible inputs

- Firmware: [official UIFlow 2.5.4 CoreMatrix release](https://github.com/m5stack/uiflow-micropython/releases/download/2.5.4/uiflow-e437118-esp32c61-16mb-corematrix-v2.5.4-20261030.bin), 16,773,120 bytes; SHA-256 `8d5b8d4e22ebc293937086b2f9f218aa5a5ebda27b5ccb32b3f8f580ee02939b`.
- ROM: `esp32c61_rev100_rom.elf`, from Espressif's `esp-rom-elfs` release `20260528`, fetched by `tools/fetch-rom-elfs.sh`; SHA-256 `c4ee94d3a8e335da4ef5f30a8a1ebb9691e639f1c773f484e9ba721aa1557c2e`.
- The firmware reports ESP-IDF `v5.5.4-dirty`, MicroPython `v1.27.0-dirty`, build date 2026-10-30, board `M5Stack CoreMatrix with ESP32-C61HR8`. Its bootloader and application both require full revision range 100–199. Native and browser loaders reject images that exclude revision 100.

## Register provenance

The implementation uses [ESP-IDF v5.5.4 C61 SoC definitions](https://github.com/espressif/esp-idf/tree/v5.5.4/components/soc/esp32c61), matching the firmware's reported IDF version. Relevant files are `register/soc/{reg_base,interrupt_matrix,clic,pcr,lp_clkrst,lp_aon,lp_timer,cache,io_mux,gpio,efuse,spi_mem_c,spi1_mem,ahb_dma,apb_saradc,i2c_ana_mst}_reg.h`, `include/soc/{interrupts,gpio_sig_map,soc_caps,regi2c_saradc,ext_mem_defs}.h`, and `rom/esp32c61.rom.ld`. MMU, cache and ADC sequence details use [the C61 HAL](https://github.com/espressif/esp-idf/tree/v5.5.4/components/hal/esp32c61/include/hal). ADC nominal conversion uses C61 `esp_adc/esp32c61/curve_fitting_coefficients.c` and `efuse/esp32c61/esp_efuse_rtc_calib.c` from the same version.

C61-specific differences exercised by tests include 320 KiB SRAM, 256 KiB ROM, a shared 32 MiB cache window with 512 MMU entries, PSRAM selection bit 9 and valid bit 10, standard CLIC at `0x20800000`, C61 interrupt source numbers, GPIO input-select bit 8 and matrix spacing, IO_MUX pin 0 at offset zero, four ADC pads (GPIO1/3/4/5), and two GDMA channels. C6's LP SRAM window and third GDMA channel are not claimed as C61 hardware.

## Commands and observed results

Run from the repository root with caller-supplied input paths:

```sh
ESP32SIM_ROM_DIR="$ROM_DIR" cargo test --release -p esp32c61 --test hardware -- --include-ignored
ESP32SIM_ROM_DIR="$ROM_DIR" ESP32SIM_COREMATRIX_FIRMWARE="$COREMATRIX_BIN" cargo test --release -p esp32c61 --test firmware -- --include-ignored
cargo run --release -p esp32c61 --example firmware -- "$ROM_DIR/esp32c61_rev100_rom.elf" "$COREMATRIX_BIN" 100000000 "$GUEST_SCRIPT"
```

The seven hardware regressions pass, including CPU instruction retirement stopping while the PM1 DCDC rail is off, a power-button wake requesting reset, and PM1 command `0xa3` restarting the actual mask ROM with the download strap. That last test observes `ESP-ROM:esp32c61` followed by `waiting for download`; it runs in CI with the fetched public ROM.

Native ROM boot reached the UIFlow 2.5.4 REPL after a 100,000,000 instruction budget with zero CPU exceptions. The unmodified distribution reports `ImportError: no module named 'M5Things'` during its automatic `boot.py` startup. This is not suppressed: `help('modules')` omits the module and the guest's `/flash/libs` contains only `README.md` and `__init__.py`. The filesystem reads match the original image's LittleFS contents. A successful interpreter boot is not reported as successful cloud startup.

Through actual USB raw REPL, `M5.Display.fillScreen(0xffffff)` reports a 16 × 16 display and changes all 256 board pixels to `[128,128,128]` at the firmware's default brightness. The TM1680 frame count increases from 3 to 4. A second regression explicitly sets brightness 255 and requires all 256 pixels to become `[255,255,255]`. It also executes `machine.PWM(Pin(25), freq=1000, duty_u16=16384)`: after one period has latched the duty, the guest reports 998 Hz / 16384 and the pad model reports 998.4025559105431 Hz / 16384. This follows the 80 MHz source, 16-bit timer and fixed-point divisor 313; the requested 1000 Hz is quantized by the actual programmed divider. The display is derived from the guest's I2C writes; no host framebuffer assignment is used.

## Limits

The CPU instruction budget is functional execution, not calibrated wall-clock performance or a silicon timing claim. ADC input is an ideal digital/nominal calibration model. The PMIC uses guest time for digital button qualification; analog rail transients and electrical debounce are not simulated. C61 I2S capture, Bluetooth and undocumented peripherals outside the exercised boot/device paths remain unqualified. WiFi has a separate evidence file and its own real firmware qualification; linking a virtual AP by itself does not prove networking.

This receipt contains public input identities and technical observations only; machine-specific paths and raw process/session inventories are omitted.

# Waveshare 4-inch LCD V4 runtime evidence

## Exact scope and outcome

This change adds two independent V4.0 board profiles: `waveshare-s3-touch-lcd4-v4` and `waveshare-s3-lcd4-v4`. Both have 16 MiB Flash, 8 MiB PSRAM and a **square 480 × 480 ST7701S display**. Only the touch profile instantiates GT911. The older TCA9554-based revision, 4B, 2.1-inch round display and CH422G boards are not aliases.

Three original diagnostic tests pass. Each exercises both profiles through actual LX7 S32I/L32I instructions, checks instruction PC advancement, and observes the resulting peripherals and board output. The display test transfers real DMA descriptors and guest-memory pixels through LCD_CAM/GDMA. It explicitly preserves four distinct pixels at all four square-screen corners. An initial circular-mask assumption was rejected during source review and removed before the retained factory runs; the final tests cover the corrected rectangular aperture.

Both exact original factory images run through the ROM for the configured five-second guest interval with no reboot. They initialize the panel, generate board-level RGB frames and return from `app_main`. **Neither writes the modeled CH32 PWM command.** The helper's initial PWM firmware state is not established by the retained host sources, so it remains unknown and the display output stays black. This is a partial runtime with successful transport diagnostics, not verified visible factory UI or full application acceptance.

Models and product metadata are maintained separately. This runtime evidence does not validate enclosure geometry, real browser WebGL, physical display timing, electrical behavior or physical hardware.

## Sources and version boundaries

The retained [source manifest](../../platform/research/waveshare-touch4-v4-runtime-sources/manifest.json) contains 28 original selected sources, their original URLs, retrieval times, original byte counts/SHA-256 hashes and compressed retained hashes. ZIP members also retain the parent archive hash and exact member name. Component licenses and version manifests are included. The original factory binaries are not committed.

The board source is pinned to [Waveshare commit 390aaba10e80f1ef08506cb8d7db5e41cea09273](https://github.com/waveshareteam/ESP32-S3-Touch-LCD-4/tree/390aaba10e80f1ef08506cb8d7db5e41cea09273). Its V4 schematic, hardware reference, Arduino helper/LVGL example, maintained SD example and factory manifest distinguish the exact touch and non-touch variants. The [BSP 3.0.0 archive](https://components.espressif.com/components/waveshare/esp32_s3_touch_lcd_4/versions/3.0.0) and [CH32 helper 1.0.1 archive](https://components.espressif.com/components/waveshare/custom_io_expander_ch32v003/versions/1.0.1) provide the implemented byte protocol and initialization behavior. These are source baselines, not proof that every byte of the older factory images was built from these component versions.

Both original application descriptors report `v5.5.1-dirty`. Public ESP-IDF v5.4.1 and v5.5.1 S3 GPIO matrix, LCD_CAM register and SPI low-level headers are retained. [Their comparison](idf-header-comparison.json) confirms byte-identical GPIO signal and LCD register definitions; the SPI command-prefix/phase operations used here are unchanged. The unpublished changes implied by `dirty` remain unknown. The existing shared GT911 and PCF85063 implementations keep their documented datasheet-based subsets; this board does not widen those shared devices.

## Physical routing and modeled behavior

| Function | Exact V4 path | Modeled boundary |
| --- | --- | --- |
| CH32V003F4U6 helper | I2C 0x24, SDA15/SCL7 | Reg02 direction, reg03 output, reg04 input, reg05 PWM, one byte each; direction 1 means output |
| ST7701S serial control | CS42, SCK2, MOSI1 | Complete nine-bit words, GPIO rising-edge decode and validated SPI2 command1/address8 transport; no MISO device |
| RGB data bit0 through15 | 5,45,48,47,21,14,13,12,11,10,9,46,3,8,18,17 | Actual GPIO matrix lane remapping, exact 480 × 480 frame size |
| RGB timing | PCLK41/HSYNC38/VSYNC39/DE40 | S3 matrix signals154/151/152/150; all control/data routes required |
| LCD reset | CH32 EX3 | Active low, including low/high writes between board advances; clears stale frame and controller state |
| GT911 reset/interrupt | CH32 EX1/EX2 | EX1 reset; EX2 low at release selects0x5D, high/released selects0x14; IRQ read through reg04 only when EX2 is input |
| GT911 reports | Shared SDA15/SCL7 | One contact, bounded480 × 480 coordinates, ready/ACK/rescan; deterministic10ms digital scan policy |
| PCF85063ATL | I2C0x51, shared SDA15/SCL7; IRQ EX7 | Guest-cycle BCD calendar and existing STOP behavior; no alarm/timer IRQ |
| microSD | CLK2/CMD1/D0=4; DAT3/SDCS on EX4 | Native one-bit transport when EX4 high and card inserted; exact routes checked by SoC; no four-bit or SPI-card alias |
| BOOT | GPIO0 active low | Only button/boot named input |
| GPIO16 | SYS_OUT power network | No invented touch interrupt or ordinary power-button input |

The GPIO route callback computes the RGB mapping directly. S3 does not deliver the S31-only `lcd_rgb_pins` notification, so implementing only that method would leave the board with zero delivered frames. The actual CPU/DMA test verifies both SoC frame production and the resulting board pixels.

Common ST7701 reset, sleep, display enable, inversion, MADCTL BGR/row scan and vendor-bank boundaries are modeled. Vendor-bank opcodes are not treated as common DCS commands. Unsupported MADCTL orientations do not silently produce an apparently correct view. The selected BSP programs COLMOD0x66 while configuring a 16-lane bus; the renderer exposes the actually wired RGB565 values for the supported0x50/0x60 RGB modes. This is not calibrated18-bit glass reproduction, gamma modeling or an analog refresh-timing claim.

GT911 exposes its known product family string and actual resolution. Unknown identity/firmware registers are rejected or read asFF after the supported range, rather than fabricated. Configuration RAM does not implement the silicon firmware/checksum engine or alter the physical report dimensions. Contact size20 is the existing shared model's explicit digital fixture value, not a measured pressure. The non-touch SKU has no0x14/0x5D endpoint and ignores injected touches. Neither profile inherits QMI8658, an audio codec or the legacy TCA9554 endpoint.

## Source conflicts retained

The BSP brightness API writes `255` for off and `0` for full brightness, while the pinned standalone ioexpander README describes the reverse. The V4 schematic connects EXIO_PWM through R40/RC and R36 into the AP3032 feedback node, not into its BL_EN/CTRL pin. The digital renderer follows the named BSP inverse policy, with linear scaling for intermediate values. The feedback topology supports that direction; its analog LED-current curve, helper PWM frequency and calibrated brightness are not modeled. **No initial PWM byte is invented.** A firmware write to reg05 is required before this subset lights the display.

The selected BSP and maintained Arduino/SD initialization use output0x2A, which releases display/touch reset and raises SYS_EN but leaves EX4 low. The supported native-SD selection therefore stays unavailable until the guest raises EX4. The retained non-touch factory image does raise it, uses output0x3A and mounts the prepared card. The touch factory image leaves0x2A and makes no SD command during the observation. A failed original example is not fixed by silently changing its output register.

The BSP supplies its EXIO bit3 mask as a panel reset GPIO value; the original UART warns that MCU GPIO8 is reserved. The actual V4 schematic assigns GPIO8 to an RGB data lane and LCD reset to CH32 EX3. The board therefore does not invent a physical GPIO8 reset wire to accommodate that source mismatch.

The BSP also leaves EX2 configured as an output low during normal polling. The model preserves that direction and output latch. It does not silently release EX2 to present a convenient IRQ reading. Tests deliberately change EX2 to input before asserting the interrupt level. SYS_EN and buzzer control bytes remain visible, but no analog power latch, battery measurement, RTC alarm, buzzer sound, CAN/RS485 peer or USB device is fabricated.

## Original factory execution

| Image/profile | Immutable SHA-256 | Native five-second observation |
| --- | --- | --- |
| Touch, FactoryOnly20251122 | `5db7ee79f1063465d8114cea3a4675c8c87bdf303c2a3edaf6a55414c8634706` | 236 board frames;124GT911 status reads;0reboots; no SD commands; app_main returned; PWM unknown, black display |
| Non-touch, FactoryOnly20251218 | `7af2ca6b44d970c65dabef06e05659596eed9156873526893b0617cf0796ba76` | 239 board frames;0touch reads;0reboots;28SD commands,2timeouts,1616bytes read; FAT32 mounted/unmounted; RTC reported00:00:04; app_main returned; PWM unknown, black display |

Both images are merged images at offset0 with ESP chipID9, valid boot/application segment checksums and digests, valid partition-table MD5 and a layout fitting the exact16MiB Flash. The application descriptor dates match the filename dates. The prepared empty card is64MiB FAT32. No firmware, Flash capacity, PSRAM size or default card bytes were edited to bypass a failure. Runner, ROM, initial card, runtime source and diagnostic source hashes are in [native inputs](native-run-inputs.json); exact measurements and artifact hashes are in [native runs](native-runs.json).

UART includes duplicated/interleaved lines, retained as observed. `uart.original.gz` preserves original bytes; the adjacent text view normalizes only line endings and trailing whitespace. Guest text such as `RS485 OK!` or successful CAN driver installation is not interpreted as proof of a modeled external RS485/CAN device. Those wire protocols are outside this board subset. The five-second stop label is recorded exactly; no longer-duration interaction or complete factory acceptance is inferred.

## Tests and reproduction

The three passing test functions each run both profiles:

1. Actual GPIO nine-bit control, real LCD_CAM/GDMA delivery and lane remapping, all four square-screen corners, wrong data/PCLK/CS negatives, vendor banks, truncated words, sleep/reset stale-frame clearing, inverse PWM scaling, plus SPI2 MSB-aligned D/C and unconnected-MISO behavior.
2. Exact I2C routing, byte-sized helper commands and unsupported ADC/readback negatives, GT911 reset/address strap, EX2 input/output IRQ behavior, clamped coordinates, non-touch absence, RTC shared between both controllers and absent legacy devices.
3. Native one-bit SD IDMAC512-byte write/read, EX4 selection, wrong CMD/D0, four-bit mode and ejection timeout negatives.

These are **three LX7 test functions, each covering two profiles**, not six separately registered tests and not direct-MMIO-only tests. Targeted Clippy with warnings denied passes. This runtime-only change has no UI or catalog edits; full integrated workspace/WASM/catalog/browser checks remain separate integration gates. The retained test command and complete result are in [guest-tests.txt](guest-tests.txt), with [Clippy output](clippy.txt).

```sh
CARGO_BUILD_JOBS=1 cargo test -p esp32s3 --test waveshare_touch4_v4_guest
CARGO_BUILD_JOBS=1 cargo clippy -p esp32s3 --lib --test waveshare_touch4_v4_guest --example waveshare_source_attempt -- -D warnings
CARGO_BUILD_JOBS=1 cargo build --release -p esp32s3 --example waveshare_source_attempt
python tools/waveshare-batches/s3-tail/verify_touch4_v4_runtime.py \
  --source-cache EXACT_V4_CACHE --runtime-cache BSP_HELPER_CACHE \
  --research RETAINED_RESEARCH_DIR --evidence EVIDENCE_DIR \
  --runner NATIVE_RUNNER --rom S3_ROM --sd EMPTY_FAT32_IMG
```

Public source code, receipts, original firmware UART and generated black frame PNGs were reviewed before commit. No private account, credential, personal card data or unrelated process inventory is included. The source and evidence hash checks are recorded in `verification.json`.

# reTerminal D1001: verified prerequisites for a real runtime

The D1001 has product knowledge, a model and a pinned official firmware source,
but it does **not** have a working simulator board runtime. The existing P4 SPI
display profile is not electrically or programmatically compatible. This check
identifies the first observed boot blocker and the additional display/input
work required before that status can change.

The review uses Seeed's official repository at
`5074d3b2f45626b261298e305aaf792036febc5a` and ESP-IDF `v5.4.2`, matching the
factory image's `v5.4.2-dirty` descriptor. The upstream IDF tag is a driver and
register reference; it is not claimed to reproduce Seeed's unpublished dirty
tree byte for byte. The compact, identifier-free receipt is
[`evidence/seeed-d1001-20261122/validation.json`](evidence/seeed-d1001-20261122/validation.json).

## Actual wiring and display path

| Component | Official BSP configuration | Runtime consequence |
| --- | --- | --- |
| Main processor | ESP32-P4; the tested image accepts revision 1.0 | Select the P4 revision below 3, with its matching ROM |
| LCD | JD9365_8, 800 × 1280, two MIPI-DSI data lanes at 1,000 Mbit/s per lane | Requires a DSI host and bridge, not a GP-SPI or parallel RGB alias |
| DPHY supply | LDO channel 3, 2,500 mV | Digital link readiness must respect power/reset/clock state |
| Control traffic | DBI virtual channel 0, 8-bit commands and parameters over DSI | Decode actual DSI packet/FIFO transactions into panel state |
| Pixel traffic | DPI stream; 60 MHz pixel clock; RGB565 by default, RGB888 option | Scanout follows actual memory/DMA configuration |
| Buffers | Factory defaults request two DPI framebuffers and DMA2D copies | Account for buffer switching and completion callbacks |
| Board I2C | Hardware I2C1, SDA 20 / SCL 21 | PCA9535 controls panel power, reset and backlight enable |
| Touch | GSL3670 on hardware I2C0, SDA 37 / SCL 38; IRQ GPIO 16 | Requires I2C0 transport and the GSL3670 protocol |
| Backlight | LEDC on GPIO 14; expander enable bit 7 | Brightness depends on both PWM and board enable |

PCA9535 bits 0, 2 and 12 control LCD supply, LCD reset and touch reset;
bit 8 is board power hold. These are expander outputs, not P4 GPIO numbers.
The chosen JD9365 DPI macro supplies horizontal back porch / sync / front
porch of 20 / 20 / 40 and vertical values of 30 / 4 / 30. The BSP calls that
macro directly. Other timing constants in `display.h`, and the unused helper
macro's 1,500 Mbit/s default, must not override the values actually selected by
`bsp_display_new_with_handles`.

The BSP creates the DSI bus, a DBI panel-control handle, then a JD9365 panel
whose underlying backend is DPI. The IDF DPI driver sends memory through
**DW_GDMA** into the DSI bridge FIFO at `0x50105000`. The bridge and host
generate the video stream. This path does not use LCD_CAM.

## Observed first blocker: bootloader ADC entropy

The unmodified official merged P4 binary is 13,697,024 bytes, SHA-256
`50e63daf5380383f9b89990b664bf4a825dd49aab0d58bdd7d52d542a2dd7c55`.
Its bootloader starts at `0x2000`, partition table at `0x8000`, app at
`0x10000` and included storage image at `0x910000`. The app descriptor names
`esp_brookesia_demo` version `1.0.1`, built 2025-12-11, IDF `v5.4.2-dirty`.
The filename's 2025-12-17 date is an artifact date, not a product launch date.

A genuine ROM boot with the existing `none` board reaches the second-stage
bootloader. It identifies revision 1.0 and 16 MiB flash, then remains in the
early RNG entropy initialization. A 3-second guest run retires 1,080,000,000
core-0 instructions, with no interrupts or exceptions, and does not reach the
app. A separate 0.05-second diagnostic run stops at PC `0x4ff2de80`, reading
`0x500de054`.

ESP-IDF's P4 bootloader entropy routine waits for SAR1 conversion completion
in `ADC_INT_RAW`, offset `0x54`, bit 31. The current P4 map labels the ADC
block but does not mount an ADC device; the access falls through to register
storage without a conversion-completion event. The native log and driver
therefore identify a core peripheral prerequisite before any LCD code runs.

```sh
target/release/esp32sim --chip p4 --board none --boot rom \
  --rom "$ROM_DIR/esp32p4_rev0_rom.elf" \
  --flash-image "$FIRMWARE_DIR/reTerminal_D1001_esp32p4_factory_firmware_2025_12_17_01.bin" \
  --flash-mb 16 --psram-mb 32 --max-seconds 0.05 --console both --log-periph
```

This is a boot diagnosis, not D1001 board validation. No firmware patch,
app-direct entry or fabricated display frame was used. These functional runs
overlapped unrelated compilation and are not CPU performance measurements.

## Additional display and input prerequisites

The following are source-backed gaps after the observed ADC blocker. The
native probe did **not** reach them, so they are not reported as measured
execution failures.

| Missing behavior | Evidence in the real driver | Existing simulator state |
| --- | --- | --- |
| DSI PHY/link readiness | Bus creation waits for PHY lock at `0x500a00b0` bit 0, then clock/data-lane stop bits 2, 4 and 7 | DSI `0x500a0000` is only a log label and register-storage fallback |
| DSI command FIFO and packet execution | DBI submits panel commands through the DSI host; command-status register is at host offset `0x74` | No packet parser, FIFO status transitions or panel-control endpoint |
| DSI bridge and video scanout | Bridge is at `0x500a0800`; configured dimensions, format, block count and output-enable control the stream | No bridge device or P4 frame-publication path |
| DW_GDMA transfer engine | DPI uses linked-list memory-to-DSI transfers with hardware handshake, fixed destination `0x50105000`, 64-bit transfer width | `0x50081000` is unimplemented; existing AXI/AHB GDMA is a different DMA IP |
| DW_GDMA descriptors and IRQ | Each list item is 64 bytes, aligned to 64 bytes; a whole-frame completion interrupt causes the driver to rearm the next list | No descriptor walk, valid/last-marker update, completion interrupt or rearm behavior |
| DMA2D framebuffer copy | Factory requests `use_dma2d`; the IDF DPI backend installs the asynchronous framebuffer-copy path | Shared DMA2D exists for S31, but is not mounted or serviced by P4 |
| PCA9535 and GSL3670 | BSP accesses expander power/reset and uploads/configures the touch controller, then reads its 44-byte report at register `0x80` | Neither device is present on a D1001 board; touch hardware I2C0 is also absent from the P4 device table |

P4 currently exposes only hardware I2C1. Its historical board-device bus index
0 maps to that controller. Adding I2C0 must preserve that existing convention
or deliberately migrate all users with tests; silently treating board bus 0
as hardware I2C0 would break the existing P4 board.

Returning a constant PHY-ready status would merely move execution to the next
missing operation. Reading an arbitrary framebuffer and painting it in the
frontend would bypass the very DMA/FIFO/interrupt behavior being debugged.
Neither is sufficient to mark the product as runnable.

## Reusable pieces and minimum useful implementation

The P4 CPU, flash/PSRAM memory, interrupt matrix, LEDC and I2C1 provide useful
foundations. `BoardModel::lcd_frame` can receive a completed, validated RGB565
frame. The S3 LCD path offers conversion/output code to study, but its
LCD_CAM register behavior is not a DSI implementation. The current P4
`waveshare-p4-lcd35` board drives an ST7796 by SPI; renaming it would select
the wrong controller, transport, dimensions, pins and touch protocol.

`esp-periph/src/dma2d.rs` implements the related P4/S31 IP for S31, including
memory copies and its 24-byte descriptors. Porting its device registration,
memory adapter, clock/reset handling and interrupts to P4 could avoid
duplicating the copy engine. Its source currently cites the S31 register
headers, so the P4 v5.4.2 mapping still needs comparison. It cannot replace
DW_GDMA's distinct 64-byte descriptors or its DSI destination handshake.

A bounded first display milestone would implement the real IDF-used DSI
subset: digital PHY power/clock/reset transitions; DBI DCS packet delivery;
RGB565 DPI scanout through validated DW_GDMA descriptors; completion IRQ and
buffer rearming; PCA9535-controlled panel power/reset; and LEDC backlight.
DMA2D must be attached for the unchanged factory configuration. The board may
then publish frames through the existing sink. Unsupported color/packet/DMA
modes must be reported rather than silently converted to a different protocol.

That milestone requires register-level positive and negative tests, an actual
IDF DPI program that changes buffers and receives completion callbacks, and
then an unchanged factory ROM boot reaching a nonblank UI. Interactive
validation additionally requires I2C0 and GSL3670 reports, reset and IRQ
handling. Camera/ISP, audio, sensors and the C6 network coprocessor are separate
behaviors; display validation must not imply those work. The current result
is an identified implementation plan and a reproducible blocked boot, not a
partially functional display runtime.

## Primary sources

- [Seeed D1001 getting started](https://wiki.seeedstudio.com/getting_started_with_reterminal_d1001/).
- Seeed pinned [BSP implementation](https://github.com/Seeed-Studio/reTerminal-D1001/blob/5074d3b2f45626b261298e305aaf792036febc5a/components/esp32_p4_re_terminal_d1001/src/esp32_p4_re_terminal_d1001.c),
  [pin definitions](https://github.com/Seeed-Studio/reTerminal-D1001/blob/5074d3b2f45626b261298e305aaf792036febc5a/components/esp32_p4_re_terminal_d1001/include/esp32_p4_re_terminal_d1001.h),
  [display configuration](https://github.com/Seeed-Studio/reTerminal-D1001/blob/5074d3b2f45626b261298e305aaf792036febc5a/components/esp32_p4_re_terminal_d1001/include/display.h),
  [JD9365 DPI macro](https://github.com/Seeed-Studio/reTerminal-D1001/blob/5074d3b2f45626b261298e305aaf792036febc5a/components/esp_lcd_jd9365_8/include/esp_lcd_jd9365_8.h),
  [factory defaults](https://github.com/Seeed-Studio/reTerminal-D1001/blob/5074d3b2f45626b261298e305aaf792036febc5a/examples/factory_firmware/sdkconfig.defaults),
  [GSL3670 driver](https://github.com/Seeed-Studio/reTerminal-D1001/blob/5074d3b2f45626b261298e305aaf792036febc5a/components/esp_lcd_touch_gsl3670/esp_lcd_touch_gsl3670.c)
  and [factory image](https://github.com/Seeed-Studio/reTerminal-D1001/blob/5074d3b2f45626b261298e305aaf792036febc5a/firmware/reTerminal_D1001_esp32p4_factory_firmware_2025_12_17_01.bin).
- ESP-IDF v5.4.2 [bootloader entropy](https://github.com/espressif/esp-idf/blob/v5.4.2/components/bootloader_support/src/bootloader_random_esp32p4.c)
  and [ADC registers](https://github.com/espressif/esp-idf/blob/v5.4.2/components/soc/esp32p4/register/soc/adc_reg.h).
- ESP-IDF v5.4.2 [DSI bus](https://github.com/espressif/esp-idf/blob/v5.4.2/components/esp_lcd/dsi/esp_lcd_mipi_dsi_bus.c),
  [DPI panel and DMA flow](https://github.com/espressif/esp-idf/blob/v5.4.2/components/esp_lcd/dsi/esp_lcd_panel_dpi.c),
  [DBI transport](https://github.com/espressif/esp-idf/blob/v5.4.2/components/esp_lcd/dsi/esp_lcd_panel_io_dbi.c),
  [DSI PHY LL](https://github.com/espressif/esp-idf/blob/v5.4.2/components/hal/esp32p4/include/hal/mipi_dsi_phy_ll.h),
  [DSI registers](https://github.com/espressif/esp-idf/blob/v5.4.2/components/soc/esp32p4/register/soc/mipi_dsi_host_reg.h),
  [DW_GDMA driver](https://github.com/espressif/esp-idf/blob/v5.4.2/components/esp_hw_support/dma/dw_gdma.c)
  and [DW_GDMA descriptors](https://github.com/espressif/esp-idf/blob/v5.4.2/components/hal/esp32p4/include/hal/dw_gdma_ll.h).

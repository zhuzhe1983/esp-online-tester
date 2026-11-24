# Waveshare ESP32-C5-Touch-LCD-1.69

Profile `waveshare-c5-touch169` implements the board's ST7789V2 display, CST816S single-contact input and BOOT input through the independent C5 transport. It has **16 MiB flash and 8 MiB Quad PSRAM**. An original SRAM diagnostic has exercised the whole virtual PSRAM, actual GDMA display reads, I2C touch report and resulting pixel feedback. Manufacturer factory boot remains blocked in the C5 ROM's unimplemented cache-control block.

## Hardware and source boundaries

The fixed manufacturer repository is `waveshareteam/ESP32-C5-Touch-LCD-1.69@136f2a8c68b3808ce4a9dbea218e9c3a4928212f`. The schematic, BSP, component metadata, configuration, canonical pages, raw download receipts and Espressif CST816S driver are retained in the [source manifest](../research/waveshare-c5-touch169-sources-2026-11-23/manifest.json).

The schematic identifies ESP32-C5HR8 and a 48 MHz crystal, plus an external M25QH128DHIQT flash. The canonical page and BSP configuration specify 16 MB flash. ESP32-C5 Series Datasheet v1.5, Table 1-1, specifies 8 MB Quad PSRAM for C5HR8; its original PDF is retained in the [C5 memory source manifest](../research/c5-memory-sources-2026-11-23/manifest.json). This capacity is independent of diagnostic allocation. The emulated PSRAM is the explicitly documented AP-compatible virtual die in [c5-memory-runtime.md](c5-memory-runtime.md), not a measured production die identity.

| Device | Actual board connection | Implemented boundary |
| --- | --- | --- |
| ST7789V2, 240 × 280 | SPI2 CLK23, MOSI7, CS27, DC24; visible rows20–299 | RGB565 window addressing, sleep/on, inversion and MADCTL; sourced DCS subset |
| Backlight | GPIO26, active high | GPIO and C5 LEDC duty |
| LCD reset | Pull-up to 3V3 | Released under the explicit virtual USB supply |
| I2C bus | SDA8, SCL9 | C5 HP I2C0, signals47/46, explicit48MHz source |
| TCA9554 | Address20h, A0/A1/A2 grounded | Four real direction/output/input/polarity registers |
| CST816S | Address15h; IRQ GPIO5; reset TCA EX1 | Single contact, report02h–06h, sleep, reset and digital pending-event IRQ |
| QMI8658C | Address6Bh, SA0 grounded; physical INT1 to EX3 | Shared sourced register/sample subset; IMU IRQ to EX3 is not implemented |
| PCF85063A | Address51h; physical INT to GPIO6 | Guest-cycle calendar subset; alarms/IRQ unsupported |
| ES8311 | Address18h, CSB grounded | Sourced control/register reset subset; no I2S samples |
| BOOT | GPIO28, active low | Actual guest GPIO input |
| PWR key | TCA EX4 and board power circuitry | No user input ID or power-off emulation |

There is **no TF/microSD slot** on this board. The public SD loader rejects it. The BQ27220 fuel gauge, battery/charger/boost system, USB, RF, I2S audio and physical timing calibration are unimplemented. No general-purpose I2C responder is attached for missing devices. Peripheral state persists across controller reattachment; controller reset is not treated as power loss for external devices.

The CST816S layer allows the known report and identity register reads, retaining zero for unknown chip/firmware identities. Unsupported register writes NACK. It inherits the explicit event-driven digital subset: no autonomous scan, auto-standby response window or measured interrupt pulse duration is claimed. Espressif's retained README describes real devices which respond only after events; this subset does not reproduce those analog/timed standby details. The diagnostic reads only after GPIO5 asserts.

### Conflicting official material

The BSP's touch initialization says reset is shared with the LCD. Schematic page1 instead shows LCD_RST on its own pull-up and TCA EX1 connected to the touch reset pin. The model follows the schematic; the regression test proves that toggling EX1 clears touch without erasing or disabling LCD output.

The resource page links an ST7262 PDF although the product text, schematic/BSP wiring and actual `esp_lcd_new_panel_st7789` call identify ST7789V2. The resource page's QMI link is specifically QMI8658C, while the schematic/BSP use the generic QMI8658 name. The C variant is selected explicitly and SA0 is bound at only6Bh. The BSP configuration retains unrelated S3 cache options and differs from the factory's OTA partition layout; neither is taken as a substitute for the actual C5 register source or parsed image.

## Original guest acceptance

`firmware/c5-touch169-probe` is project-authored diagnostic source. It runs from SRAM using the public App loading path and the board's physical16F8P capacity. It initializes the real modeled C5 native PSRAM pads, selects Quad mode, programs all128 64KiB MMU pages, writes and reads all8,388,608 bytes, and then builds a134,400-byte framebuffer in that mapped PSRAM. GDMA descriptor ownership and SPI transfer completion are checked by the guest.

The guest uses a24MHz SPI bus, a400kHz I2C bus and5kHz LEDC from an explicit48MHz crystal. These are diagnostic choices; the manufacturer BSP uses different faster clock settings. TCA configuration and QMI WHOAMI are read through the actual I2C master. Touch input passes through GPIO5 and CST816S registers before the guest changes pixels.

The [execution evidence](../evidence/waveshare-c5-touch169-2026-11-23/inputs-and-hashes.json) records:

- Full8MiB PSRAM write/read pass, then GDMA reads from the PSRAM framebuffer.
- 240 × 280 display with3,360 distinct RGB565 values.
- Quiet control:0 changed pixels.
- Touch at(80,110):1,024 changed pixels; guest logs the decoded coordinates; release restores the exact original frame.
- BOOT28:44,928 changed pixels; release restores the exact original frame.
- 90,250,000 retired/dispatched instructions;0 exceptions and0 resets.

These are diagnostic rendering/input results, not the manufacturer's desktop UI or a claim about color calibration. The physical display's response times, gamma, subpixel order under all scan configurations and touch calibration have not been measured on hardware.

Reproduce with a C5-capable RISC-V GCC and esptool5.1.0:

```sh
RISCV_GCC=/path/to/riscv32-esp-elf-gcc ESPTOOL=/path/to/esptool sh firmware/c5-touch169-probe/build.sh
CARGO_BUILD_JOBS=1 CARGO_PROFILE_RELEASE_LTO=false cargo run -p esp32c5 --release --example touch169_probe -- firmware/c5-touch169-probe/probe.bin /tmp/c5-touch169-evidence
```

The source fixture initially polled the self-clearing START bit and omitted GPIO input enable on SDA/SCL. Those guest mistakes were corrected to wait for completion/NACK and to enable the actual inputs; the earlier failure is retained in the evidence history. The emulator was not changed to make the invalid guest succeed.

## Factory investigation and ROM boundary

The exact manufacturer file is `ESP32-C5-Touch-LCD-1.69_FactoryOnly_20260626.bin`,14,010,352 bytes. It is a padded whole-flash image loaded at0. The bootloader header is at0x2000, partition table at0x8000, and OTA application headers at0x20000 and0x610000. The three actual images parse as C5 chip23, revision100–199,16MB flash with correct checksums and appended digests. The last assets partition extends to the physical16MiB boundary. The third-party binary is linked by its precise URL/hash and not mirrored in the repository.

An independent native probe placed that unchanged file into16MiB flash, configured8MiB PSRAM, attached the actual board and ran the unmodified public rev100 ROM. It stopped after6,825 instructions at the first unsupported cache read: fault PC0x4002c8ec, address0x600c8004,32-bit read, inside `Cache_Mask_All`. UART output and nonzero display pixels were both0. Raw native placement is explicitly different from acceptance by the public loader, which remains SRAM-only for C5. No host register seed, altered PC or firmware function hook is used.

The factory entry must retain this boundary. The original diagnostic is a separate project firmware option and must not replace or be relabeled as the manufacturer image. The official Xiaozhi release has a separate exact target for this board; its role and unverified status are included in the central integration patch.

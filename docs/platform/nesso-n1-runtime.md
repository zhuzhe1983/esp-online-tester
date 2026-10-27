# Arduino Nesso N1 DK001 runtime investigation

Source checkpoint; the catalog remains unavailable until a real official image produces output. Sources and actual retrieval receipts are in [sources.json](evidence/nesso-n1/sources.json). Third-party binaries and raw fetched originals are temporary ignored inputs, not published assets.

The Arduino schematic TPX00227 and M5Stack product table identify an ESP32-C6 QFN40, 16 MiB flash, ST7789 135×240 display, FT6336 touch, BMI270 IMU, two PI4IOE5V6408 expanders, BQ27220 fuel gauge and AW32001 charger. M5GFX `65c816a38386062d2c3dc47d41237a9083acbc75` and M5Unified `368ad60807161625aa65baad37a4a3af2a7eccea` supply the matching initialization and button scan. These current sources do not imply that every bundled firmware uses the same revision.

| Route | Source-backed wiring |
| --- | --- |
| Display | SPI SCK20 / MOSI21 / CS17 / DC16; 240×320 GRAM with visible x52..186, y40..279 |
| I²C | SDA10 / SCL8; FT6336 0x38, BMI270 0x68, expanders 0x43 / 0x44 |
| Expander 0 | P0 / P1 active-low keys; P5 LNA, P6 antenna switch, P7 LoRa reset |
| Expander 1 | P0 power-off pulse, P1 LCD reset, P2 Grove power, P5 USB VIN detect, P6 backlight, P7 status LED |
| Identification | G18 hard low, G8/G10 pulled high; do not substitute Unit C6L routing |
| LoRa | SX1262 SPI21/22/20, CS23, BUSY19, IRQ15; not yet modeled |

The official M5Burner project `1897200000000002638` declares sourceType OFFICIAL and exact device `ARDUINO_NESSO_N1`. Its UIFlow2 v2.5.3 image is an alternative official firmware, not a verified factory recovery image. SHA-256 `49c4a9a669a2bde5b95416fa19ab8e01fc6bdec6a81bb80f2a0fc07480e1444b`, 16,773,120 bytes: boot image at 0, partition table at 0x8000 and app at 0x10000. The existing factory audit found only a community factory-code entry; its uploader is not verified as the manufacturer, so it remains excluded from the official factory role.

PI4IOE5V6408 register behavior follows Diodes DS40583 Rev 3-5 (September 2022), retained via the manufacturer-linked M5Stack mirror. It has output=1 direction, separate high-impedance and pull controls, read-cleared reset/interrupt status and no burst-read support. It is not interchangeable with TCA9555.

Open upstream PR review: #262 adds the shared C6 board module and is already the ancestor `e5ac3605`; #310 is a separate C6 optimization, not adopted. No open PR found for the Nesso board or PI4IOE5V6408 implementation at this checkpoint. No simulation, physical-hardware, RF, charging, Grove or 1:1 model validation is claimed by this source checkpoint.

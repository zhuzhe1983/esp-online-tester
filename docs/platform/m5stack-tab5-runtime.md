# Tab5 C145: ILI9881C + GT911 digital runtime

The `m5stack-tab5` board runs the ESP32-P4 main controller with 16 MiB Flash,
32 MiB PSRAM and a rev < 3 virtual ABI. It selects the **ILI9881C + GT911 batch**.
ST7121/ST7123 are separate physical variants and are not identified as Goodix or
enabled by this profile. No factory-image or physical-hardware acceptance is claimed.

The earlier [blocker audit](m5stack-tab5-runtime-blockers-20261126.md) remains a
historical baseline. Its missing DSI/DMA blocks are implemented in this revision;
its hardware batch distinctions, official source hashes and unverified analog
boundaries remain relevant. The source contract is M5Tab5-UserDemo
`fe8aa051beb9385468f7e46b909e764169df3f7a`, M5GFX
`28d47949bdc84f30881623476f555398e2194313` and ESP-IDF v5.4.2.
[Additional clock/LDO/PHY/DMA source receipts](evidence/m5stack-tab5-dsi-20261126/source-receipts.json)
retain actual URLs, fetch timestamps, byte counts and SHA-256 values.

| Layer | Implemented path | Limit |
| --- | --- | --- |
| PHY | Host 0x500A0000; test address on falling TESTCLK, data on rising TESTCLK; programmed PLL, lane stop/ULPS status | Ideal 32-cycle lock transition; no analog signal integrity or silicon PLL calibration |
| Supply/clocks | PMU VO3 nominal 2500 mV selector and XPD; SYSCLK/PHY/DPI gates and digital reset | No analog charging, rail transients or brownout timing |
| DCS | Virtual channel 0; short/long packet FIFO, ILI9881C page selection, base-page sleep/on/off/COLMOD/MADCTL | Vendor analog/gamma registers are accepted without optical simulation; unknown read replies are unsupported |
| Pixel stream | DW-GDMA at 0x50081000 consumes actual 64-byte LLIs and guest PSRAM; fixed destination 0x50105000; bridge 0x500A0800 publishes completed frames | Memory-to-DSI flow, bounded descriptors/bytes; no CSI/ISP, ECC injection or arbitration timing |
| Drawing | Shared DMA2D at 0x50088000 and PPA at 0x50087000 use the P4 MMU-backed DMA view | Supported shared copy/PPA operations only; unsupported conversions are refused; 32 MiB service-pass bound |
| Board | PI4IO 0x43/0x44 on physical SDA31/SCL32, LCD reset P4 and touch reset P5 | LCD reset releases through input pull-up, not a fabricated high output; charging/USB/C6 nets are not emulated merely by register writes |
| Touch | GT911 at 0x14; ready/count 0x814E, coordinates 0x8150, report acknowledgement, GPIO23 level | Single contact, ideal 10 ms scanning; address strap changes and full Goodix configuration/firmware are not modeled |
| Visibility | Physical GPIO22 or routed LEDC backlight, panel sleep/off/reset and DSI link state | No calibrated optical brightness curve |

`esp32p4/tests/tab5.rs` executes original RV32 instructions to generate all
921,600 RGB565 pixels, copy the 1,843,200-byte drawing buffer through DMA2D,
then scan it through the DW descriptor and DSI bridge. The test compares all
pixels through a full-frame FNV hash and four distinct corners. A real GT911
I2C report is read by the guest and changes a pixel in its next DMA frame.
The negative probes cover wrong SDA, TP reset/NACK, bad DMA memory, invalid
LLI ownership, disabled LDO3, panel sleep, backlight and MCU reset. A host
framebuffer fixture cannot satisfy those guest instruction and DMA checks.
After link loss or panel reset, retained pixels remain hidden until a complete
fresh DMA frame arrives. Restoring the link, toggling the backlight or presenting
a short/invalid frame cannot revive stale output. Backlight changes on a still
active link retain the last complete frame.

Reproduce the native probes with:

```sh
cargo test -p esp32p4 --test tab5
```

After rebuilding the production WASM, `node tools/probe-tab5.mjs` runs the
same classes of guest operations through the platform API and actual worker,
checks a complete 720×1280 RGBA SHA-256, injects touch and checks the second
frame, then recreates the machine through platform reset. The 7,888-byte
original SRAM application is generated deterministically in memory by
`tools/fixtures/tab5-guest.mjs`; it is not an official binary or a ROM-boot test.
The diagnostic polls the read-only PHY lock before sending DCS commands.
Production Worker/WASM020 (`0cc18d96042c0b2707afbe3b80102cabcbdd6868fccfcc84504cabed8b3179a2`)
passed the complete-frame, touch, pause and reset checks. Four separately
generated faulty guests use SDA30, lose LDO3, supply an unmapped DMA source
or clear LLI validity; each publishes zero accepted panel frames and an
entirely black RGBA surface. The [actual Worker receipt](evidence/m5stack-p4-worker-20261126/tab5-worker020.json)
records exact guest, harness, fixture-module and Worker hashes. This remains
original SRAM diagnostic acceptance, with final released-batch verification separate.

Camera/CSI, ES8388/ES7210 audio, BMI270, RX8130CE, INA226, PMS150G/charging,
USB applications, the Tab5 SD socket and ESP-Hosted/C6 remain outside this
verified subset. The prepared TF card remains local when no verified guest
transport is attached. Official factory and UIFlow entries retain unverified
trial status and remote-only original download sources.

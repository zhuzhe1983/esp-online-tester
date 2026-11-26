# M5Stack classic ESP32 cameras: digital capture subset

Fourteen camera SKUs now have independent runnable board names and `partial` support. The sensor is configured by actual LX6 instructions through the hardware I2C/SCCB controller. LEDC supplies XCLK through the physical GPIO matrix; GPIO15 controls hardware RESET. Sensor bytes pass through the existing classic I2S0 camera receiver, FIFO packing and native DMA descriptors into guest DRAM, with native EOF and descriptor ownership updates. No host framebuffer, UART text or catalog flag substitutes for capture.

| Product | Sensor | SDA / SCL | D0 | VSYNC | Flash / PSRAM MiB |
| --- | --- | --- | --- | --- | --- |
| U017, default A | OV2640 | 25 / 23 | 32 | 22 | 4 / 4 |
| U017-PCBA | OV2640 | 22 / 23 | 32 | 25 | 4 / 4 |
| U038 | OV2640 | 22 / 23 | 32 | 25 | 4 / 4 |
| U037, original document identity | OV2640 | 22 / 23 | 32 | 25 | 4 / 4 |
| U007 | OV2640 | 25 / 23 | 17 | 22 | 4 / unknown |
| U109 | OV2640 | 25 / 23 | 32 | 22 | 4 / unknown |
| U109-X | OV2640 | 25 / 23 | 32 | 22 | 4 / unknown |
| U109-B | OV2640 | 25 / 23 | 32 | 22 | 4 / unknown |
| U082 | OV3660 | 25 / 23 | 32 | 22 | 4 / 8 |
| U082-X | OV3660 | 25 / 23 | 32 | 22 | 4 / 8 |
| U082-F | OV3660 | 25 / 23 | 32 | 22 | 4 / 8 |
| U121 | OV2640 | 14 / 12 | 32 | 22 | 16 / 8 |
| U121-B | OV2640 | 14 / 12 | 32 | 22 | 16 / 8 |
| U121-B-V11 | OV3660 | 14 / 12 | 32 | 22 | 16 / 8 |

Every row has D1..D7 = 35/34/5/39/18/36/19, PCLK21, HREF26, XCLK27 and RESET15. PWDN has no MCU GPIO; the documented pull-down/no-wire connection is represented explicitly. It is never interpreted as GPIO0 or GPIO255. Unknown PSRAM stays unknown in the physical knowledge: the four corresponding runtime configurations use zero PSRAM conservatively for diagnostics.

U017's manufacturer document distinguishes A and B hardware and identifies the pictured quick-start card as A. The catalog explicitly selects A; `m5stack-u017-b` selects B with SDA22/VSYNC25. It is a separate simulator selection, without automatic pin probing or a claim that all U017 units are A. U017-PCBA's own document specifies SDA22/VSYNC25. Original U007 uses D0=17. U121-B retains OV2640, while only U121-B-V11 changes to OV3660. The latter's own schematic FPC U4 independently confirms its camera wiring and R31 10k PWDN pull-down. U037's old 160° and redirected New 150° optical/factory identities remain unresolved and separate; no optical or factory acceptance is inherited.

RGB565 and YUYV use a host picture converted according to the sensor's actual SCCB configuration. JPEG uses a separately supplied baseline JPEG with exactly the configured dimensions, delivered verbatim through the FIFO and DMA. There is no JPEG encoder and a raw picture does not become a fabricated sensor JPEG. The host preview is independent of guest capture. Other wire formats are withheld.

The source-backed scheduler reuses the existing 10Hz frame / 1ms VSYNC approximation. This is not a PCLK, sensor PLL or silicon timing claim. Undriven RESET is treated as released, an explicitly inferred interface policy. Physical power is an externally supplied digital subset; battery discharge, power-hold circuitry, PoE analog conversion, RTC/alarm/wake, status LEDs, optics/exposure/ISP and radio behavior are outside this receipt. The cold camera_supply_at input represents a discrete sensor rail fault: SCCB and capture stop, geometry is lost, and power return requires guest reinitialization. It is not an analog or MCU power model.

`esp32/tests/m5_camera.rs` runs ten tests, with camera checks over all fifteen board selections. Every SCCB, GPIO, LEDC, descriptor, I2S and memory operation executes an LX6 `s32i`/`l32i` instruction. The tests assert red RGB565 DMA bytes and verbatim JPEG bytes in guest DRAM, native EOF, descriptor ownership, chip IDs, wrong address and swapped SCCB NACKs, asserted RESET NACK and discarded geometry, each of eleven wrong DVP wires, missing XCLK, sensor standby, sensor supply loss/restoration, MCU reboot with a new green source, and fresh-platform loss of the previous host picture. All ten tests pass; targeted stable Clippy passes with warnings denied. These are peripheral/instruction tests, not complete ROM boot, an unmodified public camera driver run, factory application compatibility or physical hardware validation.


The three PoE cameras additionally have a real W5500 SPI **Socket0 MACRAW** subset at MOSI13/MISO38/SCLK23/CS4 and active-low INT2. RSTn has no MCU GPIO. VDM/FDM headers, common/socket registers, readonly version, RX/TX ring memory, 16-bit pointers, SEND/RECV, IRQ masks and W1C, filtering, overflow, software reset and PHY reset follow the WIZnet v1.1.0 datasheet. Native SPI command/address phases and full/half duplex use a cold classic-controller phase snapshot with physical hardware/software CS; byte counts are not guessed. Both a full-duplex FIFO header and a 16-bit command plus 8-bit address with hardware CS execute through actual guest instructions.

Raw frames reach the independent wired EthernetLink: its default virtual peer answers a guest ARP request and an optional host relay delivers original TX/RX frames. Wi-Fi association and its TX path are unused. TX and RX tests cross the 2KiB ring end; RECV must commit read pointers before space is released. Three chip tests and three old classic SPI regressions also pass. Unsupported TCP modes remain closed, wrong pins/CS return 0xff, PHY reset blocks TX and MR reset clears protocol state. MCU reset removes SPI routes while retaining separately powered W5500 state. TCP/UDP hardwired socket offload, PPPoE, physical Ethernet/FCS, INTLEVEL pulse timing and analog PHY/PoE remain unsupported. Factory/SDK network compatibility is not inferred.

The [structured report](research/m5-classic-camera-runtime.json) records the exact source URLs, resolved URLs, UTC fetch times, media types, sizes and SHA-256 for fourteen actual product-page fetches and the V1.1 schematic, plus fixed WIZnet/IDF SPI and driver receipts, board identities, capacities, policies, test source hashes and outcomes. These receipts retain metadata, not original source bodies. Existing source/model receipts and factory investigations remain historical evidence. No manufacturer binary was downloaded or mirrored in this pass. All existing factory/reference roles, fixed remote URLs, defaults and `simulationVerified: false` are retained.

Reproduce the focused checks:

```sh
CARGO_BUILD_JOBS=2 cargo test -p esp32 --test m5_camera -- --nocapture
CARGO_BUILD_JOBS=2 cargo clippy -p esp32 --test m5_camera -- -D warnings
node --test tools/platform-m5-classic-camera-test.mjs
```

The production runtime/Worker/WASM path was then checked separately on batch 020 (WASM SHA-256 `0cc18d96042c0b2707afbe3b80102cabcbdd6868fccfcc84504cabed8b3179a2`, Rust snapshot `96dc157b`). A PoE V1.1 smoke and all fourteen exact SKUs passed their first attempts. Each original LX6 SRAM diagnostic is loaded from Flash0x1000 by the unmodified mask ROM, without stubs or app-direct boot. Guest comparisons check sixteen red and sixteen green RGB565 words, swapped SCCB NACK, wrong-D0 untouched DMA and RESET/NACK/no-DMA/reinitialization. All three PoE probes additionally compare W5500 FIFO and hardware command/address SPI reads, every byte of a 44-byte MACRAW ARP RX record, GPIO2 IRQ and RECV release. Explicit host pictures are sensor inputs; guest MMIO, native DMA and memory comparisons decide acceptance.

The [batch-020 receipts](evidence/m5-classic-camera-020-20261126/verification.json) retain per-product original probe hashes, ROM/WASM and harness hashes, serial output and host-input hashes. There were no acceptance failures or implementation fixes. This is original diagnostic execution, not manufacturer/SDK/factory application compatibility, physical validation or public deployment acceptance. Existing factory trial flags remain unchanged. The model geometry, photos, licensing, source transforms and measured deviations are unchanged.

Reproduce with production artifacts (paths are selected explicitly):

```sh
ESP32SIM_WASM=/path/to/esp32sim.wasm ESP32SIM_ROM_DIR=/path/to/fw M5_CAMERA_WORKER_RECEIPT=/tmp/camera-worker.json node --test tools/platform-m5-classic-camera-worker-test.mjs
```

Set `M5_CAMERA_WORKER_PRODUCTS=m5stack-u121-b-v11` for the single-product smoke; omit it for all fourteen.

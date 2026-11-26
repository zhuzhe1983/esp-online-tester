# Nine exact-SKU public runtime execution probes

Base platform tree: `9d1caa961e04e2f5e8b604dd20d2df2ebf0227f2`. Only documentation and test tools change. The harness constructs each exact catalog SKU independently through `createPlatformRuntime`, loads the original production `web/wasm/worker.js`, and supplies the immutable `m5-batch020.wasm` bytes. No emulator or Worker stub, host-drawn framebuffer, UART input substitution, Cargo build or factory-image substitution is used.

The fixed WASM SHA-256 is `0cc18d96042c0b2707afbe3b80102cabcbdd6868fccfcc84504cabed8b3179a2`. The harness requires this exact artifact and rechecks its bytes after execution. Each receipt additionally binds the harness, authored probe source, runtime, production Worker, catalog, ROM and generated diagnostic image hashes.

| Exact catalog SKU | Board | Flash / PSRAM MiB | Executed input and output |
| --- | --- | --- | --- |
| `m5stack-k002` | `m5stack-basic` | 16 / 0 | A39, B38, C37 → guest UART and ILI9342C pixel |
| `m5stack-k001-v26` | `m5stack-basic` | 16 / 0 | A39, B38, C37 → guest UART and ILI9342C pixel |
| `m5stack-k001-b` | `m5stack-basic` | 16 / 0 | A39, B38, C37 → guest UART and ILI9342C pixel |
| `m5stack-c001-cnc` | `m5stack-basic` | 16 / 0 | A39, B38, C37 → guest UART and ILI9342C pixel |
| `m5stack-k022` | `m5stack-basic` | 16 / 0 | A39, B38, C37 → guest UART and ILI9342C pixel |
| `m5stack-k006` | `m5stack-basic` | 16 / 0 | A39, B38, C37 → guest UART and ILI9342C pixel |
| `m5stack-k007` | `m5stack-basic` | 16 / 8 | A39, B38, C37 → guest UART and ILI9342C pixel |
| `m5stack-k007-v26` | `m5stack-basic` | 16 / 8 | A39, B38, C37 → guest UART and ILI9342C pixel |
| `m5stack-k152` | `cardputer-adv` | 8 / 0 | All 56 physical matrix positions → TCA8418 FIFO UART bytes and ST7789V2 pixel |

Flash/PSRAM values are the exact selected runtime configuration observed at every real Worker create, not a new memory-chip measurement or PSRAM traffic acceptance. Shared board profiles do not copy factory firmware identity or validate other kit peripherals.

## Reproduce

Provide the immutable WASM and original ROM ELF files explicitly; no third-party firmware download is needed:

```sh
ESP32SIM_WASM=/path/to/m5-batch020.wasm \
ESP32SIM_ROM_DIR=/path/to/original-roms \
M5_BASIC_MESH_RECEIPT=/tmp/basic-eight.json \
M5_CARDPUTER_MESH_RECEIPT=/tmp/cardputer-mesh.json \
node --test tools/platform-m5-basic-mesh-worker-test.mjs
```

The ROM directory must contain `esp32_rev300_rom.elf` and `esp32s3_rev0_rom.elf`. `M5_BASIC_MESH_IDS` optionally selects a comma-separated subset of the nine IDs. Receipts are written only after the selected product assertions pass. Test-owned runtime instances and in-memory Flash/SD stores are disposed without clearing user storage.

## Classic Basic path

The existing `m5-classic-display-probe.mjs` is extended with an explicit `m5stack-basic` layout and a GPIO-input-bank mask for physical A/B/C. Existing six layouts and thermal probes remain byte-identical to the base fixture (see `classic-fixture-compatibility.json`). The program is loaded by the original classic ESP32 mask ROM as a diagnostic bootloader at Flash0x1000; a real diagnostic partition table and app satisfy the public firmware bundle contract. `appDirect` is false and the stub list is empty. This is an authored SRAM diagnostic rather than full SDK or factory boot acceptance.

For each SKU, actual LX6 instructions initialize the panel through CLK18, MOSI23, CS14, D/C27, RST33 and backlight32. The guest enables and samples GPIO39/38/37. Each of A/B/C is pressed and released separately via public input. Fresh guest UART `BUTTON DOWN/UP` plus `FRAME COMPLETE` must accompany a real 320×240 framebuffer pixel `(0,0)` changing from red `[255,0,0,255]` to green `[0,255,0,255]` and back. The receipt records every input, pixel and whole-frame SHA-256.

While A is physically held down, public `reset()` must reload/re-execute the program and emit a fresh READY/UP sequence with a red pixel. A wrong-route diagnostic keeps the SPI MOSI signal but sends it on physical GPIO22 instead of23. The real guest still consumes a fresh A press and prints DOWN/FRAME COMPLETE, while every received display RGB channel remains black. After at least0.1 additional guest second, absence of a frame is also checked explicitly through the public `capture()` error `NO_FRAME`; the receipt distinguishes actual black output from absent output. No fallback image supplies evidence. Thus UART execution and accepted input are separately checked from the physical display route.

## K152 Adv-host path

`m5-cardputer-mesh-input-probe.mjs` is an explicit authored LX7 appDirect SRAM probe with an 8 MiB image header, not a manufacturer or mask-ROM boot image. It does not alter K152's product ID, runtime memory or firmware target. The original S3 ROM is mapped by the public loader; the probe bypasses its boot path and this boundary is explicit in the receipt.

The guest routes SPI3 CLK36/MOSI35/CS37 (signals66/68/71), D/C34 and RST33, then programs ST7789 RGB565. Controller coordinate column186,row40 maps to physical glass `(0,0)` in the 240×135 aperture. Guest I2C0 uses SDA8/SCL9 (signals90/89), address0x34, keypad row/column enables `1Dh=7Fh`, `1Eh=FFh` and `01h=01h`. It reads real event count `03h`, event FIFO `04h`, and acknowledges W1C interrupt `02h`. UART hex formatting and the pressed/released pixel decision execute entirely in the guest.

All 56 catalog positions are independently pressed and released. The expected manufacturer physical remap is `key = floor(column/2)*10 + row + 4*(column&1) + 1`, with bit7 indicating press. The host compares each raw guest-read FIFO byte and real green/red pixel; the probe itself does not calculate this expected mapping. This covers 112 transitions, including exact Enter `(2,13) → C3h/43h`.

Enter is held during public reset. Fresh READY/KEY NONE/red output must reappear, followed by another successful Enter down/up pair. Two independent negative diagnostics retain actual guest execution: wrong display CLK40 instead of36 still produces the real Enter FIFO UART byte but the real display output is black or explicitly absent (`NO_FRAME`); moving the already-configured TCA8418 SDA input/output matrix to GPIO10 produces a guest-observed I2C NACK. A public Enter input is delivered, at least0.1 additional guest second executes, no FIFO event is read, and the pixel stays red.

## Sources and scope

`sources.json` retains exact-SKU knowledge-source receipts with their original retrieval dates, plus hashes of the local primary implementation/test contracts. No new retrieval dates are assigned to old sources. The original M5Stack product pages, Core schematic, Cardputer-Adv pin table, official `M5Cardputer-UserDemo` keyboard remap, TI TCA8418 and Sitronix ST7789V2 documents are the primary hardware basis. The following local primary contracts supply the independently executable register/ISA checks:

- `esp32/src/board/m5stack_basic.rs` and `esp32/tests/m5_classic_paper.rs`: documented Core GPIOs and ILI9342C path. The existing classic fixture retains its original ESP32 register encoding.
- `esp32s3/tests/cardputer_adv_keys.rs`: ESP-IDF v5.4.2 GPIO signal map (I2C0 SDA90/SCL89), actual hardware controller/FIFO registers and manufacturer physical key mapping.
- `esp32s3/src/board/cardputer_adv.rs` and `cardputer_lcd.rs`: independent physical routing, TCA identity, original ST7789 aperture and coordinates.
- `esp-periph/src/i2c.rs`: ESP-IDF v5.5.4 S3 I2C register contract, TRANS_START bit5, FIFO reset12/13, command opcodes, TRANS_COMPLETE bit7 and NACK bit10. The used fields agree with the v5.4.2 controller tests; version scopes are retained separately.
- `xtensa-lx7/src/jit/wasm_tests/asm.rs`, `m5-camera-capture-probe.mjs` and `m5-vameter-card-input-probe.mjs`: checked encoded LX7 instructions, guest hex printing and SPI3 register sequence.

These are Node public-runtime/production-Worker execution probes, with exact guest UART and framebuffer assertions. They are not browser GPU/3D, factory application, RF/Meshtastic/LoRa/GPS, full audio/IMU, TF transport, analog power, memory-capacity traffic or physical-device acceptance. Basic variants' RGB peripherals and K152's G0 button remain outside this probe's accepted scope.

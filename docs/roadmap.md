# Roadmap

Ordered by value; each item links to its plan where one exists.

1. **WiFi (full, unmodified)** — done: open and WPA2-PSK both associate through the closed blob,
   and outbound TCP/UDP reaches the real network through a user-mode NAT, TLS included
   ([wifi-plan.md](wifi-plan.md), [networking-plan.md](networking-plan.md)). Remaining: inbound
   port forwarding (autopling's web UI from the Mac), multicast/mDNS, and a `--net tap` backend
   for real LAN presence.
2. **Testing** — hermetic CPU/SoC/board suites, conformance firmware, CI tiers
   ([testing-plan.md](testing-plan.md)). Milestone 1 (no silent skips, shared CPU harness,
   parser robustness) first.
3. **Firmware upload from the browser** — drop a `firmware.bin` on the page → written to
   flash at 0x10000 → `Machine::reboot()`. Pieces exist; ~an hour.
4. **Interpreter speed** — see [speed-plan.md](speed-plan.md). The block interpreter and an
   AArch64 JIT are in (SID player 93 → 193 Minsn/s, 2.1× real time; Atech 210; every output
   bit-identical with `--no-jit`). Left inside the JIT: inline TLB for loads/stores (35 % of
   time), register caching, inlined call/entry/retw; then NEON for the PIE lanes and the wasm
   backend for the browser build. `tools/bench.py` is the yardstick.
5. **ESP32-C3 (RISC-V)** — a draft is in ([esp32c3.md](esp32c3.md)): RV32IMC decoder verified
   against objdump, the C3 memory map and interrupt matrix, and unmodified IDF firmware booting
   from the mask ROM through the bootloader into FreeRTOS and `app_main` — verified against a
   real C3 module, 205/208 console lines identical over three boots. Left: `--boot app`, more
   peripherals on demand, WiFi/BLE. (The two CLIs are one binary now: `--chip`.)
5b. **ESP32-C6 (RISC-V, RV32IMAC)** — same shape ([esp32c6.md](esp32c6.md)): the A extension,
   the C6's unified map and MMU, PLIC/INTPRI, PCR and the LP blocks; hello_world verified against
   a Waveshare ESP32-C6-LCD-1.47, 203/204 console lines identical over three boots. The board is
   in (`waveshare-c6-lcd147`: ST7789 on SPI2+GDMA, WS2812 on the C6 RMT, BOOT button) and its
   802.15.4 energy-scan firmware runs with a synthetic spectrum from the MAC model. A second
   board is in (`xiao-esp32c6`: low-active user LED on GPIO 15, BOOT button, RF switch). Left: TF
   card, I2C on the C6 layout, watchdogs, real radios.
6. **More boards** — Touch-LCD-4B done (`waveshare-lcd4b`: LVGL panel, touch/swipe, SID player audio).
   Next candidates as firmware needs them; `--board waveshare-*` variants share the codec/PSRAM/I2C work.
7. **Peripherals on demand** — LEDC, PCNT, ADC, SPI2/3 masters, RX sides of I2S/RMT/UART DMA,
   LCD side of LCD_CAM. Each appears as "unknown register" in the log when a firmware needs it.
8. **PIE completeness** — FFT, GPIO and s32 instruction groups (decoded, not executed).
9. **Browser build (WebAssembly)** — done ([wasm.md](wasm.md)): the emulator in the page,
   hello_world / the panel with SID / Atech at real time in Chrome. Left: a WebSocket relay so
   the guest reaches the real network from a tab, and a wasm backend for the JIT.
10. **Packaging** — `cargo install esp32sim`, a `--net`/`--board` aware `examples/` runner,
   release binaries for macOS/Linux.
11. **ESP8266 (Xtensa LX106)** — todo ([#214](https://github.com/mikroverk/esp32sim/issues/214)):
   the call0 subset of the LX7 decoder, the ESP8266 ROM and memory map, UART/GPIO/FRC timers/HSPI,
   then its own Wi-Fi MAC model, since the SDK drives different hardware than the ESP32.
12. **ESP32-P4 (RISC-V, dual HP core)** — in progress ([#215](https://github.com/mikroverk/esp32sim/issues/215)):
   F, CLIC, L2MEM/PSRAM, SDMMC, USB-OTG and Wi-Fi from an
   emulated C6 over ESP-Hosted (`--coproc`) are there; Xesppie, MIPI-DSI and DMA2D/PPA for the
   display boards it ships on are not.

Not planned: blob-level WiFi/BLE emulation, cache-timing accuracy, Wokwi/cloud integration.

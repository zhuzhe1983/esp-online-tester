# Roadmap

Ordered by value; each item links to its plan where one exists.

The work ahead is grouped in [GitHub milestones](https://github.com/mikroverk/esp32sim/milestones),
with tracking issues where available that list items and what "done" means:

- [Real time in the browser](https://github.com/mikroverk/esp32sim/milestone/1) — #302
- [Wi-Fi complete](https://github.com/mikroverk/esp32sim/milestone/2) — #303
- [BLE and Web Bluetooth](https://github.com/mikroverk/esp32sim/milestone/3) — #304
- [Multiple boards and ESP-NOW](https://github.com/mikroverk/esp32sim/milestone/4) — #305
- [ESP-NOW swarm demo](https://github.com/mikroverk/esp32sim/milestone/5) — #306
- [New chips](https://github.com/mikroverk/esp32sim/milestone/6) — #214, #215
- [Boards and integrations](https://github.com/mikroverk/esp32sim/milestone/7)
- [Maintenance: fixes, compatibility, docs](https://github.com/mikroverk/esp32sim/milestone/8)

1. **WiFi (full, unmodified)** — done: open and WPA2-PSK both associate through the closed blob,
   and outbound TCP/UDP reaches the real network through a user-mode NAT, TLS included
   ([wifi-plan.md](wifi-plan.md), [networking-plan.md](networking-plan.md)). Remaining: UDP inbound,
   multicast/mDNS, and a `--net tap` backend for real LAN presence. TCP inbound is `--hostfwd`.
2. **Testing** — hermetic CPU/SoC/board suites, conformance firmware, CI tiers
   ([testing-plan.md](testing-plan.md)). Milestone 1 (no silent skips, shared CPU harness,
   parser robustness) first. CI runs the goldens on x86 and on AArch64 with the native JIT, and
   the CLI and every test find the ROMs the same way (`esp_soc::rom`).
3. **Firmware upload from the browser** — done: the page's firmware dialog boots your own
   bootloader, partition table and app (plus ELF symbols and an action script) on any chip,
   without a reload. Left: dropping a new `app.bin` onto a running board.
4. **Interpreter speed** — see [speed-plan.md](speed-plan.md). The block interpreter and an
   AArch64 JIT are in (SID player 93 → 193 Minsn/s, 2.1× real time; Atech 210; every output
   bit-identical with `--no-jit`). The inline TLB for loads/stores and the wasm backend for the
   browser build are in too. Left inside the JIT: register caching, inlined call/entry/retw; then
   NEON for the PIE lanes. `tools/bench.py` is the yardstick. The RISC-V chips (C3, C6, P4)
   still fetch and decode every instruction, though the C6's fetch and both RV32 decoders are
   inlined into the step since #310. A per-PC decode memo did not pay (EX249 in #310:
   about as costly per hit as decoding), so their next step is a decoded-block cache like the
   LX7's ([EX001](experiments.md#ex001)/[EX003](experiments.md#ex003)), behind the same
   page-version checks.
5. **ESP32-C3 (RISC-V)** — a draft is in ([esp32c3.md](esp32c3.md)): RV32IMC decoder verified
   against objdump, the C3 memory map and interrupt matrix, and unmodified IDF firmware booting
   from the mask ROM through the bootloader into FreeRTOS and `app_main` — verified against a
   real C3 module, 205/208 console lines identical over three boots. WiFi and BLE are in (the
   `wifi-station-c3` and `ble-*-c3` goldens). Left: `--boot app`, more peripherals on demand.
   (The CLIs are one binary now: `--chip`.)
5b. **ESP32-C6 (RISC-V, RV32IMAC)** — same shape ([esp32c6.md](esp32c6.md)): the A extension,
   the C6's unified map and MMU, PLIC/INTPRI, PCR and the LP blocks; hello_world verified against
   a Waveshare ESP32-C6-LCD-1.47, 203/204 console lines identical over three boots. The board is
   in (`waveshare-c6-lcd147`: ST7789 on SPI2+GDMA, WS2812 on the C6 RMT, BOOT button) and its
   802.15.4 energy-scan firmware runs with a synthetic spectrum from the MAC model. Left: TF card,
   I2C on the C6 layout, watchdogs, real radios.
6. **More boards** — board models for the S3, C6, P4 and classic ESP32 are in; [boards.md](boards.md)
   lists them and how to add one. The latest are the Ulanzi TC001 (#311) and the AI-Thinker
   ESP32-CAM with host JPEG (#328). Next candidates as firmware needs them; `--board waveshare-*`
   variants share the codec/PSRAM/I2C work.
7. **Peripherals on demand** — LEDC, PCNT, ADC, SPI2/3 masters, I2S RX and the LCD side of
   LCD_CAM are in ([peripherals.md](peripherals.md)). Left: the RX sides of RMT and UART DMA, TWAI,
   LCD_CAM's i8080 mode. Each appears as "unknown register" in the log when a firmware needs it.
8. **PIE completeness** — FFT, GPIO and s32 instruction groups (decoded, not executed).
9. **Browser build (WebAssembly)** — done ([wasm.md](wasm.md)): the emulator in the page,
   hello_world / the panel with SID / Atech at real time in Chrome, with a wasm backend for the
   JIT. Left: a WebSocket relay so the guest reaches the real network from a tab (the frame
   transport is there: `esp32sim_ethernet_relay`).
10. **Packaging** — release tarballs for macOS and Linux are built by
   `.github/workflows/release.yml`. Left: `cargo install esp32sim`, a `--net`/`--board` aware
   `examples/` runner.
11. **ESP8266 (Xtensa LX106)** — todo ([#214](https://github.com/mikroverk/esp32sim/issues/214)):
   the call0 subset of the LX7 decoder, the ESP8266 ROM and memory map, UART/GPIO/FRC timers/HSPI,
   then its own Wi-Fi MAC model, since the SDK drives different hardware than the ESP32.
12. **ESP32-P4 (RISC-V, dual HP core)** — in progress ([#215](https://github.com/mikroverk/esp32sim/issues/215)):
   F, CLIC, L2MEM/PSRAM, SDMMC, USB-OTG and Wi-Fi from an
   emulated C6 over ESP-Hosted (`--coproc`) are there; Xesppie, MIPI-DSI and DMA2D/PPA for the
   display boards it ships on are not.
13. **One model per IP block** — chips were added by copying their nearest sibling, and the
   copies drift. Done: the AES DMA of the C3 and C6 through the shared GDMA walker, one GDMA
   adapter for the C3/C6 layouts, the reboot survivors of the classic ESP32 and S3 brought in
   line, the RMT capture bounded (#326); one WiFi MAC model for the S3 and C3, with the EX238
   receive hand-over on both (#327). Left: the classic and C6 MACs on that model (both still
   lack the hand-over); before the next chip, a board/chip registry instead of hand-kept lists,
   default `SocBus` methods, reboot survivors and register reset values declared by each device,
   and one interrupt-routing block; one crate that builds machines for the CLI and the browser
   (groundwork for several boards in lock-step); usage errors instead of panics in the CLI.

Not planned: blob-level WiFi/BLE emulation, cache-timing accuracy, Wokwi/cloud integration.

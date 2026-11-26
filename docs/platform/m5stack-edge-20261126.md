# M5Stack edge controllers: 2026-11-26

Baseline: `81bc93c0333a9fd13c16565b245b3d1c0d7ce237`. This batch handles exactly five product identities. Source receipts, pinned M5GFX board specifications, firmware investigation and final asset hashes are in [the evidence directory](evidence/m5stack-edge-20261126/).

| Product | Concrete change | Remaining boundary |
| --- | --- | --- |
| NanoH2 C149 | New independent ESP32-H2 board, G9 key, G10 RGB supply, G11 RMT RGB, G4 blue output; 3D front key and light inside the actual CAD slit | SRAM app-direct only. No ROM/Flash boot, USB, radio or IR. No factory image execution. |
| Unit C6L U202 | Own SSD1306 64×48 SPI/software GPIO display, reset and RAM column offset 32; P0 expander key, native side actuator, G2 RMT RGB; actual S012 digital LoRa transport retained | No RF propagation, analog optics, buzzer, USB or a second guest CPU when used as an accessory. Factory/UIFlow image not booted. |
| PowerHub C148 | Exact 16 MB/8 MB ESP32-S3 board with physical G11 user key and native CAD actuator | STM32G031, power buttons, RGB, rails, current monitors, RTC, CAN/RS485 and USB switching remain absent. This is a CPU/key subset. |
| PowerHub Kit K148 | Separate official kit identity; explicit firmwareTargetId shares the C148 host directory | Host preview is explicitly battery-uninstalled. No fabricated NP-F550 geometry, charging or capacity model. |
| Tab5 C145 | Correct base SKU, sourced SDIO/SPI/RS485/I²C data; exterior inspection exposes the UNKNOWN board without scaling or moving its placeholder | Remains unavailable: the existing P4 CPU is insufficient without Tab5 DSI/power/touch devices. Inspection is a viewing displacement, not a verified repair sequence. |

The NanoH2 native test runs a newly authored SRAM guest through the actual H2 CPU/MMIO/RMT path and observes UART and RGB output. Unit C6L protocol tests check the wrong SPI route, hardware reset, guest OLED command/data bytes, P0 input registers and incorrect RGB pin. PowerHub tests reject fabricated PMU control mappings. Model tests load the actual exported assets and check the nearest front-button hit, the light's visibility through the CAD slit, shell-color isolation, inspection and exact reset.

No manufacturer firmware bytes were mirrored or substituted. Existing pinned factory-first defaults are retained. NanoH2 has a source-only reference because the fetched exact product page does not identify a fixed complete factory image. The K148 firmware share does not transfer a factory execution or hardware validation claim.

Validation: 4 native Rust tests, 11 platform/model/image tests, catalog schema/identity checks, all 687 knowledge records and the configured evidence privacy scan passed. Browser rendering, production WASM integration, full photo overlays and physical hardware verification remain separate pending checks. Static source facts do not establish simulated electrical behavior.

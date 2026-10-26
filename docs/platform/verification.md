# Foundation verification

Date: 2026-10-26. Upstream base: `ddf753a5c400ff79d3aa6a88a007eccd044e838d` from `mikroverk/esp32sim`. Environment: macOS arm64, Rust stable 1.99.1, Node 22.23.1, headless Chromium. Firmware: platform diagnostic v1.0.0 built with ESP-IDF v5.5.1. Binary hashes are in `web/wasm/fw/public/platform-stopwatch.sha256`; manifest pins are checked by the platform tests.

| Check | Result |
| --- | --- |
| `cargo clippy --workspace --all-targets -- -D warnings` | Pass |
| `cargo clippy --release --target wasm32-unknown-unknown -p esp32sim-wasm --features jit-tests -- -D warnings` | Pass |
| Workspace release tests, `--include-ignored --skip external_` | 1,322 tests passed, zero failures; use an absolute `ESP32SIM_ROM_DIR` |
| Production WASM build and section check | Pass; no DWARF, name section retained |
| WASM hello, C3/C6 hello, C6 energy scan/Contiki/network/RPL, panel and StopWatch manifests | All pass |
| Platform configuration | 10 behavior groups: catalogs, hashes, validation, canonical shares, private persistence |
| Platform runtime | Real worker/WASM boot, pause/resume, stale callback exclusion, reset, display, buttons, touch, complete RMC/GGA, local merged flash, input boundaries, hash errors and disposal |
| Human input controller | 10 injected-clock checks, including minimum short press and session replacement |
| Display texture lifecycle | 4 checks; StopWatch ↔ PaperMono browser roundtrip has no console warnings/errors |
| Existing worker pacing/channel and touch/StopWatch UI tests | Pass |
| Skill frontmatter and references | Validated |

Browser checks cover desktop playback, 390px mobile layout without horizontal overflow, a real 468×466 PNG capture, short mouse clicks, remapping A to Space and delivering a short key press, GPS serial echo, private DIY creation and scene URL roundtrip, PaperMono execution being disabled, parent RPC boot/control, and a cross-origin handshake between distinct loopback origins. Browser screenshots are local review artifacts under ignored `output/playwright/`; they are not physical-hardware evidence.

The diagnostic produces `PLATFORM_READY stopwatch`, `BUTTON_A running`, `BUTTON_B reset`, `TOUCH down`, `GPS_RX $GPRMC…` and `GPS_RX $GPGGA…` through actual emulated firmware. It has not been run on the owner's physical hardware. These checks establish the documented diagnostic path, not compatibility with arbitrary StopWatch or PaperMono firmware, GPS pin routing, baud fidelity or electrical behavior.

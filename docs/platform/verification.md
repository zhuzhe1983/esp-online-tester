# Foundation verification

The report below preserves the initial two-product diagnostic baseline. It is historical, including the statement that PaperMono execution was disabled. The 2026-10-27 catalog has 83 controller SKU/revision entries: 6 partial executable entries (StopWatch, PaperMono, PaperMono Lite, Cardputer-Adv, Cardputer and Cardputer v1.1) and 77 unavailable board runtimes. This is not 83 compatible devices.

Current real-firmware results and limitations are in [PaperMono](papermono-runtime.md), [Cardputer-Adv](cardputer-adv-runtime.md), [original Cardputer](cardputer-runtime.md), and [device storage](device-storage.md). The latest core gates passed both Clippy commands, 1,348 release Rust tests and eight WASM fixture runs; platform checks passed all 20 reported tests. Manufacturer model reports preserve dimensional/photo mismatches and unknown internals. Faces Kit has metadata only, with no substitute model. No physical-device validation is implied.

Date: 2026-10-26. Upstream base: `ddf753a5c400ff79d3aa6a88a007eccd044e838d` from `mikroverk/esp32sim`. Environment: macOS arm64, Rust stable 1.99.1, Node 22.23.1, headless Chromium. Firmware: internal test diagnostic v1.0.0 built with ESP-IDF v5.5.1. Binary hashes are in `web/wasm/fw/public/platform-stopwatch.sha256`; manifest pins are checked by the platform tests.

| Check | Result |
| --- | --- |
| `cargo clippy --workspace --all-targets -- -D warnings` | Pass |
| `cargo clippy --release --target wasm32-unknown-unknown -p esp32sim-wasm --features jit-tests -- -D warnings` | Pass |
| Workspace release tests, `--include-ignored --skip external_` | 1,322 tests passed, zero failures; use an absolute `ESP32SIM_ROM_DIR` |
| Production WASM build and section check | Pass; no DWARF, name section retained |
| Internal WASM hello, C3/C6 hello, C6 energy scan/Contiki/network/RPL, panel and StopWatch fixtures | All pass |
| Platform configuration | 10 behavior groups: catalogs, hashes, validation, canonical shares, private persistence |
| Platform runtime | Real worker/WASM boot, pause/resume, stale callback exclusion, reset, display, buttons, touch, complete RMC/GGA, local merged flash, input boundaries, hash errors and disposal |
| Human input controller | 10 injected-clock checks, including minimum short press and session replacement |
| Display texture lifecycle | 4 checks; StopWatch ↔ PaperMono browser roundtrip has no console warnings/errors |
| Existing worker pacing/channel and touch/StopWatch UI tests | Pass |
| Skill frontmatter and references | Validated |

Browser checks cover desktop playback, 390px mobile layout without horizontal overflow, a real 468×466 PNG capture, short mouse clicks, remapping A to Space and delivering a short key press, GPS serial echo, private DIY creation and scene URL roundtrip, PaperMono execution being disabled, parent RPC boot/control, and a cross-origin handshake between distinct loopback origins. Browser screenshots are local review artifacts under ignored `output/playwright/`; they are not physical-hardware evidence.

The diagnostic produces `PLATFORM_READY stopwatch`, `BUTTON_A running`, `BUTTON_B reset`, `TOUCH down`, `GPS_RX $GPRMC…` and `GPS_RX $GPGGA…` through actual emulated firmware. It has not been run on the owner's physical hardware. These checks establish the documented diagnostic path, not compatibility with arbitrary StopWatch or PaperMono firmware, GPS pin routing, baud fidelity or electrical behavior.

The first Pages runner exposed a timeout in the original host-duration StopWatch interaction test. Interaction checks now hold inputs until a fresh firmware acknowledgment and print the failing phase. GPS pacing uses a matching worker input receipt, then waits for simulated time to advance; stale statistics cannot release the next chunk. Regressions include delayed messages and wrong receipt IDs.

The public product catalog contains StopWatch and PaperMono. Generic ESP32-S3
Hello remains an internal lifecycle fixture in `tools/fixtures/platform-hello.json`,
not a selectable website product or public firmware entry.
The StopWatch diagnostic is likewise declared only in
`tools/fixtures/platform-stopwatch.json`; the public StopWatch firmware choice is
the official M5Stack release. Internal diagnostic source and binaries are retained
solely for automated regression checks.

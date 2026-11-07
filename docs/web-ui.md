# Web UI and protocol

`web/index.html` is the site's landing page (what GitHub Pages shows at the root: the demos,
the coding-agent story, the chip facts); every demo opens `web/run.html`, the emulator page.
`run.html` lays itself out as a workbench (`workbench.js`, `workbench.css`: the device on a stage,
the console under it, a rail with hints, telemetry, the machine's configuration and the same
run as a CLI command); `?plain` gives the bare page. Both pages follow the system's light or dark theme; the ☾/☀
button in the header picks one and remembers it in the browser (`localStorage`). The console is
dark in both. `index.html?wasm&fw=…` links from before the split redirect to `run.html`.

`--web PORT` serves `web/run.html` (no build step, no dependencies) and a WebSocket on the
same port. The page shows the board (Atech: 14-port drawing with knob, ring, buttons,
speaker VU, the TFT in its physical orientation plus a readable copy; Waveshare: camera
panel with picture upload / webcam and speaker meter; XIAO ESP32-C6: the user LED and the
BOOT button; bare: console only), the USB-CDC and
UART0 consoles, an action box for the SDK's JSON protocol, and audio through WebAudio
(click 🔇 once — browsers require a user gesture).

The native server accepts browser WebSocket connections only from pages at
`http://127.0.0.1:PORT` or `http://localhost:PORT`, using the server's port. It checks
the browser's `Origin` header, which identifies the page's scheme, host and port.
Local native clients may omit `Origin`; the server treats those tools as trusted.
See the [Origin check](../esp-soc/src/web.rs) for the implementation.

The header shows emulated time, instructions, frames, and `real time` / `⚠ N% of real time` /
resync count. The percentage is emulated seconds per wall second over the last second: a
resynchronisation resets the lag but not this, so a run that cannot keep up stays visible. The audio buffer is adaptive: it starts at 60 ms and grows on underrun (up to
400 ms) so a busy firmware phase (a full display redraw) does not produce gaps.

## Emulator → browser

Text frames (JSON):

| `t` | Fields | When |
| --- | --- | --- |
| `board` | `name`, `chip` (`esp32s3`, `esp32c3`, `esp32c6`, `esp32`) | on connect; the page switches layout and titles a bare module by the chip |
| `serial` | `src` (`usb`/`uart0`), `data` | console output (ANSI colours stripped by the page) |
| `ring` | `leds` `[[r,g,b]…]` | ring changed |
| `stat` | `time`, `insns`, `frames`, `behind`, `resyncs`, `speed` (emulated seconds per wall second over the last second, `null` before the first second and when not paced), `cam`, `gpio_in` | every 20 ms emulated |

Binary frames (first byte = type):

| Type | Payload |
| --- | --- |
| 1 | TFT frame: `w u16le, h u16le`, RGB565 pixels (160×80) — quiet-push boards defer at most one push interval; other boards send when changed |
| — | `{"t":"emu","msg":…}` — a line from the emulator itself (wasm build: stubs, chip resets, load errors), shown in the console |
| 2 | audio: `[rate u32 le]` then int16le mono samples; the rate is what the firmware programmed the I2S clock to (44.1 kHz Atech, 24 kHz autopling, 22.05 kHz the panel's SID player) and can change between chunks |
| 4 | camera preview: `w u16le, h u16le`, RGB888 (320×240) |

On quiet-push boards, continuous redraws are published on every other push opportunity rather
than waiting indefinitely for silence. This keeps drawing visible but can expose a partially
updated frame. It is a publication policy, not a model of physical panel scanout.

A late-joining tab gets a snapshot (console backlog, last frame, ring, preview) on connect.

## Browser → emulator

Text frames:

| `t` | Fields |
| --- | --- |
| `btn` | `pin`, `v` (1 = pressed) |
| `knobpress` | `v` |
| `reset` | none: the board's reset button. The page's Restart sends it to a native run; in the browser build Restart reloads the page, which is the same thing there. Honoured when the run can come back up: through the ROM, or for `--boot app` by entering the app again; ignored with `--no-reboot` |
| `knob` | `d` (+1 cw / −1 ccw per detent); the emulator queues the quadrature edges 2 ms apart |
| `serial` | `line`, optional `src` (`usb` default, `uart0`, `uart1`) — the line plus a newline into that console's RX |
| `key` | `src`, `data` — bytes exactly as typed, no newline added: the page's console is a terminal (click it, type; Enter is CR, Backspace DEL, arrows and Ctrl-letters their escape/control codes, paste sends the text). The **Terminal** tab (`web/terminal.js`) is a real VT100 on UART0 — xterm.js (MIT; `tools/fetch-web-vendor.sh` puts it in `web/vendor/xterm`, pinned by hash — not committed), 100×30 — so cursor movement, colours and full-screen programs render; it encodes its own keys into the same `key` message |
| `gpio` | `pin`, `level` |
| `touch` | `x`, `y`, `down` (1 = touching); panel coordinates — a rotated panel view (the panel header's selector, `?rotate=`, or a manifest's `display_rotate`) maps its point back first |

Binary frame type 3: camera picture, `w u16le, h u16le`, RGBA8888 — used by the picture
upload and the webcam (4 fps). Frames up to 8 MB are accepted.

## Sending is never blocking

Each client has a writer thread with a bounded queue; when a tab is frozen or slow, frames
are dropped for that client and the emulator keeps running at real time.

## Use a webcam with the browser build

Open `run.html?wasm`, select `waveshare-cam`, and load the ROM and camera firmware
files in the firmware panel. Boot the firmware, then click **use webcam** in the
camera panel. The sensor supplies black frames until the first host image arrives,
so firmware can initialize before permission is granted. The browser asks for camera permission only after that click. Serve
from localhost or HTTPS, as required by `getUserMedia`. Permission failure appears
beside the button. **stop webcam** releases the camera and keeps its last image;
leaving the page also releases it.

The existing upload and webcam controls work with both native WebSocket and WASM
runs. The worker sends type-3 camera RGBA through `esp32sim_in_bin`; the native server
uses binary type 3. Both replace the same board image. Host updates arrive at 4 fps;
the sensor repeats the latest image at its emulated capture cadence. The SCCB format,
output size and sensor-array window apply to still images and live frames alike.
See [live camera input](cli.md#live-camera-input) for supported formats and limits.

Camera errors use `{"t":"camera-error","message":"..."}` and appear beside
the source controls. Webcam tracks stop on emulator stop, boot failure, disconnect
and page exit. Insecure origins report that HTTPS or localhost is required.

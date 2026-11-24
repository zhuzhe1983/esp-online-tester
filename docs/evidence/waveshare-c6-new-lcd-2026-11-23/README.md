# C6 first-five evidence — 2026-11-23

This directory records bounded native execution, independent board regressions, factory layout/checksum inspection and production-model loader checks for five official listings / twelve SKUs. Status remains partial.

## Unmodified manufacturer images

`factory-image-verification.json` includes each original image's SHA-256, byte count and URL. The E9 boot image and present application images have C6 chip ID13, valid XOR checksums and appended SHA-256 digests. The partition table at0x8000 has a valid MD5 record; every partition fits the selected16 MiB flash. Images are loaded at offset0. Their original `.bin` files stay outside Git and are represented by pinned remote links.

The two 1.54 manufacturer filenames have byte-identical SHA-256 in this snapshot. They are retained as distinct source identities and independently executed against non-touch and touch board profiles; identical image bytes do not add a touch peripheral to the non-touch SKU.

## Native ROM execution

`native-runs.json` records exact command lines, timestamps, wall time, exit status, original firmware SHA-256, ROM SHA-256, log SHA-256 and framebuffer statistics. Original ESP32-C6 rev0 ROM SHA-256 is `788e1d38724aeb8fd974fa10c4a7b089c02627d35342ce84b9e0b12b239f3551`. No ROM hooks or project diagnostics are substituted.

| Runtime | Time | Exceptions | Final RGB colours | Observed boundary |
| --- | --- | --- | --- | --- |
| `waveshare-c6-touch-lcd128` | 5 s | 0 | 1 | Main returns, real display writes; final single-colour snapshot |
| `waveshare-c6-lcd128` | 5 s | 0 | 1 | Main returns, real display writes; final single-colour snapshot |
| `waveshare-c6-lcd273` | 5 s | 0 | 4032 | Home icons / background visible; disconnected Wi-Fi and rendering fidelity remain partial |
| `waveshare-c6-touch28` | 5 s | 0 | 1743 | Display Test / Settings / AI Chats / Touch Test home page visible |
| `waveshare-c6-lcd154` | 5 s | 0 | 1 | Display writes and Wi-Fi attempts; final single-colour snapshot |
| `waveshare-c6-touch-lcd154` | 5 s | 0 | 1 | Same original bytes independently run against touch hardware |

No card was attached for these factory runs, so missing-card messages are expected and do not claim a nonexistent socket. Exit code0 is not used as evidence of full application success. There is no browser/WebGL or real-device acceptance claim.

## Meaningful board checks

`cargo test -p esp32c6 waveshare --lib`: six regression tests pass. They cover exact I²C pin/address sets, physical BOOT/user pins, route/phase rejection, RGB565 versus RGB666, backlight/reset gates, touch bounds/read acknowledgement and reset availability, CH32 8-bit versus16-bit protocols, and attached-card CMD0/CMD8 responses on each physical MISO. A wrong input pad and simultaneous LCD/card CS do not expose card responses. These are board-facing tests, not a claim that the manufacturer application completed every interaction.

Model verification uses the actual production Three.js loader and CPU rays through framebuffer/control/occlusion geometry. `model-loader-check.json` reports exact model hashes and checked dimensions. Node texture objects verify source image bytes and projection coordinates but do not decode browser pixels. Engineering orthographic review images are separate from WebGL or physical-fit acceptance.

Sources are retained in `docs/platform/research/waveshare-c6-new-lcd-sources/` and in each model's asset directory. The native CAD export, approximate exterior reconstruction and explicit hidden `UNKNOWN` structures are recorded separately in the model plans.

## Geometry visual review

All seven model variants were inspected in front, reverse, edge and oblique geometry plots. The native 2.8-inch assembly retains its connector, spacer and rear-component geometry; the six reconstructions retain distinct documented enclosure/PCB dimensions and unobstructed display apertures. The plots intentionally omit photograph pixels, so this review does not claim texture alignment or browser rendering. `model-visual-review.json` records each reviewed image hash, observations and remaining mechanical limits.

Native console logs preserve the exact emitted bytes, including firmware CRLF and trailing spaces; local Git attributes disable whitespace cleanup/checks for these immutable captures.

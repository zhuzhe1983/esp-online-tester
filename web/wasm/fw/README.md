# Firmware for the WebAssembly build

Put images here for `run.html?wasm&fw=<name>`, described by `<name>.json`:

```json
{ "board": "waveshare-lcd4b", "flash_mb": 16, "psram_mb": 8, "wifi": "ssid=esp32sim,psk=esp32sim-pass", "stubs": ["esp_wifi_start=0"],
  "files": { "rom": "esp32s3_rev0_rom.elf", "bootloader": "bootloader.bin", "ptable": "partition-table.bin",
             "app": "energy_panel.bin", "elf": ["energy_panel.elf"], "script": "sid.txt" },
  "flash_at": { "0x610000": "energydata.json" } }
```

`kind` names: `rom`, `bootloader`, `ptable`, `app`, `elf` (one or a list), `flash` (whole image),
`script`, `picture`. `flash_at` writes files into flash at hex offsets (a data partition's contents, or
a combined bootloader + partition table + app image at its offset, as the MicroPython demo does). `terminal: true` opens the page on the xterm.js terminal tab (UART0) instead of the plain console log. `line_hint` is the example shown in the console's line box (default: the board's, JSON actions on the Atech board, otherwise a plain line); the tab title is the demo's name from `demos.json`. `display_rotate` (0, 90, 180 or 270, clockwise) turns the panel view for a firmware that draws rotated into its panel — pocket-tank renders landscape into the portrait AMOLED, so it sets 90; touch is mapped back, and `?rotate=` or the panel's selector overrides it. `stubs` are `NAME[=value]` function stubs — resolved through the ELF, or through a `symbols` map (`{"NAME": "0xaddr"}`) when no ELF is shipped; `wifi` an AP spec. `demos.json` lists the manifests the page offers as links. Everything else in this directory is git-ignored except `public/` (hello_world and the Atech firmware — our own code — plus pocket-tank's bootloader, partition table and app, MIT, with `pocket-tank-LICENSE.txt`; and the WiFi station of `examples/c6-wifi-station`, our own code, built for the S3, C3 and C6 as the WiFi goldens' input — the binaries also contain ESP-IDF, Espressif's WiFi, PHY and coexistence libraries and third-party code such as FreeRTOS, lwIP, Mbed TLS, wpa_supplicant and newlib, with their licences in `wifi-station-NOTICE.txt`) and the manifests: the mask ROM
is Espressif's and firmware is whoever built it — host them only where you may.

The committed `public/c3-ble-*` files are the C3 NimBLE advertiser and GATT server from
`examples/c3-ble-advertiser` and `examples/c3-ble-server`, with linked firmware licences in
`public/c3-ble-NOTICE.txt`. Pages deploys these with the rest of `web/`.

For TinyDraw manifests, set `"smoothDisplay": true` to opt in to 120 Hz host
publication without the quiet-pixel deferral on the AMOLED board. The default
is 50 Hz with quiet-pixel deferral, including pocket-tank's full-frame rescans.
This option changes host snapshots, not guest display timing. Unsupported boards
or older WASM modules reject the opt-in before boot. The publication evidence is
recorded in [EX117](../../../docs/experiments.md#ex117).

The public `c3-crypto-tls*` and `c6-crypto-tls*` images exercise unstubbed Wi-Fi
startup and hardware-assisted TLS. [Source and reproducible build recipe](../../../examples/crypto-tls/README.md),
[linked licences](public/crypto-tls-NOTICE.txt), and [runtime licence texts](public/crypto-tls-runtime-licenses.txt)
accompany them. They are native CI fixtures; no new browser manifest is required.
The classic ESP32 hello inputs are `public/classic-hello-*`, built from
[`examples/hello_world-classic`](../../../examples/hello_world-classic/README.md).
`classic-hello-NOTICE.txt` lists only linked firmware components.
The ESP32-P4 goldens' inputs are `public/p4-hello*`, `p4-sdcard*`, `p4-sdsectors*` and
`p4-usbmsc*`, ESP-IDF v5.5.3 examples built from [`examples/hello_world-p4`](../../../examples/hello_world-p4/README.md)
(one and both cores), [`sd_card-p4`](../../../examples/sd_card-p4/README.md),
[`sd_card-p4-sectors`](../../../examples/sd_card-p4-sectors/README.md) and
[`usb_msc-p4`](../../../examples/usb_msc-p4/README.md); the last links TinyUSB and esp_tinyusb
besides ESP-IDF, with its licences in `public/p4-usbmsc-NOTICE.txt`. The single-core
hello_world, which `p4-hello.json` runs, is committed; the rest are pinned by SHA-256 in
`tools/fetch-p4-firmware.sh`, which fetches them from
[mikroverk/esp32sim-firmware](https://github.com/mikroverk/esp32sim-firmware) at a pinned commit.
Images over 1 MB, and images only the tests boot, go there rather than into this repository.
The co-processor goldens' inputs are `public/p4-hosted*` and `c6-hosted*`: IDF v5.5.3's WiFi station
example with esp_hosted 2.12.13 on the P4 and ESP-Hosted 2.12.13's slave on the C6, and the same
pair with ESP-Hosted's raw throughput test, built as
[`examples/esp-hosted-p4`](../../../examples/esp-hosted-p4/README.md) describes; their licences are in
`public/hosted-NOTICE.txt`; `tools/fetch-p4-firmware.sh` pins them too.

`esp32/tests/fixtures/classic-camera/` at the repository root holds the
reproducible public-driver RGB565 camera fixture from
[`examples/classic-camera`](../../../examples/classic-camera/README.md).
It is a native CI input, not a Pages asset; its `NOTICE`
lists its linked components and points to the accompanying runtime licences.
The companion `lifecycle.bin` uses the same pinned project
and public driver for replacement, deinit/reinit and reboot tests; [recipe](../../../examples/classic-camera/lifecycle.md)
and `lifecycle-NOTICE` accompany it. The bootloader and partition table are shared.

The committed `public/schematik-tdisplay-*` files run a restyled Schematik T-Display
Arduino sketch on the same classic board. [Source, pinned build and button controls](../../../examples/schematik-tdisplay/README.md),
linked licences (`public/schematik-tdisplay-NOTICE.txt`) and LGPL rebuild sources
(`public/schematik-tdisplay-sources.tar.gz`) accompany the three images. Pages serves them
unchanged, without an additional download channel. `tdisplay-schematik.json` requires a
boot-console line and at least one panel frame; GPIO35 advances, GPIO0 powers off/wakes.
No physical-board verification is claimed.

The native-only `public/classic-psram-idf-{bootloader,ptable,app}.bin` inputs
exercise genuine ESP-IDF 4.4.7 unicore PSRAM startup and allocator code.
[Recipe, configuration and hashes](../../../docs/fixtures/classic-psram-idf/README.md),
`classic-psram-idf-NOTICE.txt` and `classic-psram-idf-licenses.txt` accompany them.
They do not expose a browser demo or qualify classic PSRAM under WASM.

`tdisplay-prices.json` is the LilyGO TTGO T-Display demo on the classic ESP32: stock MicroPython
v1.29.0 (`ESP32_GENERIC-20260824-v1.29.0.bin`, MIT, fetched from micropython.org by
`tools/fetch-micropython.sh` and pinned by SHA-256; not committed) written whole at flash 0x1000 through
`flash_at`, and `public/tdisplay-vfs.bin`, the LittleFS filesystem of
[`examples/ttgo-tdisplay`](../../../examples/ttgo-tdisplay/README.md) (our app, Russ Hughes's MIT
st7789py driver and fonts, `public/tdisplay-NOTICE.txt`) in the vfs partition at 0x200000. The
manifest's `min_frames` makes `tools/wasm-test.mjs` require that many panel frames besides the
console line, so a demo that boots but never draws fails.

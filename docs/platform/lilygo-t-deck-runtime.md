# Original T-Deck SX1262 runtime

The `lilygo-t-deck` product uses the independent `tdeck` ESP32-S3 board. This
applies to the original SX1262 868/915 MHz variant, not Plus, Pro, V2 or a
preinstalled-firmware product. Its official 20261024 factory image boots through
ROM and renders the real factory menu. Touch navigation, lowercase `q`,
`Shift+Q`, Enter, all four trackball directions and the centre-button Click
indicator have been checked against its framebuffer. The catalog remains
`runtime.status: partial`; no physical hardware was tested.

## Sources and wiring

The pin map and schematic are pinned in
[`lilygo-t-deck.json`](../../web/platform/knowledge/lilygo-t-deck.json).
Protocol and factory UI source receipts are in
[`sources.json`](evidence/lilygo-t-deck/sources.json): T-Deck revision
`2027dee79198af949a9e14e1a09e301f993fc7cf` and LilyGoLib revision
`57f6414e682ae124817ae0936f7b8251ac17d6c3`. These establish driver behaviour;
an exact reproducible source build of the supplied binary has not been established.

| Device | Physical connection and model |
| --- | --- |
| Peripheral rail | GPIO10 high powers LCD, touch, keyboard and radio |
| ST7789 | SPI SCK40/MOSI41, LCD CS12/DC11; backlight42; shared MISO38; exclusive LCD/radio/SD selection required |
| GT911 | I2C SDA18/SCL8, address 0x5D, interrupt16; single contact; raw 240×320 coordinates converted to factory 320×240 landscape |
| ESP32-C3 keyboard | Same I2C bus, address 0x55; command03 reads five raw column bytes, command04 reads the legacy single-character mailbox; 01/02 control stored brightness settings |
| Trackball | Active-low right2/up3/left1/down15; centre0 also serves BOOT |
| SX1262 | Shared SPI, CS9/RESET17/BUSY13/DIO145; existing digital SX1262 command model |

The keyboard's 35 physical switches map individually to the manufacturer's
5-column × 7-row electrical matrix. The platform exposes four physical rows
(10/10/10/5 keys), rejects holes in the last row and preserves simultaneous
modifiers. Input reaches the I2C controller protocol, without UART substitution.
The controller source scans each switch after a 1 ms delay. The model commits a
whole scan after 35 ms; it does not execute the second MCU or simulate partial
scans, boot latency or switch bounce. The official C3 source does not drive its
reserved interrupt pin, so GPIO46 remains released. Legacy printable, symbol,
shift, Enter, Backspace and Alt+B/Alt+C handling follows that source's mailbox
and scan order, including command01's fallthrough into command02.

The board stores the panel's native GRAM and presents its landscape orientation;
touch applies the inverse transform. Wrong SPI/I2C routing, power-off and held-key power-cycle cases
are covered by board tests. No new register constants, shared bus hooks,
interrupt sources or per-tick work were added to existing boards. Related prior
mechanisms are EX201 (S3 physical pins), EX217 (GPIO feedback), and EX235 (SPI3)
in `docs/experiments.md`; this board reuses those paths.

## Factory evidence and reproduction

Image: `firmware/tdeck-sx1262-factory-20261024-0x0.bin` at the pinned T-Deck
revision above; 16,711,680 bytes, SHA-256
`6985490c35ca5e5245321a60036f44f81ae3182423a0194fa74f8bf97870302c`.
Its application descriptor reports ESP-IDF `v5.5.5`. The test uses the complete
merged flash image and ESP32-S3 rev0 ROM, not an app-only shortcut or a diagnostic
fixture. The fixture supplies inputs and checks output from the actual image.

Build WASM and provide ROMs as described in the platform README, then run:

```sh
node tools/probe-official-tdeck.mjs \
  https://raw.githubusercontent.com/Xinyuan-LilyGO/T-Deck/2027dee79198af949a9e14e1a09e301f993fc7cf/firmware/tdeck-sx1262-factory-20261024-0x0.bin \
  6985490c35ca5e5245321a60036f44f81ae3182423a0194fa74f8bf97870302c \
  output/tdeck-final 15 tools/fixtures/lilygo-tdeck-controls.json
```

An optional `ESP32SIM_FIRMWARE_FILE` supplies a local cache of the same upstream
image; SHA and layout validation still run. Firmware bytes are not committed.
The harness samples the worker's guest clock, captures actual RGBA output, checks
specific glyph/control regions and saves screenshots under ignored `output/`.
The final run completes 19 checkpoints through 37.54 guest seconds, including
the boot screen. A rejected matrix hole is intentional; its `INVALID_INPUT`
receipt is not a guest failure. Full-frame hashes are observations, not stable
goldens: asynchronous header updates can vary. The selected region hashes are
the assertions.

The headless browser check is `tools/check-tdeck-browser.js`, run via the
Playwright skill wrapper against a local workbench. It verifies 40 regions at
front/oblique/back views, then clicks the actual Q region in the rendered CAD
and checks that the official Keyboard page displays `q`. It also restores the
exact product and keyboard model in a shared mini page. It expects the checked
image cached at `output/tdeck-source/firmware/` and fulfills only its exact
catalog URL; this tests runtime validation and UI input, not remote CORS uptime.
The wrapper's async `waitForFunction` predicate returned a false handle
immediately in a failed preliminary run; the final check polls `observe()`
explicitly. A default Python server also reset several simultaneous module
requests during mini navigation; the completed run uses a larger request queue.

Curated inputs, output hashes, software versions, checks and limits are in
[`runtime-verification.json`](evidence/lilygo-t-deck/runtime-verification.json).
Raw serial data, screenshots and event streams remain local under ignored
`output/`. The receipt omits serial streams and local identity/session data;
this does not remove any framebuffer assertion or measured guest count.

## Model inputs and limits

The existing manufacturer CAD geometry and photo texture are unchanged.
The geometry SHA-256 remains
`23a9f65ac5a1610874f67e4463d0f4a042badc9485209ebbbe253bff7042366f`.
`model-plan.json` retains 35 invisible, photo-aligned key regions and five
trackball regions. The keyboard crop is 405×229 pixels, mapped to the existing
photo surface; each region records its photo pixel centre. These regions do not
claim separately measured keycap geometry. The previous dimensional and photo
limitations still apply. Circle ray samples use a 0.01 mm interior offset to
avoid a numerical intersection exactly on the shared triangle-centre vertex.

Limitations are explicit:

- GT911 is single-contact and the LCD backlight is an on/off gate, not the
  physical 16-step brightness driver. LCD readback is not verified.
- SX1262 digital initialization is reused; over-air propagation, antenna
  behaviour and actual radio traffic were not validated. The platform does not
  advertise LoRa injection for this product yet.
- ES7210 microphone/audio, SD and external GNSS are absent in this board model.
  The original board has no built-in GNSS. Factory attempts to poll GPS receive
  no invented position data. Wi-Fi/Bluetooth and sleep/wake were not validated.
- The keyboard protocol is a device model, not execution of the C3 firmware.
  Brightness commands retain controller state but do not illuminate a calibrated
  3D keyboard backlight. Only the listed factory functions were exercised.
- `ControlLeft → sym`, `Digit4 → $`, `Escape → centre` are editable host-key
  conventions, not labels of additional physical switches. Grove plug direction
  and pin order remain unverified; no connected cable is offered.

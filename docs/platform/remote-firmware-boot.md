# Remote BIN startup and history

Remote BIN sources first use the existing merged-image validator. A standalone
ESP application can then use the existing application-direct loader if its chip
ID, application descriptor, segment bounds, XOR checksum and optional appended
SHA-256 all validate. Detection reuses the downloaded bytes before worker creation;
there is no failed startup followed by a second download.

The fallback only handles `INCOMPLETE_IMAGE`. Network, hash, chip mismatch and
capacity errors are not swallowed. An isolated bootloader, truncated application,
extra non-padding payload or damaged checksum is not guessed into a layout.
Local app uploads retain the `flasher_args.json` plus companion files workflow.

After loading, the portable remote source contains its SHA-256 and
`bootMode:"app-direct"`. A normal merged image may use explicit `bootMode:"rom"`.
The 40-entry browser history retains the selected mode, firmware name/version and
last-use timestamp; Burner history also retains its exact project/version URL.
Older history entries without a mode use automatic detection. An explicit ROM
mode never silently changes to application-direct. Shared scenes and recordings
preserve the mode, and physical flashing rejects application-direct sources.

Application-direct copies/maps the app at the emulator's existing `0x10000`
location and enters its application entry point. It does **not** reconstruct a
bootloader, partition table, NVS or publisher-specific data. Firmware can still
fail during initialization or return through the ROM into an invalid-header loop
after a reset. Such serial output remains visible; a successful loader/start
acknowledgment is not proof of a working firmware. A supplied complete build layout
is preferable whenever available.

## M5Burner layout audit

Checked 2026-10-27. The public v1 responses for UserDemo, M5Guruguru, Kaleidoscope,
UIFlow2 and Xiaozhi (16 public versions in total) expose no structured boot-mode,
flash-offset or image-layout field. The official site's ordinary and shared
flashing paths pass the main downloaded BIN at address `0`. This is not a
validation of that BIN's layout. The legacy `pluginAddr` field locates generated
Wi-Fi/UIFlow configuration; it is not the app image's offset. Filenames and OTA
feature descriptions are not authoritative layout declarations.

[The source receipt](evidence/burner-layout-audit.json) retains checked URLs,
timestamps, response hashes, relevant field names and JavaScript observations.
It excludes personal profiles, raw API captures and firmware bytes.

The binary checks follow the [Espressif image format](https://docs.espressif.com/projects/esptool/en/latest/esp32s3/advanced-topics/firmware-image-format.html)
and [ESP-IDF v5.5.4 app descriptor](https://github.com/espressif/esp-idf/blob/v5.5.4/components/esp_app_format/include/esp_app_desc.h).

## Verification and retained negative result

- `node tools/platform-app-image-test.mjs`: real committed app bytes, corruption,
  checksum/digest, chip/size/bounds, bootloader rejection, mode history/scene
  round-trip and physical-flash refusal.
- `node tools/platform-runtime-test.mjs`: actual worker/WASM loading and dispatch,
  one download for initial detection, saved-mode reload and strict ROM/hash
  failures; existing merged/local/build-layout execution checks remain in place.
- Negative result: `public/hello_world.bin` boots through its complete manifest,
  but application-direct with the real S3 ROM resets and prints
  `invalid header: 0xffffffff` instead of reaching `Hello world!`. This is retained
  as a compatibility limitation, not hidden by a fixture substitution or forced
  reset behavior. The application-direct regression tests assert mode dispatch
  and guest execution only; they do not claim successful IDF initialization.

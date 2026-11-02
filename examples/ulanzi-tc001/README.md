# Ulanzi TC001 · AWTRIX 3 in the emulator

`fetch-awtrix.sh` downloads the stock AWTRIX 3 v0.98 images the AWTRIX web flasher writes to the clock
(Blueforcer/awtrix3, pinned by commit and SHA-256) and lays them out as a 4 MB flash image. Run it on
`--board ulanzi-tc001`:

    examples/ulanzi-tc001/fetch-awtrix.sh /tmp/awtrix-flash.bin
    target/release/esp32sim --chip esp32 --board ulanzi-tc001 --boot rom --rom web/wasm/fw/esp32_rev300_rom.elf \
      --flash-image /tmp/awtrix-flash.bin --flash-mb 4 --net none --max-seconds 20 --no-dump --tft-png tc001.png

Expected: on the first boot LittleFS finds no filesystem and formats it, the app restarts once, then
`Framebuffer_GFX::begin Width: 32 Height: 8 Num Pixels: 256` and `AP mode.`; the matrix shows
"AP MODE" (`tc001.png`, 3× scale) and the board report counts the frames, about 80 a second. The
colours are the values the firmware sends, before any brightness or diffuser.

The images are not part of this repository and the script only downloads them. AWTRIX 3 is
Blueforcer's, under CC BY-NC-SA 4.0 (`LICENSE.md` at the pinned commit): non-commercial use only.
`external_awtrix_tc001` in `cli/tests/goldens.rs` runs the same check with `AWTRIX_FLASH` set to the image.

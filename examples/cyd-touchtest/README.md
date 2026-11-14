# CYD touch test

Runs the [ESP32-Cheap-Yellow-Display](https://github.com/witnessmenow/ESP32-Cheap-Yellow-Display)
`Examples/Basics/2-TouchTest` sketch (Brian Lough, MIT; `src/TouchTest.ino` is unmodified, commit
`0564a140c76de76ef34a8b7115044e7c6c852695`, licence in `LICENSE-ESP32-Cheap-Yellow-Display`) on the
`cyd` board: TFT_eSPI on the ILI9341 (HSPI) and the Paul Stoffregen XPT2046 library on its own SPI
bus. It draws "Touch Screen to Start", then the pressure and raw X/Y of each touch.

Arduino-ESP32 2.0.x, so ESP-IDF 4.4. Build with PlatformIO (`pio run` here), then:

```sh
H=examples/cyd-touchtest/.pio/build/cyd
target/release/esp32sim --chip esp32 --board cyd --boot rom --flash-mb 4 \
  --bootloader $H/bootloader.bin --ptable $H/partitions.bin --app $H/firmware.bin \
  --script examples/cyd-touchtest/touch.txt --tft-png touch.png --max-seconds 8 --no-dump
```

`touch.png` shows the values of the second tap. Add `--web 8766` (and drop `--max-seconds`) to
click the panel in a browser at `http://127.0.0.1:8766/run.html`.

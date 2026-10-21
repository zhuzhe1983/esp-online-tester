# Seeed Studio XIAO ESP32-C6 — the user LED and the BOOT button

The XIAO ESP32-C6 is a small ESP32-C6FH4 board (4 MB flash, no PSRAM) with a user LED on
GPIO 15, the BOOT button on GPIO 9 and the RF switch on GPIO 3 (power, low active) and GPIO 14
(the antenna). The board model is `esp32c6::board::xiao::XiaoEsp32C6`; `--board xiao-esp32c6`
selects it (aliases `xiao-c6` and `xiao`).

The sources here are an ESP-IDF project that blinks the LED and prints the BOOT button's state.
Build it with ESP-IDF 5.x and run it:

    cd examples/xiao-esp32c6 && idf.py set-target esp32c6 build && cd ../..
    examples/xiao-esp32c6/run.sh --max-seconds 10

What runs: the real mask ROM and the second-stage bootloader, FreeRTOS, `app_main`, the GPIO
driver. The console prints one line per half second:

    LED on, BOOT button released

The LED is **low active** — the anode goes to 3V3 through a resistor and the cathode to GPIO 15,
so the firmware's `gpio_set_level(PIN_LED, 0)` lights it. The model follows the drive masks, not
the edges, so a released pin is a dark LED and a driven-high pin is a dark LED too.

In the browser (`?wasm`, board `xiao-esp32c6`) the page shows the LED as a dot and the BOOT
button as a button; the button drives GPIO 9 low, and the firmware's `gpio_get_level` sees it.
In a script, `press boot 150` holds the button for 150 ms.

The RF switch is tracked and reported (`[emu] xiao-esp32c6: ... rf switch powered ... antenna
onboard ceramic`) but nothing is behind it: the radio is the C6's own 802.15.4 and WiFi MAC,
which the emulator models on the chip, not on the board.

See ../../docs/boards.md and ../../docs/esp32c6.md.

# CoreMatrix official-firmware exercises

`exercise.py` is an internal test program, uploaded through USB Serial/JTAG raw REPL into the unmodified UIFlow2 image. It is not a replacement firmware, public catalog entry, boot script, or host-generated matrix animation. All pixels, brightness and power writes are executed by MicroPython on the guest CPU.

Sources are pinned to M5Stack UIFlow2 commit `e437118892520ba987c2d930a2428db1cab65ab1`: `docs/source/controllers/corematrix.rst`, `m5stack/modules/startup/corematrix.py`, `m5stack/libs/driver/m5ioe1.py`, `m5stack/machine_sdcard.c` and the M5Unified bindings. C158's official schematic/doc maps the shared hardware I²C bus to G0/G1, TM1680 to 0x72, M5IOE1 to 0x4F, LED power to IO4, SD power to IO1, SD SPI to SCK25/MISO26/MOSI27/CS28, buzzer power to IO2 and buzzer PWM to IO11/channel2. The SDK in the downloaded image is `v5.5.4-dirty`.

Run from the integrated checkout after building WASM and fetching the C61 ROM:

```sh
node tools/probe-corematrix-uiflow.mjs /path/to/original-corematrix.bin 8d5b8d4e22ebc293937086b2f9f218aa5a5ebda27b5ccb32b3f8f580ee02939b
```

The runner creates isolated in-memory Flash and SD storage, checks the supplied image hash, boots the original firmware, interrupts only its application task, uploads the source in 64-byte chunks, and compares real 16×16 framebuffer pixels after guest commands. It does not change code bytes in the firmware image. The same `createCoreMatrixRepl(runtime)` helper can operate a browser's `window.espTester` API, provided its `stats.time` reports guest seconds.

`cm_matrix`, `cm_brightness`, `cm_led_power`, `cm_buttons`, `cm_imu`, `cm_power`, `cm_buzzer`, `cm_buzzer_pwm`, `cm_buzzer_power`, `cm_sd` and `cm_wrong_address` remain callable in raw REPL. A serial command acknowledgement establishes only that the guest executed the command. Validate the corresponding framebuffer, key sample, sensor bytes, SD contents or audio/PWM output before claiming a peripheral passes. `cm_sd()` writes a dedicated test file and must only be used with a task-owned test card.

The original manufacturer's startup script displays a monochrome Wi-Fi icon and connection animation. Its internal color names select icons; they are not RGB emitters. The physical product has 256 white LEDs.

No physical device is flashed or tested by this probe. Factory firmware identity, analog current consumption, radio propagation and optical calibration are separate from these digital checks. The first runnable receipt must be recorded before any entry is marked simulation-verified.

The firmware gitlinks pin M5Unified `b67ea7bd2d187d3fa8719edf1b915fd34a128c0d` and M5GFX `eef855b41c8ab7611a235055cc9afca76751ea95`. Its `IMU_Class.inl` maps CoreMatrix API acceleration and gyro to `[rawY, -rawX, rawZ]`. The fixture compares three injected orientation samples against the BMI270 signed register bytes and this exact transform; scene mounting is a separate photo/axis calibration.

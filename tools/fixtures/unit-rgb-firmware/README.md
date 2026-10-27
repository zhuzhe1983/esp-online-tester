# U003 internal guest fixture

Source-built ESP-IDF 5.5.1 firmware for StopWatch only; no BIN is committed or published. Build with `idf.py -C tools/fixtures/unit-rgb-firmware -B "$PWD/output/unit-rgb-build" -D SDKCONFIG="$PWD/output/unit-rgb-build/sdkconfig" build`, then `node tools/probe-unit-rgb.mjs --build output/unit-rgb-build` after `tools/wasm-build.sh` and fetching ROMs.

The guest uses IDF's real RMT TX/copy encoder at 10 MHz, 72 GRB bits and a 160 us reset, and switches M5PM1 0x6e over I2C47/48. UART0 commands: `a` RGB pattern, `b` rotated pattern, `w` wrong GPIO11, `i/o` Grove power on/off, `h/n` hold5V on/off, `s` PMIC shutdown, `r` esp_restart. TX logs attest guest activity; exact decoded `accessoryOutputs` separately attest output. No host serial text is substituted for LED data.

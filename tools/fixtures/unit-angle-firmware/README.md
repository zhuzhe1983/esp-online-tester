# Internal U005 ADC fixture

Build with ESP-IDF **v5.5.1**: `idf.py -C tools/fixtures/unit-angle-firmware -B "$PWD/output/unit-angle-build" -D SDKCONFIG="$PWD/output/unit-angle-build/sdkconfig" build`. Binary outputs remain ignored and are not public firmware catalog entries.

The fixture boots through the real ROM/bootloader, reads white StopWatch G11 through ADC2_CH0 with 12dB attenuation and IDF curve calibration, and checks unrelated G12 and yellow G10. UART0 commands: `q` sample, `i/o` Grove5V on/off, `h/n` HOLD5V on/off, `s` PMIC shutdown, `r` guest restart. The fixture never synthesizes an ADC result from serial text or host variables.

Run `node tools/probe-unit-angle.mjs --build output/unit-angle-build` with built WASM and required ROMs. The probe checks ten groups and prints source/binary hashes and actual guest results. The nominal input curve and unresolved official drawing conflicts are documented in `docs/platform/unit-angle-runtime.md`.

# U006 internal guest fixture

ESP-IDF 5.5.1 source fixture; no binary is published. Build with the command in `docs/platform/unit-hub-runtime.md`, then run `node tools/probe-unit-hub.mjs --build output/unit-hub-build` after obtaining the S3 ROM and building WASM.

Real GPIO11 input with pull-down, GPIO12 negative control, GPIO10 RMT GRB output, optional ADC2_CH0 one-shot read, and M5PM1 on I2C47/48. UART0: q reads GPIO, v reads ADC, a/b sends two RGB patterns, i/o enables/disables Grove5V, h/n changes power hold, s shuts down PMIC, r calls esp_restart. These commands execute guest drivers; host text is never substituted for peripheral output. q before v is intended; ADC configuration changes the G11 pad mode.

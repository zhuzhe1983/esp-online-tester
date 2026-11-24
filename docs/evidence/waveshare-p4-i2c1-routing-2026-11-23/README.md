# ESP32-P4 I2C1 physical routing

The I2C1 command-start path now samples GPIO-matrix signals SDA71 and SCL70, including pad mux and input/output route eligibility. I2C0 continues to use SDA69/SCL68. Only devices declaring physical pins opt into this routing check; older controller-only devices keep their existing behavior.

The source is Espressif ESP-IDF v5.5.3, commit `2c211b236707889e8400c4dc5644dd5c4ee071e0`, `components/soc/esp32p4/include/soc/gpio_sig_map.h`, lines defining I2C0/I2C1 pad indices. The exact response bytes and SHA-256 are in `sources.json`; gzip files expand to those original bytes. A failed lookup for the differently located IO_MUX header is retained and is not treated as evidence.

The targeted MMIO test sends the same slave address with no route, with I2C0 selectors, with the correct I2C1 selectors, and after disabling SDA input. Only the correctly routed transaction reaches the device. This does not establish electrical edge timing, bus arbitration or physical-board calibration.

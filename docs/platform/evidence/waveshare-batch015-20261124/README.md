# Waveshare batch 015 evidence

Five controller families: C5 Mini Kit, C5 Pico, C5 Zero, C5 WiFi6 Kit and P4 Nano. There are 21 exact SKU entries, including 16 C5 variants and five P4 Nano packages. The storefront `N16R4` URL is retained as a listing identity; exact current products are N16R8 and N32R8, according to their original SKU sources.

Two native C5 tests execute actual guest RISC-V instructions through RMT/GPIO, including wrong pad and IO_MUX negatives. Pico's TCA9554 address, physical I2C route, register round trips, and absent-device NACK behavior are independently checked. The release CLI passes 20 cases across five profiles: default and explicit memory choices execute the original SRAM diagnostic; incorrect Flash and PSRAM capacities are rejected.

The public platform runtime and production Worker/WASM execute the separate original SRAM diagnostic on all five C5 profiles. Observed RGB values follow their exact RGB/GRB order, GPIO28 BOOT press/release produces fresh guest UART responses, and resetting reloads the same SRAM image. This diagnostic is not a manufacturer image or a factory acceptance result. Four exact-capacity/revision/chip registry tests, catalog/knowledge validation and six watchlist tests pass.

The original C5 Zero TestCode image is hash checked and explicitly blocked by `C5_SRAM_REQUIRED`. C5 full ROM/Flash boot is still unsupported. Mini/Pico/WiFi6 source examples without a verified binary remain reference-only entries.

Both unchanged P4 Nano factory images, for 7-inch and 10.1-inch panels, run for 3.00 guest seconds. UART reaches SPIFFS, codec control initialization, the LVGL adapter and MIPI DSI PHY power. Neither produces a frame. DSI pixels, Ethernet, radio and audio PCM are not accepted. Raw UART, original image SHA, actual production WASM SHA and observed execution statistics are retained in the original-attempt directories. Manufacturer binaries remain remote references.

Five models pass the production Three loader, geometry, source hash and occlusion checks. All five four-view reports and four source-photo contour registrations were visually reviewed. C5 Zero, WiFi6 and P4 Nano use their own original CAD; Mini/Pico retain unchanged source photo pixels with dimensioned XY and explicitly estimated Z, USB depth and package height. Their unknown reverse remains marked. C5 Zero's source CAD does not establish the RF shielding visible in newer photographs. P4 Nano kit peripherals absent from the controller CAD are not fabricated. The existing WiFi6 CAD component 275 is now bound to RGB state at the source-confirmed position between USB ports, without moving or rescaling it.

CPU geometry reports do not establish browser DOM/WebGL, physical fit or hardware acceptance. All factory `simulationVerified` and `hardwareValidated` flags remain false.

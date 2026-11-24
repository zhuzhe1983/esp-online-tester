# Source-defined monochrome e-paper dimensions

`Ssd16xxMono::sourced_dimensions(width, height, busy_pin, cycles_per_ms)`
exposes the existing bounded byte-X / gate-Y digital command subset without
asserting a controller die identity. Dimensions outside 1…512 on either axis
are rejected; existing named 2.13/2.9-inch constructors keep their dimensions.

The first added caller is the current Waveshare ESP32-S3-ePaper-1.54 V2,
including its distinct touch and non-touch storefront variants. Manufacturer
revision `2f287c01d763e44b4307c48af95044acee5c891e`,
`02_Example/ESP-IDF/V2/11_FactoryProgram/components/port_bsp/epaper_config.h`
sets 200 × 200 pixels, while `port_display.cpp` initializes byte-X range
0…24 with 44h, gate-Y 0…199 with 45h, cursor 4Eh/4Fh, image RAM24h/26h,
and activation 22h/C7h followed by20h. Its BUSY polling waits while high.
Original files, source URLs, fetch UTC and SHA-256 receipts are retained in
[the source manifest](research/waveshare-sourced-mono200/source-manifest.json).
The manufacturer's source does not establish the fitted die; the constructor
and product knowledge therefore do not call this display an SSD1681.

The monochrome subset retains visible pigment after power loss, clears digital
RAM on power loss, and publishes a frame at its explicit digital BUSY deadline.
That deadline is not measured optical settling. Partial refresh waveforms,
manufacturer LUT calibration, grayscale, temperature response and ghosting
remain outside the model. Touch and non-touch board variants differ at the
board layer; this shared pixel model does not imply a touch controller exists.

The retained native test writes only byte-X24/gate-Y199 bit0 and observes the
last visible black pixel after the BUSY boundary, checks cursor25 rejection,
and verifies optical retention and subsequent fresh white RAM. Both previous
named-controller tests continue passing (3 tests total).

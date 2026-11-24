# S3 RGB route delivery correction

The S3 SoC sends GPIO matrix changes through `gpio_waveform_at`; it does not call the optional `lcd_rgb_pins` callback. These three board adapters now derive their physical LCD signals from that real route update and pass them through the existing strict pad validator. This fixes missing RGB frame delivery without relaxing display reset, backlight, resolution, or pin gates.

The six distinct board profiles are exercised by original LX7 instructions through actual GPIO, I2C, LCD_CAM and GDMA MMIO. Nonuniform frames verify first, middle and final pixels; wrong data and clock signals and source-based power/reset gates remain negative controls. The existing serial unit tests and all-target Clippy also pass. See the hash-bound receipt and raw logs.

These diagnostics are not original manufacturer firmware. They do not replace earlier recorded monochrome frames or failures. A later original-image run must be recorded as a separate attempt.

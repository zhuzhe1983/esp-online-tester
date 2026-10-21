//! The Waveshare ESP32-C6-LCD-1.47 board model on its own: the ST7789 fed command and pixel bytes
//! with the D/C line, and the WS2812 decoded from an RMT frame.
//!
//! And the Seeed Studio XIAO ESP32-C6: the low-active user LED on GPIO 15, the BOOT button on
//! GPIO 9 and the RF switch on GPIO 3 and GPIO 14.
use emu_core::Bus;
use esp32c6::board::xiao::{XiaoEsp32C6, LED_COLOUR, PIN_BOOT, PIN_LED, PIN_RF_ANT, PIN_RF_POWER};
use esp32c6::board::{WaveshareC6Lcd147, LCD_COL_OFFSET, LCD_VISIBLE_COLS, PIN_LCD_DC};
use esp_soc::{BoardModel, SocBus as _};

fn cmd(b: &mut WaveshareC6Lcd147, c: u8, params: &[u8]) {
    b.gpio_changes(&[(PIN_LCD_DC, false)]); b.spi_tx(2, &[c]);
    if !params.is_empty() { b.gpio_changes(&[(PIN_LCD_DC, true)]); b.spi_tx(2, params); }
}

#[test]
fn st7789_window_madctl_and_visible_columns() {
    let mut b = WaveshareC6Lcd147::new();
    // the D/C pin idles low: the very first command must be seen as a command
    b.spi_tx(2, &[0x11]);                                              // SLPOUT
    assert!(!b.panel.sleeping, "SLPOUT before any D/C edge must still be a command");
    cmd(&mut b, 0x36, &[0x48]);                                        // MADCTL: MX | BGR, as the scanner's firmware sets it
    cmd(&mut b, 0x3a, &[0x55]);
    cmd(&mut b, 0xb0, &[0x00, 0xe8]);                                  // RAMCTRL: pixels low byte first, as Waveshare's driver sets it
    cmd(&mut b, 0x21, &[]); cmd(&mut b, 0x29, &[]);
    // a 2x2 window at logical x=0..1 (RAM column 34+x), y=10..11, in LVGL's red (0xF206)
    cmd(&mut b, 0x2a, &[0, 34, 0, 35]); cmd(&mut b, 0x2b, &[0, 10, 0, 11]);
    b.gpio_changes(&[(PIN_LCD_DC, false)]); b.spi_tx(2, &[0x2c]);
    b.gpio_changes(&[(PIN_LCD_DC, true)]); b.spi_tx(2, &[0x06, 0xf2, 0x06, 0xf2, 0x06, 0xf2, 0x06, 0xf2]);
    assert_eq!(b.panel.pixels_written, 4);
    let (w, h, px, _) = b.display().unwrap();
    assert_eq!((w, h), (172, 320));
    // MX mirrors RAM columns and the glass mirrors them back: logical x = 0 is visible column 0.
    // With BGR set this glass shows the colour as sent: the scanner's bars are red on the board.
    assert_eq!(px[10 * 172], 0xf206, "top-left of the window at visible (0, 10)");
    assert_eq!(px[11 * 172 + 1], 0xf206);
    assert_eq!(px[10 * 172 + 2], 0, "outside the window stays black");
    assert_eq!(b.panel.bbox(), Some((LCD_COL_OFFSET + LCD_VISIBLE_COLS - 2, 10, LCD_COL_OFFSET + LCD_VISIBLE_COLS - 1, 11)));
    // the reset pin low clears the panel state (RAM survives, as on silicon)
    b.gpio_changes(&[(esp32c6::board::PIN_LCD_RST, false)]);
    assert!(b.panel.sleeping && b.panel.resets == 1);
}

/// What the board showed with BGR clear and pixels sent high byte first into a controller set to
/// low byte first (seen on the real module, 2026-09-21): a firmware's green text was blue, its
/// cyan title cyan, its white text white.
#[test]
fn bgr_clear_exchanges_red_and_blue_and_the_byte_order_is_the_controllers() {
    let mut b = WaveshareC6Lcd147::new();
    b.spi_tx(2, &[0x11]);
    cmd(&mut b, 0x36, &[0x40]); cmd(&mut b, 0x3a, &[0x55]); cmd(&mut b, 0xb0, &[0x00, 0xe8]); cmd(&mut b, 0x29, &[]);
    cmd(&mut b, 0x2a, &[0, 34, 0, 36]); cmd(&mut b, 0x2b, &[0, 0, 0, 0]);
    b.gpio_changes(&[(PIN_LCD_DC, false)]); b.spi_tx(2, &[0x2c]);
    b.gpio_changes(&[(PIN_LCD_DC, true)]); b.spi_tx(2, &[0x07, 0xe0, 0x07, 0xff, 0xff, 0xff]);   // green, cyan, white, high byte first
    let (_, _, px, _) = b.display().unwrap();
    let rgb = |p: u16| (p >> 11, p >> 5 & 0x3f, p & 0x1f);
    let (green, cyan, white) = (rgb(px[0]), rgb(px[1]), rgb(px[2]));
    assert!(green.2 > 24 && green.0 < 8 && green.1 == 0, "green shows as blue: {green:?}");
    assert!(cyan.1 > 48 && cyan.2 == 31 && cyan.0 < 8, "cyan stays cyan: {cyan:?}");
    assert_eq!(white, (31, 63, 31));
}

#[test]
fn ws2812_from_rmt_bits() {
    let mut b = WaveshareC6Lcd147::new();
    let bits = |bytes: [u8; 3]| -> Vec<bool> { bytes.iter().flat_map(|&v| (0..8).map(move |i| v & (0x80 >> i) != 0)).collect() };
    b.rmt_frame(0, &bits([0x10, 0xab, 0x03]));                          // G, R, B on the wire
    let (leds, updates) = b.leds().unwrap();
    assert_eq!((leds[0], updates), ([0xab, 0x10, 0x03], 1));
    b.rmt_frame(0, &bits([0, 0, 0])[..20]);                              // a short frame changes nothing
    assert_eq!(b.leds().unwrap().1, 1);
    assert_eq!(b.named_pin("boot"), Some(esp32c6::board::PIN_BOOT));
}

/// The XIAO's user LED is low active: it is lit while GPIO 15 is driven low, and off when the
/// pin is driven high or released. The state follows the drive masks, not the edges.
#[test]
fn xiao_user_led_is_low_active() {
    let mut b = XiaoEsp32C6::new();
    assert_eq!(b.leds().unwrap().0[0], [0, 0, 0], "the LED is off at power-on");
    assert_eq!(b.leds().unwrap().1, 0);
    b.gpio_output_at(0, &[(PIN_LED, false)], 1 << PIN_LED, 0);           // GPIO 15 as an output, low
    assert!(b.led_on);
    assert_eq!(b.leds().unwrap().0[0], LED_COLOUR, "the orange LED");
    assert_eq!(b.leds().unwrap().1, 1);
    b.gpio_output_at(1, &[(PIN_LED, true)], 1 << PIN_LED, 1 << PIN_LED); // driven high: dark
    assert!(!b.led_on);
    assert_eq!(b.leds().unwrap().0[0], [0, 0, 0]);
    assert_eq!(b.leds().unwrap().1, 2);
    b.gpio_output_at(2, &[(PIN_LED, true)], 0, 0);                        // released: the pin is high
    assert!(!b.led_on);
    assert_eq!(b.leds().unwrap().1, 2, "no change, no update");
}

/// The BOOT button on GPIO 9 reads high while it is released (it is pulled up on the board), and
/// the RF switch follows GPIO 3 (power, low active) and GPIO 14 (the antenna).
#[test]
fn xiao_boot_button_and_rf_switch() {
    let mut b = XiaoEsp32C6::new();
    assert_eq!((b.named_pin("boot"), b.named_pin("btn"), b.named_pin("btn1")), (Some(PIN_BOOT), Some(PIN_BOOT), Some(PIN_BOOT)));
    assert_eq!(b.named_pin("sw"), None);
    assert_eq!(b.input_levels(), vec![(PIN_BOOT, true)], "the button is pulled up");
    assert!(!b.rf_powered && !b.rf_external, "nothing drives the RF switch at power-on");
    b.gpio_output_at(0, &[], 1 << PIN_RF_POWER, 0);                      // GPIO 3 low: the switch is powered
    assert!(b.rf_powered);
    assert!(!b.rf_external, "the onboard ceramic antenna is the default");
    b.gpio_output_at(1, &[(PIN_RF_ANT, true)], 1 << PIN_RF_ANT, 1 << PIN_RF_ANT); // GPIO 14 high: the external U.FL
    assert!(b.rf_external);
    b.gpio_output_at(2, &[(PIN_RF_POWER, true)], (1 << PIN_RF_POWER) | (1 << PIN_RF_ANT), (1 << PIN_RF_POWER) | (1 << PIN_RF_ANT));
    assert!(!b.rf_powered, "the power enable is low active: high switches the RF off");
    assert!(b.rf_external, "the port select keeps its level");
}

/// The same LED, through the C6's GPIO registers: the bus hands the board the drive masks with
/// every output write.
#[test]
fn xiao_led_follows_gpio15_through_the_bus() {
    let mut m = esp32c6::machine([0; 6], 4 << 20);
    m.bus.board = Box::new(XiaoEsp32C6::new());
    m.bus.attach_board_devices();
    // IDF v5.5.4 components/soc/esp32c6/register/soc/gpio_reg.h: the GPIO block is at 0x6009_1000,
    // OUT_W1TS 0x08, OUT_W1TC 0x0c, ENABLE_W1TS 0x24.
    m.bus.write32(0x6009_1024, 1 << PIN_LED).unwrap();                    // GPIO 15 an output, low
    assert_eq!(m.bus.board_ref().leds().unwrap().0[0], LED_COLOUR);
    m.bus.write32(0x6009_1008, 1 << PIN_LED).unwrap();                    // drive it high
    assert_eq!(m.bus.board_ref().leds().unwrap().0[0], [0, 0, 0]);
    m.bus.write32(0x6009_100c, 1 << PIN_LED).unwrap();                    // and low again
    assert_eq!(m.bus.board_ref().leds().unwrap().0[0], LED_COLOUR);
    assert!(m.bus.board_ref().report().starts_with("[emu] xiao-esp32c6: led on"));
}


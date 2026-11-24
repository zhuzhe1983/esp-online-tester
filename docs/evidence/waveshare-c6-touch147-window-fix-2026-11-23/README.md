# C6 Touch-LCD-1.47 JD9853 visible window correction

The pinned manufacturer factory main sets portrait `esp_lcd_panel_set_gap(panel_handle, 34, 0)`. Its JD9853 driver adds that offset to CASET before writing RAM. The earlier board incorrectly allocated only 172 columns and exposed columns 0–171, which produced a 34-pixel left strip and discarded the last 34 written columns.

This correction preserves a distinct JD9853 DCS memory subset with 240×320 GRAM and exposes columns 34–205 as the physical 172×320 display. It does not identify the controller as ST7789 or add unsupported private registers. The manufacturer source bytes, URLs and hashes are retained in `source-receipts.json` and the two source gzip files.

The native regression writes different values to hidden column 0, first visible column 34 and last visible column 205. It checks the physical first/last pixels while preserving hidden memory. The source-based test passed; `unit-test.log` records the completion.

The exact original complete manufacturer image was rerun through the same C6 ROM for 5 emulated seconds. The earlier original frame remains in `before.png`; `after.png` visibly shows centered Squareline and DrawPanel icons/text with no former left white strip. The final image has 530 RGB colors, 30 RAMWR operations, 165120 pixels, and zero rejected board transactions. CLI output is a 3× rendering of the 172×320 panel. `native-run.json` preserves firmware/ROM hashes, command, frame hashes and raw/readable log hashes.

Original UART bytes remain in `after.raw.log.gz`; `after.log` only normalizes line endings and trailing whitespace for review. No firmware bytes were modified. This is native display-window verification, not complete wireless/application, physical touch or browser WebGL acceptance.

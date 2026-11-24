# SH8601 manufacturer-BSP RGB565 transport subset

`QspiDcsPanel::sh8601_bsp_rgb565` names a deliberately bounded digital compatibility implementation. The shared underlying address/pixel storage is code reuse; it is not a CO5300 silicon identity or a full SH8601 model.

Primary evidence retained byte for byte (gzip wrappers, original URL/time/length/SHA-256 in manifest): official SH8601A preliminary V0.0 (October2019), and Waveshare ESP32-C6-Touch-AMOLED-1.8 Arduino driver at commit4617895114cfef835b56c22bd4535979990e7669.

- Data-sheet p56: single-lane opcode02 +24-bit address00/command/00 for command/parameters, opcode32 for quad RAMWR/RAMWRC pixels. The same CS-delimited phase validation is reused.
- Data-sheet p57 says quad SPI supports only24-bit color. The actual manufacturer Arduino_SH8601.h initializes COLMOD05 and sends RGB565. **This unresolved conflict is exposed in the constructor name and board knowledge.** The implementation supports only the evidenced BSP RGB565 path, rejects other pixel formats, and never claims full data-sheet acceptance.
- Data-sheet pp42–43: Sleep In, Display Off, MADCTL00, COLMOD77, brightness00ff and control-display28 are SH-specific reset defaults. Brightness pp136/138 is10-bit low byte then two high bits; a one-byte BSP write preserves the upper bits. Zero gates output, nonzero intensity is uncalibrated.
- Reset RAM is declared random. Deterministic zero is an explicit emulator UNKNOWN-memory policy. CO5300 retention behavior is not inherited. Counters stay monotonic.
- Data-sheet p119 defines only MADCTL MX40 and BGR08. The manufacturer header's X-flip02/Y-flip05 constants conflict with reserved bits; the model rejects those orientations instead of silently converting them to CO5300 MX/MY/MV. No rectangular rotation is claimed.
- No SH8601 FE-page selection, identity/readback, OTP, private gamma, TE waveform, analog power or physical settle timing is inferred from CO5300. Unsupported vendor payload stays opaque.

Two source-derived tests exercise the10-bit brightness boundary, independent reset defaults and undefined memory, horizontal flip, reserved orientation rejection, wrong quad header/CS recovery, and absence of CO5300 FE semantics. The five existing CO5300 tests are unchanged and continue to pass.

# CH422G static digital I/O subset

The original WCH data sheet (§5.4–6.4) sets IO_OE at bit 0, A_SCAN at bit 2, OD_EN at bit 4 and SLEEP at bit 7. Its examples explicitly use 0x11 for bidirectional outputs plus open-drain OC and 0x05 for scanning. The pinned Waveshare 7-inch vendor driver instead defines OD_EN as bit 2. This is a retained source conflict, not a second accepted physical mode. The model follows the data sheet and rejects scanning/sleep. No electrical timing, current drive, LED scanning or power-on delay is claimed.

Power-on: mode 0 (IO inputs), OC high, weak pull-up on undriven IO. I2C 7-bit addresses 0x24,0x23,0x38–0x3b,0x26 carry exactly one byte, without a register index. Modes and output aliases are tested through the shared I2cDevice contract; board tests separately verify physical controller routing. The model exposes push-pull drive separately from sampled input to avoid treating high impedance as output-high.

Validation: `CARGO_BUILD_JOBS=1 cargo test -p esp-soc --lib waveshare_ch422g`; tests cover initial state, input stimulation, uniform direction, OC high impedance, one-byte command boundaries, command aliases, unsupported scan/sleep, and read/write direction mismatch. Source rights remain with WCH/Waveshare; publicly available source is not automatically MIT licensed.

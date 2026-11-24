# PCF85063A optional averaged offset

`Pcf85063State::with_averaged_offset(cpu_hz)` adds a deliberately approximate
digital calendar-rate model. The existing `new(cpu_hz)` constructor keeps its
previous behavior, including rejection of nonzero offset writes. Existing TCA
RGB and other callers are unchanged. This extension is needed by the official
Waveshare PCF85063A driver 2.1.0, whose initialization newly programs 02h to 73h
(-13, normal mode); version 2.0.0 did not program an offset.

NXP PCF85063A Rev. 7 §8.2.3, Tables 12–13 define a signed seven-bit two's
complement value from -64 through +63. Published steps are 4.34 ppm in normal
mode and 4.069 ppm in fast mode. Figure 12 establishes that positive values
reduce the frequency: a positive value lengthens the average second; a
negative value shortens it.

The opt-in model stores the actual byte and scales future complete calendar
seconds by those published decimal ppm values, using an integer fraction of
guest CPU cycles. Already scheduled seconds remain scheduled when software
changes the offset. It never rewrites past samples or reads a host clock.
Reset clears the offset, preserves the existing power-on OS-invalid policy,
and starts with the documented calendar defaults. STOP/restart and one-pending-
tick I²C burst behavior remain active. Huge guest-time jumps are bounded without
iterating once per second.

This is **an averaged approximation, not the physical correction schedule**.
The silicon modifies selected seconds: normal-mode corrections occur in a
two-hour cycle and fast-mode corrections in a four-minute cycle. The model
does not reproduce those minute/second pulse placements, correction interrupt
edges, oscillator drift, temperature response, CLKOUT, alarms or timer
corrections. CIE remains unsupported and rejects. NXP Table 16 also prints
"2" in a column where the surrounding text describes four-minute mode; this
extension does not pretend that the contradictory table proves an exact pulse
schedule. No physical clock accuracy or calibration completion is asserted.

## Sources and fresh checks

The retained NXP PDF and its original retrieval receipt are in
[`../research/tca9554-pcf85063-sources/manifest.json`](../research/tca9554-pcf85063-sources/manifest.json).
The exact two official component ZIPs, repository commits, original UTC/bytes/
SHA-256 and member hashes are in
[`../research/pcf85063-offset-sources/manifest.json`](../research/pcf85063-offset-sources/manifest.json).

- [Official PCF85063A driver 2.0.0](https://components.espressif.com/components/waveshare/pcf85063a/versions/2.0.0), component commit `9daffb7168b3c093a330da700988a22092294eef`: initialization only writes Control_1.
- [Official driver 2.1.0](https://components.espressif.com/components/waveshare/pcf85063a/versions/2.1.0), component commit `30d0ac3b8b6b27ebd402c5852819cbe0dab92749`: initialization additionally calls `pcf85063a_set_offset` with -13 / normal mode.
- [NXP datasheet, official Waveshare copy](https://files.waveshare.com/wiki/common/PCF85063A.pdf): numeric offset definition and calibration sign; the averaged model's limitations are stated above rather than attributed to the chip.

Eight protocol tests passed, including the unchanged six legacy tests and two
new checks of offset sign/mode, causal register updates, fractional deadlines,
OS preservation, reset, rejected interrupt enable and extreme guest cycles.
`cargo clippy -p esp-soc --all-targets -- -D warnings` passed. These checks are
separate from board original-firmware or public Worker acceptance.

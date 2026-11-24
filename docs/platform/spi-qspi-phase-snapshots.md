# Opt-in SPI and QSPI phase snapshots

`SpiQspiPins` describes the command, address, dummy, write and read phases submitted
to a GP-SPI controller, together with the physical CLK, D/Q/WP/HD and active-low
select routes. It is an opt-in transport for boards whose devices need these
distinctions. `SpiPins`, its `keep_cs` field and all existing callback defaults
retain their meaning. The new trait methods are appended after existing methods.

## Source and supported boundary

The ESP32-S3 adapter uses Espressif ESP-IDF v5.5.4, commit
`735507283d5b2f9fb363a1901172dbd9e847945d`:

- `components/soc/esp32s3/register/soc/spi_reg.h`: CTRL, USER, USER1, USER2,
  MS_DLEN, MISC and SLAVE fields.
- `components/soc/esp32s3/include/soc/gpio_sig_map.h`: FSPI and SPI3 signals.
- `components/soc/esp32s3/register/soc/io_mux_reg.h`: FSPI native pad groups.

Exact repository bytes, Git blob identifiers, retrieval timestamps and SHA-256
values are retained in
[`waveshare-spi-phase-sources/manifest.json`](../research/waveshare-spi-phase-sources/manifest.json).
The compressed files are deterministic gzip archives; the main source hash is
over decompressed bytes. Retrieval used the public GitHub connector's base64 file
response; these receipts do not claim an original HTTP response representation.

The shared route decoder must only be called by a chip adapter with a verified
matching register layout. This change installs that adapter for S3 only. It does
not infer C3, C5, C6, P4 or S31 pad numbers.

Widths are 0 for a disabled phase, or 1/2/4/8 for the programmed lane count. Bit
lengths remain explicit, so a board can reject non-byte commands or an unexpected
address/dummy phase. IO0..3 mean D/Q/WP/HD, which on S3 SPI2 have signal numbers
103/102/105/104. They are not four consecutively numbered signals. SPI2 supports
the documented native groups (CLK12, D11, Q13, WP14, HD9, CS10; or CLK36, D35,
Q37, WP38, HD33, CS34). SPI3 is matrix-only.

This is a byte transport, not a bit waveform model. Octal, DTR, non-default bit
order, multiple conflicting lane-width selectors, quad input swap and slave or
configuration modes are explicitly marked unsupported. Boards must check this
flag and their physical routes before accepting data. Clock idle/output-edge,
SIO and full-duplex flags are observations; the transport does not claim analog
timing or clock-edge calibration. Peripherals must separately enforce the modes
they implement. Unknown controllers cannot become compatible by opting in.

`hardware_cs` and `software_cs` distinguish hardware selects from low driven GPIO
selects; `cs` is their union. `keep_cs` captures MISC.CS_KEEP_ACTIVE. A low software
select is released by a subsequent physical GPIO change, not by this bit. A board
parser decides whether consecutive transfers form one device command and discards
an invalid packet until the appropriate physical select boundary.

## Submission and cancellation

The S3 bus captures routes and phase fields immediately before an accepted USR
write. A blocked DMA transaction retains that snapshot while later register and
matrix writes occur. A busy USR does not replace it. The opt-in path also preserves
an existing scheduled SPI2 wire deadline on busy USR; legacy timing behavior is
not changed. Both CPU FIFO and DMA payloads reach the same phase-aware callback.

DMA descriptor errors, an unbound DMA source, scheduled source reset/rebinding
and chip reboot discard the opt-in snapshot. No incomplete payload reaches the
board. The default phase-aware callback returns disconnected input (`0xff`) and
does not call the legacy callback. The legacy SPI route/callback selection remains
in use for boards that have not opted in.

## Verification

`esp32s3/src/bus/qspi_tests.rs` exercises real S3 MMIO writes, both hosts, native
and matrix routing, four independent data lanes, input inversion rejection,
phase enables/lengths, select kinds, `keep_cs`, unsupported widths/DTR/bit order,
DMA delay, register edits, repeated busy USR, scheduled cancellation, descriptor
failure and reboot. Separate probes verify that a legacy board never enters the
new callback and that an empty opt-in hook never falls back to legacy bytes.

Run `cargo test -p esp32s3 qspi_tests --lib`. These are protocol/transport tests;
they are not evidence that any product's factory firmware or physical display
has been validated. Product acceptance is recorded separately after fresh runs.

Fresh verification on 2026-11-23 with Rust 1.100.0: all 5 new transport tests
passed; the `spi` library test filter passed 102 tests with ignored tests enabled
and `external_` fixtures excluded. `cargo clippy -p esp-soc -p esp32s3
--all-targets -- -D warnings` passed. These were native debug-profile checks;
workspace release tests and CPU regression measurements remain integration gates.

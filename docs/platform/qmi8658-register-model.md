# QMI8658A and QMI8658C register model

The opt-in model in `esp-soc/src/devices/qmi8658.rs` implements a digital I2C
register subset for QST's QMI8658A and QMI8658C, using their respective Rev A
datasheets. It is separate from the existing S3 legacy `i2c::Qmi8658`; existing
boards do not change behavior merely because this module is present.

## Sources and corrected address selection

The primary documents were obtained again from QST on 2026-11-23:

- [QMI8658A, document 13-52-25, Rev A](https://www.qstcorp.com/upload/pdf/202301/13-52-25%20QMI8658A%20Datasheet%20Rev%20A.pdf),
  3,085,534 bytes, SHA-256 `bc371af17cd94fca481a3b40a89417776996b7baa50dacf20e9f53d5a6a57b85`.
- [QMI8658C, document 13-52-27, Rev A](https://www.qstcorp.com/upload/pdf/202210/13-52-27%20QMI8658C%20Datasheet%20Rev%20A%20%281%29.pdf),
  3,067,766 bytes, SHA-256 `9887d8827b1769dff31ad5d5c0c4672458d593af5309aa0ed183d38440f7defd`.

The original bytes, new retrieval receipts and two public Waveshare Arduino
driver files at commit `e0325e262a0c081ef554bd05d052c901ded58933` are retained as
deterministic gzip archives under
[`qmi8658-rev-a-sources`](../research/qmi8658-rev-a-sources/manifest.json).
Source hashes refer to decompressed bytes; archive hashes are separate.

**Correction:** an earlier board-address inference reversed the strap mapping.
Both primary PDFs §16.3.1 and their pin tables explicitly specify:

| SA0 at reset | 7-bit I2C address |
| --- | --- |
| Ground / low | `0x6B` |
| High | `0x6A` |
| Unconnected, with the documented internal reset-time pull-up | `0x6A` |

The A variant documents a 200 kΩ pull-up; C documents 2 MΩ. This reverses the
previous unsupported “ground means 0x6A” inference. A firmware accessing `0x6B`
on a grounded-SA0 board is not evidence of a firmware-versus-schematic conflict.
Board-specific address assignments must be checked against that board's original
schematic; this model does not answer at both addresses to hide a mismatch.
`Qmi8658::i2c_address(false)` returns `0x6B` as a board-construction helper.

## API and modeled behavior

Create A with `Qmi8658::new(Arc<ImuMotion>, guest_clock_hz)` and C with
`Qmi8658::new_c(...)`. The board owns the physical SDA/SCL, supply, reset and
address. Hardware I2C controllers and GPIO I2C should share one instance through
the existing `I2cBitBang` adapter rather than separate copies. The board must
update `ImuMotion.cycle` with guest time; host wall time is not used.

The implementation includes:

- Read-only identity and documented register defaults. `WHO_AM_I` is `0x05`.
  `REVISION_ID` is the detailed Table 21 value `0x7C`; overview Table 19 says
  `0x68`, a retained source conflict rather than a measured silicon revision.
- Soft reset at register `0x60`, value `0xB0`, and a 15 ms guest-time reset
  boundary. `0x4D` becomes `0x80` at completion. Table 27 and §5.9 specify `B0`;
  §7.4's prose contains `0B`. The register definition is used, and `0B` is
  rejected. During reset, identity/default reads remain possible and control
  writes other than reset are refused, an explicit model policy.
- CTRL1 address increment, read endianness and sensor-disable state; CTRL2/3
  accelerometer and gyro ranges/ODRs; CTRL7 enable, snooze and SyncSample state.
  C rejects A's reserved INT-enable bits and the unsupported ±2048 dps range.
- Nominal ODR scheduling using exact integer ratios. For example, setting 3 is
  1000 Hz for accelerometer-only, but 896.8 Hz with the gyro clock active.
  Acceleration uses 16384/8192/4096/2048 LSB/g; gyro scale follows Table 8.
  Timestamp counts samples of the enabled sensor with the highest ODR.
- STATUS0 new-data flags, consumed after the last byte of each complete XYZ
  group. This full-vector consumption boundary is the model's interpretation
  of Table 24's “since last read”; partial-vector silicon clearing is not claimed.
- SyncSample locking after the documented Table 40/41 ODR-dependent delay,
  with release at GZ_H when the gyro is enabled or AZ_H for accel-only. Samples
  during a lock are dropped, not retroactively delivered. CTRL9's AHB-gating
  configuration command retains the specified CAL1_L setting; internal clock
  gating glitches and electrical bus timing are not simulated.
- CTRL9 `COPY_USID` and `ACK` handshake. Completion takes one guest tick, an
  explicit digital scheduling policy because the source does not provide the
  copy-command execution time. Firmware version and USID bytes are virtual
  unknown values (all zero), not a fabricated manufacturer identity.

The virtual environment is stationary at `[0, 0, +1 g]`, zero angular rate and
25 °C by default. Motion mode 1 selects the existing deterministic motion script.
Those vectors are stimuli to the register model; they do not establish a physical
PCB-to-screen orientation or real sensor calibration. Sampling begins after one
configured ODR period; analog wake-up and filter settling durations are not
modeled. CTRL5 filter settings retain their digital values without simulating
the analog/filter transfer function. The fixed temperature is published with a
sensor sample, not an independently calibrated temperature clock.

### Explicit host sample input

`Qmi8658::set_host_sample(accel_g, gyro_dps)` accepts finite three-axis vectors
in the **sensor's** coordinate system. It holds that stimulus until another
host sample or an explicit `set_motion_mode(0|1)` replaces it. Selecting mode 0
restores the stationary default; mode 1 restores the deterministic script.
Invalid vectors and mode values leave the previous selection unchanged. A
sensor soft reset preserves the external stimulus, as resetting a sensor does
not move the physical environment around it.

The caller must first deliver deferred device ticks and publish the current
guest cycle to the board. The S3 browser ABI now does this on its cold input
path, including when no GPIO edges are delivered. The setter then updates
already-due conversions using the previous stimulus and installs the new value. It neither advances guest time nor retroactively
changes a sample timestamped before the input. The next configured conversion
uses the new held value. Disabled sensors remain disabled; range saturation,
byte order, data-ready consumption and SyncSample locking still apply. A new
host value cannot unlock a frozen SyncSample register block.

Boards that expose this API retain one `Rc<RefCell<Qmi8658>>` and attach a
`Qmi8658Port` to their physical I2C adapter. That port and the board setter
operate on the same object; this avoids an unsynchronized parallel sample
state or a second sensor instance. The existing constructors still work
without the shared port, and default samples remain unchanged. The legacy
S3 `i2c::Qmi8658` and the shared `ImuMotion` type are not modified.

Host injection establishes a digital sensor-input path. It does not establish
a measured PCB-to-screen rotation, a 3D-model pose calibration, sensor noise,
filter dynamics, or a motion-responsive factory UI. Those claims require
separate evidence for the particular board and firmware.

## Explicit limits

This subset does not implement FIFO streaming, calibration/self-test engines,
tap/wake/motion/pedometer engines, offset calibration, sensor noise, interrupt
pin waveforms, SPI or I3C transport. Unsupported enabling writes/commands are
NACKed and do not raise `CmdDone`. FIFO size/watermark settings can be retained
while the FIFO stays in bypass, but cannot generate a fake sample stream.

Outside SyncSample mode, STATUSINT exposes only its CTRL9 command-done bit.
DRDY pulse width and INT1/INT2 external levels are not fabricated. Firmware that
requires those signals needs a separately implemented source-backed interrupt
path; it cannot be considered verified by this register subset. STATUS0 remains
available for ordinary data-ready polling. Unknown register pointers and writes
to read-only registers are rejected.

The public Waveshare Arduino driver retained with the sources probes both I2C
addresses. Its `CTRL1=0x60` configuration sets the documented BE bit, while its
sample conversion code uses low-byte-first expressions. The model follows the
primary register definition; that example's behavior is not used to redefine BE.
Any resulting factory behavior must be observed in a fresh exact-image run.

## Verification

`cargo test -p esp-soc qmi8658 --lib` covers reset deadlines, register identity,
RO rejection, command completion/ACK and unsupported commands; range and variant
differences; exact ODR, byte order and data consumption; and SyncSample lock
delay, frozen values and release. Product factory execution and host motion
interaction are separate board-level checks. No historical firmware result is
used as acceptance for this implementation.

Initial register-model verification on 2026-11-23 with Rust 1.100.0: 4 protocol tests passed and
`cargo clippy -p esp-soc --all-targets -- -D warnings` passed. These are native
debug checks; workspace release and board-firmware acceptance remain separate.

### Host input verification extension

The new shared tests cover held samples, invalid/non-finite input, saturation,
byte order, reset persistence and unchanged SyncSample locks. A separate native
board integration test configures both real S3 I2C controllers and their GPIO
matrix routes on the two 1.28 products and all six TCA RGB products. It injects
two host vectors, executes Xtensa `L32I.N` instructions against the I2C receive
FIFO and checks the exact signed sample words. Wrong SCL and the unstrapped
`0x6A` address must NACK. Switching to the stationary mode restores its original
register values.

The S3 browser ABI boundary test puts one actual deferred CPU cycle across both
a 16 MHz system-timer tick and a 500 Hz QMI conversion. The input call must flush
the system timer, preserve the already-due old sample and publish the new input
only on the next conversion. It checks boards with GPIO-edge delivery enabled
and disabled, and verifies no instructions or guest time are added.

Fresh execution receipts, the eight-board values and representative factory
default comparisons are recorded in
[`waveshare-qmi-host-input-2026-11-23`](../evidence/waveshare-qmi-host-input-2026-11-23/manifest.json).
These are digital input tests; the earlier factory UI status and uncalibrated
physical pose limits are retained.

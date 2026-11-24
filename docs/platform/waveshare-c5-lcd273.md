# Waveshare ESP32-C5-LCD-2.73

`waveshare-c5-lcd273` is a source-wired, experimental C5 profile with **32 MiB
flash and 8 MiB PSRAM**, a non-touch 320×320 ILI9488 display, three GPIO keys
and SPI microSD. The project's original SRAM diagnostic has exercised those
register paths. The manufacturer's unchanged factory image remains a separate
reference because ROM startup stops before its application.

## Exact hardware and source boundary

The fixed repository is
[`waveshareteam/ESP32-C5-LCD-2.73@418de818f4f40c5a04c4292dbe293bfaa1351068`](https://github.com/waveshareteam/ESP32-C5-LCD-2.73/tree/418de818f4f40c5a04c4292dbe293bfaa1351068).
The canonical overview lists base SKU35224 and battery-free `-EN` SKU35225.
The schematic identifies the ESP32-C5-WROOM-1-N32R8 module. Both factory image
headers select 32 MiB flash; the physical PSRAM remains 8 MiB and is exercised
in full by the independent diagnostic. The shared C5 profile uses its sourced
nominal 48 MHz crystal. Firmware and document dates are not product release
dates or proof of an unrecorded hardware revision.

Original source bytes, download timestamps, SHA-256, verified Git blob hashes
and compressed-archive hashes are in the
[source manifest](../research/waveshare-c5-lcd273-sources-2026-11-23/manifest.json).
The repository tree is retained as the connector's retrieved JSON content,
separately labeled from raw HTTP responses. The ILI9488 manufacturer PDF is
retained locally with its receipt and cited pages; the restricted PDF is not
redistributed in the source tree.

| Function | Actual connection and bounded implementation |
| --- | --- |
| Display | ILI9488, 320×320 glass; controller GRAM320×480×18 bits. SPI2 SCLK6, MOSI7, CS8, DC4. Reset is CH32 EX0; brightness uses CH32 PWM command05. |
| Serial pixel format | Three complete bytes per RGB666 pixel; each component uses its upper six bits. The source driver explicitly converts RGB565 into this wire format. The profile does not reinterpret RGB565 transfers as valid ILI9488 serial pixels. |
| Native view | Actual bundled `mirror(false,false)` sets MX. Final source MADCTL48/BGR and INVON define this profile's reference view, with no gap. Other guest MX/MY/inversion values remain observable. Physical color and scan calibration are unverified. |
| CH32V006 | I²C24h on SDA27/SCL26. Sixteen-bit little-endian direction02, output03, input04; byte PWM05. Upper and lower startup direction/output bytes remain unknown until explicitly written. |
| QMI8658C | I²C6Bh, grounded SA0. The canonical resource identifies the C variant. The shared protocol advances in guest time; the diagnostic checks WHO_AM_I only. No physical axis calibration is claimed. |
| PCF85063A | I²C51h, guest-cycle clock. STOP is forwarded to release burst snapshots. No RTC application was exercised in this batch. |
| PREV | Active-low GPIO2, schematic KEY_L, 10 kΩ pullup. Host control `prev`. |
| NEXT | Active-low GPIO3, schematic KEY_R, 10 kΩ pullup. Host control `next`. |
| BOOT/ENTER | Active-low GPIO28, 10 kΩ pullup. Host control `boot` or `button`. |
| microSD | Shared SPI2 SCLK6/MOSI7, MISO5, CS9. Real CS routing and MISO input selection apply; simultaneous LCD and SD selection is rejected. |

The canonical text calls the IO MCU CH32V003, but the schematic fits
**CH32V006K8U7** and this exact factory repository's `io_extension` sends full
16-bit values. The independent V006 model is used. Its initialized output
cache in the C driver is not treated as evidence of hardware power-on state.
Known-byte masks preserve unknown upper state when firmware writes only the
low byte. Read command04 reports configured pin levels; it does not invent
wire readback for the host driver's cached direction/output registers.
ADC06, RTC07 and unsupported commands reject. PWM has a configured duty byte,
without a claimed physical carrier frequency.

This board has a discrete regulator/charger/power latch. Its declared virtual
scene supplies USB-derived nominal3V3 without a battery. LCD reset has the
schematic pullup; quiet sensor/RTC interrupts and charge-detect are high in
the declared scene; unattached header inputs are low. These are explicit
virtual inputs, not an analog charger simulation or measured hardware defaults.
SHTC3 and power-key behavior are absent. No AXP2101, FT6336, CST3530 or codec
is inherited from the superficially similar Touch3.5 board.

## ILI9488 implementation

The primary document is
[ILI9488 V100](https://files.waveshare.com/upload/2/2d/ILI9488_Data_Sheet.pdf),
SHA-256 `aeb23170f809610458e05ab514a5e3017344938ccdba1c2031acb453941df38b`.
Pages43,48–52 describe four-wire serial framing: a CS pause after a complete
byte preserves the stream, and only complete pixels enter memory. The model
therefore preserves a partially assembled pixel between complete-byte SPI
transactions. Page179 §5.2.24 defines RAMWR window traversal and ignoring
excess pixels; the implementation does not wrap excess data onto the first
pixel. Page192 defines MADCTL bits; page200 gives DBI format bits and reset
COLMOD06. Page282 establishes the full 320×480×18-bit GRAM.

The model retains all six component bits internally. The existing host display
API exports RGB565, dropping the sixth red/blue bits; this quantization is
explicit. The visible source configuration uses the first320 GRAM rows.
Undefined reset RAM becomes a deterministic zero placeholder in the virtual
scene, without a retention claim.

This bounded parser implements MV0 window writes, MX/MY storage mapping,
sleep/display state, inversion and RGB666. MV1, RGB565 serial data and RAMWRC
are rejected. Reads, tearing-effect timing, scrolling, analog gamma and other
panel scan modes are not modeled. Unmodeled analog initialization commands
have no claimed display effect. No legacy ST7789 RAMCTRL behavior is applied.

## Native guest acceptance

The project-authored
[diagnostic](../../firmware/c5-lcd273-probe/README.md) uses the public C5 App,
GPIO-script and SD APIs. Its4224-byte image has SHA-256
`44cf80f57df4227ac568084955bb4a26aec49803e2e7390c9e845ab5fc444fcd`.
It performs real C5 MMIO, PSRAM/MMU setup, GDMA descriptors, I²C writes and
GPIO reads. No host drawing call or firmware-function hook replaces the guest.

| Measurement | Fresh observed result |
| --- | --- |
| PSRAM | Entire8,388,608 bytes written/read by the guest, followed by its framebuffer in mapped PSRAM |
| Transfer | 307,200 RGB666 wire bytes via PSRAM-to-GDMA-to-SPI2, nominal24 MHz; I²C400 kHz |
| Frame | 320×320,5,120 distinct host RGB565 colors |
| Quiet control | Zero changed pixels |
| PREV GPIO2 | 26,752 changed pixels; release restores every pixel |
| NEXT GPIO3 | 26,752 changed pixels; release restores every pixel |
| BOOT GPIO28 | 73,728 changed pixels; release restores every pixel |
| Execution | 226,000,000 retired/dispatched instructions, zero exceptions/resets |
| SD | Actual CMD0/CMD8/ACMD41/CMD17/CMD24; three reads and two writes of LBA32 |
| Card preservation | Original block saved, test pattern read back, original block restored/reread; entire8 MiB nonzero sample card byte-identical after completion |

The SD workload does not mount a filesystem and does not promise transactional
recovery if execution stops before restoration. Protocol tests additionally
exercise wrong routes, absent device addresses, unsupported CH32 commands,
reset/backlight effects, all three electrical inputs, RGB666 partial pixels,
window overflow and unsupported pixel modes.

The final targeted C5 release suite passed53 tests, including its actual ROM
test. Release all-target Clippy passed with warnings denied. These are C5
checks, not a whole-workspace gate or CPU performance comparison. The
[compact receipt](../evidence/waveshare-c5-lcd273-2026-11-23/inputs-and-hashes.json)
binds source/input hashes and execution counts. Raw images, RGB565 buffers,
UART and test logs remain under ignored `target/c5-lcd273-2026-11-23/`, with
a reproducer and hashes in Git. No remote archive upload is claimed.

## Factory boundary and separate firmware roles

The unchanged `ESP32-C5-LCD-2.73_FactoryOnly_20260724.bin` is3,346,992 bytes,
SHA-256 `cf10a4358d03898458c16390f3f16ceb0306befe2260160de0919d755b920942`.
Its official instructions load the merged file at0. Bootloader2000h,
partition table8000h and application20000h are independently parsed;
the later OTA partition has no image in this merged file. Both present
images pass checksums and appended digests, select32 MiB flash and target
C5 chipID23/revision100–199. The app identifies IDFv5.5.3, July24,2026.
Shared C5 register models cite separately preserved IDFv5.5.4 sources; this
batch does not claim complete factory compatibility between SDK versions.

Raw native placement at0 with the unchanged revision100 ROM executes6831
instructions, then faults reading PMU600b00e4 at PC4001df26 in
`pmu_enable_unhold_pads`. No UART bytes or display pixels were produced.
The [factory record](../evidence/waveshare-c5-lcd273-2026-11-23/factory-validation.json)
keeps that failure distinct from SRAM diagnostic success. The public loader
remains SRAM-only, and the factory stays `reference`.

No exact LCD2.73 asset was found among the retrieved Xiaozhi v2.6.0 release's
200 assets. No exact UIFlow package was found in the inspected fixed README,
firmware instructions and canonical overview/resources. These are bounded
search results; another chip or touch model is not substituted.

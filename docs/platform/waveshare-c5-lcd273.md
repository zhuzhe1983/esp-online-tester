# Waveshare ESP32-C5-LCD-2.73

`waveshare-c5-lcd273` is a source-wired, experimental C5 profile with **32 MiB
flash and 8 MiB PSRAM**, a non-touch 320×320 ILI9488 display, three GPIO keys
and SPI microSD. Normal ROM startup now executes the manufacturer's unchanged
IDF5.5.3 SD and LCD reference examples, separately from the project's original
SRAM diagnostic. The SD example mounts FAT32 and verifies a71-byte file; the
LCD example produces its real LVGL configuration-warning screen. The unchanged
FactoryOnly image reaches `app_main` in strict mode and stops during Wi-Fi
MAC initialization. An explicit offline driver compatibility profile now runs
its original menu, all three GPIO keys and Photo JPEG reads/decoding/display
from the virtual TF card in native and production browser execution. The ROM
loader remains experimental; the original Sensor page has known timing and
byte-order failures, and complete factory or RF compatibility is not accepted.

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
| PCF85063A | I²C51h, guest-cycle clock. STOP releases burst snapshots. The original factory logs successful initialization; calendar/application behavior remains unverified. |
| SHTC3 | I²C70h on SDA27/SCL26. Timed no-stretch subset with a declared virtual25°C/50%RH environment, product ID0807 and word CRC. Original factory wake/ID now succeeds; full UI/environment behavior remains unverified. |
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
Power-key behavior is absent. No AXP2101, FT6336, CST3530 or codec is inherited
from the superficially similar Touch3.5 board.

The [SHTC3 model and source receipt](../evidence/c5-shtc3-2026-11-25/README.md)
use guest cycles for startup/wake/reset240µs, normal conversion12.1ms and
low-power conversion0.8ms, choosing the datasheet maximums as an explicit
deterministic timing policy. The sample is latched when measurement starts;
busy addresses NACK. CPU/controller resets preserve the powered sensor.
Stretch commands, general call, serial-number queries and result retries
without a fresh command are outside this subset. The I²C callback interface
does not model per-byte master ACK waves, and large controller catch-up uses
the current guest-time horizon rather than calibrated byte phases. A fixed
virtual environment and successful ID response are not physical calibration.

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

The November23 targeted C5 release suite passed53 tests, including its actual ROM
test. Release all-target Clippy passed with warnings denied. These are C5
checks, not a whole-workspace gate or CPU performance comparison. The
[compact receipt](../evidence/waveshare-c5-lcd273-2026-11-23/inputs-and-hashes.json)
binds source/input hashes and execution counts. Raw images, RGB565 buffers,
UART and test logs remain under ignored `target/c5-lcd273-2026-11-23/`, with
a reproducer and hashes in Git. No remote archive upload is claimed.

## Normal ROM references with the original IDF5.5.3 BSP

The fixed manufacturer's `example/ESP-IDF-V553/03_sd_card` and `02_lvgl_demo`
were built without changing their source, effective sdkconfig, dependency lock
or BSP. Both happen to use the CMake project name `01_factory`; that name does
not identify the separately distributed FactoryOnly binary. The
[exact build inputs and full BIN/ELF/map hashes](../evidence/c5-lcd273-idf553-examples-2026-11-25/README.md)
pin SDK5.5.3, its recommended GCC14.2 toolchain, button4.1.7 and LVGL9.5.0.
Their normal ROM/bootloader startup uses32MiB Flash and all8MiB of PSRAM;
no app-direct mode, ROM stubs or firmware-function hooks are selected.

The unmodified SD program mounts the prepared64MiB FAT32 card, writes71 bytes
to `/test.txt`, closes it, reopens it, reads the file, checks the length/error
and compares its content before printing `TF CARD TEST PASS`. The exported
card contains that exact71-byte payload. A real browser run also flushes the
changed pages to IndexedDB; reloading the same origin and reselecting the
same firmware restores a byte-identical card before starting another guest.
This verifies virtual file I/O and browser retention for this particular
program, rather than all SD commands or physical electrical timing.

The unmodified LCD program initializes ILI9488, the original display buffers,
LVGL and brightness control and produces a real320×320 framebuffer. Its
original configuration disables `LV_USE_PERF_MONITOR`, so the actual screen
says **“LV_USE_PERF_MONITOR is not enabled”**. This is basic display output,
not a functioning LVGL benchmark or a three-button application. Keep that
configuration and this limitation when reproducing the reference.

The SPI and zero-only HP-selected LP_AON mux fixes that enable these unchanged
programs are documented in the
[source contracts and native results](../evidence/c5-lcd273-spi-lp-2026-11-25/README.md).
SD diagnostic counters count inner command calls and backing-store loads or
writes. They do not count every SPI wire command, prove response consumption
or replace the guest file comparison and exported-card checks. Published
snapshots may precede the final paused CPU state.

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

The initial November23 raw native placement at0 with the unchanged revision100 ROM executes6831
instructions, then faults reading PMU600b00e4 at PC4001df26 in
`pmu_enable_unhold_pads`. No UART bytes or display pixels were produced.
The [factory record](../evidence/waveshare-c5-lcd273-2026-11-23/factory-validation.json)
keeps that historical failure distinct from SRAM diagnostic success.

The later [native integration checkpoint](../evidence/c5-coex-gate-2026-11-25/integration.md)
executes the unchanged ROM, bootloader and application through the full PSRAM
test, real FreeRTOS ticks, ordinary watchdog initialization and `app_main`.
RTC/QMI initialization and the sourced virtual SHTC3 ID0807 are observed.
The subsequent [Wi-Fi clock checkpoint](../evidence/c5-wifi-clock-requests-2026-11-25/integration.md)
executes all four MODEM_SYSCON `CLK_CONF1` requests (`600a9c14`), then
stops as a ModelLimit at `408142c6`, reading private MAC word `600ad858`.
Its whole-word POR/readback contract is not publicly documented in the
checked C5 sources, so the model does not invent a value. Both these
requests and the earlier LPCON COEX/WIFIPWR request are retained without
synthesizing RF/PHY readiness.
In default strict mode this original image still stops before LCD or SD
output. Complete revision100 C5 Flash images can be loaded through the
experimental ROM path; SRAM application direct startup remains separate.
The catalog entry stays `trial`; the opt-in results below do not certify
strict hardware, other C5 products or every factory application.

No exact LCD2.73 asset was found among the retrieved Xiaozhi v2.6.0 release's
200 assets. No exact UIFlow package was found in the inspected fixed README,
firmware instructions and canonical overview/resources. These are bounded
search results; another chip or touch model is not substituted.

## Explicit offline driver compatibility

Select **Wi-Fi 驱动兼容（离线）** and apply the mode before reloading the fixed
factory image. Strict mode remains the default. The
[configuration and API contract](c5-wifi-driver-compat.md) describes the pinned
`waveshare-lcd273-idf553-offline-v1` profile and its visible runtime badge.

The unchanged original ROM, bootloader and factory application now produce
their real menu. Guest-timed PREV/NEXT/BOOT GPIO pulses change visible focus,
open Photo and return through its Back control. On a separate prepared64MiB
FAT32 card, the original application reads a5200-byte JPEG, decodes it and
sends the resulting picture and controls through SPI to ILI9488. Native and
production-browser RGB565 frames match exactly; the browser's normal reload
preserves the complete card without another import. See the
[separate browser runs and retained failures](../evidence/c5-factory-browser-2026-11-25/README.md)
and [cached-memory correction](../evidence/c5-factory-photo-2026-11-25/README.md).

This is driver-assisted offline execution, with no hotspot association or IP.
Wi-Fi configuration is session-only RAM. It does not modify BIN/ROM bytes,
clear guest NVS/card data or supply host-drawn images. The API signatures,
physical Flash mapping guards and differences from the original driver are
recorded in the [ABI contract](../evidence/c5-wifi-driver-compat-2026-11-25/README.md).

The original Sensor page opens, but its displayed measurements are not
accepted as correct. Two actual SHTC3 reads occur about11.23ms after the
measurement command, before this model's declared12.1ms maximum conversion
deadline. The sensor NACKs; the factory driver ignores the failed receive
and converts its untouched zero buffer to−45°C/0%RH. For QMI8658, the original
code selects big-endian output but decodes little-endian, giving about0.04m/s²
and0.10°C for the model's+1g and25°C bytes. The model keeps its sourced timing,
NACK and byte-order behavior; [source and actual transaction evidence](../evidence/c5-factory-sensor-2026-11-25/README.md)
retain these failures. They are separate from Photo and key acceptance.

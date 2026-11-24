# Waveshare ESP32-C5-Touch-LCD-3.5

`waveshare-c5-touch35` is an experimental, source-wired C5 board profile. The
project's original SRAM diagnostic exercises its actual register paths, full
8 MiB virtual PSRAM, PSRAM-to-GDMA display transfers, FT6336 input, BOOT and
SPI microSD. The unchanged manufacturer factory image is a separate reference:
it still stops in the C5 ROM before reaching its application.

## Hardware identity and sources

The fixed manufacturer repository is
[`waveshareteam/ESP32-C5-Touch-LCD-3.5@a39067a9b897f54c2b8dccb597542caa4e826349`](https://github.com/waveshareteam/ESP32-C5-Touch-LCD-3.5/tree/a39067a9b897f54c2b8dccb597542caa4e826349).
The schematic shows **ESP32-C5-WROOM-1U-N32R8**, and the README, factory build
configuration and image headers agree on **32 MiB flash and 8 MiB PSRAM**.
The Arduino instructions' 16 MiB selection is a limitation of that menu; it
does not reduce the physical module capacity. The crystal is the existing
C5 profile's sourced nominal **48 MHz**.

The canonical overview lists base SKU **35417**, battery-free `-EN` **35416**,
camera `-C` **35419** and camera/battery-free `-C-EN` **35418**. This profile
implements the common display/IO board and the base model has no camera.
The manufacturer's firmware README expressly names all four variants as
compatible, but this work has not emulated a camera or tested battery operation.

Original source bytes, retrieval times, Git blob identities where available,
SHA-256 and deterministic compressed archive hashes are retained in the
[source manifest](../research/waveshare-c5-touch35-sources-2026-11-23/manifest.json).
It distinguishes HTTP response bytes from the GitHub connector's JSON content.
The linked source files are not reconstructed responses.

| Function | Exact board connection and modeled behavior |
| --- | --- |
| ST7796 LCD | 320×480; SPI2 SCLK GPIO6, MOSI7, CS8, DC5; reset CH32 EX0. DBI RGB565 uses the actual init list's COLMOD05, also accepts55; 66's RGB666 is explicitly unsupported on this profile. |
| Native view | The fixed BSP explicitly selects portrait, MX=true, BGR and INVON. The output scan convention corresponds to that source configuration; guest changes to MADCTL/inversion remain observable. Physical color calibration and alternative panel scan registers are not claimed. |
| FT6336 | I²C38h, SDA27/SCL26, reset EX1, active-low IRQ GPIO3. One host contact, coordinates320×480; the manufacturer's zero axis flags require no touch swap/mirror. |
| CH32V006 | I²C24h, same SDA27/SCL26. Explicit 16-bit little-endian direction02, output03, input04 and byte PWM05. LCD backlight is its PWM net, not C5 LEDC. |
| AXP2101 | I²C34h. Schematic DCDC1 output is VCC3V3 feeding LCD, touch, CH32, QMI, codec and SD. The digital board scenario accepts the sourced nominal3300mV request; other voltages are outside this board model, not measured electrical thresholds. |
| QMI8658 | I²C6Bh, grounded SA0. The physical A/C suffix is unconfirmed; the stricter shared A/C register intersection is used. The SRAM diagnostic verifies WHO_AM_I only. |
| PCF85063A | I²C51h, separate RTC rail. Its shared virtual clock advances with guest cycles. I²C STOP is forwarded to release calendar read snapshots. |
| ES8311 | I²C18h control-register model. No PCM playback or microphone samples are synthesized. |
| microSD | SPI2 shared SCLK6/MOSI7, MISO2, CS9. Simultaneous LCD+SD select is rejected. SD input follows the real MISO route; hardware keep-CS and GPIO select both matter. |
| BOOT | GPIO28 active-low; host `boot` or `button`. |
| PWR | SYS_OUT from the external power/inverter circuit reaches CH32 EX6. It is not registered as an ordinary GPIO button. The declared scene has an unpressed key. |

### Source conflicts and honest boundaries

The physical expander is **CH32V006K8U7**, with sixteen EXIO nets in the
schematic and full two-byte accesses in the dedicated `06_exio` example.
The current factory BSP nevertheless calls the old `custom_io_expander_ch32v003`
1.0.1 component and writes only one value byte. The board implements those
observed low-byte accesses while preserving the upper byte independently.
It never inherits the V003's assumed 8-bit startup defaults: V006 direction
and output begin unknown until the guest configures the corresponding byte.
Unconfigured pins are not silently treated as known outputs. The input scene
is explicit: R41 pulls LCD reset high in input mode, inactive interrupt nets
are high, PWR is unpressed, and unconnected header inputs are low. It is not
a measurement of an attached external circuit.

CH32 command06 ADC and command07 RTC, unknown commands, output-register wire
readback and excessive payloads reject. The official host component caches
direction/output; that is not proof of physical readback support. PWM retains
the configured duty byte, but no physical carrier frequency is inferred.

The board's schematic and README call the sensor QMI8658 without a suffix.
The repository contains a QMI8658A PDF and a board example named `qmi8658c`;
that example also defines interrupt bits reserved by the C datasheet.
The name alone cannot certify the fitted variant. Using the shared C model
here conservatively rejects A-only CTRL1 bits and the A-only2048dps range;
other implemented common registers keep their sourced behavior. Unsupported
FIFO/calibration/IRQ generation is not bypassed. The sensor's static virtual
scene and host motion are not calibrated to physical board axes.

The FT6336 subset follows the exact bundled driver, which reads count02 then
six bytes from03. Those separate transfers share a stable contact snapshot.
Finishing the report (or reading zero count for a release) consumes only that
snapshot's pending event; a newer event remains pending. This is a digital
host-input contract, not a simulated scan clock or IRQ pulse width. Gesture
enableD0 is stateful after an explicit guest write; D3 can be cleared/read as
no gesture. No gesture algorithm, pressure or unknown vendor/firmware ID is
invented. Unknown register writes and identity requests reject.

AXP uses the shared model's explicitly declared USB5V/no-battery scenario
and published reference startup rail preset; it is not a measured board
EFUSE dump. Disabling DCDC1 gates the main-rail devices. LCD/FT/CH32 and SD
lose their digital state; the QMI and codec devices are reconstructed on that
power epoch. RTC retains its independent rail. This does not implement
whole-system CPU poweroff, analog ramps, brownout, battery charging or PWR
long-press timing. SHTC3, camera and external EXIO peripherals remain absent.

## Fresh guest verification

The original diagnostic lives in
[`firmware/c5-touch35-probe`](../../firmware/c5-touch35-probe/README.md).
Its4688-byte App image has SHA-256
`9395b2b72f506fabdcbe238f1a8fe39ce7825ecf78ab535c572d054f45f3b1e3`.
It executes through the real C5 App loader and register/bus models, using
32F8P configuration. It does not patch ROM, call host drawing functions, fake
controller completion or use firmware-function hooks.

| Measurement | Observed result |
| --- | --- |
| PSRAM guest writes and reads | Entire8,388,608bytes, then framebuffer stored in that mapped PSRAM |
| Frame transfer | 307,200bytes RGB565, actual GDMA descriptors and SPI2; nominal24MHz SPI/400kHz I²C |
| Image | 320×480,7,680 distinct RGB565 colors |
| Quiet control | Zero changed pixels |
| Touch80,110 | 1,024 changed pixels; release restores every pixel |
| BOOT28 | 119,808 changed pixels; release restores every pixel |
| Execution | 170,250,000 retired/dispatched instructions, zero exceptions/resets |
| SD transport | Actual CMD0/CMD8/ACMD41/CMD17/CMD24; three reads and two writes of LBA32 |
| SD preservation | Guest saves original block, tests a pattern, restores and rereads it; entire8MiB nonzero initial card remains byte-identical after completed run |

The SD test is a raw-block diagnostic, not a filesystem mount. Terminating it
between test write and restore is not transactional storage protection.

The [compact result](../evidence/waveshare-c5-touch35-2026-11-23/inputs-and-hashes.json)
records source/input/artifact hashes and reproducible commands. Raw UART,
PNG and framebuffer captures remain under ignored
`target/c5-touch35-2026-11-23/`; they are not embedded in Git evidence.
No remote evidence archive is claimed. The reproduction script regenerates
the captures and verifies their byte hashes.

## Manufacturer firmware remains a reference

The fixed merged `ESP32-C5-Touch-LCD-3.5_FactoryOnly_20261022.bin` is8,580,095
bytes, SHA-256
`4aaac1808cfc5fe7f7e518e5a700dcd2df49395875427f9bc96d76f36b766aec`.
It targets C5 chipID23, full revision100–199,32MiB flash. Bootloader at2000h,
partition table8000h and application10000h all match the unmodified image;
checksums and appended digests pass. The app identifies IDFv5.5.4 and build
October22,2026. These are image checks, not execution acceptance.

With the unchanged public revision100 ROM ELF and raw native placement at0,
execution takes6831instructions and faults reading PMU600b00e4 at PC4001df26
in `pmu_enable_unhold_pads`, before any UART or display activity. The
[factory validation record](../evidence/waveshare-c5-touch35-2026-11-23/factory-validation.json)
retains this failure. The public C5 loader remains SRAM-only. Factory desktop,
Xiaozhi networking, RF, camera and audio therefore are not marked verified.

The independently identified Xiaozhi v2.6.0 package is a linked candidate,
not a downloaded or simulated manufacturer factory image. No exact UIFlow
package was found within the inspected pinned README, firmware instructions
and canonical overview/resources; that scope is recorded, not asserted as a
universal absence.

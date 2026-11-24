# Waveshare ESP32-S3-Touch-AMOLED-1.43C runtime

This is an opt-in board model for the 1.43C, using its own fixed schematic and
factory BSP. The original 2026-10-27 factory image boots from the mask ROM, opens
the nine-application round desktop, accepts a real CST820 drawing trajectory and
returns to the desktop when BOOT is pressed. The result is partial peripheral
support, not acceptance of the radio, audio signal path or every application.

## Source identity and integration

Waveshare repository revision: `8cea11ddae6167dd591d1f5465fd7b615080360f`.
The public original source bytes, acquisition times and individual SHA-256 values
are in [the source manifest](../research/waveshare-amoled143c-sources/manifest.json).
Each gzip contains the exact original response; the manifest distinguishes its
decompressed SHA from the gzip wrapper SHA. The factory BIN remains a remote
link and is not copied into this repository.

The current [official overview](https://docs.waveshare.com/ESP32-S3-Touch-AMOLED-1.43C)
lists SKU 34383 with a lithium battery and SKU 34384 `-EN` without a battery.
The same page specifies the same electronics. This adapter tests USB-powered
digital behavior; it does not model battery inclusion or state of charge. The
exact PCB revision is unspecified. Neither the repository date, schematic date
nor the factory filename is asserted to be the product launch date.

Central registration tuple:

| Item | Value |
| --- | --- |
| Module / constructor | `waveshare_qspi::WaveshareQspi::new(QspiVariant::TouchAmoled143C)` |
| Runtime name | `waveshare-s3-touch-amoled143c` |
| SoC | ESP32-S3-PICO-1-N8R8, two LX7 cores |
| Flash / PSRAM | 8 MiB / 8 MiB octal, from the exact BSP SDK configuration |
| Display | CO5300, QSPI, 466 × 466 RGB565, nominal round aperture |
| Touch | CST820, one contact |
| Host button | `boot`, GPIO0, active low |
| Physical TF slot | None in this board schematic and board layouts; pinned BSP header line 68 explicitly sets `BSP_CAPS_SDCARD 0` |
| IMU / host pose | None |

The opt-in `QspiDcsPanel` wraps the existing CO5300 GRAM/display-state engine;
`esp-soc/src/devices/co5300.rs` and its existing callers are unchanged.
The board uses the full accepted-USR phase/pad snapshot, including `keep_cs`,
through `BoardModel::spi_transfer_qspi_pins`. It never falls back to a single-lane
callback to claim that four physical data lines were checked.

## Exact wiring and power

The [schematic](https://raw.githubusercontent.com/waveshareteam/ESP32-S3-Touch-AMOLED-1.43C/8cea11ddae6167dd591d1f5465fd7b615080360f/hardware/schematic/ESP32-S3-Touch-AMOLED-1.43C-Schematic.pdf)
contains one circuit page and two PCB layout pages. The circuit page, the BSP
header and the factory configuration agree on the following connections.

| Device / signal | Connection | Modeled behavior |
| --- | --- | --- |
| LCD SCLK / IO0 / IO1 / IO2 / IO3 | GPIO14 / 9 / 10 / 11 / 12 | Each participating physical route checked |
| LCD CS / RESET | GPIO15 / 13 | Hardware or software CS; reset must be released |
| LCD TE | GPIO8 | Wire known; no synthetic TE waveform added |
| Shared SDA / SCL | GPIO47 / 48 | Both I2C controllers view the same physical devices; GPIO bit-banging shares those devices |
| CST820 address / RESET / IRQ | `0x15` / GPIO16 / GPIO17 | Bounded report reads, reset gating, falling-edge host input notification |
| ES8311 / ES7210 address | `0x18` / `0x40` | Existing source-backed digital control subsets |
| Codec rail enable | GPIO18, RT9193 U4 EN, R18 10 kΩ pull-up | Floating GPIO leaves power on; driven-low power cycles both digital register models |
| BOOT K1 | GPIO0, external pull-up | Active-low guest input |

Codec state is shared between I2C0, I2C1 and GPIO bit-banging. A rail generation
counter resets the codec register state after an actual modeled off/on transition,
even if no I2C access occurred while the rail was off. This is digital power gating,
not simulation of the LDO rise time, noise, microphone current or audio samples.
The independent PWR button, charging circuitry, GPIO5 LED and battery ADC are not
enabled as host controls by this adapter.

## Serial protocol and coordinates

The CO5300 V0.01 data sheet, the exact Espressif `esp_lcd_co5300` component 2.2.0
and ESP-IDF v5.5.4 transport are archived in the source manifest. The latter uses
32 header bits sent through the normal one-wire data phase, keeps CS asserted,
then submits one-wire parameters or four-wire pixel chunks. It does not require
the header to be encoded as the SPI peripheral's separate command/address phases.

The model checks phase widths, bit counts, byte alignment, unsupported bus modes,
clock mode, dummy cycles, read phases, all participating output pads, physical CS,
and reset. A malformed phase discards the rest of that CS assertion. A later
pixel chunk whose first bytes happen to resemble a header cannot resynchronize
the packet. A real CS deassertion starts a new packet.

Supported packet headers are `02 00 CMD 00` and `32 00 2C/3C 00`. Four-wire
addresses, QPI, dual-lane transfer and SPI reads are outside this subset. Vendor
page selection is tracked, so a private register on a nonzero page cannot be
mistaken for a standard DCS command. Vendor voltage/gamma bytes are counted as
opaque; they are not reported as completed analog work.

The visible window is x=8…473, y=0…465. The exact BSP applies MADCTL `C0h` and
uses this as its digital presentation reference. MX/MY/BGR changes are relative
to that reference. Only the visible window is stored; it is not an assertion of
the complete silicon GRAM dimensions. The data sheet's RDDMADCTL summary and
write-command wording disagree, and the component supplies an MV bit that the
data-sheet write table calls don't-care. A later bounded extension supports the
manufacturer BSP's bit-5 row/column exchange for square visible windows as an
explicit digital presentation policy. It composes exchange and mirrors with
the inverse fixed board reference; it does not discard a guest orientation bit.
Rectangular exchanged address ranges remain unsupported and reject pixel writes.
The 1.43C factory continues using C0h, so its existing presentation is unchanged.
The source conflict remains unresolved at the physical-glass level; this is not
a calibration claim. The independently pinned C6 2.16 source at commit
`294543798f1a44e2f2c4d2976522323f2beee11d`, file
`02_Example/ESP-IDF-v5.5.3/08_LVGL_V8_Test/components/port_bsp/display_bsp.cpp`,
initializes MADCTL30h and explicitly uses30h/00h in its rotation method. Original
bytes and receipt are retained under
`docs/platform/research/waveshare-co5300-square-mv/`.

COLMOD bit 7 selects VIPF or IFPF as documented. The implemented pixel format is
RGB565. Sleep, display enable and explicit brightness-zero gating affect the
visible frame without inventing pixel writes. Software/hardware reset preserves
GRAM, as the CO5300 CASET/RASET reset tables require; register state returns to
reset and the display remains hidden until re-enabled. Reset and input are
ordered by guest cycles.

CST820 exposes only the factory driver's supported report at `02h…06h`. Unknown
identity/configuration registers do not receive invented values. The fixed BSP
inverts both raw touch axes, so host coordinates are converted to
`raw=(465-x,465-y)` before the guest reads them. Each changed host report produces
a one-guest-millisecond falling IRQ pulse: that is an explicit digital input
policy, not a measured sensor scan cadence. The nominal circular crop and the
BSP coordinate convention do not certify physical glass-to-PCB registration or
the USB connector's orientation relative to the rendered text.

## Original firmware and observed results

The [official firmware README](https://raw.githubusercontent.com/waveshareteam/ESP32-S3-Touch-AMOLED-1.43C/8cea11ddae6167dd591d1f5465fd7b615080360f/firmware/README.md)
identifies `ESP32-S3-Touch-AMOLED-1.43C_FactoryOnly_20261027.bin` as the recommended
complete merged factory image, loaded at address zero. It is 7,631,074 bytes and
has SHA-256 `522d14b3a2079733ac4c6481f3e36716428e96ef4cc5502505dde608c61366cc`.
The earlier `20260420` file exists but was not substituted or used in this test.
The factory source guide names the touch drawing and BOOT-exit behaviors used
for acceptance.

Fresh native execution from the original mask ROM produced these observations:

| Check | Result |
| --- | --- |
| Boot through 8 guest seconds | Nine-app desktop; zero reboots; 90 RAMWR commands, 651,468 pixels |
| Quiet desktop, 8.0–8.5 seconds | Zero changed pixels |
| Tap Touch Test at (305,295) | 169,363 changed pixels; original application opens |
| Forty-step line, (150,180) to (310,260) | 2,710 changed pixels against a matched no-drawing run; 2,493 newly red pixels |
| BOOT low for 150 ms, then release | Canvas closes; restored desktop differs from the initial desktop by zero pixels |
| End at 11.22 guest seconds | Zero reboots and zero rejected QSPI transfers |

The original Wi-Fi calibration warnings and repeated not-connected messages are
retained in the raw UART capture. There is no injected successful network status.
The codecs complete their guest-visible control setup; PCM, microphone capture,
speaker output, speech recognition, online Weather and Xiaozhi service operation
are unverified and are not claimed as working. No factory BIN, ROM instruction,
guest source or application condition was modified for these results.

Compact results, artifact hashes, exact inputs and reproduction instructions are
in [the evidence summary](../evidence/waveshare-amoled143c-2026-11-23/README.md).
Full UART, screenshots, frame buffers and source snapshots remain in an ignored
output archive. Earlier pre-correction captures are retained separately and do
not substitute for the final R18-aware run.

## Focused verification

Four new transport tests cover split headers, four-wire pixels, embedded header
bytes, rejection until CS release, window/format errors, vendor pages, reset and
display gating. Three board tests cover exact four-lane pads, software CS, reset,
CST reports/IRQ, shared I2C state, R18's pull-up and codec power-cycle reset.
Seventeen existing CO5300 tests pass without changes to that shared engine.
`cargo clippy -p esp-soc -p esp32s3 --all-targets -- -D warnings` is the scoped
static check. The two ignored original-image tests are opt-in because they
require the caller's downloaded factory image and original S3 mask ROM.

## SD registration correction

The later S3 listing fragment incorrectly advertised `sdCard: true`. The exact
manufacturer header at the revision above explicitly defines `BSP_CAPS_SDCARD 0`;
the retained schematic and both PCB layout pages show no TF socket. Both exact
SKU knowledge records already stated `tf-slot: false`, and neither product has
a TF interface or a default card declaration. The fragment and generator profile
now agree with that physical evidence and with the board model, which provides
neither an SDMMC socket nor an SPI SD attachment. No SD operation was part of the
factory validation described here.

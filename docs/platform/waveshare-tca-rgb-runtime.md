# Waveshare S3 TCA9554 / ST7701S RGB profiles

This implementation was rebuilt and tested on 2026-11-23 after the previous
workspace was lost. Only the fresh results below constitute acceptance evidence.
The module is `esp32s3/src/board/waveshare_tca_rgb.rs`; it depends on the opt-in
SPI phase snapshot, QMI8658C, TCA9554 and PCF85063A implementations. Central CLI,
WASM and catalog registration is a separate integration step.

## Exact product scope

| Product | Rust variant | Selector | Glass / touch | Fresh factory status at this checkpoint |
| --- | --- | --- | --- | --- |
| ESP32-S3-Touch-LCD-2.1 | `TouchLcd21` | `waveshare-s3-touch-lcd21` | 480 × 480 round / CST820 | Trial: display and early scroll verified; later input stalls |
| ESP32-S3-Touch-LCD-2.1B | `TouchLcd21B` | `waveshare-s3-touch-lcd21b` | 480 × 480 round, curved cover / CST820 | Trial: independently executed; same limits |
| ESP32-S3-LCD-2.8B | `Lcd28B` | `waveshare-s3-lcd28b` | 480 × 640 rectangular / no touch | Trial: factory display verified; no distinct BOOT response |
| ESP32-S3-Touch-LCD-2.8B | `TouchLcd28B` | `waveshare-s3-touch-lcd28b` | 480 × 640 rectangular / GT911 | Trial: factory display and early scroll verified; later input stalls |
| ESP32-S3-LCD-2.8C | `Lcd28C` | `waveshare-s3-lcd28c` | 480 × 480 round / no touch | Trial: factory display verified; no distinct BOOT response |
| ESP32-S3-Touch-LCD-2.8C | `TouchLcd28C` | `waveshare-s3-touch-lcd28c` | 480 × 480 round / GT911 | Trial: factory display and early scroll verified; later input stalls |

All six profiles specify 16 MiB flash and 8 MiB octal PSRAM. They are separate
products, not aliases to an existing board. The official 2.1 series firmware
README explicitly supports both 2.1 and 2.1B; the latter does not require an
invented separately named image. A common digital display profile does not
certify identical mechanical construction or optical behavior.

The source manifest contains exact original PDFs, selected unmodified source
members from the official demo ZIPs, fixed-commit firmware READMEs, and clearly
labelled search-tool extraction representations. Every compressed source has
both the archive SHA-256 and the SHA-256 of its decompressed original bytes.
Full demo ZIPs and firmware BINs are not committed.

- [Source receipts](../research/waveshare-tca-rgb-sources/manifest.json)
- [Remote firmware receipts](../research/waveshare-tca-rgb-sources/firmwares.json)
- [Central integration patch](../research/waveshare-tca-rgb-central-patch.json)
- [Incremental four-product 2.8B/C patch](../research/waveshare-tca-rgb-28-central-patch.json)
- [Fresh run evidence](../evidence/waveshare-tca-rgb-2026-11-23/manifest.json)

## Wiring and digital behavior

| Function | Exact connection |
| --- | --- |
| Shared physical I2C | SDA 15, SCL 7; both hardware masters and GPIO bit-banging share one device state |
| TCA9554 | Address 0x20; P0 panel reset, P1 touch reset, P2 panel chip select, P3 TF DAT3/native-mode selection (SPI chip select in SPI mode) |
| Panel serial initialization | Clock 2, data 1; one D/C command bit followed by an eight-bit address phase; TCA P2 owns CS |
| RGB data, least significant first | 5, 45, 48, 47, 21, 14, 13, 12, 11, 10, 9, 46, 3, 8, 18, 17 |
| RGB controls | PCLK 41, DE 40, VSYNC 39, HSYNC 38 |
| Backlight | GPIO 6 / LEDC duty |
| CST820, 2.1 and 2.1B | Address 0x15, reset TCA P1, interrupt 16 |
| GT911, touch 2.8B/C | Reset TCA P1, interrupt 16; INT low at reset release selects 0x5D, high selects 0x14 |
| QMI8658C | Address 0x6B, SA0 grounded; source-based A/C model, not the old generic device |
| PCF85063A | Address 0x51; advances using guest cycles; I2C STOP forwarded |
| Physical TF slot | Native one-bit SDMMC: CLK 2, CMD 1, D0 42; DAT3 held high by TCA P3. D1/D2 have no MCU route. Host attaches the media image. SPI uses the same physical CLK/MOSI/MISO/DAT3 wiring as an explicit alternate subset. |
| BOOT | GPIO 0 |

The 2.8B non-touch schematic and its own demo source independently support its
wiring. The non-touch 2.8C Resources page links a Touch-named schematic whose
bytes equal the touch product PDF. That is retained as an official cross-link,
not represented as a separately named non-touch drawing. Its own non-touch demo
has no touch endpoint. The curved 2.1B SKU is identified on the official 2.1
series documentation, and the fixed firmware README establishes the shared
software scope.

RGB delivery checks all sixteen routed data lanes and all four control pads.
It handles a real matrix permutation instead of assuming that a frame callback
implies correct wiring. The current S3 path exposes matrix routing through the
GPIO callback; this board also implements the explicit RGB-pins hook. Invalid
routes and dimensions cannot update the visible display.

Serial initialization uses the new complete phase snapshot but remains
single-lane. It verifies actual pads, phase widths and bit lengths, mode zero,
no dummy/read phase, external CS, panel reset and no simultaneous TF selection.
The MCU's `keep_cs` flag does not override a physical TCA output. The IDF
`spi_ll_set_command` implementation at commit
`735507283d5b2f9fb363a1901172dbd9e847945d` places a one-bit MSB-first command in
bit 7 of its first serialized byte. A local parser error that initially read
bit 0 was corrected before acceptance. An MMIO test now configures USER,
USER1, USER2 and ADDR, checks the resulting bytes, and verifies bank selection
across separate CS pulses. The earlier diagnostic result is explicitly
withdrawn in the evidence manifest. No shared SPI behavior was changed by this
local correction.

The ST7701S model implements reset, common sleep/display/inversion commands,
FFh vendor bank selection, ML/BGR scan behavior and the RGB frame. It records
the COLMOD VIPF field. The stream supplied by the existing S3 LCD peripheral is
RGB565; vendor analog voltage/gamma programming and electrical 18/24-bit timing
are outside this subset. The round aperture and linear PWM brightness are
digital presentation conventions, not calibrated optics. The `gram()` hook
returns the latest sampled RGB frame and does not assert that ST7701S stores it
in internal controller GRAM.

The CST820 model supplies the source-supported single-contact report and
FEh=FFh control storage. Its ID values are unknown zero, explicitly visible in
the console; it does not impersonate a CST816S identity. A one-guest-millisecond
interrupt pulse is a documented injection policy because the available source
does not establish the pulse duration. Gestures, scan physics and automatic
sleep are not implemented.

The board-local GT911 wrapper adds actual read-only resolution registers
8146h–8149h, separately from writable configuration RAM at 8048h/804Ah. The
unimplemented configuration engine does not falsely apply arbitrary RAM values
to physical resolution. Reset address strapping exposes only the selected
address; attaching two address views does not make both ACK. Shared legacy
GT911 behavior is unchanged. Single-contact reporting and the existing digital
scan model remain a subset of the real part.

QMI register sampling, RTC time and optional SD command handling have protocol
coverage. This checkpoint does not provide host pose samples, sensor-to-PCB
calibration, QMI interrupts/calibration/FIFO, RTC alarm/clock outputs, physical
reset timing, battery voltage, buzzer sound, charging or RF behavior. No SD
image was inserted for the factory runs, so the original program's SD
initialization failure and 0 MB display are expected.

## Fresh validation, 2026-11-23

Eight focused tests passed: all-six routing/reset/geometry; strict nine-bit
phases; real SPI MMIO command alignment; RGB matrix and scan behavior; separate
non-touch products plus shared I2C/RTC state; CST820 reports/reset; GT911
resolution/strapping; and optional TF chip-select/MISO behavior. S3 all-target
Clippy passed with warnings denied. Six external-firmware tests remain ignored
by default because they require the original mask ROM and remote BINs.

All six products were independently executed. Each booted
its applicable unmodified factory image from flash offset 0 with the original S3 ROM.
The independent source/firmware hashes and actual UTC run intervals are in the
receipts; no previous-workspace result is used.

| Observation | 2.1 | 2.1B |
| --- | ---: | ---: |
| Initial observation | 8,000 guest ms | 8,000 guest ms |
| Reboots | 0 | 0 |
| Received RGB frames | 424 | 424 |
| Panel commands / vendor parameter bytes | 39 / 151 | 39 / 151 |
| Rejected SPI / RGB transfers | 0 / 0 | 0 / 0 |
| Final COLMOD field / PWM duty | 0x60 / 32000 | 0x60 / 32000 |
| Quiet 2,000–2,200 ms pixel changes | 0 | 0 |
| Scroll vs matched no-input run, at 3,060 ms | 41,118 pixels | 41,118 pixels |
| Contact reports read during early run | 14 | 14 |
| Touch at 8,000–8,400 ms, pixel changes | 0 | 0 |

The reproducible early gesture starts at (420, 360) at 2,200 ms, holds for
60 ms, moves through twenty evenly spaced points toward (420, 120) at 20 ms
intervals, then releases for 60 ms and settles for 340 ms. The separate control
machine executes the same guest intervals without input. This distinguishes
the positive scroll response from autonomous UI activity. Screenshots show the
Onboard page scrolling from its storage/flash fields to the backlight and
buzzer controls.

The final UART marker is `BLE Test`; touch reads do not advance between the
6,000 and 8,000 ms snapshots, and later injected contact does not change the
display. The blocked instruction/peripheral has not been established, so this
is a **trial** result with an observed later stall, not proof of a specific
BLE-core diagnosis. The console also reports I2C write failures. QMI revision
read succeeds at the grounded-SA0 address 0x6B; the failing write bytes have
not been traced in these factory binaries. The source driver contains a
reserved-bit CTRL7 write, but it is not sufficient evidence to attribute every
binary failure to that write. No extra I2C address or reserved-bit ACK was
added to conceal it.

### Four-product 2.8B/C follow-up

These four runs used the committed board source without further code changes.
Each selected its own exact BIN from the fixed official firmware directory.
The 2.8B touch factory prints actual resolution 480 × 640 and product ID 911
through the source-supported GT911 registers. Its 8146h–8149h read path is
covered separately from configurable RAM; it was not patched into the image.

| Observation | LCD 2.8B | Touch LCD 2.8B | LCD 2.8C | Touch LCD 2.8C |
| --- | ---: | ---: | ---: | ---: |
| RGB frames by 8,000 ms | 505 | 505 | 820 | 819 |
| Reboots / rejected SPI / rejected RGB | 0 / 0 / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / 0 / 0 |
| Commands / vendor parameter bytes | 49 / 159 | 49 / 159 | 41 / 153 | 41 / 153 |
| Quiet raw pixel changes, 2,000–2,200 ms | 53 | 53 | 21 | 53 |
| Early input vs matched no-input control | 0 | 53,351 | 0 | 38,135 |
| Contact reports read during early run | — | 15 | — | 14 |
| Late input raw pixel changes, 8,000–8,400 ms | 0 | 0 | 0 | 0 |

The rectangular touch gesture uses (440, 530) to (440, 130); the round touch
gesture uses the 2.1 coordinates above. Timing and twenty intermediate steps
are the same. BOOT0 is held during the corresponding early window for the
non-touch products; neither produced a distinct response compared with its
matched control. The small quiet differences come from autonomous displayed
values, so only the matched-control differences support the touch result.
No factory button action is claimed for a program that may not consume BOOT.

All four finish their observed UART stream at `BLE Test` and show no response
to the tested late input. I2C write and absent-SD errors remain visible. The
factory programs are therefore retained as trial configurations with the same
unresolved later-stall boundary; continuous RGB DMA frames alone do not prove
continued application progress.

Firmware filename dates identify firmware artifacts, not product release
dates. This batch does not infer device release order from repository creation
time or the shared 2.1 image's 20250224 suffix.

## Reproduction

Set `ESP32SIM_WAVESHARE_RGB_SOURCES` to a temporary tree containing each pinned
repository's `firmware/<fileName>` and `ESP32SIM_ROM_DIR` to the original ROM
directory. Optionally set `ESP32SIM_WAVESHARE_RGB_EVIDENCE` for fresh PNG,
RGB565, console and state output. Then run:

```sh
cargo test --release -p esp32s3 --lib board::waveshare_tca_rgb::tests
cargo test --release -p esp32s3 --lib board::waveshare_tca_rgb::tests::external_touch21_factory -- --ignored --exact --nocapture --test-threads=1
cargo test --release -p esp32s3 --lib board::waveshare_tca_rgb::tests::external_touch21b_factory -- --ignored --exact --nocapture --test-threads=1
```

The raw merged BINs remain remote links with fixed commits and local download
receipts. The test does not patch the image or substitute a host-rendered UI.

## Subsequent host IMU input extension

The board now retains the shared QMI object for both I2C transport and explicit
host samples. `set_imu_sample` accepts finite acceleration in g and angular rate
in degrees per second, expressed in sensor axes. The configured ODR, range,
enables and SyncSample lock still govern guest-visible data; selecting a script
or stationary mode clears the host hold. The S3 browser input boundary delivers
deferred device time before installing a sample.

The eight-board native guest-FIFO and browser-ABI boundary checks are recorded
in `docs/evidence/waveshare-qmi-host-input-2026-11-23/manifest.json`. This extends
the digital sensor input capability; it does not change the earlier factory
UI acceptance, calibrate the physical PCB/3D pose, or claim a motion-responsive
factory application. Previous acceptance receipts remain historical snapshots
of their stated source versions.

## Native TF correction and inserted-card verification

The initial board implementation only attached the optional image to its SPI
card object. A later public Worker run inserted the default 64 MiB FAT32 image
but the original firmware still reported SD initialization failure. This was
a real adapter defect, not evidence that the image had been read or mounted.

All five independently retained `SD_Card.cpp` files use
`SD_MMC.setPins(2, 1, 42, -1, -1, -1)` and one-bit `SD_MMC.begin`. They raise
`EXIO_PIN4` before starting; the expander driver numbers pins from one, so this
is physical TCA9554 P3. The schematics connect that net to the socket DAT3/CS
pin. The native data route is therefore CLK GPIO2, CMD GPIO1 and D0 GPIO42.
D1 and D2 are not connected to MCU SD data signals; the expander-driven DAT3
is also not a fourth SDMMC data lane. The fixed ESP-IDF GPIO signal map
identifies slot 1's CLK 173, CMD 179 and D0 213 matrix signals.

`sd_card_slot()` now exposes `[2,1,42,255,255,255]`, so the ordinary host media
attachment reaches the existing S3 SDMMC host and its IDMAC. Native readiness
requires media present and DAT3 high. This is a supported native-mode selection
condition, not a claim that DAT3 controls power. Switching a live card between
SPI and native mode, or driving DAT3 low after a native session is established,
is outside this subset. The explicit SPI board attachment remains separately
tested; the ordinary platform image path now selects native SDMMC.

The negative native reproduction inserted the exact browser-generated FAT32
image and ran the unmodified 2.1 factory for 2,200 guest milliseconds: 10 SDMMC
commands, 9 timeouts, zero bytes read and no filesystem mount. Its source
snapshot and UART are retained as a pre-fix diagnostic. The subsequent six-
board route test exercises actual SDMMC registers and IDMAC sector write/read,
then verifies wrong CMD/D0 routes, unwired four-bit mode, low DAT3 and removed
media all fail. Factory mount results and original UART are recorded separately
for each product in
`docs/evidence/waveshare-tca-sdmmc-2026-11-23/manifest.json`. The image generator
and exact 64 MiB image hash are recorded; neither the disk image nor factory
firmware bytes are committed.

This reuses the S3 host's immediate digital transfer model. It does not add
wire timing, analog pull-up behavior, card power-up timing or hot mode switching.
The earlier I2C-write and late BLE-stage limitations remain separately visible.

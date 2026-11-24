# Fifth S3 group: original firmware observations and exact board subsets

This group covers five official storefront listings and eleven exact SKUs. It adds seven source-backed partial mechanical models. The support boundary remains partial; neither native digital checks nor a model preview establish physical hardware acceptance.

| Listing | SKUs | Source-based digital path | Original ROM observation |
| --- | --- | --- | --- |
| DualEye LCD 1.28 | 32267 non-touch; 32265 touch | Two independent GC9A01 memories/CS/reset/backlight; separate touch buses; primary only in public display API | Touch factory image reaches Bluetooth initialization within the five-second budget; zero display frames |
| Touch LCD 4B | 32052 | ST7701S nine-bit serial through real TCA9554 writes, RGB16, GT911 IRQ on EX6; GPIO4 backlight active-low | About 1.855 guest seconds, then QMI8658 WHO_AM_I error and software reset; initial and corrected-backlight attempts retained |
| LCD 3.16 | 31786 | Real GPIO nine-bit serial, RGB16, RTC and exact one-bit SDMMC | Five guest seconds; actual SIMSD reads and Wi-Fi/BT initialization, 253 LCD serial words, zero complete RGB frames |
| LCD 7B | 31726 touch/onboard antenna; 31727 non-touch; 35876 touch/external antenna | Source's byte-wide helper protocol, exact RGB16, GT911 only on touch versions, native SDMMC | 16_UI aborts on deliberately unsupported helper ADC command 06; separate 06_LCD has no touch dependency and runs five seconds on non-touch with zero frames |
| Knob Touch LCD 1.8 | 31623/31624 blue, 34875/34876 black; battery options retained | S3 QSPI source-defined LCD commands, CST common report, real GPIO8/7 encoder phases, four-bit SDMMC | Five guest seconds, 78 frames and 25 colors; clock/low-battery display is upside-down in the current model. Physical glass orientation and battery ADC remain uncalibrated |

The Knob's other bundled image targets a separate classic ESP32, not the S3. It is not loaded into the S3 runtime, and the second MCU is not executed in parallel. The vendor's `SH8601` driver class name does not establish the fitted LCD die; the board has a backlight and source-specific F0 register banks. No AMOLED identity is inferred from that class name.

## Firmware identity and distribution

Original binaries stay remote-only. `../firmware-manifest.json` records their original byte counts, hashes, chip IDs and partition layouts. `../fifth-original-archive-audit.json` records both ZIP directories. The fixed original ZIP and selected BIN hashes are separate:

| Distribution | Original ZIP SHA-256 | Selected member SHA-256 |
| --- | --- | --- |
| LCD 3.16 demo ZIP | `e5914bb732f47eb6a3d76856f707253ee6008847e6675011796180943683c3de` | `731aa7fc07a9bec11d857bc156fb53235be8a1591ec11c0ecaa43c2aa50f51cb` |
| Knob BIN ZIP | `7d29fc1fb356059f7291eccd74bfb5c9fa7538998bc3f5ff811cd87f04c1691c` | `f7c1cc18b687559f3bd69e5c9ab526bc61c2b2d9c502f38367f7f2bfe4ff8e87` |

Selected members are respectively `ESP32-S3-LCD-3.16-Demo/Firmware/ESP32-S3-LCD-3.16-factory.bin` and `ESP32-S3-Knob-Touch-LCD-1.8-BIN/WX-ESP32S3-KNOB_V1.2.bin`. The former archive is 214,706,718 compressed bytes and declares 717,568,656 total expanded bytes. The bounded `zip-bundle` loader reads only the selected member. Static ZIP/Drive releases use the explicit manual metadata provider instead of an invented GitHub release API.

The recorded HTTP range probe returned a ZIP signature and `Access-Control-Allow-Origin: *` for the current LCD 3.16 confirmed Drive URL. The Knob static ZIP returned a ZIP signature but no ACAO header. These probes do not establish that a browser can load and run the complete images; the missing Knob CORS header remains a documented limitation.

DualEye's manufacturer README identifies the factory image as the touch model's music player. Only SKU 32265 receives that factory/default mapping. The separately published Xiaozhi v2.5.4 file retains its own official-demo role. The 7B files remain official demos because no independent source establishes their factory purpose. The shared pinned current Xiaozhi/UIFlow release inventory is recorded without substituting generic board images or inheriting compatibility claims.

## Source and model boundaries

The native 7B STEP retains original coordinates and all 808 components for the touch version. The non-touch presentation omits only the sourced cover/touch component and retains 807 common components; exact mechanical revision equivalence is still unknown. Antenna assembly differences are not invented. The remaining models use unchanged official photos and documented envelope/window dimensions, with explicit UNKNOWN carrier boards where internal geometry is missing. The black Knob has its own identity and neutral untextured material because a corresponding verified black photograph was not obtained.

Optical windows are not relabeled as active areas. The 7B source windows differ by about five percent from the pixel aspect ratio; their actual dimensions remain unchanged and independent AA is unknown. Model plans retain the reason for the optical-window tolerance, source image pixels/UV mappings and source hashes. Production Three Node loading, source-byte validation, geometry and ray checks passed for all seven assets. Browser WebGL visual acceptance and physical validation were not performed here.

The 4B backlight correction follows the retrieved official component `waveshare/esp32_s3_touch_lcd_4b` version 2.0.0: its brightness setter computes `100 - brightness_percent` before writing LEDC duty. The test covers raw high, raw low, floating and three PWM duty values, while keeping the 3.16 active-high path independent. This change does not fabricate a QMI8658 identity to bypass the observed factory failure.

## Reproduction

Run the board tests with `cargo test -p esp32s3 --lib waveshare_ -- --nocapture` and lint with `cargo clippy -p esp32s3 --all-targets -- -D warnings`. The recorded result is 55 tests passed, zero failed, with 12 explicitly externally provisioned tests ignored. No ignored hardware/fixture-dependent test is claimed as passed.

Build `cargo build -p esp32s3 --release --example waveshare_source_attempt`. The observation harness accepts caller-supplied board, ROM path, original BIN path, flash MiB, PSRAM MiB, guest milliseconds, output directory and optional SD image. It preserves UART, framebuffer and stop reason without converting a software reset or blank screen into a compatibility assertion. Each attempt records input, ROM, SD, source and executable hashes; `original-initial` and `original-final` preserve both sides of the backlight correction.

The eleven product records and seven firmware records pass the integration catalog schema including `zip-bundle`; eleven generated knowledge records and seven watchlist entries pass the production validators independently of the pending central-index merge. Source originals and their retained gzip hashes are in the adjacent source manifest. The recorded schema and model results identify their exact scope.

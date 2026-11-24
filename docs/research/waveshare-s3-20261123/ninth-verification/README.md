# Final five S3 display listings: source, runtime and mechanical evidence

This group contains **5 storefront listings, 12 distinct SKUs and 16 electrical
profiles**. The extra four profiles explicitly distinguish the old SPI and newer
SDMMC wiring of AMOLED 1.91. They are not extra storefront SKUs. All products are
partial; all firmware entries retain `simulationVerified=false` and
`hardwareValidated=false`. No physical board was connected.

## Identity and electrical scope

| Listing | Exact SKUs | Implemented source-specific scope |
| --- | --- | --- |
| Touch LCD 1.46B / 1.46 / 1.46C | 29564, 29565, 33837 | SPD2010 412×412 QSPI; actual vendor-page selection; SPD contact boot/status/report subset at 53h; TCA9554 at 20h; PCF85063 at 51h; QMI8658C at 6Bh; SDMMC 1-bit; BOOT0 and PWR6. The factory README explicitly covers all three glass variants. |
| Touch / non-touch LCD 2.8C | 29086, 30254 | Existing precise ST7701S 480×480 RGB, TCA9554, GT911 on the touch model, RTC, IMU and native 1-bit TF. Source models and their earlier evidence are preserved. |
| AMOLED / Touch AMOLED 1.91, with / without headers | 28872, 28873, 28596, 28871 | 536×240 QSPI source subset; FT3168 only on touch SKUs; QMI8658C; actual shared LCD/TP reset17. V1 uses SPI47/42/8/CS9; V2 uses SDMMC9/42/8. Each SKU gets two explicit electrical profiles. |
| Original Touch / non-touch LCD 1.85 | 28514, 28511 | ST77916 360×360 QSPI and actual F0 vendor page; TCA/RTC/IMU/native 1-bit TF; CST816 point subset only on the touch variant's separate SDA1/SCL3 bus. This is not the B or C product. |
| Original LCD 1.47 | 28317 | ST7789 native 240×320 RAM with 172×320 visible window and X gap34; SPI40/45/CS42/DC41/RST39; BL48; one WS2812 at38; SDMMC 4-bit14/15/16/18/17/21. No touch or I2C devices are invented. |

Every profile has physical 16 MiB Flash and 8 MiB PSRAM. Slot metadata and
digital transport remain separate: exact native and SPI TF transports are not
interchangeable. The provided 64 MiB FAT32 test card is simulation media, not a
claim that retail packaging includes a card.

## Original complete ROM observations

The original binaries are remote-only catalog entries. Their exact downloaded
bytes were supplied to the native `waveshare_source_attempt` runner with the
real ESP32-S3 mask ROM, 16/8 MiB memory and a 5,000 guest millisecond budget. The
runner stores its executable hash, source hashes, input BIN hash, card/ROM hashes,
command, exit status and observed guest state. It does not patch the original
firmware or replace it with a diagnostic application.

| Original image / configuration | Observed outcome | Remaining boundary |
| --- | --- | --- |
| 1.46 series FactoryOnly_20261119 | 5,000 guest ms; 412×412; 489 RAMWR frames; 55 colors; LVGL Onboard page; 64 MB SIMSD mounted | Unknown SPD firmware/OTP register26 remains NACK. Audio/recognition initialization logs do not establish acoustic, speech or network acceptance. |
| AMOLED 1.91 FactoryOnly_20241111, V2 touch and non-touch | Each 5,000 ms; 536×240; 45 frames; 17 colors; early green display with FPS; 64 MB SIMSD mounted | This is not a full factory home-screen or touch acceptance claim. The actual controller identity and physical coordinate calibration remain unresolved. |
| Same AMOLED 1.91 V2 image, V1 electrical profiles | Same early display; 10 native SD attempts, 9 timeouts, no SD data read | Deliberate counter-evidence: the present image uses V2 wiring. It is not registered as a compatible default for V1. |
| Touch LCD 1.85 official demo, touch profile | About 1,535 ms; 360×360; 16 frames; 14-color test bands; 64 MB TF mounted; then software reset | Original CST driver reads A7, gets NACK, reports `Read version failed`, and aborts. No suffix-specific identity is guessed to bypass this assertion. |
| Same 1.85 touch demo, non-touch profile | Same bounded failure observation | This image is not registered as a compatible non-touch default. A suitable complete non-touch image has not been established. |
| LCD 1.47 official Test BIN | 5,000 ms; 172×320; 11 frames; 46 colors; original screen retained | Original BIN ends with MADCTL C0, while current published BSP calls `mirror(true,false)` (40 reference). Vertical image mirroring remains visible. There were 1,720 rejected non-RGB565 pixel transfers; final COLMOD is55. No SD command was observed in this interval. |
| Touch and non-touch 2.8C exact FactoryOnly images | Each 5,000 ms; 480×480; 496 frames; 43 colors; 33 SD commands / 1,744 bytes read; 64 MB TF mounted | Two early initialization timeouts are retained. RF/BLE, complete UI, physical touch and analog circuitry are not validated. |

`original-run-index.json` locates all ten configurations and the additional
LCD1.47 diagnostic repeat. Raw RGB565 and UART are retained losslessly in gzip,
with original PNGs, JSON state and per-file hashes. Readable UART only normalizes
line endings and escapes invalid UTF-8; it is not substituted for the raw log.

## Manufacturer distribution and purpose

The 1.46 README names all three physical variants and the exact factory image.
The 2.8C pinned README separately names touch and non-touch factory restoration
images and offset0. AMOLED1.91 has an explicit factory README. These are genuine
factory entries, subject to the electrical targeting described above.

For LCD1.85 and LCD1.47, an official demo archive and complete merged image have
been found, but the words `Firmware` or `Test` do not establish a factory recovery
purpose. These entries are `official-demo` and have no fabricated factory
default. Two 2.8C DemoZIP images are also kept as separate example entries,
behind the correctly targeted original factory files.

LCD1.85's observed Google Drive form exposed an anonymous one-time parameter.
That private scratch response is not included in Git. The canonical form URL
without that parameter was independently downloaded in full: HTTP200,
201,161,399 bytes, SHA256
`fa6cd4c47923d559e15571a20ec0d7029febee76e14e6afbd99760f9ad5d6990`, identical
to the retained original archive. The response also carried
`Access-Control-Allow-Origin: *`. The complete ZIP directory contains4,832
entries and466,634,065 declared uncompressed bytes. The exact selected member
is `Firmware/ESP32-S3-Touch-LCD-1.85.bin`, SHA256
`e9b87dc42f0fb581d5e4236eccdd780495c937ef6298eaed4c43b161c1156595`.
The independent full-download receipt is
`../ninth-drive-canonical-full.json`; full archive/BIN layout audits are in
`../ninth-original-archive-audit.json`.

Manual watchlist providers accurately describe static ZIP, Drive and fixed Git
file distribution. They do not invent a GitHub Release API for these sources.

## Controller identity and bounded behavior

The AMOLED1.91 manufacturer repository names its driver SH8601 but links an
RM67162 V0.5 datasheet. The observed F0 MADCTR and one-byte brightness sequence
are retained as an explicit RM-linked manufacturer BSP subset. RM MVbit5 has the
opposite meaning from common DCS MV. Exchanged axes, RSMX/RSMY and reserved-bit
combinations are deliberately unsupported rather than silently borrowed from
CO5300 or SH8601. No fabricated die ID or OTP value is exposed.

The original1.85 source only logs the value read from A7; it does not establish
the fitted CST816 suffix or identity value. Unknown A7 remains NACK. The point,
sleep and observed autosleep-control subset is independent of that missing
identity. The exact original-image abort is a material limitation.

LCD1.47 consumes physical MOSI during full-duplex transfers and mode0/mode3,
returning undriven FF for unimplemented reads. Wrong wires remain rejected.
The retained original C0 frame is not rotated to hide a mismatch with the
source40 coordinate reference. RGB666 writes remain outside this RGB565 subset.

## Mechanical source coverage

There are12 model identities: ten newly exported models and two preserved2.8C
models. Original STEP and archive bytes are retained with source SHA/UTC and
per-component coverage. Geometry stays at unit scale; only rigid orientation
transforms are used. The separate production-loader review records final model
and photo hashes, button occlusion, output bindings and source-image comparison.

- 1.46B and the44.77mm standard cover use their own original424-component
  manufacturer STEP assemblies. The49.00mm C cover is a declared dimension-photo
  derivation over the original bare reference board. Its36.96mm optical opening
  is a true aperture. Glass adhesive stack, edge curvature and internal fitting
  remain unknown. The standard cover photo's overall depth is12.30mm.
- AMOLED1.91 uses the27-component non-touch-M STEP. Header-free variants omit
  exactly the two source header assemblies2/3, with a complete omission ledger.
  Touch variants add the independently dimensioned59.60×28.50mm cover rather
  than stretching the native57.50×24.50mm panel. Hidden touch FPC and revised
  internal components remain unknown. Touch optical window44.82×20.40mm is not
  mislabelled as independently measured AA.
- Touch1.85 retains the524-component July2026 native reference assembly. The
  non-touch SKU explicitly omits source0's55mm touch-glass assembly, preserves
  the original board and backing521, and derives the48.08mm circular front
  optical body /45.68mm window from the exact non-touch photograph. Layer
  thickness is a declared visual estimate, not a physical measurement.
- LCD1.47 preserves all158 native source components, including the two source
  header rows. Their population differs from the bare-hole retail photo and is
  disclosed. Drawing AA17.39×32.35mm remains independent of the37mm source
  envelope/FPC. Original XDE LED1_ASM uniquely matches source2 at1.80×1.50×1.00mm;
  the exact schematic identifies LED1 as WS2812B-0807 on GPIO38. This actual
  front-side emitter is bound to ledIndex0 and revealed in the inspection view.
  Its acrylic light spreading is not simulated. The earlier source56 / rear
  photo item4 inference was withdrawn: item4 is an LDO, not an LED. BOOT source153 is located
  from the labelled rear photo after the rigid180degree Z rotation.

The LED source identity can be independently reproduced offline with
`python tools/waveshare-batches/s3/check_lcd147_led_source.py`. The script reads
the retained original XDE assembly, accumulates its placements, compares all158
native topology bounds, and requires a unique source2 match. Its final result is
retained in `ninth-led-source-recheck.json`; package size and the rejected rear
photo annotation are not used to infer identity.

These are bare PCB / optical assemblies, not solid placeholder enclosures.
Where an optical cover is dimension-derived, its real screen aperture and
unmeasured construction are explicit. Optical assembly and live framebuffer
move as one inspection group. All physical fit, glass touch calibration,
connector mating and material appearance remain unvalidated.

## Ecosystem investigation and verification

The previously pinned complete UIFlow and Xiaozhi tree/release inventories are
referenced. Exact Xiaozhi1.46 and1.85 README, config headers, build JSON and board
source at92d400d were additionally fetched and retained. Their v2.6.0 named
release assets are candidates, not independently audited executable images.
No generic, B or C target is substituted for a missing exact model. The bounded
UIFlow inventory result does not prove that third-party ports do not exist.

Eight new protocol tests exercise actual output and negative wiring/reset/
presence/unknown-ID behavior. The final test-status receipt binds them to the
Rust module hashes; all-targets Clippy uses warnings as errors. Schema/source
contract checks run on all16 product knowledge records. Public production WASM,
worker, catalog integration and final browser constraints are recorded in
separate integration evidence; this native evidence does not claim them.

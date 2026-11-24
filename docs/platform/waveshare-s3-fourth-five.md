# Fourth five ESP32-S3 storefront families

The exact source manifests, 13 SKU records and seven source-based mechanical
models accompany the per-listing integration fragments. Runtime support remains
partial. Manufacturer binaries are immutable remote trial links; no rebuilt
application is substituted for a factory image.

| Listing | Current digital subset | Mechanical evidence |
|---|---|---|
| RLCD4.2 |16F8P; native300×400 ST7305 fixed-window packed monochrome SPI; RTC, audio control, 1bit SDMMC|881-component native manufacturer CAD, explicitly V1 mechanical reference; current V2 spacer UNKNOWN|
| Touch AMOLED1.32 |8F8P; CO5300 RGB565 QSPI gap6; CST820 common single-contact report; ES8311 control|408 components; diameter33.75mm is TP VA optical window, not asserted AMOLED AA|
| Touch LCD1.83 |16F8P;240×284 ST7789P source window; CST816D-family report; PMU/RTC/audio control|615-component bareboard CAD; outer case/battery omitted and UNKNOWN|
| Touch LCD3.49 |16F8P; currentV2/PCB1.1;172×640 AXS15231B mode3 QSPI with RAMWRC; independent I2C touch bus/TCA9554;1bit SDMMC|Distinct A/B case source drawings/photos; exact front98.5×34.8mm, hidden three-dimensional shape UNKNOWN|
| ePaper1.54 |Current non-touchV2 and single published touch variant;8F8P;200×200 compatible byte-X/gate-Y monochrome; optional FT6336;RTC/audio-control gate/1bit SDMMC|Different non-touch52/touch53 component archives;20250818 reference predates current electronics, revision correspondence UNKNOWN|

## Protocol boundaries

RLCD4.2 stores four adjacent columns and two rows per transmitted byte. The
source15000-byte frame is decoded as `index=(y/2)*75+x/4`,
`bit=7-((x%4)*2+y%2)`. Only sourceCOLMOD11/MADCTL48 and full CASET12…2A,
RASET00…C7 are accepted; this is not a generic linear monochrome bitmap alias.
The model has no backlight and does not fabricate ST7305 voltage or TE behavior.

AMOLED1.32 uses the overview's CO5300 identity and the source's X gap6. The
factory source calls an SH8601-named driver class; this discrepancy is retained
instead of silently changing the physical controller identity. Source touch
coordinates are raw=(466-x,466-y); unknown controller/version IDs remain zero.
Its GPIO17 PWR input is source-defined, while the battery latch/ADC is absent.

LCD1.83 uses ST7789P240×320 GRAM with a240×284 visible region starting0,0,
as the fixed Arduino constructor states. Its physical TF socket remains in the
catalog, but no SD transport is advertised until exact current source pins are
established. The prepared64MiB image is not a guest mount. PMU register support
is distinct from a verified analog power topology.

LCD3.49 uses currentV2 pins only. The manufacturer moved the TE/reset and
backlight/expander assignments fromV1; two alternative maps are not accepted
simultaneously. SPI mode3 maps to `clock_idle_high=true`,
`clock_output_edge=false` in ESP32-S3's LL register convention (the retained
`docs/research/waveshare-tca-rgb-sources/s3-spi_ll.h.gz`,
`spi_ll_master_set_mode`, with original receipt in that source manifest). Each actual
serial phase and each required pad are checked. RAMWR2Ch starts the first
vertical chunk and RAMWRC3Ch continues later chunks without resetting Y.
Touch is on independent SDA17/SCL18 at3Bh; system devices use47/48. EX0 touch
IRQ feeds TCA9554 then GPIO8, with an explicit one-guest-ms host-input pulse.
Factory queries14 bytes while the Arduino driver queries8; both exact11-byte
requests are recognized. Unknown extra bytes returnFF. Source-native touch is
raw=(640-y,x). The introductory172×320 text conflicts with the explicit spec
and BSP172×640; the latter defines this model. Vendor analog commands, TE
cadence and rectangular MV are outside the subset.

Monochrome ePaper1.54 is not the four-color1.54G. The retained source establishes
200×200, byte-X0…24, gate-Y0…199, active-high BUSY, RAM24h/26h and
22h/C7h+20h activation. Its fitted controller die is not established; no SSD1681
identity is invented. The touch endpoint38h/RST7/IRQ21 exists only in touch
variants. ES8311 control is gated by sourceGPIO42 and its digital registers
reset after a modeled power cycle. Visible pigment survives display power
loss; timing, LUTs and optical physics remain uncalibrated.

## Firmware and models

The source firmware manifest validates complete original S3 merged layouts and
checksums. RLCD4.2, AMOLED1.32 and LCD1.83 have explicit manufacturer factory
README roles; LCD1.83's image includes factory demo and Xiaozhi AI. The3.49V2
original has no retained explicit factory-role statement, so it is registered
as an official-demo trial rather than a confirmed factory default. ePaper1.54
20260416 is explicitly V2 and20260415 is V1; only current non-touchV2 SKUs get
the mapped default. Touch source support exists, but an exact original-image
mapping for the separately named touch SKUs is not claimed.

All models retain raw source bytes, SHA256 and UTC receipts. Manufacturer CAD
is never stretched to match a case drawing. Every native CAD component is
accounted for, inspection offsets are reversible illustrations, and missing
case/battery/revision geometry is marked UNKNOWN. AA27mm ePaper and
63.6×84.8mm RLCD dimensions are explicitly labelled in their source drawings;
other VA/window measurements remain optical-window mappings. No source photo
covers the dynamic framebuffer.

## Verification

Retained evidence includes production Three.js model loading, source-image
hash/UV checks, geometry/dimension bounds, reversible inspection transforms,
actual native framebuffer writes, wrong bus/pin negatives, touch register
outputs, reset/backlight gates, BUSY publication, and physical SD route records.
All53 accumulated S3 exact-SKU knowledge records pass both JSON schema and the
production knowledge contract; this batch contains13 of those records.
Native test and Clippy final receipts are retained in
[the evidence directory](research/waveshare-s3-fourth-five-evidence/).
A Node geometry load does not establish browser WebGL appearance. Original full
application boot, service connectivity, physical battery/audio/temperature,
optical calibration and real hardware acceptance remain separate unfinished
verification layers, not implied by the digital subset tests.

# S3 industrial batch 02: five families, nine exact SKUs

This source commit covers Audio Board, RS485-CAN, Relay-1CH,
POE-ETH-8DI-8DO and ETH-8DI-8RO-C. `manifest.json` lists the five catalog
fragments, all nine SKU/profile/model identities and their knowledge records.
Central catalog registration and release patch numbering belong to integration.

## Exact product distinctions

| Family | SKUs | Distinction retained |
| --- | --- | --- |
| ESP32-S3-AUDIO-Board | 32184 / 32185 | Included 1000 mAh battery / EN option without battery |
| ESP32-S3-RS485-CAN | 32154 / 34781 | Onboard antenna / U external antenna |
| ESP32-S3-Relay-1CH | 32152 / 35086 | Onboard antenna / U external antenna; coil GPIO47 |
| ESP32-S3-POE-ETH-8DI-8DO | 32108 | Eight Darlington outputs, distinct from relay contacts |
| ESP32-S3-ETH-8DI-8RO-C | 31282 / 31284 | Common Ethernet / PoE; eight SPDT relay outputs |

All nine listed default hardware configurations have 16 MiB Flash and 8 MiB
PSRAM. Product options, original source URL/fetch identities and schematic/BSP
members are recorded under the corresponding sibling research directory.
Generic optional external cameras, displays, batteries, loads and bus peers are
not silently added to a product.

## Digital runtime verification

`native-tests.log` and `native-tests-status.json` retain an actual successful
six-test execution of `cargo test --release -p esp32s3 --test
waveshare_industrial_controls`. The tests execute real LX7 load/store
instructions and exercise:

- exact I2C pin routing, GPIO mux/direction and wrong-route/address rejection;
- PCF85063 register/time behavior and STOP control;
- Audio TCA9555 three-key inputs and ES8311/ES7210 control register endpoints;
- actual RMT byte streams for seven Audio RGB devices and one industrial RGB;
- Relay-1CH GPIO47 coil control, with no relay invented for RS485-CAN;
- separate eight-channel expander output state and GPIO4–11 digital inputs;
- five profiles' native 1-bit SDMMC IDMAC 512-byte round trips, wrong-route
  rejection and ejection behavior.

`clippy.log` and its status receipt retain the corresponding successful focused
release Clippy gate. These are digital emulator tests, not physical load,
contact, audio, Ethernet, PoE, isolation, optocoupler-voltage, RF or CAN/RS485 peer
acceptance. Full original factory execution is a separate integration check.

## Original firmware provenance

Each family has an exact official ZIP, selected original BIN entry and separate
archive/image SHA-256. The five `firmware-image-audit.json` files record checks
using the production archive decoder and merged ESP32-S3 image validator:
original bootloader at 0, partition table at 0x8000, chip ID 9 and 16 MiB flash
layout. No binary is rebuilt, patched or mirrored into the repository.

The Audio factory role is supported by the official ESP-IDF factory-program
section, its flashing guide's Firmware directory and address 0x0, and the
original `factory_01` application descriptor. The other four official guides
identify the factory restore directory and factory demonstration. Original
images are trial entries with `simulationVerified: false` and factory defaults
remain first for each exact SKU.

Fixed upstream UIFlow and Xiaozhi releases and M5Burner's 55 device categories
were checked. Only Audio has an exact Xiaozhi `waveshare/esp32-s3-audio-board`
source target and matching v2.6.0 release archive; its unmodified image passes
production structural validation and remains a non-default trial. The records
make no exhaustive community-upload or generic-chip firmware compatibility
claim. Watchlist entries are manual, pinned and have no automatic catalog
update targets.

## Mechanical evidence and review

`loader-checks.json` and `loader-expanded-checks.json` each cover the nine final
models with the production Three.js loader. Source hashes, declared dimensions,
shell material isolation, independent inspection transforms and exact restore
behavior pass. Seven photo-derived hidden PCBs are deliberately blank and
marked UNKNOWN; each is obscured when assembled and revealed when expanded.

`model-reviews/` and `model-reviews-expanded/` contain the actual loader geometry
rendered in front, reverse, edge and oblique views. `photo-reviews/` compares
actual mesh XY envelope projections with unchanged manufacturer images.
`visual-review.json` records model and review-image hashes and the visual
findings. These CPU checks do not decode product textures or certify browser
WebGL, acrylic optical appearance, physical fit or manufacturing tolerances.

Audio's original native assembly has 801 components. SKU 32184 retains all of
them. The exact 32185/EN storefront option says the 3.7 V MX1.25 battery is NOT
Included, so its model omits only component 2, `BATTERY-803040-1000MAH`, leaving
800 source components. The original STEP, identity receipt, SKU option and
omission reason are bound in each model plan. No unverified native EN/RESET
button identity is attached to an interactive input.

The unscaled Audio CAD full height is 49.700000763 mm. The photo callouts sum to
43.7 + 5.1 = 48.8 mm, a roughly 0.9 mm unresolved difference in stack extent,
protrusions or source revision. Its 58 mm body diameter agrees with the photo.
The discrepancy is retained explicitly; the geometry is not fitted to the
photograph.

One export validation exception applies only to the original STEP SHA-256
`0a2557e0a975f29049ab5e55222c01f13d2559d79ffade488106107f0bea521d`,
component 2. `check_audio_battery_bounds.py` calls `BRepTools.Clean_s` before
three independent mesh densities. `audio-battery-bounds-check.json` retains
vertex counts, analytic and mesh extrema: all densities preserve mesh Y max
20.8875813702385 mm, versus conservative analytic Y max 21.11408048289337 mm,
a 0.22649911265487077 mm difference. The fixed source/component export bound
limit is 0.23 mm; the global 0.05 mm check remains unchanged. This concerns
analytic bounding-box conservatism and is not a physical accuracy allowance.

The four DIN photo models retain real open USB/terminal apertures and a separate
front cap; U models alone include an open external antenna envelope. The three
wide models retain hollow housings, independent caps, different terminal counts,
open RJ45/USB and antenna envelopes. Their 1.5 mm walls, connector heights and
photo-derived placement remain estimates. Native source geometry is never
replaced with a sibling device's CAD.

## Reproduction and final integration

Rebuild each committed plan through `tools/rebuild-product-model.py`. The
batch-specific `review_second_photos.py` accepts the production loader's mesh
dump directory and an output directory. Model checking and four-view rendering
used the current integration versions of the tools; hashes are retained in the
visual receipt without duplicating their unrelated source changes here.

`schema-checks.json` validates nine products, fifteen firmware records, fifteen
watchlist entries and nine knowledge records against current integration
schemas. `metadata-checks.json` applies production knowledge-record rules and
manual watchlist validation. `privacy-check.log` covers staged new research and
model assets. Integration must run the central catalog/registry, original
firmware bounded execution and complete repository gates before including this
source commit in the next incremental patch. No full firmware or hardware
compatibility claim follows from these focused checks.

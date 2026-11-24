# Waveshare S3 core options: final four listing families

This batch completes the exact Nano, Zero, DEV-KIT and Pico options in the
inspected storefront inventory: **4 families, 16 SKU profiles, 16 models**.
The original four base model/firmware identities remain; twelve additional
header, memory or antenna options have distinct product and runtime IDs.
`manifest.json` points to the four central-integration fragments. No third-party
BIN is committed, rebuilt, merged, patched or hosted by this change.

## Scope and evidence

Each listing directory next to this batch contains current official article
text, schematic, original archive directory/hash records, selected LED code,
SKU option identity, factory/UIFlow/Xiaozhi investigation and runtime receipt.
Every original file retains requested/final URL, real retrieval time, byte
count and SHA-256. The model plans retain unmodified manufacturer assets and
reproduce through `tools/build-waveshare-models.py`; no private generation
cache is needed to reconstruct the final geometry.

The baseline four original ROM/bootloader/image execution records are retained
at `docs/evidence/waveshare-s3-core-2026-11-23`. Their conclusions apply only to
those four board/firmware identities. None of the twelve new SKU options
inherits those execution conclusions. Catalog firmware status stays trial,
and no physical board or complete original application acceptance is claimed.

## Exact hardware differences

Nano and Nano-M use the same 16 MiB Flash / 8 MiB PSRAM target. Their discrete
common-anode RGB channels are GPIO46/0/45, active low; the separate GPIO48 SCK
indicator is active high. Only a physical RESET is documented. GPIO0 is an LED
channel, so the product has no invented BOOT button. Historical schematic
package-pin versus ROHM datasheet disagreement remains documented; displayed
R/G/B names follow the official example and pinned Arduino Nano mapping.

The FH4R2 Zero / Zero-M are 4F/2P. The separately sold Zero-N8R8 and
Zero-N8R8-M use PICO-1-N8R8 and 8F/8P. All use GPIO21 addressable GRB and
GPIO0 BOOT. The old FH4R2 CAD or firmware is not presented as the newer SiP.

DEV-KIT has eight SKU options: N8R8, N16R8, N32R16V and N16R8-U, each with
its own header option. The documented GPIO38 RGB wire order and GPIO0 BOOT
are common. GPIO35/36/37 are NC in the manufacturer's comparison. The
WROOM-2 **N32R16V is 32 MiB OPI Flash plus 16 MiB PSRAM**, with GPIO47/48 in
its documented 1.8 V domain. That voltage is a hardware fact; this digital
model does not simulate analog voltage or certify physical pin safety.

Pico and Pico-M remain 16F/2P with GPIO21 RGB and GPIO0 BOOT. The old generic
Octal-PSRAM pin paragraph conflicts with the precise S3R2/2 MiB identity and
is not used to invent another memory size. The manufacturer-linked 4 MiB
MicroPython software layout does not reduce the physical Flash to 4 MiB.

## CLI / WASM / reboot integration

The source commit includes the minimal central code changes. When integrating
with a branch that already has RGB-Matrix OPI startup, retain the union of
both selectors:

- `esp32s3/src/board/waveshare_core.rs`: `Variant::ALL` holds all sixteen
  exact selectors. `Variant::flash_octal()` is true **only** for
  `waveshare-s3-devkit-n32r16v` and `waveshare-s3-devkit-n32r16v-m`.
  `memory_mib()` returns `(32, 16)` for both.
- `cli/src/lib.rs`, `setup_s3`: after allocating Flash/PSRAM and before ROM
  boot, call `set_flash_octal(true)` when the explicit `--flash-octal`, an
  existing RGB-Matrix OPI selector, or this exact core selector requests it.
  Existing `waveshare_core::variant(...).memory_mib()` default allocation
  already covers the new profiles.
- `wasm/src/lib.rs`, `esp32sim_new` S3 branch: after allocating Flash/PSRAM,
  before `prepare`, use the same RGB-Matrix OR exact-core OPI selector.
  The pre-existing exact-core memory guard rejects 32F/8P for N32R16V.
- `esp32s3/src/soc.rs`, `SocBus::reboot`: the existing implementation already
  preserves SPI0/1 `octal_flash`, SPI1 `flash_cr2`, physical JEDEC identity,
  eFuse contents and PSRAM MR2 density. **Do not call `set_flash_octal` again
  in the chip reboot path**, because that setter clears the physical Flash
  CR2 protocol state. No reset implementation change is needed.
- `wasm/src/storage.rs`: a new C-ABI test verifies both exact selectors,
  rejects the wrong PSRAM capacity, checks fresh platform creation, and
  checks physical state across a guest chip reboot. Recreating the platform
  uses `esp32sim_new` and reapplies the exact SKU identity.

A cherry-pick may conflict on the one-line CLI/WASM OPI conditions if another
batch added the RGB-Matrix selector concurrently. The intended result in this
commit explicitly includes **both** selectors. No other CLI/WASM defaults or
board registries are replaced by this lane.

## Mechanical models and visible limitations

Zero-M retains all 40 native FH4R2 source components and adds the photo-backed
two 9-pin rows. DEV-KIT-N8R8-M restores native components 417/418, the two
1x22 headers: all 420 original topology components are present and unscaled.
The baseline four assets are unchanged. Nano-M and Pico-M preserve the
existing photographed boards and add two 15-pin or two 20-pin rows.

The matching new-module CAD was not found for Zero-N8R8, DEV-KIT-N16R8,
N32R16V or 1U variants. Their models are explicitly partial photo/dimension
envelopes with readable UNKNOWN faces. They do not inherit N8R8 package
internals, original SiP CAD, silkscreen or PCB stackup. The U version has an
actual open board-end notch and photo-estimated RF socket. The pictured
coax/whip is retained in the reference photograph; its length, mating and
routing are not fabricated as a fitted accessory.

The exact N8R8 Zero photograph showed a roughly 45-degree package rotation.
`photo-placement-correction.json` retains the earlier-model hash and the
source-bound manual PCB-plane inverse projection used to correct the initial
axis-aligned envelope. The rendered 7.8 mm package and 1 mm height remain
estimates: perspective of the raised package is not a metrology measurement.
The unmodified high-resolution -M photo informs the common SiP orientation;
the bare variant still has no headers. The separate bare thumbnail is also
retained and explicitly described as low resolution.

`loader-checks.json` covers all 16 production Three.js constructions,
dimensions, button rays, LED bindings and materials. Four existing/debug
inspection groups are checked in `expanded-loader-checks.json`; soldered
headers stay with their PCB. `additional-geometry-checks.json` checks actual
normal rays to UNKNOWN labels, hollow USB envelopes, every derived header
pin through the board's real hole, and the two U-end notches. The 0.05 mm
native-export bounds checks remain unchanged for these models; they are
export consistency checks, never a physical manufacturing tolerance.

There are 36 final review images: 16 assembled four-view plots, 4 expanded
plots, 8 baseline/native registered-photo plots and 8 exact-option oblique
photo overlays. The overlays preserve original pixels and show the actual
loader footprints. Their manual plane registration, omitted small passives,
height/parallax deviations and unknown fit are explicit. This is CPU geometry
and manual visual review, not a browser/WebGL or hardware fit acceptance.
No enclosure is fabricated for these bare boards, and no PCB is recolorable
as if it were an enclosure.

## Original firmware inventory

`firmware-image-audit.json` rechecks six source image sets and records one
rejection. Original ZIP/member identities, ESP segment checksums/appended
hashes, chip9 identities and applicable partition MD5/layout are checked.
The original Zero `BlinkRGB/.../combine/target.bin` is chip5 ESP32-C3 and
remains rejected. Its valid three S3 files keep their documented separate
offsets; they are not recombined.

The DEV-KIT ZIP additionally contains two original N16R8 MicroPython images.
The `Wavesahre_N16R8_SPIRAM_OCT.bin` member is offered as development trial
for the documented 16F/8P N16R8/M/U/UM targets, with no inherited execution
acceptance. **SPIRAM_OCT means octal PSRAM, not OPI Flash.** N32R16V and
Zero-N8R8 lack a confirmed corresponding bootable image in the inspected
exact sources and retain reference entries. No image is mislabeled factory.

The 25 firmware and 25 manual watch entries preserve factory-reference-first
and explicit development/example defaults where complete original images
exist. UIFlow and Xiaozhi retain the reviewed pinned upstream release and
lack of a unique exact-SKU mapping; no near-name substitution is automated.

## Reproduction and checks

Run from the repository root, using the CAD Python environment with
the dependencies described in `docs/platform/model-reproduction.md`:

```sh
CARGO_BUILD_JOBS=1 cargo test -p esp32s3 --test waveshare_core_skus
CARGO_BUILD_JOBS=1 cargo test -p esp32s3 --lib board::waveshare_core::tests
CARGO_BUILD_JOBS=1 cargo test -p esp32sim-wasm --lib core_n32r16v_physical_identity_survives_guest_and_platform_restart
CARGO_BUILD_JOBS=1 cargo clippy -p esp32s3 --test waveshare_core_skus --lib -- -D warnings
CARGO_BUILD_JOBS=1 cargo clippy -p esp32sim-wasm -p esp32sim --all-targets -- -D warnings
python tools/waveshare-batches/s3-industrial/audit_core_firmware.py --sources SOURCE_CACHE --output AUDIT_JSON
python tools/waveshare-batches/s3-industrial/validate_fourth.py
node tools/check-waveshare-model-loading.mjs MODEL_IDS --output RECEIPT_JSON --dump MESH_DIRECTORY
node tools/waveshare-batches/s3-industrial/check_fourth_geometry.mjs
python tools/review-waveshare-models.py --meshes MESH_DIRECTORY --output REVIEW_DIRECTORY
python tools/waveshare-batches/s3-industrial/review_core_photos.py --meshes MESH_DIRECTORY --output EXACT_REVIEW_DIRECTORY
```

The source audit/metadata curator accepts caller-provided downloaded original
source and inventory directories. The final committed model plans are the
standalone geometry recipe; the temporary first-pass authoring script is not
required. `qa-tool-versions.json` binds the integrated shared geometry tools
used for this review; the source commit does not overwrite those tools.
Full merged-workspace Host/WASM strict Clippy, ignored release tests and CPU
regression remain the integrator's final gate, separate from these scoped
receipts.

## Exact selector table

| SKU | Product | Flash / PSRAM (MiB) | Runtime selector |
| --- | --- | --- | --- |
| 26745 | ESP32-S3-Nano | 16 / 8 | `waveshare-s3-nano` |
| 26752 | ESP32-S3-Nano-M | 16 / 8 | `waveshare-s3-nano-m` |
| 25081 | ESP32-S3-Zero | 4 / 2 | `waveshare-s3-zero` |
| 25517 | ESP32-S3-Zero-M | 4 / 2 | `waveshare-s3-zero-m` |
| 33879 | ESP32-S3-Zero-N8R8 | 8 / 8 | `waveshare-s3-zero-n8r8` |
| 33549 | ESP32-S3-Zero-N8R8-M | 8 / 8 | `waveshare-s3-zero-n8r8-m` |
| 24363 | ESP32-S3-DEV-KIT-N8R8 | 8 / 8 | `waveshare-s3-devkit-n8r8` |
| 24243 | ESP32-S3-DEV-KIT-N8R8-M | 8 / 8 | `waveshare-s3-devkit-n8r8-m` |
| 28409 | ESP32-S3-DEV-KIT-N16R8 | 16 / 8 | `waveshare-s3-devkit-n16r8` |
| 28836 | ESP32-S3-DEV-KIT-N16R8-M | 16 / 8 | `waveshare-s3-devkit-n16r8-m` |
| 32837 | ESP32-S3-DEV-KIT-N32R16V | 32 / 16 | `waveshare-s3-devkit-n32r16v` |
| 32588 | ESP32-S3-DEV-KIT-N32R16V-M | 32 / 16 | `waveshare-s3-devkit-n32r16v-m` |
| 34549 | ESP32-S3-DEV-KIT-N16R8-U | 16 / 8 | `waveshare-s3-devkit-n16r8-u` |
| 34550 | ESP32-S3-DEV-KIT-N16R8-UM | 16 / 8 | `waveshare-s3-devkit-n16r8-um` |
| 23803 | ESP32-S3-Pico | 16 / 2 | `waveshare-s3-pico` |
| 24023 | ESP32-S3-Pico-M | 16 / 2 | `waveshare-s3-pico-m` |

The RMT reboot diagnostic distinguishes a powered LED from a new platform
device: an already latched WS2812 light survives an MCU reset, then changes
after a fresh guest RMT frame. A newly created platform device starts dark.

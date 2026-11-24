# Final core-board catalog reconstruction

The final four storefront families are Nano, Zero, DEV-KIT and Pico. Their
sixteen exact SKU identities, memory sizes, board selectors and firmware
relationships come from the retained manufacturer sources and the existing
explicit runtime registry. This metadata operation does not change Rust,
Cargo, firmware binaries or previously published catalog objects.

## Inputs and preservation boundary

`input-checks.json` records a current offline verification of 28 retained
primary-source bodies (both compressed and decoded SHA-256), three pinned
ecosystem metadata responses, thirteen original public firmware manifests,
and sixteen exact runtime memory selectors. Original retrieval timestamps
remain distinct from this verification time and each knowledge record's
generation time. A decoded article excerpt does not replace the original
HTTP response identity: the retained-source transform remains explicit.

`published-catalog-guards.json` binds the four catalog documents at published
commit `61447a576c36f40adcd40810fd9180f137937bdc`. The generator verifies their
canonical JSON identities after removing only its own final-four records.
It therefore preserves all 391 published Waveshare product objects, the 269
other product objects, and all existing firmware/watch/knowledge records.
The guard does not require unpublished Git commit objects on a user's
checkout: sequentially applied patches and the retained guard are enough.

The final addition is 16 products, 16 knowledge records, 25 remote firmware
entries and 25 manual watch entries. With this published base, integration
must produce exactly 676 products (407 Waveshare), 609 firmwares, 441 watch
entries and 684 knowledge-index entries. A count mismatch or unrelated
catalog edit stops integration instead of being silently overwritten.

`catalog-recovery.json` records the successful integration, per-model final
plan/metadata hashes, retained-source checks, strict generated-entry schema
checks and unchanged registry digests. `platform-catalog-check.json` retains
the completed full-catalog validator result and its input catalog hashes;
all sixteen model/plan hashes were checked again after the assets froze.

## Firmware identity and defaults

All thirteen source manifests are kept byte-for-byte. They reference original
manufacturer ZIP members with fixed container/member hashes and original
offsets. No BIN is rebuilt, combined, patched, committed or hosted here.

The original Nano test and Zero BlinkRGB retain `example` roles; the linked
MicroPython images retain `development` roles. Factory investigations remain
reference entries, ordered before these alternatives, with explicit example
or development defaults where a complete exact-target manifest exists.
Header variants and N16R8/M/U/UM trials do not inherit the original base
SKU's execution acceptance. Zero-N8R8 and N32R16V do not receive nearby
capacity or protocol variants as substitute images.

The retained image audit in `../batch-04/firmware-image-audit.json` separately
records segment checks and the rejected Zero `combine/target.bin` (ESP32-C3
chip ID 5). That rejected member never enters these selectable manifests.
`simulationVerified` and `hardwareValidated` remain false for every firmware;
watch entries are manual, reference-only contracts with no automatic
promotion targets.

## Current and historical model evidence

The previously published Zero base assets remain unchanged. The other fifteen
models have explicit current model plans and source identities, with new
verification under
`docs/platform/evidence/waveshare-batch030-models-20261124/` (`nano-pico/`
and `zero-devkit/`). Knowledge records read the actual final plan dimensions,
method and sources, checking every retained and decoded source digest.

The earlier `../batch-04/` model reports and 36 review images retain their
original hashes. They document that earlier model revision and are not
silently reused as acceptance of newly generated assets. Current export,
production-loader, photo-registration and geometry checks must bind the
current `model.json` and geometry hashes. Partial photo envelopes, UNKNOWN
surfaces, estimated heights and physical-fit limits stay explicit.

This is a source/model/catalog reconstruction. Native guest diagnostics,
production WASM, browser/WebGL and real hardware acceptance remain separate
claims, each requiring its own evidence. No physical-board acceptance is
claimed by this catalog.

## Reproduction

With the committed final assets present, generate the sixteen knowledge
records and four fragments into a separate directory:

```sh
WAVESHARE_METADATA_REBUILD_DIR="$(mktemp -d)"
python3 tools/waveshare-batches/s3-industrial/recover_fourth_from_retained.py \
  --output-root "$WAVESHARE_METADATA_REBUILD_DIR" \
  --receipt "$WAVESHARE_METADATA_REBUILD_DIR/catalog-recovery.json"
```

This command performs source, model and schema checks before writing the
generated metadata. It needs the repository's catalog-validation Python
dependencies, but no first-pass authoring directory or network access.

To integrate into the matching catalog snapshot, use `--integrate` without
`--output-root` and provide a receipt destination. This only replaces this
generator's sixteen owned entries when rerun; every unrelated existing
object must still match the published guard. The generated Rust memory
table and browser capability registry are read and hash-checked, never
regenerated or modified.

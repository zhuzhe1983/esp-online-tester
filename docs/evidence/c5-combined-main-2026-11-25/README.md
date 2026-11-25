# C5 combined with current main

Private checkpoint9dcb8a52 merges public main5492cc4c without conflicting paths,
preserving all43 changed main blobs exactly: Adv audio/DMA, CodeCache cleanup,
native window helpers, tests and catalog metadata. The C5 branch and its private
input/history have not been published.

Both required Clippy commands pass with warnings denied. The new default-release
production WASM is8,854,206B, SHA256
`91497e8f539de0e537cebf7f394378718d76964aab8cf24ce0b6c4724c88c8b2`.
Twenty Node checks in seven files pass with0 failures/skips; all14 actual WASM
firmware demos plus scheduler JIT handoff pass. The build used8 jobs with default
release/LTO settings, no target-directory override. Recorded elapsed values are
operational; concurrent input builds may overlap, so no performance claim.

The real worker executes unchanged original LCD2.73 through ROM, bootloader,
PSRAM test, app_main and SHTC3 ID0807, then stops at the same MAC ModelLimit,
PC408142c6. Result is partial, exit3, acceptancePassed:false. Its4536B UART has
the exact previous hash. The exported64MiB card is directly byte-equal to input,
with0 changed sectors/bytes. Host attachment/preparedFAT32 is no guest mount;
Node memory storage supplies no browser durability proof. Runtime disposed and
all workers exited. No browser was launched here; earlier browser observations
remain a separate checkpoint.

The helper's21 preservation/structure/corruption checks were independently
rerun and all551/602 artifacts stayed unchanged. Reviewed fresh-path/merged-
layout defects are repaired. The575 focused native C5 cases belong to preceding
16f4cfc3, not a complete native test of merged main. The earlier3459-test global
success also predates this combined source. Full workspace and CPU controls are
batched after manufacturer LCD/SD execution and any final kernel fixes. Original
factory LCD/buttons/TF acceptance and publication remain incomplete.

Reproduction uses `cargo clippy --workspace --all-targets -- -D warnings` and
`cargo clippy --release --target wasm32-unknown-unknown -p esp32sim-wasm --features jit-tests -- -D warnings`,
then `CARGO_BUILD_JOBS=8 sh tools/wasm-build.sh` with target/LTO overrides unset.
Run the seven Node files documented in the previous runtime receipt; the14
manifest names and exact log/artifact identities are in [result.json](result.json).
The pinned original worker command remains in that preceding receipt, using
this new WASM hash and a fresh private output directory. No goldens regenerated.

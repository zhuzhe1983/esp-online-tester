# C5 CPU memory policy integration checkpoint

The unchanged rev100 ROM and original Waveshare ESP32-C5-LCD-2.73 factory now execute their own PMA/PMP configuration and finish the IDF PSRAM test. The guest test writes and compares one u32 every32 bytes over8MiB;262144 backing cells were independently checked in the native regression. This is not an every-byte memory test, application/UI acceptance, or a hardware timing claim.

The optional C5 CPU owner handles52 CSRs through checked instruction access. CSRRW rd=x0 preserves the omitted-old-read rule. Epoch/generation snapshots apply at CPU Fetch/Load/Store and LR/SC/RMW boundaries; raw ROM replacement cannot unlock the CPU. DMA keeps its separate rules. Known permission denial produces the architectural fault; uncertain or unimplemented policy semantics stop the host model without inventing a guest exception or an instruction word. CLIC vector uncertainty precedes guest interrupt-entry state. Ordinary and priced observer early stops clear CPU context before host inspection.

The original firmware returns from protection setup at0x42000934 with its own8MiB heap observation, then reports `SPI SRAM memory test OK`. Its next boundary is an unimplemented read of PCR PLL divider gates at0x60096128, PC0x4211f8ee. No app_main, LCD, button or TF success is inferred.

[checks.json](checks.json) records source/input/log hashes,337 C5 tests including6 input cases,124 RV32 tests, lint commands, negative outcomes and review fixes. The canonical policy contracts also run in the normal workspace through a source-sharing test bridge. See the [pure helper contract](../c5-memory-policy-2026-11-25/README.md) and [ROM POR staging correction](../c5-rom-pma-staging-2026-11-25/README.md) for fixed SDK/TRM sources and explicit uncertainty.

Focused checks use LTO=false. Full default-LTO CI, identical-work CPU controls, final production WASM/browser checks and product acceptance remain required before publication. Raw captures stay ignored; curated evidence omits personal paths and process/session data without changing measured facts.

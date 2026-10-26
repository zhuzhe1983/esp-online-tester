# Cardputer-Adv official User Demo evidence

The M5Burner v0.3 image is fetched directly into RAM, checked against SHA-256 and booted from the ESP32-S3 mask ROM. No firmware bytes are retained in this repository or its server assets. No hardware flashing or silicon comparison was performed.

- `source.json`: actual remote asset size/hash and CORS response; internal app version is separately recorded in `console-excerpts.txt`.
- `sources.json`: retrieved official driver code, IDF headers and chip datasheets, with UTC timestamps and hashes. Factory branch commit `b549eac0a3c65bc108186c276b8fac0a214aaa4e` is source evidence, not a claim of reproducing the release binary.
- `interaction.json`: final WASM hash, actual guest-clock input receipts, frame hashes, elapsed host time and storage scope. The harness has no browser IndexedDB and truthfully uses memory-only storage.
- `boot.png`, `launcher.png`, `right.png`, `left.png`: real register-backed framebuffer output. Enter release leaves the boot page; right selects Chat; left returns to Record. The selected menu interior matches pixel-for-pixel after a 1 px horizontal translation. No image was altered for this comparison.
- `negative-results.json`: black display before SIO routing, ordinary-Cardputer misidentification before open-drain resolution, false partial-frame success before GDMA continuation, and exact-position hash failures. Original failed artifact hashes are preserved.
- `checks.json`: final source hashes and checks. `native-regression.json` is populated separately by the parent native timing comparison.

The old `short_ms_dlen_retires_an_overlong_eof_descriptor` test from commit `7627eeb5` codified retiring and discarding an entire descriptor after a shorter SPI phase. M5GFX 0.2.15 `execDMAQueue` explicitly issues multiple USR phases on one GDMA chain for transfers larger than the 18-bit SPI length limit. The revised digital model preserves unread bytes and delays descriptor EOF/owner changes until consumption. New tests cover SPI2/SPI3, existing timed completion, crossings, owner/EOF, STOP, OUT_RST and fresh START/RESTART. Internal DMA FIFO prefetch and early EOF timing are not modeled or claimed silicon-accurate.

Open upstream PRs were checked before shared changes. PR #293 modifies the GPIO array size and configuration access for P4; its reviewed patch has no open-drain implementation. The narrow PAD_DRIVER change uses the current shared block and must be carried into its future `set_pin_config` method if that PR is integrated. No second GPIO block was created.

Reproduce with the caller-supplied URL/hash command in [runtime scope](../../cardputer-adv-runtime.md). Prior stable GPIO/input and SPI3 mechanisms are EX217/EX235; CPU/JIT and guest-clock cadence are unchanged. Test captures contain only public firmware identifiers, counts and generated pixels; full console logs are not committed. No private firmware or user file paths occur in this evidence.

## Native regression comparison

`native-regression.json` preserves three alternating baseline/candidate pairs per workload against main `9e48897`, timed with `/usr/bin/time -p` at 30 guest seconds. All instruction counts, cycles and stdout hashes match. Median user CPU seconds: S3 hello 0.25→0.25, C3 hello 2.31→2.27, C6 hello 1.50→1.50, Pocket Tank 38.50→38.48. No task-owned builds, probes or CAD renders ran during sampling. These short samples show no measured regression; they do not establish a speedup or cover every workload. Commands, input/binary/source hashes and every sample are in the receipt.

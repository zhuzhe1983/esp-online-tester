# U004 on current main

This receipt migrates the earlier U004 stage onto `5a9b68e0`, retaining all 28 M5Stack controller identities and the current StopWatch internal I²C/RTC routes. The former `4670491d` receipts in the parent directory remain historical. This is not a merge of that old baseline or an adaptation of another PIR SKU.

- `checks.json`: exact production WASM/source hashes and complete CI command results. No golden updates.
- `probe.json`: actual source-built IDF firmware via the page worker. Eleven groups cover G11/G10, nominal 2-second hold/retrigger, PMIC supply, unplug/pulls, guest/platform reset, recording and U006 parent/child state.
- `browser.json`: actual UART0 transitions after human panel input in Studio and mini, mini share autostart, plug views, materials and UNKNOWN PCB.
- `source-recheck.json`: fresh GET of official product/schematic; bytes unchanged. Original media/URL/fetch records remain in `../../source-audit.json`.
- `rebuild-current.json`: public `rebuild-unit-library.py` scratch rebuild reproduces all 11 generated files exactly. `model-plan.json` is the input, not a generated output. Historical dimension and photo-contour metrics are reused without changing their timestamps.
- `cpu.json`: completed current-baseline idle CPU comparison. All four guest work/output pairs match; no speed or no-regression claim.

The binary fixture is retained only in ignored local output. Its source and all three artifact hashes equal the old source-built fixture receipt. The public catalog contains no diagnostic image. Host/lane identities, browser profiles and raw local logs are not included. Screenshots contain only public product data and internal fixture UART output.

The first material script passed the inner shell object rather than `{shell: ...}` and was rejected before applying it; corrected calls passed. Passing null to the review helper uses the StopWatch black default, so the standalone back screenshot is explicitly a black-shell isolation check. Original white geometry/color is visible in the connector and Studio rear captures; none of these are newly measured photo-overlay scores. A test initially expected a G11 conflict for not-yet-migrated ENV/DLight IDs; it now checks `UNKNOWN_ACCESSORY` when absent and `PIN_CONFLICT` after their own catalog entries are integrated. No product runtime relaxation was made.

## CPU reproduction

Use a fresh scratch source directory for `git archive 5a9b68e0`, build that source and this branch with `cargo build --release --bins`, and supply populated public ROM/demo/Pocket Tank inputs at the paths pinned in `cpu.json`. The immutable baseline receipt and copied binary hashes are in `baseline.json`. This is the delayed U004 idle-path comparison, not a new execution optimization. It reuses EX201 physical edge/deadline delivery and the EX212/EX220 cold-allocation/layout lessons; the materially new contract is a powered, retriggerable U004 GPIO deadline.

```sh
python3 docs/platform/evidence/m5stack-unit-pir-u004/runtime/bench.py \
  --base-bins <baseline-target>/release \
  --base-revision 5a9b68e0cd5aee1c6c51ca17c9fd2250a3c23fa7 \
  --candidate-bins target/release --rom-dir web/wasm/fw \
  --receipt <new-cpu-receipt.json>
```

The historical U027 C6 +1.875%, U003 Pocket +1.68%, and earlier ENV-IV C6 +2.994% are separate comparisons and are not erased or reinterpreted by this receipt. Concurrent compiler/emulator jobs were deferred; unrelated desktop load was not controlled. Raw alternating samples and aggregate load are retained.

## Observed CPU differences

| Workload | Baseline median user s | U004 median user s | Change | Slower pairs |
|---|---:|---:|---:|---:|
| hello | 0.25 | 0.25 | +0.00% | 1/5 |
| c3-hello | 2.43 | 2.42 | -0.41% | 2/5 |
| c6-hello | 1.63 | 1.64 | +0.61% | 4/5 |
| pocket-tank | 38.48 | 38.72 | +0.62% | 4/5 |

One excluded warmup pair preceded five alternating measured pairs per workload. Every measured instruction count and stdout SHA-256 matches. C6 +0.61% and Pocket Tank +0.62% remain observed, unattributed differences; they are not discarded as noise. No source was changed to chase those samples and no unchanged-candidate rerun was used. The optional PIR state is allocated only for StopWatch, its input parser is S3-only/cold, and its trait method is appended after existing methods; there is no new common Machine field or C3/C6 per-tick service. These source facts do not prove zero code-layout cost.

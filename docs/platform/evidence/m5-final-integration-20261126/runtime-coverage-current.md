# Current production constructor coverage

[runtime-coverage-current.json](runtime-coverage-current.json) binds the 141-SKU
snapshot `977a32b3c664a7c08368edf0fa21e2582e16a2d4` to the production WASM
SHA-256 `2db0cec618a53063c309962c3e37c94fc2b839e8d0e6f338224e42f54ea15902`
(9,326,209 bytes). All 141 actual constructors accept their catalog board and
memory profiles. No firmware is loaded or executed by this check.

The generator accepts production `web/wasm/esp32sim.wasm` by default. An explicit
`--wasm` or existing `ESP32SIM_WASM` override is separately identified. Optional
`--expect-wasm-sha256 HEX` fails before construction/report creation on mismatch.
The default report filename is distinct from the retained historical 020 matrix.

The three focused tests pass without skips: a wrong pin fails, a missing WASM
fails, and the real default production artifact constructs 141/141. Separate
negative checks against the actual production bytes and a missing path both
exit 1 and create no report. Their normalized error lines and original stderr
hashes are in the manifest. Only relevant errors are retained; local path labels
are normalized and private stacks/process inventories are omitted.

Construction is distinct from the 98 retained historical anchors. Of those,
78 record a different WASM hash and 20 record none; none is rerun by this audit.
Matching an artifact hash would establish identity only. Exact-SKU execution
references, native board tests, and factory-application acceptance retain their
original individual scopes. The current snapshot still has all 141 products
marked partial; 35 SKUs lack a dedicated retained firmware-survey anchor.

Reproduction after building production WASM:

```sh
M5_COVERAGE_SOURCE_ROOT="$SOURCE_ROOT" \
M5_COVERAGE_REVISION=977a32b3c664a7c08368edf0fa21e2582e16a2d4 \
M5_COVERAGE_REPORT="$IGNORED_OUTPUT/production-matrix.json.gz" \
  node --test tools/audit-m5-runtime-coverage.test.mjs
```

This test uses no `--wasm` argument or expected-hash pin for its successful run.
For a pinned direct check, invoke the generator with `--source-root`, `--revision`,
`--out` and `--require-all-construct`, adding the expected SHA-256 above.

The full generated matrix compresses to 155,624 bytes. It is retained losslessly
as a header plus four useful product groups: original101 S3 with PSRAM, original101
S3 without PSRAM, original101 other chips, and continuation40. Each retained
file is below 50 KB. The manifest records file hashes, the original whole-matrix
hashes and product order. Reassemble the original matrix with:

```js
const header = JSON.parse(gunzipSync(await readFile(manifest.header.path)));
const rows = (await Promise.all(manifest.productGroups.map(async group =>
  JSON.parse(gunzipSync(await readFile(group.path)))))).flat();
const byId = new Map(rows.map(row => [row.productId, row]));
const products = manifest.productOrder.map(id => byId.get(id));
const original = JSON.stringify({...header, products}, null, 2) + '\n';
// SHA-256(original) equals manifest.fullGeneratedMatrix.uncompressedSha256.
```

The historical `m5-runtime-coverage-final-20261126/matrix.json` is unchanged;
its prior SHA-256 and byte size are recorded in the current manifest.

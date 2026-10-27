# WASM JIT test process isolation

The CI differential suite exhausted Node's heap in one synchronous call. The test driver now runs the same assertions in 78 separate Node processes (77 without `wasm-cache-inline`). Production compilation, release behavior, execution and goldens are unchanged.

A local Node 22.23.1 / V8 12.4 arm64 run with a **512 MiB old-space limit** passed **111,126 differential cases**, releasing all **99,679 compiled modules**. The separate GPIO waveform run passed 24 cases and released all 738 modules. The largest child RSS was 623,968 KiB; the old-space limit does not cap WASM memory, code memory or total RSS. See [the measured groups and hashes](results.json).

## Why explicit GC was rejected

The original CI failure was [run 40402594341](https://github.com/zhuzhe1983/esp-online-tester/actions/runs/40402594341), with about 3,520.5 MiB live heap reported near the fatal allocation failure. Using the original test artifact locally, both the normal host and a diagnostic host calling `global.gc()` after each 256 releases still exhausted a 512 MiB heap. In the GC run, the last sampled point had 4,864 compiled and released modules, no live table entries, 19 explicit collections and 200,240,024 bytes of heap.

A 4,096 MiB diagnostic run with the same GC policy was stopped intentionally after its last sample reached 66,240 compiled/released modules, 258 explicit collections, 2,847,792,112 bytes of heap and 149.907871792 seconds. This is evidence of continuing growth, **not** a claimed OOM or completion at that limit. Explicit GC was not added to production or the CI driver. Released table entries alone did not bound V8 heap in this workload; the exact internal ownership mechanism was not proven with a heap snapshot.

The retained [diagnostic harness](gc-reproducer.mjs) accepts caller-supplied host and legacy WASM paths:

```sh
node --expose-gc --max-old-space-size=512 gc-reproducer.mjs /absolute/path/to/web/wasm/jit.mjs /absolute/path/to/legacy-jit-tests.wasm 256
```

Use `0` for the no-explicit-GC control. The measured legacy artifact hash is in `results.json`; it is a local test build, not a deployed firmware. The raw abort logs are not committed because their stacks contain local installation paths. Curated numeric samples preserve the values relevant to the conclusion.

## Case preservation

The pre-change [original dispatch](original-dispatch.txt) is retained for comparison. Every original suite remains in `xtensa-lx7/src/jit/wasm_tests.rs`. Both the original `esp32sim_test_block_jit` entry and the new grouped exports use the same canonical function list. No assertion, operand array or inner budget loop was removed.

Only three Cartesian products were partitioned at existing outer loops:

| Original function | Partition | Total cases, each pricing mode |
| --- | --- | ---: |
| `arithmetic::integer_ops` | Opcode index modulo 3; unsupported-op check only in part 0 | 4,539 |
| `float::floating_point` | Opcode index modulo 4; non-opcode tail only in part 0 | 5,600 |
| `control::entry_and_shifts` | Two shift opcodes and nine window/flag combinations | 14,300 |

Each original outer-loop item maps to exactly one registered part. The priced wrapper retains the same `PRICED` state for each original case, including the 147 directed cases, totaling 38,137 priced cases. The core suites total 110,670; unchanged SoC and memory suites add 452 and 4, for 111,126. The equality with the original case set is established by this one-to-one partition and the retained dispatch, **not by claiming that the legacy monolithic process completed**; it did not.

## Reproduction

```sh
tools/wasm-jit-test.sh
node --max-old-space-size=512 tools/wasm-jit-test.mjs
node --max-old-space-size=512 tools/wasm-jit-test.mjs gpio-waveform
```

The first command builds with the shared production WASM compiler policy, then runs the driver. The latter two commands impose the measured heap bound on the same test artifact. The parent forwards Node arguments to each child, waits sequentially and fails on any child error, assertion, invalid count or unreleased module. Large suites retain their assertions together; no guest execution is skipped. Both new Rust exports are gated by the existing `jit-tests` feature.

Release WASM Clippy with `jit-tests` and warnings denied passed, as did the driver syntax check. No native or browser speed conclusion follows from this test-only change. The earlier asynchronous-installation experiment EX119 changes production behavior; it was read for context and was not retried here.

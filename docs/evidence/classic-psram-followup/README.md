# Classic PSRAM integrated follow-up (EX234)

Base: upstream main `77e43c20a70054ac84d1724d757fa9ce12668151`, including #232.
Reviewed starting head: `304125aab8b3e933a31cc682dc687fb40415207d`.
Memory-miss implementation and regression: `9823ed9a978c0fc798ffebc322662624566758f5`.
Documentation-only descendants do not change the tested Rust. There is no rebase
or unmerged dependency. Historical receipt source lists now name this integrated
ancestry rather than superseded feature-stage revisions.

## Change

The external-window checks now execute only after `memory()` fails to supply
ordinary memory. The cold miss handlers dispatch PSRAM loads/stores, preserve
unmapped versus prohibited faults and last-fault metadata, and prohibit PSRAM
instruction fetch. Peripheral accesses and mapped-memory checks precede this
dispatch. SPI commands, DCLK decoding, cache/MMU rules, CLI configuration, backing
ownership and reboot semantics are unchanged by this follow-up.

The new regression checks both ends of the window, byte accesses just outside
it, load/store/fetch fault distinctions, last-fault metadata and an absent device
with otherwise enabled cache/MMU state. Six individually rebuilt mutants must
fail named assertions: drop load dispatch, drop store dispatch, shorten either
load/store bound, include HIGH in fetch's window, and allocate backing when absent.
Compiler errors or host bounds crashes are not kills. See `mutations.json` and
`mutate.py`; sources are restored in a finally block and the positive control reruns.

## Final linked codegen gate

`base-hotpaths.disasm.gz` and `head-hotpaths.disasm.gz` are the only retained
classic-PSRAM hot-path pair. `disassembly.json` identifies the binaries and full
function instruction totals. They are normally linked release binaries, built
in separate source trees/targets with the same `cargo +1.99.0 build --release
--workspace`, bounded jobs and nice priority. This is static evidence, not timing.

Reproduce with `disasm.py --base BASE_BINARY --head HEAD_BINARY --out OUTPUT`.
Each file contains tick, fetch, read8/read16/read32 and shared store. Instruction
addresses are relative to each function; opcode bytes and source/debug metadata
are omitted, while instruction operands and resolved targets are retained. The
binary SHA-256s allow the complete raw listing to be regenerated.

Codegen gate: **explained deltas, CPU table pending**. Literal main-equivalent
codegen is not claimed. Whole-function static instruction counts are:

| Function | Main 77e43c20 | Head implementation 9823ed9a |
| --- | ---: | ---: |
| tick | 2207 | 2207 |
| fetch | 122 | 127 |
| read8 | 106 | 104 |
| read16 | 122 | 117 |
| read32 | 171 | 178 |
| store | 2722 | 2739 |

Tick retains its mnemonic sequence with relocated operands/field offsets. LLVM
merges slow-path Result construction into successful return blocks: read32 gains
dynamic Result repacking, read8 adds a discriminant mask, read16 changes packing,
and fetch changes byte assembly. These are real instruction differences, not
merely layout. The shared store also includes inherited active SPI-path changes;
its successful writable-memory path retains the existing translation, bounds,
copy and invalidation checks. Inspection of the linked pair finds no additional
PSRAM-window conditional branch on successful ordinary-memory fetch/load/store
hits. Store's ROM-write/fault path does contain the miss-path window guard; that
is not a successful writable-memory hit. Some branch directions and placement
change, so this is not a claim of identical executed instruction sequences.

The explained-delta criterion permits these disclosed differences without a
broader return/bus-boundary redesign. Static function totals are not executed
counts or CPU clearance. The exact final pair and counts are recorded in
`checks.json`; the pending alternating CPU comparison must establish whether
these deltas cost time, and any flagged slowdown must be reported honestly.

## Evidence archive and compact receipts

Before deletion, all 167 original tracked classic-PSRAM evidence files at
`304125aab8b3e933a31cc682dc687fb40415207d` were copied and byte-verified in the
separate local evidence archive, under
`retire/classic-psram/evidence-originals/304125aa/`.
The archive's `SHA256SUMS` has SHA-256
`24cd108e6dfd71641a707a06d723d35daedf15c54341f0818f4a00bbab6196a5`.
The same originals remain recoverable from that ancestor's repository tree.
No raw archive, backup, local identity or absolute host path is committed here.

The compact area receipts retain public source citations, reproduction inputs,
exact SDK accounting, relevant negatives and limits; old codegen pairs and
repeated stage-result JSON are not current-head proof. `fixture-inputs.json`
records the pinned external firmware reused for fresh qualification. The recipes
remain in `docs/fixtures/`. No new firmware or SDK rebuild is claimed.

Retained negative findings:

- Normal-only translation originally failed the stock dual-core Arduino low/high
  memory test. Low/high support was necessary, not an optional fixture edit.
- Self-consistent guest readback did not kill historical slot/shift probes.
  Independent native physical-layout tests did. The older bounds-removal probe
  included a host panic; it is not one of the new assertion-only kills.
- An older extra WASM JIT assertion failed after reboot despite production-mode
  success; only 13/24 planned comparisons ran. The integrated selected-owner/reset
  regression fixes that diagnostic/ownership contract, without treating unexecuted
  rows as passes. See [the bounded WASM receipt](../classic-psram-wasm/README.md).

The SPI test's own source header points from the repository root to
`docs/fixtures/classic-psram/README.md`, which retains the captured A2/A30
USER/USER1/USER2, PIN, ID-word and failed-KGD values. The evidence SPI receipt
also links the tracked test. The P0 document is explicitly historical; the IDF
fixture names integrated ancestors and links current low/high and DCLK results
without erasing the original failed low/high and 40/40 profiles.
The tests index links this integrated receipt and the retained WASM negative.
EX234 is added immediately below main's current final row because the catalog
also contains comparable feature/emulation rows (EX224-231), not only perf work.
EX232 and EX233 are reserved by independent camera/deep-sleep branches; integration
must preserve those rows rather than reusing their IDs.

## Verification

The reference recheck reads all tracked UTF-8 text, including Rust comments and
fixture documentation, and checks classic-PSRAM Markdown links, bare repository
provenance paths and current integrated ancestry claims. It resolves 56 path
occurrences and verifies 25 current ancestry claims across 2594 text files:
zero dangling targeted references and zero stale current ancestry claims.
This is not a claim that all repository links or arbitrary prose are validated.
The previous 375-path evidence-Markdown-only pass is now labeled historical in
`checks.json`. Historical capture revisions, SDK source revisions and negative
profiles are retained, not treated as current integrated ancestry.

This remediation changes documentation/comments only. The functional, mutation
and linked-codegen results below were run on the unchanged executable sources;
they are not fresh executions on a new documentation-only commit. Privacy,
fixture provenance, source equivalence, targeted references and mergeability
are rechecked; no CPU timing clearance is claimed.

All 24 local gate command entries pass. The empty-HOME suites pass 1037 tests
with ROMs and 1006 without ROMs (60 ignored); virtual-quantum tests pass 43/43.
The native AArch64 package passes 53 tests with one ignored. The full WASM JIT
run passes 111126 differential cases with 99679 compiled modules released, then
24 GPIO waveform cases with 738 modules released. All nine manifest demos pass.
All 55 tracked golden files are byte-identical to the base. The linked hot-path
gate is explained deltas, CPU table pending, as detailed above.

`checks.json` gives fresh command exits, test totals, exact codegen gate status,
fixture results and unchanged golden hashes. Native ROM-enabled and plain tests
use an empty HOME with explicit Rust installation/cache homes. All builds use
Rust 1.99.0, nice priority and two jobs, on AArch64 macOS 27.0.1. Both linked
binaries use the workspace's release fat-LTO defaults. Every executable step
in the CI workflow
is run locally, including native AArch64 tests on this AArch64 host, production
WASM sections, nine demos, timing/BLE APIs, worker/browser/receipt checks, JIT
differential tests and profiling-feature compilation. This is not a remote CI run.

Four fixture profiles are exercised: standalone IDF 4.4.7 and stock Arduino
2.0.17, 3.0.7 and 3.1.1, with present/absent controls. MicroPython uses the official
SPIRAM image identified in the [DCLK receipt](../classic-psram-dclk/README.md),
with no ROM/guest patch, function stubs or reboot retry. Required free-byte values
remain exactly 4178272 (4 MiB) and 160096 (absent); receipt verification checks
these captured values in addition to the guest test's broader heap-size assertions.
The standalone low/high prerequisite replay is a separate control.

Evidence privacy checking covers tracked text and gzip files; manual inspection
supplements configured patterns. Only public fixture data and emulator outputs
are used. No production query, identity export, replay or hardware capture.

## Unchanged model limits

1. The shared PRO/APP gate has no caller-core identity. In normal mode a live APP
   requires matching tables and enabled/unmasked caches even for PRO accesses.
   Low/high uses the owning cache for each address half. These are qualification
   restrictions, not independent per-requester hardware routing.
2. SPI reads beyond the backing array return 0; an absent device reads 0xff.
   These are distinct outcomes. Supported guests probe ID/capacity and access only
   the configured 4 MiB; the qualified allocator and mapped accesses do not rely
   on out-of-array SPI fill. No claim is made for guests deliberately reading
   beyond capacity or for hardware-compatible out-of-array bus values.
3. Arduino qualification uses local external pinned public fixtures. The standalone
   IDF allocator and real CLI boot tests use committed firmware and run in CI.
   Passing the CI workspace alone does not run those external Arduino builds.
4. Warm machine reboot zeroes PSRAM and invalidates SRAM tables while retaining
   configured capacity/allocation. Hardware warm reset retains RAM contents;
   zero-fill is the emulator's deterministic power-cycle policy.
5. Cache-disabled access raises Prohibited, not the hardware cache-IA interrupt.

Only optional 4 MiB data PSRAM is supported. No instruction execution from PSRAM,
external DMA, camera integration, other capacities, himem API, line-level timing,
cycle-accurate caches, full dual-core fidelity or silicon equivalence is claimed.

## CPU comparison

Measured by the maintainer (joakimeriksson, PR #246 review of `f8cfc039`): main
`6cf190c6` vs `f8cfc039`, user CPU, 7 alternating pairs, identical instruction
counts and console hashes, load gate 5.

| workload | main | PR | change | PR slower |
| --- | ---: | ---: | ---: | ---: |
| classic hello, 30 s | 0.370 s | 0.370 s | +0.0% | 0/7 |
| T-Display MicroPython, 60 s | 0.920 s | 0.920 s | +0.0% | 3/7 |

Neutral: nothing flagged (slower 6/7 or more). The non-layout differences above
remain explained codegen deltas; this table is the timing evidence.

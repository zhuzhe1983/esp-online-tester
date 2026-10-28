# StopWatch official V0.5 display tearing

The unchanged official User Demo V0.5 reproduces a horizontal discontinuity while
swiping the launcher from AlarmClock to WatchFace. The discontinuity already
exists in the raw framebuffer received by the workbench. All 68 captured frames
are pixel-identical to the 2D canvas at the same event; the 3D renderer is not
needed to reproduce it.

This is an actual product reproduction following the source audit of
[EX056](../../experiments.md#ex056), [EX117](../../experiments.md#ex117),
[EX118](../../experiments.md#ex118) and [EX128](../../experiments.md#ex128).
The material difference is the official V0.5 image, guest-timed launcher swipe,
and pixel-level comparison of consecutive original frames and their 2D canvas.
No CPU clock, JIT, quantum, publication policy or firmware was changed.
That statement describes the original reproduction below; the follow-up at the
end records the narrow presentation fix and its separate validation.

## Exact counterexample

For output frame 153 (capture index 15), a unique split at **y = 240** satisfies:

- Rows 0–239 equal frame 154 exactly, and differ from frame 152.
- Rows 240–465 equal frame 152 exactly, and differ from frame 154.
- Concatenating those regions produces the complete frame-153 RGBA SHA-256
  `074afa218f0150dd317d177d0638731a595ea480d2181b70616e090e5df04990`.

Frame 153 has a visibly displaced lower portion of the moving icon. The
[receipt](receipt.json) retains the neighboring frame hashes, inputs and exact
checks. Five captures have unique split rows of 180, 240 or 300. The analyzer
also reports matches in empty bands with a range of possible split rows; those
matches alone are not evidence of a visible discontinuity.

At frame 153, the latest guest stat was 20.96 s; the latest worker sample reported
speed 1.010657, behind 0.0062 s, and zero resynchronizations. These are **diagnostic
context, not a performance result**: the browser recorded PNGs and screenshots,
and other work was running on the same host. They do not establish throughput on
other machines or scenarios. `stat.time` is not an exact frame-generation time.

## Reproduction

Use the existing production WASM, S3 ROM and current workbench; the measured
artifact and served source hashes are in the receipt. The official catalog
selects the original full V0.5 image with SHA-256
`6395799089341e0ae96072c2a3418d469d6797b2ab8ee0cab1c9ed24a42acbbe`.
Execution uses ROM boot and the existing StopWatch interpreter policy.

Open the workbench in a fresh headless browser with the Playwright skill wrapper.
Install [the probe](../../../tools/diagnose-stopwatch-tearing.mjs) by importing
it from a same-origin checkout, or evaluate its function body with the wrapper.
Do not use an existing user profile or clear user Flash. The probe rejects a
restored/non-pristine Flash state in `prepare()`.

```js
const probe = installStopwatchTearingProbe(window.espTester);
await probe.prepare();  // 15 guest seconds, A+B held 1.5 guest seconds, launcher
await probe.swipe();    // supplied down-swipe path: primarily the battery pill
// Save probe.export() to an ignored output path if retaining this first run.
await probe.adoptPaused();
probe.frames.length = 0;
await probe.swipe([[400,233],[340,233],[280,233],[220,233],[160,233],[90,233]]);
const capture = await probe.export();
probe.dispose();
await window.espTester.dispose();
```

The horizontal swipe waits at least 0.12 guest seconds between points and 0.15
guest seconds before release, then records 1.5 further guest seconds. Actual
acknowledged times are retained. The first down swipe captured 63 frames but did
not provide a clear full-icon discontinuity; this negative result is retained.

Save the export JSON under `target/` and run:

```sh
python3 tools/verify-stopwatch-tearing.py target/tearing-export.json
```

The checker requires Pillow. It verifies each raw RGBA hash, compares every raw
image with its captured 2D canvas, and finds exact previous/next row splices.
The original browser run also took before/during/after Playwright screenshots
of only the 2D canvas element, excluding serial logs. Close the task-owned browser
with the same wrapper session name and verify its exit.

## Interpretation and limits

`Co5300::write_pixel_bytes` modifies current GRAM as transfers arrive;
`StopWatch::display` clones that GRAM. Every pixel-changing SPI transaction bumps
the board version. The 50 Hz guest-time publisher samples the current memory;
it does not know the application's complete draw boundary. The worker copies
whole snapshots, and the page decodes whole snapshots. This explains how a
partially updated guest screen can become a complete but visibly split host
image. The trace needed to distinguish one split window from multiple separate
windows is not included in this first receipt. A full-screen-only commit rule
could hide legitimate partial updates and is not justified yet.

The 466-pixel circular glass uses a 468×466 GRAM stride in this firmware; this is
intentional and must not be changed to address tearing. Existing TE is synthetic
and does not model physical scanout. This test establishes the visible emulator
failure, not correspondence with a physical display's scan timing.

Only compact numeric evidence and scripts are committed. Original PNGs and the
export remain in the ignored capture directory named in the receipt, with hashes
and byte lengths recorded; they have not been published to an artifact host.
No raw serial text, private Flash, account details or machine identifiers are
retained. The captured screen contains only the official launcher. The browser
session was disposed, closed, and verified exited.

Checks: the real browser scenario completed; all 68 image hashes and canvas
comparisons passed; the row-splice checker found the unique y=240 counterexample.
The browser probe passes `node --check`; the evidence privacy check and Git
whitespace check pass.

## Window trace and presentation fix

The [temporary trace patch](window-trace.patch) records window commands and
pixel-transfer entry points only between guest seconds 20 and 22. It was removed
before implementing the fix. The [trace summary](window-trace-summary.json)
contains 183 RAMWR commands, no RAMWRC, and 19 complete top-to-bottom sweeps.
Each sweep consists of **eight independent full-width windows**: x=6..473,
y=0..59, 60..119, 120..179, 180..239, 240..299, 300..359, 360..419 and 420..465.
First-to-last window starts are about 15.2–16 guest milliseconds apart. Separate
small label/dot windows also occur. Waiting for one address window would still
expose stripes; requiring every update to fill the screen would hide local UI.
Trace overhead prevents using this run to measure speed.

The fix therefore changes **StopWatch host presentation**, while existing GRAM
and firmware readback remain live. A new cold `BoardModel::display_for_ui` method
defaults to `display()` on all other products and is appended to preserve existing
hot vtable offsets. StopWatch retains the previous picture when a full-width
top strip begins, and releases it only after contiguous, fully written windows
reach the bottom. It recognizes window geometry, not a hard-coded eight-window
count. The actual software-CS header and per-row payload sequence is covered.

Partial/local or out-of-order windows terminate the grouping. A new top strip
restarts the group while retaining the preceding coherent picture. If a pass is
abandoned, 20 guest milliseconds without pixel writes or a new RAMWR releases
the current GRAM at the next existing publication opportunity (normally within
20–40 guest milliseconds). No extra tick/deadline callback was added. The web
publisher records the **returned presentation version**, preserving a pending
raw generation until this final fallback is delivered, even without new writes.
Reset discards the retained picture. Existing unmodeled sleep/display-off optical
behavior is unchanged; those commands cannot leave the last GRAM update hidden.

This is not physical TE, scanout, a CPU clock change, or an enablement of the
generic quiet-publication option. Continuous animation and local updates still
use the existing cadence. Firmware with a different redraw structure may need a
separate presentation policy; interrupted passes intentionally reveal the latest
partial GRAM after the finite fallback rather than hiding it indefinitely.
A single y=0..465 RAMWR window split across multiple SPI transfers also retains
the previous picture until its last pixel. That extension has a directed native
row-transfer test; the official V0.5 browser scenario uses adjacent windows.
This does not claim general atomic presentation for every possible firmware.

The fixed real-browser scenario is compared with the unchanged production
negative control above. The [fix receipt](fix-receipt.json) retains candidate
artifact hashes, actual inputs, captured frame hashes, exact splice counts and
settled-output parity. The first default-release candidate captured 56 frames:
all matched the 2D canvas, with zero unique row splices versus five in the
baseline. Its final full RGBA hash remained
`db93ac735c2d0df17157968f7e9b8b7e1125b295e41444dc94d4a1bb0521a666`.
Ambiguous matches in empty bands are retained separately and are not classified
as moving-icon tearing. Final production-flag validation is recorded separately.

Directed native tests cover complete strips, immediate GRAM readback, local
rectangles, incomplete/skipped/overlapping/reordered windows, a new top pass,
reset and supply reset, split CS/zero-payload RAMWR, sleep/display-off commands,
bounded idle fallback, and publishing the final generation without further
pixel changes. Existing goldens were not regenerated. Full command outcomes and
the fixed-work CPU comparison are retained in the fix receipt. The reusable
[CPU harness](../../../tools/compare-display-cpu.py) alternates baseline/candidate
order and checks instructions, cycles, exceptions, interrupts and console hash;
it is a regression check, not a browser speedup claim.

Native CPU medians on baseline `17a5f68a` and candidate `04bbbad5` (equivalent
implementation on integration commit `202301a4`) were:

| Workload | Baseline user seconds | Candidate user seconds | Change |
| --- | ---: | ---: | ---: |
| S3 hello, 3 alternating pairs | 0.278086 | 0.255779 | −8.02% |
| C3 hello, 3 alternating pairs | 2.538495 | 2.882480 | +13.55% |
| C6 hello, 3 alternating pairs | 1.702320 | 1.681914 | −1.20% |
| Pocket Tank, 3 alternating pairs | 41.450287 | 41.388182 | −0.15% |
| C3 hello, fixed 7-pair confirmation | 2.552972 | 2.560267 | +0.29% |

The C3 first-run increase warranted a separately fixed seven-pair confirmation;
both sets are retained. The confirmation ranges overlap and do not reproduce a
stable 13.55% cost. Baseline and candidate use the same stable toolchain, release
profile and features. These short hello samples are noisy, so neither the S3
decrease nor the small final C3 difference establishes a speed change. Every
sample has identical guest instructions/cycles, exceptions, interrupts and stdout
hash within its workload. The later single-full-window extension changes only
StopWatch's presentation predicate and adds its directed regression test; the
generic CPU measurements were not repeated for that boundary extension.

Final production-flag validation uses the same frozen workbench page for both
artifacts, eliminating the earlier page-source difference. Old WASM
`afd57d99…` captured 67 frames with six unique mixed-frame boundaries at
y=180/240/300. Final WASM `dff72297…` captured 53 frames with **zero unique
mixed-frame boundaries**. Every frame in both runs matches its 2D canvas, and
both settle to the identical full RGBA hash above. Each run retains one
ambiguous empty-band match. Three moving/settled candidate raw frames and the
final canvas screenshot were visually inspected; the moving icons are complete.
The frame count difference is presentation grouping, not a CPU-rate measurement.
Both task-owned headless sessions were closed and verified absent.

Final source `a60ba6f5` is equivalently integrated as `50801fc4`. Rust 1.99.1
passed both required Clippy commands and **1,708 release workspace tests** with
ignored tests included and only `external_` excluded. StopWatch's 15 directed
tests passed. The production WASM retains names and has no DWARF; real WASM
hello/C3/C6/StopWatch smoke runs and the four required platform test files passed.
No golden output was regenerated. Evidence privacy and whitespace checks pass.

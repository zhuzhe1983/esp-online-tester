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

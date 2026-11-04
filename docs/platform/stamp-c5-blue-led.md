# Stamp C5 blue LED source and guest-output check

S016 and S016-DIP use the same populated native PCB. The pinned embedded STEP
keeps designator D1 above the shared 0402 LED solid. D1 resolves to native
component 32; its two top triangles, `[248,250)`, form a 0.43×0.78 mm face.
Only this face binds to board LED index 0. D2/component 19 is the charging lamp
and has no binding. Per-SKU `blue-led-source-audit.json` records the STEP entity
chain, source hashes, transform and schematic reference. No new lamp geometry
or photo window was added. `python tools/audit-stamp-c5-blue-led.py` repeats the
source and mesh checks without network/CAD dependencies.

On snapshot `7ffe2997`, the public workbench loaded the real source-built
`firmware/c5-uart-probe/probe.bin` (560 bytes, SHA-256
`2db5f2a79411f8b529a690e6369bd65a32f270a204ce54806debeba3b3f5f7f3`).
The C5 interpreter executes its UART0 interrupt handler. `1` drives G28 low,
`0` drives G28 high, and `W` drives only G27 low. Both product records passed
ready/off → on → off → wrong-pin/off → on → host reset/off → on → off.
Every command requires a fresh guest echo and the expected observed board LED
before capturing the public 3D canvas. No host API sets a lamp color.

For each SKU, all five off captures are byte-identical and all three on
captures are byte-identical. S016 changes 181 pixels (180 blue-dominant);
S016-DIP changes 180 pixels (179 blue-dominant). Every changed pixel lies within
the projected native D1 face with a two-pixel rounding/antialias allowance.
The C/charging lamp and the rest of the canvas remain unchanged. Each SKU's
`blue-led-runtime.json` retains guest output, input acknowledgements, actual
worker counters, artifact hashes and projection bounds; the two distinct PNGs
are retained instead of five/three duplicate files. The fixed WASM is
5,000,313 bytes, SHA-256
`bd1c688cff2cbd2ac608761dbe122d0c2346f65ea3d0a6e53d3034ee170ba3ef`.

To repeat, serve a built platform and the source-built probe with same-origin
or CORS access. Open the platform URL with `c5ProbeUrl` (URL-encoded) and
`c5ProbeSha` query parameters using the tracked, headless Playwright wrapper.
From the checkout, run:

```sh
playwright_cli.sh -s=<owned-session> run-code --filename tools/check-stamp-c5-led-browser.js
python tools/audit-stamp-c5-led-captures.py --receipt <result-json> --capture-dir output/playwright --output <pixel-audit.json>
playwright_cli.sh -s=<owned-session> close
```

The JSON under the CLI's `Result` heading is the second command's receipt.
Use the pinned model Python environment for Pillow. Close and verify removal
of only the task's session. The harness awaits the asynchronous public API and
polls/asserts the actual expected LED state; a serial echo alone can arrive
before the corresponding LED publication. This check does not establish
hardware photometry, clock accuracy, RF operation, stock ROM boot or UIFlow
compatibility. The protocol fixture is not a factory firmware substitute.

The later runtime-note edits change only each model's `limitations`. Complete
model objects, excluding only that key, compare identically. Both plans also
refresh importer runtime defaults to the exact board, 4 MiB Flash, 0 PSRAM and
partial status; the remaining complete plan objects are identical after
excluding those two catalog fields and limitations. These are runtime-profile
defaults, not new physical hardware measurements. Per-SKU
`runtime-note-carry-forward.json` records all before/after hashes. Original
tested hashes remain recorded with explicit applicability to the current
models. No new browser capture or rebuild is claimed for these metadata edits.

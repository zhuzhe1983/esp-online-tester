# Spark loading and T-Embed model regression checks — 2026-10-28

Code baseline: `0834e669` in this repository. Platform suite: 189 passed, 0 failed with Node 22.23.4; also passed on Node 24.22.1. Catalog schema validation passed (213 products, 10 accessories, 165 firmwares, 111 watchlist entries). A fresh WASM build was used. The first isolated full run lacked the ignored T-Display MicroPython test input; supplying the existing official image allowed that test and the complete suite to pass. No firmware bodies were added to Git.

Commands, from the repository root:

```sh
tools/wasm-build.sh
node --test tools/platform-*-test.mjs
python tools/validate-platform-catalog.py
node tools/check-evidence-privacy.mjs
python tools/build-lilygo-models.py lilygo-t-embed-cc1101 --output /tmp/lilygo-model-review
python tools/serve-platform.py --port 8926 --bind 127.0.0.1
```

The browser was headless Chromium. UI steps: select T-Embed CC1101 → remote address → LILYGO Spark → Bruce v1.10.3; import the original ZIP, verify the payload, confirm trial execution, and wait for nonzero guest instructions. Pause, browse `cc1101 send irq`, copy iframe code and verify the link still carries Bruce's actual loaded URL/hash/archive. Open the iframe URL in the same browser and verify cached loading uses ROM boot. This cache reuse does not establish that the OSS server permits CORS in a fresh browser. The separate example check selects `cc1101 send irq`, downloads its pinned historical file, confirms trial execution and waits for guest instructions. Both checks establish loading/execution only; neither establishes peripheral completeness or a successful application UI. Finally, the original source-less embed scene stays configured and the API generates `autostart=0&autoplay=0`.

`archive-share-checks.json` and `recovered-example-and-preview.json` retain source hashes, byte counts, boot modes and sampled instruction evidence. The loopback ROM origin in the latter is replaced by `<platform-origin>`; the path, ROM hash and byte count are preserved. Browser session IDs, personal paths, account details and raw console captures are omitted. The ZIP request returned HTTP 200, 2,449,070 bytes and `application/zip`, with no `Access-Control-Allow-Origin` header for the Pages origin. Its extracted payload hash is recorded in the archive receipt.

## Model comparison

All before/after images use the same official CAD, lighting and camera; only the PCB placeholder shape changes. Orthographic camera: ±62.5 mm horizontally, ±37.5 mm vertically, near/far 1/1000 mm, 1000 × 600 pixels; look-at origin. Front/back camera positions are (0,0,±250), side (250,0,0), oblique (180,-130,80). Side camera up is +Z, others +Y. Hemisphere light: white / #536159, intensity 2. Directional light: white, intensity 2, position (-30,60,100). Gold and translucent-purple front checks use the existing presentation presets. Front/back were inspected alongside the retained side/oblique comparisons. The small UNKNOWN marker remains on the internal placeholder.

`pcb-clearance.json` measures radial rays in the PCB plane (z=3 mm) against the official shell meshes with double-sided intersection. Four old rectangle corners fall outside the outer shell by up to 1.1813 mm. The capsule has no outside samples among 227 intersecting rays; 9 rays pass through shell openings and are excluded rather than treated as clearance measurements. The nearest sampled exterior gap is 0.84195 mm. This is a sampled shell-envelope check, not a fitted PCB assembly or minimum internal wall distance. The automated geometry test separately checks every vertex against the 98 × 39 mm rounded outer envelope and retains the 1.2 mm thickness.

The dedicated LILYGO generator reproduced model metadata and binary assets. `geometry.bin` and `unknown-label.png` remained byte-identical. `asset-hashes.json` pins the reviewed metadata, plan and binary assets. The real PCB outline and mounting transform remain unknown; no components or traces were invented.

Privacy: screenshots contain only the product model on a plain background. Text/JSON were manually reviewed; the repository privacy checker also passed for its configured tracked-evidence scope. The firmware endpoint responses were summarized without request IDs or server/session headers.

# Device tools internationalization

Baseline: `9f1ff87b365c86382032d79138935a4a3c797024`. Checks ran on 2026-11-14 with Node.js 26.13.0 and headless Chromium 157.

The change adds 179 English dictionary entries and four dynamic templates. It covers TF card preparation/import/creation/conflicts, shell finishes and stickers, CoreMatrix hardware states, virtual configuration errors, audio toggle states, and 67 previously untranslated catalog control/connector labels. The existing language priority and scene-sharing format remain unchanged.

Raw log/editor contents remain untranslated while their accessible labels now follow the UI language. Sticker filenames, custom configuration names, firmware-history names/versions, error-context product names and network SSIDs are preserved even when they equal UI words such as `启动` and `暂停`. Built-in configuration names and the workbench's initial status message are UI text and are translated explicitly.

## Validation

The following command passed all 100 tests, including the actual worker/WASM runtime checks:

```sh
node --test tools/platform-i18n-test.mjs tools/platform-config-test.mjs tools/platform-runtime-test.mjs tools/platform-input-test.mjs tools/platform-view-test.mjs tools/platform-firmware-history-test.mjs tools/platform-flash-configuration-test.mjs tools/platform-virtual-firmware-configuration-test.mjs tools/platform-network-test.mjs tools/platform-guest-network-test.mjs tools/platform-audio-browser-test.mjs tools/platform-hardware-diagnostics-test.mjs tools/platform-sd-card-test.mjs tools/platform-device-storage-test.mjs tools/platform-sticker-test.mjs
```

An initial runtime check could not find its ROM fixture. Supplying the public ROM/demo assets and the post-upstream baseline WASM resolved that prerequisite; the complete rerun passed. No emulator source or golden output changed. The WASM hash and final test log digest are in [result.json](result.json).

`node tools/audit-platform-i18n.mjs <acorn-8.19.0>/dist/acorn.mjs` scanned 109 platform modules: zero untranslated literals and zero untranslated template samples. HTML templates are now checked as individual text/attribute fragments, including hidden controls; this caught the sticker Delete button that a whole-markup scan obscured. Eleven composed strings remain translated through known phrases. See [source-audit.json](source-audit.json).

Both browser fixtures passed against the static site:

- `tools/fixtures/i18n-browser-check.js`: existing workbench, mini, agent guide, flash configuration/progress, original CAPTCHA target characters, opaque logs, and English/Chinese round trips.
- `tools/fixtures/i18n-device-tools-check.js`: prepared-card and conflict UI, replacement confirmation scope, shell/sticker labels and local upload, fullscreen controls, built-in/custom configuration choices, history identity, network SSIDs, audio errors, hardware diagnostics, raw-content accessible labels, and shared-scene language round trips in the mini player. A real 390 px iframe also passed.

Both returned zero page errors. Hardware, audio-error and conflict states in the UI fixture are explicitly synthetic and do not establish peripheral execution. No physical device was flashed, microphone access requested, CAPTCHA submitted, TF card replaced or virtual Flash configuration written. The synthetic sticker was uploaded locally, then its newly created entry was removed through the UI.

Desktop 1440×1100, mobile 390×844 and iframe screenshots were inspected. TF format selects now stack at narrow widths, and English shell presets/finishes use the panel width so their selected labels are readable. The mobile language selector keeps enough width to show English in full. Long dialogs/panels scroll vertically. Curated examples: [TF confirmation on mobile](i18n-storage-en-mobile.png), [shell editor on desktop](i18n-shell-en.png), [shell editor on mobile](i18n-shell-en-mobile.png), [storage inside an iframe](i18n-iframe-en.png).

The task-owned Playwright session was closed and verified absent; the local static server was stopped.

## Reproduction and boundaries

Serve `web/`, open `/platform/?lang=en` through the repository's Playwright wrapper in a fresh headless session, and run each fixture with `run-code`. The fixtures use synthetic local data; use an isolated browser context. Close that session after checking screenshots. Runtime tests additionally need the documented WASM, public demo files and ROMs.

The source audit excludes diagnostic model-review pages, vendored libraries and free-form catalog/knowledge prose. This change translates public control/connector labels without altering catalog values, protocol IDs, official product names or guest firmware text. Unknown product prose remains original. Legacy simulator pages and technical documents retain their existing language boundary. Arbitrary dynamic runtime combinations are not exhaustively enumerated.

Only curated screenshots of public UI and synthetic data, commands, counts and hashes are retained. Raw local server/browser logs and local filesystem/session identifiers are omitted; this does not change the measured outcomes. The [result receipt](result.json) records fixture and screenshot hashes.

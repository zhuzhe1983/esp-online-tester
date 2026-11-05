# Workbench internationalization audit

Baseline: `d099b6d7dbe69b38b9eb482fb6630430e61e5f41`. Source audit and browser checks were run on 2026-11-05 UTC (2026-11-06 Asia/Shanghai). This change adds 345 dictionary entries and 50 declared templates. Product identity, firmware text, diagnostic output, API values and verification target characters remain original data.

## Checks

- `node --test tools/platform-*-test.mjs`: 345 passed, zero failed.
- Catalog schema: 217 products, 10 accessories, 173 firmware entries and 117 watchlist entries passed.
- Full local CI passed with stable Rust 1.99.1, both warnings-denied Clippy commands, release tests including ignored tests except external inputs, native/WASM/JIT checks and evidence checks. See [ci-result.json](ci-result.json) for commands and the original log digest. Final UI changes were checked again by the platform suite and browser fixture. No simulator source or golden output changed.
- `node tools/audit-platform-i18n.mjs <acorn-8.19.0>/dist/acorn.mjs`: 96 modules scanned; no untranslated template samples. Three untranslated literals remain in the unused `readImagePoints` helper. They are not connected to the manual verification UI. Nine composed literals are whitespace, suffixes or HTML containing translated UI phrases. See [source-audit.json](source-audit.json).
- Headless Chromium, `tools/fixtures/i18n-browser-check.js`: workbench, embedded player and agent guide, English/Chinese round trips, blocked local storage (unit check), shared firmware groups, P4 disabled reason, flashing configuration and progress, dynamic accessibility attributes, inherited skip boundaries and original diagnostic/verification text. See [browser-result.json](browser-result.json).
- Screenshots cover desktop 1440×1100 and mobile 390×844. English controls and guide fit both widths. The flashing progress is synthetic UI data; no serial port was requested, no hardware was written and no verification answer was submitted.

One browser attempt failed while loading `hints.js` with `ERR_CONNECTION_RESET` from the local development server. The application reported the loading error. A complete rerun passed without page errors. This was a transport failure, not counted as a passing run.

## Reproduction and limits

Build WASM and fetch ROMs as documented in the platform README, serve `web/`, and open `/platform/?lang=en` with the repository's Playwright wrapper. Run the fixture through `run-code` and close the task-owned browser afterward. The fixture uses stubbed flash state and a blank verification image to inspect labels; it does not test physical flashing or CAPTCHA success. Firmware execution is covered separately by the full CI checks.

The source scan excludes diagnostic model-review pages, vendored libraries and catalog prose. Arbitrary runtime combinations are not exhaustively enumerated. Unknown product prose remains unchanged. P4 and S31 flashing stays disabled; the [support investigation](../../flash-chip-support.md) records the integration gaps.

Only curated results, public-source hashes and synthetic screenshots are retained. Raw logs with local paths and browser session details are excluded. The original CI log digest identifies the private local log without publishing it.

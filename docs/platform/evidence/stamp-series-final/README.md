# Final Stamp series acceptance

`checks.json` retains local CI-equivalent results, preserved failures and exact production WASM identity. `independent-review.json` records the separate read-only source/model audit and public browser gate at its pinned snapshot. Product runtime execution remains in the per-product evidence, including the final StampFly and C5 receipts linked from the [inventory](../../stamp-series-onboarding.md).

The unchanged `browser-final-smoke.js` is the exact passing Playwright `run-code` function. Open the caller-supplied workbench URL headlessly through `~/.codex/skills/playwright/scripts/playwright_cli.sh`, create `/tmp/stamp-final-review/screens`, and pass the file contents to `run-code`. It uses that already-open page and pins all 22 model hashes to the recorded snapshot. Inspect the returned JSON: `errors` and `console` must be empty, every model hash must match, every declared inspection must change pixels then reset exactly, and every mode-switch result must match the reported contract. A CLI exit status of zero alone is not acceptance.

The harness removes its listeners in `finally`; the caller must close its task-owned wrapper session and verify it has exited. Mini-page restoration and K141 transparent full-assembly readability were separate public interactions, not assertions in this harness. Their procedure and scope remain in the review and [K141 receipt](../m5stack-k141/public-unknown-review.json).

No private firmware bytes, browser profiles, process inventories or raw private logs are retained here. Raw logs are represented by content hashes and relevant command/result facts.

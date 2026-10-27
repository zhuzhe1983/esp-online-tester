# Firmware selection and run UI checks — 2026-10-27

Scope: human run controls, issue drafts, accessory selection, and metadata-only firmware catalog updates. These checks do not establish firmware compatibility.

- Main workbench: unverified PaperMono UIFlow2 opens confirmation only after Run. Cancel produces no firmware request. Continue reaches the normal loader; the test aborts the BIN request before any payload arrives. Missing-board NanoH2 hides both run controls and shows the red `尚未完成适配` label.
- Mini player: repeats confirmation/cancel/continue through a shared PaperMono scene. Exactly one request is intercepted after confirmation; zero firmware bytes downloaded or executed. Missing-board shared scenes also hide Run and show the label.
- Issue draft: `window.open` is intercepted. The generated GitHub draft contains C153, UIFlow2 v2.5.3 and FETCH_FAILED. No issue was submitted. Unit checks additionally cover sensitive text redaction and encoded URL bounds for long logs/names/errors.
- Accessory pickers select the first real brand, its first interface and first matching model. No “all” or empty option remains. Searching U003 selects Unit RGB and keeps the matching brand/interface.
- Confirmation screenshots checked at 1280×720 and 390×844. No clipped buttons or horizontal overflow observed.
- Firmware metadata sync checks 44 watch entries and 45 explicit firmware targets; the current snapshot yields zero upgrades. Five general project references have no exact device target. Only publisher release metadata was requested; no catalog BIN download or execution.

Reproduction: run `node --test tools/platform-*-test.mjs`, `python tools/validate-platform-catalog.py`, and `node tools/sync-firmware-catalog.mjs --snapshot-only --dry-run`. For browser checks use the repository Playwright wrapper, intercept remote BIN requests, and capture issue draft URLs instead of opening/submitting them. Run controls must not request firmware before consent. Preserve the distinction between a request reaching the loader and successful firmware execution.

# U004 on current main

This receipt migrates the earlier U004 stage onto `5a9b68e0`, retaining all 28 M5Stack controller identities and the current StopWatch internal I²C/RTC routes. The former `4670491d` receipts in the parent directory remain historical. This is not a merge of that old baseline or an adaptation of another PIR SKU.

- `checks.json`: exact production WASM/source hashes and complete CI command results. No golden updates.
- `probe.json`: actual source-built IDF firmware via the page worker. Eleven groups cover G11/G10, nominal 2-second hold/retrigger, PMIC supply, unplug/pulls, guest/platform reset, recording and U006 parent/child state.
- `browser.json`: actual UART0 transitions after human panel input in Studio and mini, mini share autostart, plug views, materials and UNKNOWN PCB.
- `source-recheck.json`: fresh GET of official product/schematic; bytes unchanged. Original media/URL/fetch records remain in `../../source-audit.json`.
- `rebuild-current.json`: public `rebuild-unit-library.py` scratch rebuild reproduces all 11 generated files exactly. `model-plan.json` is the input, not a generated output. Historical dimension and photo-contour metrics are reused without changing their timestamps.
- `cpu.json`: current-baseline idle CPU comparison, once measured. No speed or no-regression claim is inferred from functional tests.

The binary fixture is retained only in ignored local output. Its source and all three artifact hashes equal the old source-built fixture receipt. The public catalog contains no diagnostic image. Host/lane identities, browser profiles and raw local logs are not included. Screenshots contain only public product data and internal fixture UART output.

The first material script passed the inner shell object rather than `{shell: ...}` and was rejected before applying it; corrected calls passed. Passing null to the review helper uses the StopWatch black default, so the standalone back screenshot is explicitly a black-shell isolation check. Original white geometry/color is visible in the connector and Studio rear captures; none of these are newly measured photo-overlay scores. A test initially expected a G11 conflict for not-yet-migrated ENV/DLight IDs; it now checks `UNKNOWN_ACCESSORY` when absent and `PIN_CONFLICT` after their own catalog entries are integrated. No product runtime relaxation was made.

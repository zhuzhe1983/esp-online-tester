# Contribute a product, accessory or firmware

Open a pull request against this fork. Public records live in `web/platform/catalog/`; schemas live in `web/platform/schemas/`. Private DIY accessories can be created in the workbench, stored locally and exported without entering the public catalog.

## Products

Add a stable product ID, manufacturer, model/revision, chip, memory, display, controls, connectors, pin mapping, source links and an explicit support level. Do not infer support from the chip family: booting an ESP32-S3 image does not implement its display, power controller or attached devices. A product without a board model must remain unavailable for execution. Simplified preview geometry must not be advertised as a dimensionally accurate CAD model.

Implement missing emulated hardware separately under the upstream Rust conventions in `CONTRIBUTING.md` and `AGENTS.md`. State which behavior came from documentation, tests or physical hardware. Include firmware and an observable result when changing support claims.

## Accessories

Give each revision its own ID, connector signals, voltage requirements, protocol, parameters and provenance. State whether an accessory is metadata-only, logically injected, or modeled on an actual bus. The first executable template is NMEA input on logical UART1; this does not validate GPIO routing, baud rate, voltage or RF reception. Arbitrary uploaded JavaScript or native plugins are not accepted by the browser designer.

## Firmware

Include compatible product IDs, a version/revision, license/source links and limitations. A catalog reference to a project is distinct from a verified executable artifact. Prefer a manifest with pinned SHA-256 hashes and explicit bootloader, partition table and app files. Files must be redistributable and accessible to browsers through HTTPS and CORS. Keep large release binaries out of catalog JSON.

Do not publish credentials, device identifiers or personalized flash dumps. A full flash image may contain these even when the app source does not.

## Validate

```sh
node --test tools/platform-config-test.mjs tools/platform-runtime-test.mjs tools/platform-input-test.mjs tools/platform-view-test.mjs
node tools/check-evidence-privacy.mjs
```

For executable firmware, also boot it through the real WASM worker, exercise its controls and inspect serial/framebuffer output. Describe tested versions, commands, results and remaining gaps in the PR. Follow the broader Rust checks in `CONTRIBUTING.md` for core changes.

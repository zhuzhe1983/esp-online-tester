---
name: esp-online-tester
description: Configure, operate, and test ESP product simulations in ESP Online Tester, including board and firmware selection, accessory wiring, input replay, and compatibility reports. Use for this project's simulation scenarios and catalogs, not generic ESP flashing.
---

# ESP Online Tester

## Draft status

This is a project skill draft. The website, `window.espTester`, and the API in [references/api-contract.md](references/api-contract.md) are proposed, not implemented as of 2026-10-26. Do not announce a successful simulation or invent commands when only these documents exist. In this state, prepare a scene and validation plan, identify missing artifacts/capabilities, and report that execution is unavailable. This draft is stored with the project and is not installed globally.

When a working deployment becomes available, discover its actual version and capabilities first. This skill's contract does not override a deployed API. Ask for a deployment URL only if it cannot be found in the current project or task context.

## Configure a scenario

Read [the API contract](references/api-contract.md) for programmatic operations and report semantics. Use the product revision, accessory revision, firmware target and connection topology requested by the user. Keep firmware SHA-256, engine revision and model versions with the scene.

Distinguish catalog presence, partial models, verified firmware boot, verified interaction and hardware comparison. ESP32-S3 support alone does not certify a PaperMono or StopWatch image. A display asset or synthetic preview is not evidence of firmware execution.

For this project's initial hardware:

- PaperMono is C153, ESP32-S3, SSD1677/FT6336G; it is neither original Paper nor PaperS3.
- StopWatch is C152, ESP32-S3, CO5300/CST820B. GPIO10/11 are its Grove signals; validate UART direction, selected controller and external power against the scene/firmware before use.
- GPS U032 defaults to 9600, U032-V11 to 115200. Preserve the variant as unresolved until identified; do not silently substitute one. No PaperMono GPS wiring is approved by this draft.

Resolve a complete flash layout. An app-only/OTA binary is insufficient without its matching bootloader, partition map and required data. If a remote download fails CORS, use an authorized local artifact or explain the source-host requirement; do not disable browser security or treat an opaque fetch as valid bytes.

## Operate and test

Prefer semantic control IDs through the stable automation API, such as `button-a` or `screen`, over screen coordinates. Exercise the visible UI separately when verifying 3D picking or key mapping. Map inputs to actual emulated GPIO, touch, matrix keyboard or UART device behavior; never write a desired framebuffer as a substitute for exercising firmware.

Validate capabilities before starting. Missing UART routing, SPI/I²C models, storage, RTC or reset behavior is an unsupported capability, not a passing test. If a test uses stubs, record which ones and their scope.

Use bounded simulation-time events, with a separate wall-clock timeout. For GPS, deliver protocol-correct UART data to the configured peripheral controller; cover loss of fix, disconnect and recovery. Freeze supported clocks/random sources and record any uncontrolled sources. Verify observed firmware output rather than merely confirming that input injection returned successfully.

Collect the resolved scene, UART output, relevant bus events, raw framebuffer, assertions and limitations. Report simulation pass separately from real-hardware equivalence. Do not claim cycle, radio, power or analog accuracy without supporting measurements.

Run browser automation headlessly unless interaction is required. Follow repository browser-session ownership rules; close task-created browser/context resources in `finally`. On failure, retain logs and a reproducible scene, then clean up the task's emulator sessions.

## Catalog and personal accessories

Personal DIY definitions remain local or exported until the user requests contribution. Use existing model templates and schema-validated data; a metadata entry cannot create an unimplemented chip model. For PR preparation, provide sources, revision-specific pin maps, asset licenses, model capabilities and test evidence. Submitting/publishing follows the user's requested scope, not an implied permission from creating a local definition.

## Completion report

State the board/revision, firmware hash, backend/version, tested operations, observed assertions and remaining limitations. Link the evidence and share configuration if one actually exists. If execution was unavailable, distinguish a prepared draft from an executed result and identify the exact missing capability or artifact.

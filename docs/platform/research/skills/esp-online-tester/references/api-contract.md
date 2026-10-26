# ESP Online Tester API contract draft

Status: proposed v0.1, not implemented. The names below belong to this project and are not existing esp-emulator or esp32sim APIs.

## Entry points

A deployed static site should publish `api/capabilities.json`, `schemas/scene-v1.json`, versioned catalog indexes and `skills/esp-online-tester/SKILL.md`. These are static files, not HTTP mutation endpoints. `capabilities.json` describes the shipped build; a live `capabilities()` call also checks the active browser/backend.

Expose `window.espTester` as an asynchronous facade over a versioned Worker RPC protocol. Browser automation invokes it in the page execution context. An optional local CLI/MCP wrapper may launch a headless browser, but no remote service is required to host the website. The same commands power the UI.

Every result uses one of:

```ts
type Result<T> =
  | { ok: true; value: T }
  | { ok: false; error: { code: string; message: string; details?: unknown } };
```

Methods return `Promise<Result<T>>`; callers must inspect `ok`. Worker requests include `protocolVersion`, `requestId`, `sessionId`, `method`, and `params`; responses preserve the request ID. Events include an increasing sequence and simulation timestamp. Calls made after disposal fail, not target another session.

## Proposed methods

| Method | Meaning |
| --- | --- |
| `capabilities()` | API/build versions, engines and per-engine operations, browser restrictions, model versions |
| `catalog.list({kind, query})` | Search products, accessories or firmwares; no runtime GitHub token |
| `catalog.get({kind, id, revision})` | Resolve an exact record |
| `validateScene(scene)` | Return errors, warnings, missing capabilities and unresolved artifacts |
| `createSession({scene})` | Resolve hashes and models, load bytes; return session ID and immutable resolved configuration |
| `start({sessionId})` / `pause({sessionId})` | Start or pause execution with observable state |
| `reset({sessionId, kind})` | Distinguish soft restart, hard reset and fresh flash; only advertise supported variants |
| `input({sessionId, control, action, value, atUs})` | Logical button/touch/keyboard events, with defined press and release semantics |
| `accessory.set({sessionId, instanceId, parameters, atUs})` | Configure a supported model, such as GPS fix or track |
| `advance({sessionId, deltaUs, wallTimeoutMs})` | Controlled simulation-time progress, only when the backend supports it |
| `observe({sessionId, topics, sinceSeq, limit})` | Bounded UART/GPIO/bus/display/state event batches |
| `capture({sessionId, kind})` | Raw framebuffer or other supported artifact; 3D UI screenshot is a separate kind |
| `runScenario({sessionId, scenario, wallTimeoutMs})` | Execute bounded actions and evaluate observable assertions |
| `exportReport({sessionId, format})` | Structured JSON or packaged evidence |
| `share({sessionId, includeLocalDefinitions})` | Configuration link and unresolved local resources; never implicit firmware/secret upload |
| `dispose({sessionId})` | Stop and release the session's Worker and resources |

Suggested errors: `API_VERSION_MISMATCH`, `DRAFT_SCENE`, `UNRESOLVED_ARTIFACT`, `HASH_MISMATCH`, `REMOTE_FETCH_BLOCKED`, `INCOMPLETE_FLASH_LAYOUT`, `CHIP_MISMATCH`, `PIN_CONFLICT`, `VOLTAGE_MISMATCH`, `MODEL_UNAVAILABLE`, `ENGINE_CAPABILITY_MISSING`, `ASSERTION_FAILED`, `SIM_TIMEOUT`, `WALL_TIMEOUT`, `SESSION_DISPOSED`.

Capabilities are explicit, e.g. `uart.rx.by-controller`, `uart.tx.by-controller`, `gpio.input`, `gpio.events.ordered`, `spi.transactions`, `i2c.devices`, `display.framebuffer`, `clock.controlled`, `rng.seeded`. The capability names are proposed. A concrete adapter maps them to verified engine APIs or returns unsupported.

Do not advertise arbitrary UART input from esp-emulator 0.51.0's `uart_input(data)`: the public function has no controller parameter. Native CLI capabilities are a separate adapter. Do not advertise deterministic replay solely because an engine exposes a cycle counter.

## Scene requirements

`schemaVersion` is a wire-format version, separate from product revision, catalog revision, engine revision and model version. A runnable scene requires exact resolved versions and artifact hashes. A draft can have nulls, but validation must reject it for execution.

The scene contains:

- A primary device instance and board parameters; optional further active devices are independent simulated machines.
- Accessory instances referring to fixed catalog models or inline declarative personal definitions.
- Explicit wiring edges between endpoint contacts, plus controller/pin-route configuration and power-domain dependencies.
- Firmware artifact or manifest; segment offsets; optional ROM, eFuse and storage images.
- Input mapping, environment clock/random seed, offline network policy and initial state policy.
- Engine/build pin and requested capabilities.

Share encoding is canonical JSON plus optional bounded compression and base64url in a fragment. For large configurations, use an HTTPS manifest URL plus SHA-256. A share link reboots the configuration; it is not a snapshot. Private local artifacts and definitions must be listed as missing unless explicitly embedded/exported by the user.

## Execution sketch after implementation

This pseudocode illustrates the contract. It must not be run against today's documentation-only project.

```js
const api = window.espTester;
if (!api) throw new Error('ESP Online Tester runtime is unavailable');
const must = result => {
  if (!result.ok) throw new Error(JSON.stringify(result.error));
  return result.value;
};
const caps = must(await api.capabilities());
// Check API version, exact board/model support and required capabilities here.
const validation = must(await api.validateScene(scene));
if (validation.errors.length) throw new Error(JSON.stringify(validation.errors));
const session = must(await api.createSession({scene}));
try {
  must(await api.start({sessionId: session.id}));
  const report = must(await api.runScenario({
    sessionId: session.id, scenario, wallTimeoutMs: 60000
  }));
  // Inspect assertion outcomes and evidence, not just successful RPC delivery.
} finally {
  must(await api.dispose({sessionId: session.id}));
}
```

Scenario actions use simulation timestamps; `runScenario` owns scheduling to avoid races with live input. Tests have finite duration and assertion deadlines. Clock seeding/time control must be capabilities checked at runtime, not assumptions in this sketch.

## Report semantics

Reports contain `status`, `assertions`, `artifacts`, `resolvedScene`, `engine`, `modelVersions`, `firmwareHashes`, `simDurationUs`, `wallDurationMs`, `seed`, and `limitations`.

Status is `passed`, `failed`, `unsupported`, or `inconclusive`. A mock preview, missing model, unresolved firmware, absent API or unloaded runtime cannot be `passed`. Inconclusive describes insufficient observable evidence, not a way to hide an assertion failure. Hardware comparison is a separate evidence record identifying the physical board and procedure.

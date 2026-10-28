# Agent workflows and integration boundaries

Read this when running unattended firmware tests, diagnosing unsupported IO, or planning audio/network integration. The API reference lists callable methods; proposed features below are not available commands.

## Choose an entry point

| Environment | Current route |
| --- | --- |
| Browser agent | Run the platform and call `window.espTester` in that page through an available browser automation tool. |
| Embedded website | Use the origin/session-bound `postMessage` handshake and allowlist in [the API reference](api.md#embedded-parent-control). |
| Local Node test | Repository tests construct the same runtime with a Worker adapter. `tools/probe-official-stopwatch.mjs` is a StopWatch-specific real-WASM probe, not a general network service. |
| MCP client | This repository does not provide a dedicated MCP server. A client needs a browser tool or an adapter to the runtime; do not assume tool names or localhost endpoints. |

The public skill is served at `https://zhuzhe1983.github.io/esp-online-tester/platform/skill/SKILL.md`. Relative links within this skill work on Pages and in a checkout. Repository source documents use explicit GitHub links.

## Run a reproducible test

1. Await `window.espTester`, then read `version`, `capabilities()` and `observe()`. Preserve an existing scene and selected firmware unless the task requests replacing them. Query capabilities again after a product change.
2. For a fresh scene, import `catalog.js` relative to the platform URL, call `loadCatalog()` and `createDefaultScene(catalog, productId)`, then resolve the product's `defaultFirmwareId`. The helper itself leaves firmware null. Validate before configuring; retain warnings and trial status.
3. Load the requested artifact. `ready` means loaded, not executed. Register a firmware-specific `waitFor` before `start()` or an input when fresh output is required. Handle the wait rejection as well as the action failure; never leave an unhandled pending wait.
4. Inspect real serial, frame, LED or radio output after the action. `input()` receipts mean queued; `setGpsFix()` changes parameters without UART delivery; `setImu()` and accessory input state do not prove guest consumption. `press()` durations use host time, which is not a reliable guest-duration hold under load.
5. Retain source URL/hash, board, actual backend, assertions, relevant serial excerpts and framebuffer. Report missing capabilities and unsupported behavior. Do not infer custom-firmware compatibility from a different official build.
6. Stop/pause/dispose only the test's session. Close task-owned browsers. Preserve writable Flash/SD and downloaded firmware cache unless the task explicitly calls for clearing the relevant data.

| Symptom | Next action |
| --- | --- |
| `WAIT_TIMEOUT` | Inspect `error.details`, guest time, status and per-port serial; check for a stalled guest versus a wrong assertion. Do not indefinitely extend timeouts. |
| `NO_FRAME` | Check whether this product has a display and firmware has submitted pixels. LED-only boards have no framebuffer. |
| Download/CORS/hash error | Report the actual source failure; use a user-authorized reachable artifact, never silently substitute a demo. |
| Load cancellation | Use `cancelPendingLoad()` on the window API; handle `FETCH_ABORTED` on the original promise. During recording preparation use `recordingTakeOver()`. |
| `RECORDING_STORAGE_NOT_PRISTINE` | Preserve private data. Use a separate clean browser/firmware scope for portable tests. |
| `RECORDING_NOT_SHAREABLE` | Restore the matching scene or explicitly call `share({includeRecording:false})`. |
| Unsupported input/radio/IMU | Inspect product capabilities and actual model; do not fall back to fabricated output. |

## Network: implemented scope

Virtual Wi-Fi association, an assigned IP and real Internet access are separate outcomes.

- Native CLI: virtual STA/AP connectivity plus host TCP/UDP NAT is available through `--wifi "ssid=esp32sim,psk=esp32sim-pass" --net nat`, subject to chip/firmware support. Firmware credentials must match this virtual network; a real home Wi-Fi password is unnecessary. See the [networking guide](https://github.com/zhuzhe1983/esp-online-tester/blob/main/docs/networking-howto.md).
- Product browser runtime: a firmware manifest's `wifi` string configures the virtual AP. The WASM Wi-Fi path has no native NAT. Internal DNS/SNTP/ICMP responses are simulated and cannot prove external connectivity. Downloading firmware with browser `fetch` does not connect the guest to the Internet.
- The WASM Ethernet relay ABI exists, but the product worker has not connected it to an external gateway. Scene-level network configuration, network state/probe methods and a SoftAP provisioning bridge are not exposed by `window.espTester` or iframe RPC.
- Firmware AP SETUP/SSID output does not create a real computer hotspot. Do not change the host's Wi-Fi connection or browse the host's `192.168.4.1` as a substitute for reaching the simulated device. A virtual client and inbound TCP/HTTP path are still missing.

For a network-dependent test today, report this browser limitation; use native CLI NAT when suitable for the requested test. Do not claim cloud success based on a local fixture, a synthetic DNS/ping answer or a UI connectivity icon.

## SoftAP and automatic setup: required future workflow

Both modes must use the same guest network implementation. See the [implementation plan](https://github.com/zhuzhe1983/esp-online-tester/blob/main/docs/platform/wifi-platform-plan.md); the following is a specification, not a callable skill workflow today.

- **Provisioning test:** discover the guest's SoftAP, connect a virtual client, obtain its IP and route requests to the guest's actual provisioning service. For HTML firmware, expose an isolated browser URL for the original page and let the agent inspect/fill it. ESP-IDF protocomm requires a compatible protocol client and the firmware's supported security/PoP flow; not every SoftAP serves an HTML form. Supply only the virtual router's test credentials. Observe the guest save/restart, STA association, DHCP and a response from the intended external test service.
- **Automatic setup:** create a default virtual router, reuse existing saved configuration, and auto-provision supported firmware through that same service on first boot. Match documented fixed credentials for firmware that already tries a known SSID. Unknown provisioning protocols must report an actionable unsupported state or expose their real page. Firmware waiting forever in AP mode cannot be made application-ready by a fake Wi-Fi success flag.
- Do not silently modify NVS, bypass PoP, return canned cloud responses, or overwrite existing user configuration. A future firmware-specific seeded test image must be explicit, isolated and separately verified; it does not validate provisioning.
- Persist successful guest configuration with the existing product/firmware Flash scope. A shared URL does not carry private Flash, so a recipient may need automatic first-run provisioning again. Provisioned storage may also fail the current pristine-storage requirement for portable recording; do not clear it to force recording.
- Report provisioning, association, DHCP and external-response evidence separately. Relay setup is independent of firmware provisioning; automatic setup still needs a usable browser gateway to reach real services.

The requested Web UI defaults to automatic setup. Disabling it preserves the network transport and existing credentials, and exposes an iframe for the guest's real SoftAP page when available; it does not force the guest back into provisioning. A separate offline control disconnects transport. The bottom status area should list known guest services with their discovery/verification source. HTTP/HTTPS entries open through an isolated browser bridge; arbitrary TCP/UDP services require explicit localhost port mappings through the network helper. A conventional port number is only a protocol hint, and passive traffic cannot reveal every listening service. Manual service entries remain necessary. None of these controls or service-discovery/mapping APIs is implemented yet.

## Audio: implemented scope and next interface

Rust already has PCM input sources and I2S TX sample capture. The existing WASM binary output type `2` carries a little-endian sample rate followed by mono PCM16 samples. The older `web/run.html` player can play this output; the product platform's runtime currently only consumes framebuffer binary output and does not play audio. Its generic binary input queue has a camera decoder, not a public microphone PCM decoder.

StopWatch ES8311 control-register initialization now passes in official Xiaozhi v2.2.6. That result does not verify microphone input, speaker playback, gain/mute, power gating or complete codec behavior. There are no platform audio stream methods to invoke today.

The next shared interface should carry PCM format/sample rate/channels, sequence or guest timestamp, bounded queues and consumed/underflow/overflow counters. Browser microphone capture and speaker playback, file-based test input/output and an optional Node/MCP adapter should all use that contract. Browser device access remains a user-activated permission flow. Requested microphone and speaker controls belong at the upper-right of the 3D view, with independent active/muted/error state and track cleanup on session teardown.

Use MCP calls for stream setup/teardown, file submission and observations; continuous PCM should use a suitable binary transport rather than one tool call per audio block. Preserve original speech/music speed and pitch. If the simulator cannot keep up, report the condition or use offline capture; do not silently stretch, accelerate or truncate voiced content.

Priority gaps after the documented API corrections are: shared browser network state and relay; SoftAP client/provisioning with automatic setup; PCM input/output; explicit guest-duration input helpers. Add their callable examples only after implementation and real guest-output verification.

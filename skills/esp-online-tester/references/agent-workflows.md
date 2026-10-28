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

## Network and guest services

The default mode is `public-relay`. SoftAP DHCP/TCP/HTTP runs locally in the browser without a helper. Internet uses the site-configured public relay; an empty `web/platform/network-config.json` URL means no deployed relay, not Internet connectivity. Read the [relay guide](https://github.com/zhuzhe1983/esp-online-tester/blob/main/docs/platform/public-network-relay.md) for operator deployment. Public relay has no inbound mappings or LAN access.

The virtual router defaults to `esp32sim` / `esp32sim-pass`; a manifest router takes priority. Configure known firmware credentials before loading; no home Wi-Fi password is needed. Automatic provisioning waits for a connected external transport so it does not exit AP into a nonfunctional Internet connection.

```js
await api.configureNetwork({mode:'public-relay', autoProvision:false});
// Configure/load/start the intended firmware, then wait for interfaces[1].leaseReady.
const response = await api.requestNetworkHttp({interface:1, path:'/'});
const page = await api.mapNetworkPort({interface:1, protocol:'http', guestPort:80});
// page.browser=true: use the workbench iframe, not the OS browser's 192.168.4.1.
// For the supported Xiaozhi page contract, explicitly submit real guest HTTP:
await api.provisionNetwork();
```

The client learns the firmware address from DHCP. Never change the host Wi-Fi or navigate to a host private address as a substitute. Browser pages run in an opaque sandbox; relative assets, fetch/XHR, ordinary forms and known navigation are bridged to the guest. CSP/X-Frame-Options, WebSockets, TLS pages and unsupported navigation report limitations; use the optional helper when the original origin/protocol is needed.

For LAN access or native ports, read the [helper guide](https://github.com/zhuzhe1983/esp-online-tester/blob/main/docs/platform/network-helper.md). Start `node tools/network-helper.mjs` with its built native backend, use the emitted token URL or `configureNetwork({mode:'local-helper',endpoint:helperEndpoint})`, and stop the task-owned process after testing. Firmware still executes in WASM. Tokens stay in session memory and are removed from the URL fragment; never retain them in evidence or share links.

A portable skill package can ship the Node launcher plus a matching prebuilt `esp-network-backend`, then pass `--backend /path/to/esp-network-backend`. The repository skill does not currently include cross-platform release binaries. Do not claim the source-only skill eliminates Node/Rust setup, or download an invented release URL. Bundling a local helper helps agents; ordinary website users still use browser SoftAP and a deployed public relay.

Automatic provisioning inspects the original page and recognizes the Xiaozhi web `/scan` + `/submit` contract. Other HTML pages can be operated manually through the iframe/browser. ESP-IDF protocomm requires its version/security/PoP client; an HTTP bridge alone does not implement that protocol. Unknown schemes return `PROVISIONING_UNSUPPORTED`, not fabricated success or direct NVS writes.

Turning automatic provisioning off keeps transport and saved configuration. It does not force already-provisioned firmware back into AP mode or clear storage. Use the firmware's actual menu/button flow if reconfiguration is intended. Successful guest configuration persists under the existing product/firmware Flash scope; recipients of shared links do not receive private Flash.

The bottom network area lists known services and supports explicit entries. In local-helper mode HTTP services get an isolated proxy origin; HTTP pages with restrictive CSP/X-Frame-Options may need the independent-window link. HTTPS is a raw TCP mapping and retains normal certificate validation. Other TCP/UDP services map to localhost for a native protocol client. A conventional port number is only a hint, a mapping alone is not proof of a listener, and passive traffic cannot reveal every silent service. No automatic full-port scanning or LAN-wide exposure occurs.

Verify four separate results: guest provisioning, Wi-Fi association, DHCP lease, and actual target-service response. Native CLI NAT remains available too. Synthetic DNS/SNTP/ICMP in offline mode and browser firmware downloads are not external guest responses. Explicitly distinguish testing the helper with packet fixtures from observing unmodified firmware behavior. Network state, helper tokens and host mappings are not portable recording events; provisioned Flash can fail the recorder's pristine-storage requirement. Do not clear user data to force recording.

## Audio streams and browser devices

Read the [audio runtime guide](https://github.com/zhuzhe1983/esp-online-tester/blob/main/docs/platform/audio-runtime.md) for PCM/WAV workflows and sample-rate boundaries. The top-right microphone/speaker buttons use real browser AudioWorklets. Microphone capture requests permission only after activation; speaker playback also needs a browser user gesture. Device tracks and contexts are released during session teardown. Cross-origin embedded players need iframe `allow="microphone; autoplay"` in addition to the user's permission.

The agent transport does not require a physical microphone or speaker:

```js
await api.audioConfigure({input:{enabled:true,port:0,sampleRate:16000,channels:1}});
await api.pushAudio({port:0,sampleRate:16000,channels:1,samples:pcm16Chunk});
const capture = await api.readAudio();
// Counters only: await api.readAudio({clear:false,includeChunks:false});
// capture.input has current queued/consumed/rejected counters.
// capture.chunks carry exact PCM16 output with sampleRate/channels/sequence/guestTime.
await api.audioConfigure({input:{enabled:false,port:0}});
```

Input uses an explicitly logical I²S controller source and the guest's RX clocks/DMA. It bypasses physical GPIO routing, ES8311 gain/mute and power gates. Check per-board ports; output capture currently uses the S3 TX path and is mono. Codec initialization alone does not prove the voice application processed the input.

PCM input accepts 1–16384 frames per call and holds two source seconds. Keep chunk format fixed, inspect queue counters and handle backpressure. A partial-acceptance error includes the accepted frame count: retry only the unaccepted suffix, never the whole block. Output capture retains ten seconds; `droppedFrames` / `overflowed` explicitly report lost history. Read regularly for a complete longer capture. Browser live playback has its own bounded queue and does not consume the agent's captured chunks.

Subscribe to `audio` events for window-local streaming; iframe clients can push/read structured-cloned arrays. There is still no dedicated MCP server. An MCP adapter can use these APIs for setup/files/results and a suitable binary stream for continuous audio. PCM, microphone access and network state are excluded from portable input recording.

Preserve original duration and pitch. Sample-rate conversion uses the declared source rate; it does not adjust playback speed to match simulator lag. Browser controls default to realtime: keep recent microphone samples, discard queued stale audio when necessary and display loss duration. `pushAudio` remains lossless by default; opt into `{overflowPolicy:'drop-oldest',maxLatencyMs:250}` only for intended realtime tests, and inspect droppedFrames/skippedFrames. Complete mode retains audio and reports overflow. Playback prebuffers 200 ms without changing sample rate. When capture/playback cannot keep up, report loss or use offline PCM/WAV testing. Do not silently stretch, accelerate or truncate voiced content.

A guest-duration press helper remains a useful unimplemented addition. Do not invent its method name; current `press` still uses host time.

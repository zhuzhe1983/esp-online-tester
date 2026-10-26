# Implemented automation API v1.0.0

The platform page installs `window.espTester`. Await its presence before use. Runtime status is one of `idle`, `configured`, `loading`, `ready`, `running`, `paused`, `error`, `disposed`.

| Method | Behavior |
| --- | --- |
| `configure(scene)` | Validates and replaces the configuration; destroys an earlier running worker and clears outputs. Returns state and configuration warnings. |
| `updateSceneMetadata({keyMapping?,view?})` | Updates presentation and keyboard mapping without destroying execution. Each supplied object replaces that whole field; a `view` must include both `mode` and `deviceView`. Other fields require `configure`. |
| `loadFirmware(source?)` | Uses scene firmware by default. Accepts `{manifestUrl,sha256?}`, `{url,sha256?}`, or local `{file,sha256?}` where `file` is a browser File. Loads artifacts and returns `ready` state. |
| `start()` | Boots loaded firmware or resumes a paused session. |
| `pause()` | Pauses an executing session without resetting it. |
| `reset()` | Reloads already downloaded artifacts and boots a fresh session. |
| `input(event)` | Waits for a matching worker queue receipt and returns `guestTime` in simulated seconds; it does not certify firmware consumption. Shapes below. |
| `press(id,{durationMs=100}={})` | Presses then releases a button using host wall time (0–10000 ms), not cycle-exact guest time. |
| `injectGps(options)` | Constructs RMC/GGA sentences with checksums and queues them on logical UART1. Requires an attached `uart-nmea` accessory, logical UART1 wiring and StopWatch. |
| `observe()` | Returns `{version,status,scene,product,firmware,serial,serialByPort,stats,frame,lastError}`. Each serial string retains its last 65,536 JavaScript string code units; this is not an unbounded event log. |
| `capabilities()` | Reports actual product inputs, backend, firmware limits and unsupported features. |
| `capture({format='png'}={})` | Real framebuffer: PNG `{dataUrl,width,height,frameNumber}` or RGBA `{pixels,width,height,frameNumber}`. Rejects `NO_FRAME` before a frame exists. |
| `waitFor({status?,serialIncludes?,port?,fromNow=false,timeoutMs=10000})` | Waits for state and/or serial substring; at least one condition required. Timeout 0–120000 ms. `fromNow:true` ignores old buffered serial. |
| `share({mode?,baseUrl?,includeRecording=true}={})` | Returns a scene URL, plus `recording=` when a stopped matching recording exists. Uses observed source hashes; rejects incomplete/mismatched recordings. `includeRecording:false` explicitly shares the scene alone. Local firmware bytes cannot be encoded. |
| `recordingStart()` | Requires loaded remote firmware; resets and boots it, then starts input capture. |
| `recordingStop()` | Releases held inputs, closes capture and returns a portable recording. |
| `recordingPlay(recording,{timeoutMs=180000}={})` | Validates scene/receipt/event limits, loads pinned firmware if needed, resets and replays. Resolves with the final receipt; cancellation returns `result:'cancelled'`. The per-wait wall timeout is 100–300000 ms. |
| `recordingTakeOver()` | Cancels preparing/playback (including a pending download) and releases held inputs. If recording, stops it. |
| `recordingClear()` | Cancels/releases an active operation, then clears the retained recording and receipt for both UI and automation sharing. |
| `recordingObserve()` | Returns `{status,recording,progress,receipt,error}`; progress contains `{index,total,elapsedGuestSeconds}`. |
| `subscribe(listener)` | Synchronous subscription; returns an unsubscribe function. Events include `state`, `serial`, `frame`, `stats`, `board`, `error`, acknowledged `input`, and `{type:'recording',state}`. |
| `dispose()` | Ends this session and releases its worker. Create a new runtime/page session to run again. |

Input shapes:

```js
{type: 'button', id: 'button-a', pressed: true}
{type: 'touch', x: 100, y: 120, down: true}
{type: 'serial', port: 'uart1', data: '$GPRMC,...\r\n'}
```

Buttons and touch must be released (`pressed:false`, `down:false`). Touch uses integer native framebuffer coordinates. StopWatch's modeled framebuffer is 468×466 even though its visible circular display is 466×466. Available serial input ports are `usb`, `uart0`, `uart1`; UART2 is not exposed. USB input accepts at most 4096 UTF-8 bytes per call; UART input is limited to 128 bytes by its RX FIFO. `injectGps` handles paced chunks so the guest can drain its FIFO.

Console messages may appear on both USB and UART0. Prefer `waitFor({port:'uart0',serialIncludes:...})` for a specific console and read `serialByPort` when collecting evidence. An omitted port matches each stream independently; the combined `serial` string may interleave duplicate chunks.

GPS options:

```js
{
  accessoryId: 'gps-1', latitude: 31.2304, longitude: 121.4737,
  altitude: 12, speedKnots: 0, course: 0, satellites: 8,
  timestamp: '2026-01-01T00:00:00.000Z'
}
```

Provide fixed timestamps for replayable tests; the default uses current time. Satellite count is 1–99; this API currently generates a valid-fix scenario, not loss-of-fix simulation. Serial injection can supply custom NMEA where a different test case is needed.

## Diagnostic example

Run this inside the platform page through the available browser automation tool:

```js
const {loadCatalog, createDefaultScene} = await import(new URL('./catalog.js', location.href));
const catalog = await loadCatalog();
const scene = createDefaultScene(catalog, 'm5stack-stopwatch');
scene.firmware = catalog.firmwares.find(f => f.id === 'platform-stopwatch-diagnostic').source;
scene.accessories = [{id: 'gps-1', catalogId: 'm5stack-gps-u032', parameters: {},
  wiring: {mode: 'logical', port: 'uart1'}}];
const api = window.espTester;
await api.configure(scene);
await api.loadFirmware();
await api.start();
await api.waitFor({port:'uart0', serialIncludes: 'PLATFORM_READY stopwatch', timeoutMs: 30000});
const buttonResult = api.waitFor({port:'uart0', serialIncludes: 'BUTTON_A', fromNow: true, timeoutMs: 15000});
await api.press('button-a', {durationMs: 300});
await buttonResult;
const gpsResult = api.waitFor({port:'uart0', serialIncludes: 'GPS_RX $GPRMC', fromNow: true, timeoutMs: 15000});
await api.injectGps({accessoryId:'gps-1', latitude:31.2304, longitude:121.4737,
  timestamp:'2026-01-01T00:00:00.000Z'});
await gpsResult;
const evidence = {state: await api.observe(), frame: await api.capture({format:'png'}),
  shareUrl: await api.share({mode:'embed'})};
await api.pause();
```

Host speed affects how much guest time elapses during a button press. A timeout is a failed or inconclusive observation; retain it and investigate rather than relabeling the action as successful.

## Recording and replay

The recorder has states `idle`, `preparing`, `recording`, `playing`, `completed`, `cancelled`, `error`. Start requires loaded firmware with a remote, hash-pinned source and performs a reset; recording does not resume from an arbitrary saved CPU state. It records delivered semantic buttons and normalized touch coordinates, including movement while down. A key's physical `KeyboardEvent.code` is not recorded; the mapped button ID is. Camera frames, serial/GPS messages, flashing and arbitrary scripts are not recorded.

`web/platform/recording.js` exports `encodeRecording`, `decodeRecording`, `validateRecording(recording,{scene?})` and limits `MAX_RECORDING_BYTES=32768`, `MAX_RECORDING_EVENTS=500`, `MAX_RECORDING_SECONDS=120`. Validation returns `{valid,errors,warnings}` with `{code,path,message}` issues. A recording contains its `scene`, remote firmware artifact receipt, reset-relative `guest-seconds` clock, native display dimensions, duration and ordered `events`. Button events are `{at,type:'button',id,pressed}`; touch events are `{at,type:'touch',x,y,down}` with x/y normalized to 0–1. Every held input must have a final release. The optional `expectedFrame` contains a SHA-256 of actual RGBA bytes and dimensions.

The link is `index.html#scene=<encoded-scene>&recording=<encoded-recording>` or the equivalent `embed.html`. The UI's portable JSON bundle is `{schemaVersion:'1',scene,recording}`; a scene without recording still uses the original scene JSON format. Importing a bundle or opening a URL only prepares configuration. Replay requires an explicit action. Reject unsupported fields, excessive sizes, unordered times or scene mismatch before fetching firmware. Firmware URLs/hashes, accessory configurations and local definitions must match; view mode and keyboard mappings may change without changing the semantic input sequence.

The host waits for guest time, sends each input in order and records its actual worker-acknowledged delivery time. To avoid collapsing short presses when a host delivery is late, the next target is at least the previous actual acknowledgment time plus the original guest interval. Lateness therefore accumulates rather than erasing a down/up or movement interval. The replay receipt includes each event's `originalGuestSeconds`, `scheduledGuestSeconds`, `actualGuestSeconds`, `lateBySeconds`, timing differences over 50 ms, `frameComparison` (`equal`, `different`, `unavailable`) and `bitExact:false`. It does not infer identical CPU state or application semantics from equal pixels.

`recordingTakeOver()` is the explicit human-control boundary. The automation `input`/`press` methods invoke it before new input during playback/preparation. UI controllers do the same. Pausing finishes recording and releases controls; configuration/disposal interrupts playback. A cancelled guest-clock wait is not allowed to submit later events. Direct camera or serial actions should not be used as a surrogate for recorded touch/button behavior.

The real WASM test verifies A/B short presses in both capture and replay, taps and both directions of continuous touch trajectory, firmware `TOUCH down` output, and non-collapsed guest intervals. The bundled diagnostic has no left/right page-navigation implementation: do not claim that gesture navigation was verified.

## Embedded parent control

Embed the platform's `embed.html#scene=...` URL in an iframe. In the parent, generate a session token (letters, digits, `_`, `-`, at most 80 characters), then send:

```js
iframe.contentWindow.postMessage({
  channel:'esp-workbench', version:'1', type:'hello',
  session:'session_random_token', requestId:'hello1'
}, new URL(iframe.src).origin);
```

The child replies `{channel,version,type:'ready',session,requestId,result:{version,capabilities}}`. Wait for this handshake before sending:

```js
iframe.contentWindow.postMessage({
  channel:'esp-workbench', version:'1', type:'call',
  session:'session_random_token', requestId:'observe1', method:'observe', args:[]
}, new URL(iframe.src).origin);
```

Replies have `type:'result'` and either `result` or `error:{code,message}`. Verify the reply's `event.source`, exact `event.origin`, session and requestId in the parent. Use a new requestId for every command. Up to eight calls can be pending. The child accepts only its direct parent and locks to the first HTTP(S) origin/session handshake; opaque `null` origins cannot connect. Use an explicit `targetOrigin`, never `'*'`.

Exposed methods: `observe`, `capabilities`, `configure`, `updateSceneMetadata`, `loadFirmware`, `start`, `pause`, `reset`, `input`, `injectGps`, `capture`, `waitFor`, `press`, `share`, `recordingStart`, `recordingStop`, `recordingPlay`, `recordingTakeOver`, `recordingObserve`, `recordingClear`. `subscribe` and `dispose` are not exposed through this bridge; poll `recordingObserve` for cross-origin playback progress. Physical Web Serial flashing is absent from both this bridge and `window.espTester`; it is a separate user-initiated UI action.

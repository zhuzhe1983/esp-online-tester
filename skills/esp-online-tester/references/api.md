# Implemented automation API v1.0.0

The platform page installs `window.espTester`. Await its presence before use. Runtime status is one of `idle`, `configured`, `loading`, `ready`, `running`, `paused`, `error`, `disposed`.

| Method | Behavior |
| --- | --- |
| `configure(scene)` | Validates and replaces the configuration; destroys an earlier running worker and clears outputs. Returns state and configuration warnings. |
| `updateSceneMetadata({keyMapping?,view?})` | Updates presentation and keyboard mapping without destroying execution. Each supplied object replaces that whole field; a `view` must include both `mode` and `deviceView`. Other fields require `configure`. |
| `loadFirmware(source?)` | Uses scene firmware by default. Accepts `{manifestUrl,sha256?}`, `{url,sha256?}`, local `{format:'merged',file,sha256?}`, or `{format:'build',files}` containing browser Files for `flasher_args.json` and every referenced binary. Files and JSON manifests are limited to 256 MiB each, with a 256 MiB firmware bundle limit; product Flash capacity still applies. Loads artifacts and returns `ready` state. |
| `start()` | Boots loaded firmware or resumes a paused session. |
| `pause()` | Pauses an executing session without resetting it. |
| `reset()` | Preserves current writable Flash and virtual SD contents, reloads the machine and boots it. Does not clear device data. |
| `flushDeviceStorage()` | Flushes current writable Flash changes; returns the storage object. Requires loaded firmware. |
| `clearDeviceStorage()` | Stops execution, clears only the active device/firmware scope and restores original image bytes. Returns full state in `ready`; does not auto-start. Never use as routine test cleanup on a user's data without authorization. |
| `clearFirmwareCache()` | Removes downloaded firmware cache. Independent of writable device data. |
| `attachSdCard({bytes?,format:'fat32'}={})` | PaperMono/Lite with loaded firmware: replaces the current card with a complete raw disk image (`Uint8Array`/`ArrayBuffer`), or an empty 64 MiB FAT32 card if bytes are omitted. Saves current Flash/SD, rebuilds the machine, returns full state in `ready`; explicitly call `start()` afterward. |
| `clearSdCard()` | Replaces only the active product/board/firmware card with the default empty FAT32 card and returns `ready`. Does not clear writable Flash, other SD scopes or firmware cache. Requires explicit intent to discard this card. |
| `exportSdCard()` | Flushes and reads the current full disk image; returns `{bytes:Uint8Array,filename}`. The API does not itself save a file to Downloads. |
| `flushSdCard()` | Collects current changed pages and awaits saving them; returns the SD status object. Requires loaded SD media. |
| `input(event)` | Waits for a matching worker queue receipt and returns `guestTime` in simulated seconds; it does not certify firmware consumption. Shapes below. |
| `press(id,{durationMs=100}={})` | Presses then releases a button using host wall time (0–10000 ms), not cycle-exact guest time. |
| `injectGps(options)` | Constructs RMC/GGA sentences with checksums and queues them on logical UART1. Requires an attached `uart-nmea` accessory, logical UART1 wiring and StopWatch. |
| `setImu({quaternion,angularVelocityDps?,source='agent',enabled?})` | Sets a virtual sensor sample on boards reporting IMU capability. Quaternion is `[x,y,z,w]`; explicit angular velocity is degrees/second. Returns the sample and, with a loaded worker, its register-delivery receipt. See the axis limitations below. |
| `observe()` | Returns `{version,status,scene,product,firmware,serial,serialByPort,stats,frame,storage,sdCard,leds,imu,lastError}`. Each serial string retains its last 65,536 JavaScript string code units; this is not an unbounded event log. |
| `capabilities()` | Reports actual product inputs, backend, firmware limits and unsupported features. |
| `capture({format='png'}={})` | Real framebuffer: PNG `{dataUrl,width,height,frameNumber}` or RGBA `{pixels,width,height,frameNumber}`. Rejects `NO_FRAME` before a frame exists. |
| `waitFor({status?,serialIncludes?,port?,fromNow=false,timeoutMs=10000})` | Waits for state and/or serial substring; at least one condition required. Timeout 0–120000 ms. `fromNow:true` ignores old buffered serial. |
| `share({mode?,baseUrl?,includeRecording=true}={})` | Returns a scene URL, plus `recording=` when a stopped matching recording exists. Uses observed source hashes; an old mismatched recording is kept separately and omitted from ordinary scene sharing. `includeRecording:false` explicitly shares the scene alone. Local firmware bytes cannot be encoded. |
| `recordingStart()` | Requires loaded remote firmware; resets and boots it, then starts input capture. |
| `recordingStop()` | Releases held inputs, closes capture and returns a portable recording. |
| `recordingPlay(recording,{timeoutMs=180000}={})` | Validates scene/receipt/event limits, loads pinned firmware if needed, resets and replays. Resolves with the final receipt; cancellation returns `result:'cancelled'`. The per-wait wall timeout is 100–300000 ms. |
| `recordingTakeOver()` | Cancels preparing/playback (including a pending download) and releases held inputs. If recording, stops it. |
| `recordingClear()` | Cancels/releases an active operation, then clears the retained recording and receipt for both UI and automation sharing. |
| `recordingObserve()` | Returns `{status,recording,progress,receipt,error}`; progress contains `{index,total,elapsedGuestSeconds}`. |
| `subscribe(listener)` | Synchronous subscription; returns an unsubscribe function. Events include `state`, `serial`, `frame`, `stats`, `board`, `error`, acknowledged `input`, `{type:'recording',state}`, `{type:'storage',storage}`, `{type:'sd-card',sdCard}`, and `{type:'leds',leds}`. |
| `dispose()` | Ends this session and releases its worker. Create a new runtime/page session to run again. |

Input shapes:

```js
{type: 'button', id: 'button-a', pressed: true}
{type: 'matrix-key', row: 0, column: 0, pressed: true} // Only products reporting keyboard capability
{type: 'touch', x: 100, y: 120, down: true}
{type: 'serial', port: 'uart1', data: '$GPRMC,...\r\n'}
```

Matrix keys use the device’s physical row/column, not UART text. Cardputer-Adv uses 4 rows × 14 columns; check `capabilities().input.keyboard` before sending. `press(controlId)` translates declared matrix controls to this protocol.

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

## IMU input and virtual axes

Check `capabilities().imu` before using `setImu`: StopWatch exposes a BMI270 register subset and AtomS3 C123 exposes an MPU6886 polling subset. Neither establishes physical mounting calibration. `observe().imu` is null on boards without a host-fed sensor model.

The quaternion uses virtual model axes: +X right, +Y top, +Z out of the display. An identity quaternion supplies `[0,1,0]` g; rotating 90° around +Z supplies `[1,0,0]` g. The firmware may apply its own axis transform. Agent input defaults to zero angular velocity unless supplied explicitly. Pausing clears angular velocity and preserves acceleration. This is a host-fed register state, not FIFO, interrupt or sensor sample-rate simulation.

StopWatch's viewer forwards scene rotation. AtomS3 currently accepts explicit agent input only: `sceneOrientation:false` means the model-to-sensor transform has not been established, and `source:'scene'` rejects with `IMU_SCENE_UNSUPPORTED`. Do not infer calibrated physical tilt from a successful register receipt. IMU changes are not included in portable input recordings.

## Virtual TF/SD cards and RGB output

PaperMono/Lite automatically receive a generated 64 MiB FAT32 card when firmware loads. Check `capabilities().sdCard.available` and `observe().sdCard` instead of assuming every board supports SD. Imports are complete raw disk images, 1–128 MiB in multiples of 512 KiB; an EPUB, directory or partition-only file is not a disk image. Import preserves its bytes and does not reformat or certify its filesystem. FAT32 is the tested default; firmware still determines whether an imported filesystem can mount.

The storage namespace is product ID + board + SHA-256 of the original full Flash contents. Different device types or firmware images cannot share a card implicitly. Imported base images and changed 4 KiB pages are stored separately in the user's IndexedDB; reset, same-firmware reload and normal page reload retain them. The workbench and mini player use the same controls and runtime API. On the same origin they use the same namespace; a newer save or clear in another page stops a stale session with `STORAGE_CONFLICT` instead of merging competing writes. No card bytes are stored on the server or included in a share URL.

`observe().sdCard` returns `{supported,present,bytes,format,status,backend,restored,pristine,changedPages,error}`. `format` is `fat32` for the generated card or `image` for an imported base; it does not identify the imported filesystem. `backend` is `indexeddb` or `memory`; memory fallback does not survive closing/reloading the page. Status includes `unloaded`, `loading`, `clean`, `saved`, `memory-only`, `conflict`, `error`. `pristine:false` means an imported base or written pages are present and portable recording must reject that private state.

`capabilities().sdCard` reports `defaultBytes`, `maxBytes`, `formats`, `scope`, `autoSaveIntervalMs:1500`, and `crashSafe:false`. Dirty pages are scanned about every 1.5 seconds while running; pause/reset and explicit flush collect them too. Visibility-hidden/pagehide saves are best effort: a killed browser, crash or power loss can lose the last uncommitted writes. A successful flush with `backend:'memory'` is not durable disk persistence.

```js
const api = window.espTester;
// After configuring PaperMono/Lite and loading its firmware:
const sd = (await api.observe()).sdCard;
await api.flushSdCard();
const backup = await api.exportSdCard(); // Preserve locally; never add private bytes to a URL.
// Only when the user asks to replace the current card:
await api.attachSdCard({bytes:new Uint8Array(await selectedFile.arrayBuffer())});
await api.start(); // attach/clear leaves ready, not running.
// await api.clearSdCard(); // Explicitly discards this scope's card and creates blank FAT32.
```

RGB output is `observe().leds`, an array of `[red,green,blue]` byte triples, and the same `{type:'leds',leds}` subscription event. PaperMono's first LED uses PM1 LED_EN for red and M5IOE1 PWM2/PWM1 for green/blue; it is not a WS2812. The viewer consumes these actual guest-driven channels. There is no host API that changes their color as a substitute for firmware behavior. A changed LED tuple proves the modeled digital output, not real-world brightness or timing.

## Official StopWatch example

Run this inside the platform page through the available browser automation tool:

```js
const {loadCatalog, createDefaultScene} = await import(new URL('./catalog.js', location.href));
const catalog = await loadCatalog();
const scene = createDefaultScene(catalog, 'm5stack-stopwatch');
scene.firmware = catalog.firmwares.find(f => f.id === 'm5stack-stopwatch-official').source;
const api = window.espTester;
await api.configure(scene);
await api.loadFirmware();
await api.start();
await api.waitFor({port:'uart0', serialIncludes:'[Launcher] on open', timeoutMs:120000});
const state = await api.observe();
const shareUrl = await api.share({mode:'embed'});
// Wait for and inspect the actual framebuffer before claiming a visible UI result.
// Hold A+B to enter the launcher; A/B select a feature and touch opens it.
```

A host delay is not an exact guest-time pulse. Retain actual input receipts and verify the resulting official UI; strict short-pulse timing remains a documented limitation. The repository's diagnostic/GPS tests use internal fixtures and their own serial assertions; those firmware files are not part of the public Pages site.

## Recording and replay

Writable Flash is isolated by product ID, board, chip, Flash capacity and SHA-256 of the original full Flash contents. File names and remote URLs alone are not identities. `observe().storage` reports backend (`indexeddb` or `memory`), status, scope, `changedPages`, `pristine`, and errors. Memory fallback does not survive a page reload. Browser clear of device data is distinct from download-cache clear. Cross-tab conflicts stop stale sessions rather than silently overwriting newer data.

Portable recording/replay requires pristine Flash and default, unwritten SD data and rejects `RECORDING_STORAGE_NOT_PRISTINE` before resetting a modified device. The recording does not export private Flash pages or SD base images/pages. Tests should use an isolated browser/firmware scope; preserve user data rather than automatically clearing it to make a recording pass.

The recorder has states `idle`, `preparing`, `recording`, `playing`, `completed`, `cancelled`, `error`. Start requires loaded firmware with a remote, hash-pinned source and performs a reset; recording does not resume from an arbitrary saved CPU state. It records delivered semantic buttons and normalized touch coordinates, including movement while down. A key's physical `KeyboardEvent.code` is not recorded; the mapped button ID is. Camera frames, serial/GPS messages, flashing and arbitrary scripts are not recorded.

`web/platform/recording.js` exports `encodeRecording`, `decodeRecording`, `validateRecording(recording,{scene?})` and limits `MAX_RECORDING_BYTES=32768`, `MAX_RECORDING_EVENTS=500`, `MAX_RECORDING_SECONDS=120`. Validation returns `{valid,errors,warnings}` with `{code,path,message}` issues. A recording contains its `scene`, remote firmware artifact receipt, reset-relative `guest-seconds` clock, native display dimensions, duration and ordered `events`. Button events are `{at,type:'button',id,pressed}`; matrix events are `{at,type:'matrix-key',row,column,pressed}`; touch events are `{at,type:'touch',x,y,down}` with x/y normalized to 0–1. Every held input must have a final release. The optional `expectedFrame` contains a SHA-256 of actual RGBA bytes and dimensions.

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

Exposed methods: `observe`, `capabilities`, `configure`, `updateSceneMetadata`, `loadFirmware`, `clearFirmwareCache`, `flushDeviceStorage`, `clearDeviceStorage`, `attachSdCard`, `clearSdCard`, `exportSdCard`, `flushSdCard`, `start`, `pause`, `reset`, `input`, `setGpsFix`, `injectGps`, `setImu`, `capture`, `waitFor`, `press`, `share`, `recordingStart`, `recordingStop`, `recordingPlay`, `recordingTakeOver`, `recordingObserve`, `recordingClear`. `subscribe` and `dispose` are not exposed through this bridge; poll `recordingObserve` for cross-origin playback progress and `observe` for SD/RGB state. SD byte arrays are structured-cloned through this explicit RPC, never embedded in a scene URL. Physical Web Serial flashing is absent from both this bridge and `window.espTester`; it is a separate user-initiated UI action.

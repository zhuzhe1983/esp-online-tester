# Implemented automation API v1.0.0

The platform page installs `window.espTester`. Await its presence before use. Runtime status is one of `idle`, `configured`, `loading`, `ready`, `running`, `paused`, `error`, `disposed`.

| Method | Behavior |
| --- | --- |
| `configure(scene)` | Validates and replaces the configuration; destroys an earlier running worker and clears outputs. Returns state and configuration warnings. |
| `updateSceneMetadata({keyMapping?,view?})` | Updates presentation and keyboard mapping without destroying execution. Each supplied object replaces that whole field; a `view` must include both `mode` and `deviceView`. Other fields require `configure`. |
| `loadFirmware(source?)` | Uses scene firmware by default. Accepts `{manifestUrl,sha256?}`, `{url,sha256?,bootMode?}`, local `{format:'merged',file,sha256?}`, or `{format:'build',files}` containing browser Files for `flasher_args.json` and every referenced binary. Files and JSON manifests are limited to 256 MiB each, with a 256 MiB firmware bundle limit; product Flash capacity still applies. Remote raw BINs auto-detect merged versus validated application-only images; explicit `bootMode` is `rom` or `app-direct`. Application-direct bypasses the bootloader and may fail at firmware initialization or reset. Loads artifacts and returns `ready` state; readiness is not successful guest startup. |
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
| `readLoRa()` | Full PaperMono SX1262 digital state and bounded TX history, plus actual `guestTime`. Requires a loaded worker; does not drain history. |
| `receiveLoRa(packet)` | Supplies an explicit virtual peer packet to full PaperMono. Requires running/paused firmware in a matching RX state. Acceptance schedules completion; verify the guest's resulting output. |
| `setImu({quaternion,angularVelocityDps?,source='agent',enabled?})` | Sets a virtual sensor sample on boards reporting IMU capability. Quaternion is `[x,y,z,w]`; explicit angular velocity is degrees/second. Returns the sample and, with a loaded worker, its register-delivery receipt. See the axis limitations below. |
| `observe()` | Returns `{version,status,scene,product,firmware,serial,serialByPort,stats,frame,storage,sdCard,leds,imu,accessoryInputs,lastError}`. Each serial string retains its last 65,536 JavaScript string code units; this is not an unbounded event log. |
| `capabilities()` | Reports actual product inputs, backend, firmware limits and unsupported features. |
| `capture({format='png'}={})` | Real framebuffer: PNG `{dataUrl,width,height,frameNumber}` or RGBA `{pixels,width,height,frameNumber}`. Rejects `NO_FRAME` before a frame exists. |
| `waitFor({status?,serialIncludes?,port?,fromNow=false,timeoutMs=10000})` | Waits for state and/or serial substring; at least one condition required. Timeout 0–120000 ms. `fromNow:true` ignores old buffered serial. |
| `share({mode?,baseUrl?,includeRecording=true,autostart=true,autoplay=true}={})` | Returns a scene URL, plus `recording=` when a stopped matching recording exists. Uses observed source hashes; an old mismatched recording is kept separately and omitted from ordinary scene sharing. `includeRecording:false` explicitly shares the scene alone. Local firmware bytes cannot be encoded. |
| `recordingStart()` | Requires loaded remote firmware; resets and boots it, then starts input capture. |
| `recordingStop()` | Releases held inputs, closes capture and returns a portable recording. |
| `recordingPlay(recording,{timeoutMs=180000}={})` | Validates scene/receipt/event limits, loads pinned firmware if needed, resets and replays. Resolves with the final receipt; cancellation returns `result:'cancelled'`. The per-wait wall timeout is 100–300000 ms. |
| `recordingTakeOver()` | Cancels preparing/playback (including a pending download) and releases held inputs. If recording, stops it. |
| `recordingClear()` | Cancels/releases an active operation, then clears the retained recording and receipt for both UI and automation sharing. |
| `recordingObserve()` | Returns `{status,recording,progress,receipt,error}`; progress contains `{index,total,elapsedGuestSeconds}`. |
| `subscribe(listener)` | Synchronous subscription; returns an unsubscribe function. Events include `state`, `serial`, `frame`, `stats`, `board`, `error`, acknowledged `input`, `{type:'recording',state}`, `{type:'storage',storage}`, `{type:'sd-card',sdCard}`, `{type:'leds',leds}`, and `{type:'accessories',accessoryInputs}`. |
| `dispose()` | Ends this session and releases its worker. Create a new runtime/page session to run again. |

Input shapes:

```js
{type: 'button', id: 'button-a', pressed: true}
{type: 'accessory-button', accessoryId: 'button-unit', id: 'button', pressed: true}
{type: 'accessory-connection', accessoryId: 'button-unit', connected: false}
{type: 'matrix-key', row: 0, column: 0, pressed: true} // Only products reporting keyboard capability
{type: 'encoder', id: 'encoder', steps: 1} // Only declared encoder controls; signed detents
{type: 'touch', x: 100, y: 120, down: true}
{type: 'serial', port: 'uart1', data: '$GPRMC,...\r\n'}
```

Unit Button U027 uses an audited physical route, unlike logical GPS injection. On StopWatch only, configure `{id:'button-unit',catalogId:'m5stack-unit-button-u027',parameters:{},wiring:{mode:'physical',interfaceId:'grove'}}` in `scene.accessories`. Check `capabilities().input.accessories` and `observe().accessoryInputs` for the resolved instance/port/pin and current connection/press state. Its white signal drives G11; the yellow G10 wire is NC on the unit. Firmware must enable Grove 5V (M5PM1 PWR_CFG 0x06 bit 3). Press reads low; release reads high only with power. A disconnect releases the external drive so the guest's GPIO pull configuration applies. This is a steady digital model; it does not calculate RC, bounce, analog voltage/current or output contention.

Both events are recordable; use the scene instance ID, not the catalog ID. Detached button input is rejected. Host `reset()` restores scene-connected/released state; guest `esp_restart()` retains the held physical switch. Pause retains state; release/disconnect are accepted while paused, and guest observations resume with execution. Reconfigure removes the old worker and routes. The 3D cap and panel use these same events. `press('accessory:button-unit:button')` is a convenience pulse; assert a fresh firmware response using `waitFor({fromNow:true,...})` rather than treating a queue receipt as consumption. Only the catalog U027 + StopWatch Port A pair is enabled; arbitrary pin overrides and other hosts are rejected for physical routing. Session unplugging is an input, not an edit to the saved scene. Replay resets and reproduces it.

Matrix keys use the device’s physical row/column, not UART text. Cardputer-Adv uses 4 rows × 14 columns; check `capabilities().input.keyboard` before sending. `press(controlId)` translates declared matrix controls to this protocol.

Encoder inputs require `capabilities().input.encoders` and a declared control ID. `steps` is an integer from -64 to 64 excluding zero. Positive steps queue A/CLK falling, B/DT falling, A rising, B rising; negative steps reverse that sequence. Each of four phases occupies 2 guest milliseconds, followed by 8 ms idle (16 ms per detent); messages queue without overlap. A worker acknowledgement means queued, not completed motion. The front-view clockwise drag maps to positive steps as a platform convention; physical shaft direction is not calibrated. M5Dial's official decoder reports -4 raw counts for each positive step. Encoder events have no held state or release.

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
// After configuring PaperMono/Lite or PaperS3 and loading its firmware:
const sd = (await api.observe()).sdCard;
await api.flushSdCard();
const backup = await api.exportSdCard(); // Preserve locally; never add private bytes to a URL.
// Only when the user asks to replace the current card:
await api.attachSdCard({bytes:new Uint8Array(await selectedFile.arrayBuffer())});
await api.start(); // attach/clear leaves ready, not running.
// await api.clearSdCard(); // Explicitly discards this scope's card and creates blank FAT32.
```

RGB output is `observe().leds`, an array of `[red,green,blue]` byte triples, and the same `{type:'leds',leds}` subscription event. PaperMono's first LED uses PM1 LED_EN for red and M5IOE1 PWM2/PWM1 for green/blue; it is not a WS2812. The viewer consumes these actual guest-driven channels. There is no host API that changes their color as a substitute for firmware behavior. A changed LED tuple proves the modeled digital output, not real-world brightness or timing.

## Virtual LoRa packets

Only full `papermono` exposes `capabilities().lora.available`; Lite returns false. `readLoRa()` reports power/reset/antenna/BUSY/mode/IRQ, raw radio config, counters and the last 16 completed transmissions (`payloadHex`, config, guest cycle). Old TX entries are dropped when the queue fills; reset/power loss clears hardware configuration and FIFO while diagnostic counters/history remain until a new board is constructed. No received packet is generated without explicit input. No RF power, range, collision, remote relay or radio network is simulated.

```js
const radio = await espTester.readLoRa();
// This is the fixed official UserDemo 1.2.0 peer configuration, not a universal default.
const receipt = await espTester.receiveLoRa({
  config: {frequency:0x36400000, sf:12, bandwidth:3, codingRate:4,
    syncWord:0x3444, implicitHeader:false, crc:true, invertIq:false},
  payloadHex:'50454552', rssiDbm:-70, snrQuarterDb:24, crcError:false
});
// Wait for actual firmware FIFO consumption / UI output; acceptance alone is insufficient.
```

`frequency` is the raw SX1262 32-bit RF word (Hz = word × 32 MHz / 2^25), not hertz. SF is 5–12. `bandwidth` is a datasheet code: 0/8/1/9/2/10/3/4/5/6 correspond to approximately 7.81/10.42/15.63/20.83/31.25/41.67/62.5/125/250/500 kHz. `codingRate` is 1–4 (4/5 through 4/8); `syncWord` is an unsigned 16-bit register value. All numbers must be integers. Payload is even-length hex, at most 255 bytes. RSSI is −127…0 dBm; SNR is signed quarter-dB (−128…127); `crcError` is boolean. These signal values are explicit test inputs, not measured RF quantities.

The receiver must be powered, out of reset, antenna-enabled, not BUSY and in LoRa RX. All config fields must match; implicit-header payload length must also match. Overlapping reception or a packet whose air time exceeds the pending RX timeout is rejected. `LORA_PACKET_REJECTED` indicates this modeled state/config mismatch, `INVALID_LORA_PACKET` indicates malformed host data, and `LORA_UNSUPPORTED` indicates no loaded full-PaperMono radio. An accepted packet completes only when guest time advances; a paused machine remains paused. IRQ/FIFO handling belongs to guest firmware. `crcError:true` raises the CRC error along with RX_DONE; it does not silently become a valid packet.

The workbench API and origin-bound iframe RPC share these methods. The automation wrapper interrupts recording/playback before LoRa injection; LoRa payloads/state do not enter a share URL or portable recording. There is no ordinary UI receive control yet; this stage exposes agent/test API access. GFSK, CAD, LR-FHSS and analog calibration results are outside the digital subset.

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

Share links default to `autostart=1&autoplay=1`: a valid restored scene loads/starts, then its recording plays. `{autostart:false}` disables both; `{autoplay:false}` starts without replay. Legacy scene links without these flags use the same default. Ordinary visits without a scene remain idle. Trial catalog firmware needs one human confirmation before download; no-board products, missing firmware, invalid scenes and invalid recordings do not silently fall back. Edit links explicitly disable autostart. Hardware flashing and storage clearing remain separate user actions.

Writable Flash is isolated by product ID, board, chip, Flash capacity and SHA-256 of the original full Flash contents. File names and remote URLs alone are not identities. `observe().storage` reports backend (`indexeddb` or `memory`), status, scope, `changedPages`, `pristine`, and errors. Memory fallback does not survive a page reload. Browser clear of device data is distinct from download-cache clear. Cross-tab conflicts stop stale sessions rather than silently overwriting newer data.

Portable recording/replay requires pristine Flash and default, unwritten SD data and rejects `RECORDING_STORAGE_NOT_PRISTINE` before resetting a modified device. The recording does not export private Flash pages or SD base images/pages. Tests should use an isolated browser/firmware scope; preserve user data rather than automatically clearing it to make a recording pass.

The recorder has states `idle`, `preparing`, `recording`, `playing`, `completed`, `cancelled`, `error`. Start requires loaded firmware with a remote, hash-pinned source and performs a reset; recording does not resume from an arbitrary saved CPU state. It records delivered semantic buttons, matrix keys, encoder detents and normalized touch coordinates, including movement while down. A key's physical `KeyboardEvent.code` is not recorded; the mapped button ID is. Camera frames, serial/GPS messages, flashing and arbitrary scripts are not recorded.

`web/platform/recording.js` exports `encodeRecording`, `decodeRecording`, `validateRecording(recording,{scene?})` and limits `MAX_RECORDING_BYTES=32768`, `MAX_RECORDING_EVENTS=500`, `MAX_RECORDING_SECONDS=120`. Validation returns `{valid,errors,warnings}` with `{code,path,message}` issues. A recording contains its `scene`, remote firmware artifact receipt, reset-relative `guest-seconds` clock, native display dimensions, duration and ordered `events`. Button events are `{at,type:'button',id,pressed}`; matrix events are `{at,type:'matrix-key',row,column,pressed}`; encoder events are `{at,type:'encoder',id,steps}`; touch events are `{at,type:'touch',x,y,down}` with x/y normalized to 0–1. Every held input must have a final release. The optional `expectedFrame` contains a SHA-256 of actual RGBA bytes and dimensions.

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

AtomS3R C126 also exposes `setImu` through its BMI270 polled register subset. The virtual axes are explicit agent input; `source:"scene"` is rejected until physical mounting is calibrated. BMM150 AUX magnetometer, RGB/IR and old GC9107 batches remain unsupported. See [AtomS3R runtime scope](../../../docs/platform/atoms3r-runtime.md).

## Unit RGB U003 output

Only U003 on StopWatch Port A is physically routed: yellow wire G10 receives actual guest RMT GRB output for three SK6812 RGB LEDs. Configure `{id:'rgb-unit',catalogId:'m5stack-unit-rgb-u003',parameters:{},wiring:{mode:'physical',interfaceId:'grove'}}`. One physical unit occupies the Grove socket. Firmware must enable M5PM1 Grove 5V; merely selecting the accessory does not supply it.

`observe().accessoryOutputs` contains `{accessoryId,model:'sk6812-u003',port:'grove',leds:[[r,g,b],...]}`. Subscribe to `accessory-outputs` for fresh guest values. There is no colour-setting API: drive the guest's RMT peripheral. `input({type:'accessory-connection',accessoryId:'rgb-unit',connected:false})` removes power/data, blacks LEDs and is recordable. Reconnection stays dark until a fresh guest transmission. Guest reboot retains connection; host `reset()` restores the scene connection. Software GPIO pulses, daisy-chain output, white-wire behaviour, physical LED-hole order and optical brightness are not verified. See `docs/platform/unit-rgb-runtime.md`.

### Unit Angle U005

An audited StopWatch Port A physical instance with `catalogId:'m5stack-unit-angle-u005'` and `wiring:{mode:'physical',interfaceId:'grove'}` supports `input({type:'accessory-angle',accessoryId,id:'knob',value})`. Finite `value` is normalized Rp/R2 in 0–1, not encoder detents or degrees. `parameters.position` sets the scene's initial position (default 0.5). `observe().accessoryInputs[].value` and `accessory-positions` are input state, not proof of a guest ADC conversion. The guest must enable Grove5V and sample G11/ADC2_CH0. The explicit `potentiometer-u005-10k-curve` profile follows the published 10k formula; conflicting official drawings and uncalibrated physical travel/direction remain documented. Disconnect while paused is allowed; rotation while paused rejects. Guest reboot retains physical input, platform reset restores scene position. See [scope and real firmware evidence](../../../docs/platform/unit-angle-runtime.md).

U006 passive Hub: attach `m5stack-unit-hub-u006` directly with `wiring:{mode:'physical',interfaceId:'grove'}`. A U027/U003/U005 child uses `wiring:{mode:'physical',viaAccessoryId:'hub-instance-id',portId:'port-1'}` (ports1–3, one layer). All outlets share original G10/G11; same-signal contention is rejected. Disconnect the Hub with `input({type:'accessory-connection',accessoryId:'hub-instance-id',connected:false})`; children retain local `connected`, while `effectiveConnected` becomes false and real guest paths disconnect together. This field is continuity, not a power measurement. See `docs/platform/unit-hub-runtime.md`; output socket visual pin1 remains unknown, so output cables are hidden.

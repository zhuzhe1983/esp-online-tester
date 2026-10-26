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
| `input(event)` | Queues an input; it does not certify firmware consumption. Shapes below. |
| `press(id,{durationMs=100}={})` | Presses then releases a button using host wall time (0–10000 ms), not cycle-exact guest time. |
| `injectGps(options)` | Constructs RMC/GGA sentences with checksums and queues them on logical UART1. Requires an attached `uart-nmea` accessory, logical UART1 wiring and StopWatch. |
| `observe()` | Returns `{version,status,scene,product,firmware,serial,serialByPort,stats,frame,lastError}`. Each serial string retains its last 65,536 JavaScript string code units; this is not an unbounded event log. |
| `capabilities()` | Reports actual product inputs, backend, firmware limits and unsupported features. |
| `capture({format='png'}={})` | Real framebuffer: PNG `{dataUrl,width,height,frameNumber}` or RGBA `{pixels,width,height,frameNumber}`. Rejects `NO_FRAME` before a frame exists. |
| `waitFor({status?,serialIncludes?,port?,fromNow=false,timeoutMs=10000})` | Waits for state and/or serial substring; at least one condition required. Timeout 0–120000 ms. `fromNow:true` ignores old buffered serial. |
| `share({mode?,baseUrl?}={})` | Returns an `index.html` or `embed.html` URL with a canonical `#scene=` payload. Uses observed source hash after loading. Local firmware bytes cannot be encoded. |
| `subscribe(listener)` | Synchronous subscription; returns an unsubscribe function. Events include `state`, `serial`, `frame`, `stats`, `board`, `error`. |
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

Exposed methods: `observe`, `capabilities`, `configure`, `updateSceneMetadata`, `loadFirmware`, `start`, `pause`, `reset`, `input`, `injectGps`, `capture`, `waitFor`, `press`, `share`. `subscribe` and `dispose` are not exposed through this bridge.

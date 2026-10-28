# StopWatch Xiaozhi audio throughput screen

The official Xiaozhi 2.2.6 image completes 15 emulated seconds in about 7.1 wall
seconds with the interpreter in this browser-worker-only workload. Enabling the
existing JIT did not show a stable improvement. **No runtime policy changed.**
This is a computation screen, not a measurement of the full workbench, audible
playback, microphone input or an online conversation.

This extends [EX011](../../experiments.md#ex011) with a pinned product firmware
and audio correctness contract. Unlike the earlier kernel ceiling measurement,
the workload starts the complete original flash image through the S3 ROM and
retains the emitted PCM. Related history: [EX194](../../experiments.md#ex194)
covers a different firmware workload; the [StopWatch IMU receipt](../../platform/stopwatch-imu.md)
records JIT watchdog failures with official UserDemo V0.5. This short Xiaozhi
result does not resolve those failures or justify enabling JIT for StopWatch.
[EX117](../../experiments.md#ex117) concerns production scheduling; it was not
changed or benchmarked here.

## Results

ABBA order, fresh emulator and JIT host each time, no warmup run:

| Run | Execution | Wall seconds | Guest / wall | Maximum PCM arrival gap, ms |
| --- | --- | ---: | ---: | ---: |
| A1 | Interpreter | 7.0756 | 2.11996 | 65.1 |
| B1 | JIT | 7.5779 | 1.97944 | 235.0 |
| B2 | JIT | 7.0490 | 2.12796 | 256.1 |
| A2 | Interpreter | 7.0714 | 2.12122 | 67.2 |

JIT takes 7.10% more wall time in the first pair and 0.32% less in the second.
The two-run medians are 7.0735 s and 7.31345 s respectively, 3.39% more for JIT.
Two pairs are insufficient for a general performance claim. JIT compiled 8,325
modules, with no compilation failures, spending 608.9 / 631.5 ms in compilation.
The larger observed gaps are associated with the JIT arms; no timestamped
compilation trace was taken to attribute individual gaps to compilation.

All four runs agree exactly on:

- 3,600,000,000 guest cycles and 383,784,029 retired instructions.
- 337 published frames and the final binary framebuffer SHA-256.
- UART0 text SHA-256 and length of 8,612 UTF-8 bytes.
- 342,746 PCM samples at 24,000 Hz, including 38,794 nonzero samples, peak 32,079,
  and the SHA-256 of the complete concatenated little-endian signed 16-bit PCM.
- Firmware codec-open and SoftAP web-server log markers; no detected panic.

The full hashes and per-second samples are in [results.json](results.json).
Hash equality checks output consistency; it does not establish accurate hardware
timing, correct spoken content, or absence of every possible guest fault. No
auditory listening check was performed. The PCM includes silence and a short
startup announcement; this is not sustained speech decoding.

## Inputs and method

- JavaScript source checkout: `078ad5825c1545b38789c917949570d437a035c5`.
- Existing production WASM reused without rebuilding, identified by its SHA in
  the result. Its build-source revision was not independently recovered.
- Full original vendor image: [M5Burner CDN](https://m5burner-cdn.m5stack.com/firmware/05a2cf9f8c71e46dbd0dca65137ad0c1.bin),
  SHA-256 `6ed1ed429c275504c1cc32a1535f8f961b16509e87b231fd1dfd15fcff1fc21a`.
- S3 revision-0 ROM ELF, SHA-256 recorded in the result.
- StopWatch board, 16 MiB flash, 8 MiB PSRAM. ROM boot; no firmware patches,
  stubs, app-direct boot, external network, injected microphone or guest inputs.
- Virtual router configuration `ssid=esp32sim,psk=esp32sim-pass`. Firmware enters
  its fresh-storage AP setup flow. No production SoftAP peer or STA relay is
  attached, and no persisted browser flash state is loaded.
- Apple M5 Max, 18 CPU cores, 128 GiB RAM; macOS 27.0.1; headless Chrome 156.
  Coordinated builds and other emulator benchmarks were paused. Aggregate CPU
  load and thermal state were not sampled; other interference is not excluded.

The [probe](../../../tools/audio-runtime-probe.mjs) is a dedicated module worker.
It runs 2,000,000-cycle slices to exactly 15 guest seconds, drains the outbox each
slice, and yields once per guest second. It enables the same WASM PCM output
stream, retains bytes in memory, and hashes them after the timer stops. The
timer includes JIT compilation, frame/serial draining, PCM copying and sample
scanning, and those worker yields. It excludes fetches, WASM instantiation, ROM
and flash loading, boot setup, and final hashing. Host time supplied to the
firmware is fixed to Unix milliseconds `1700000000000 + guestMilliseconds`.

The probe has no wall-time pacer, UI rendering, production frame acknowledgments,
Flash watchers, network relay, AudioContext or audio worklet. Its arrival gaps
measure outbox drain timestamps, not a speaker's underrun counter. Average
throughput above 1 does not guarantee continuously available audio. Production
worker scheduling, main-thread/renderer contention, burst delivery and playback
buffer policy still require a separate end-to-end measurement. Changing JIT is
not supported as a solution by this screen.

## Reproduction

Build or supply the WASM and S3 ROM as described in the project setup. Serve the
repository through a local HTTP server, open a headless browser with the
Playwright skill wrapper, and navigate to any same-origin page without the
workbench running. Pass explicit URLs and the firmware hash to this snippet in
the page; relative URLs resolve against that page. No firmware URL is built into
the reusable probe.

```js
async function runAudioScreen({firmwareUrl, firmwareSha256, wasmUrl, romUrl}) {
  const runs = [];
  for (const jit of [false, true, true, false]) {
    const worker = new Worker('/tools/audio-runtime-probe.mjs', {type:'module'});
    let timer;
    try {
      runs.push(await new Promise((resolve, reject) => {
        timer = setTimeout(() => reject(Error('90 second watchdog')), 90000);
        worker.onerror = event => reject(Error(event.message));
        worker.onmessage = ({data}) => {
          if (data.type === 'result') resolve(data.result);
          if (data.type === 'error') reject(Error(data.message));
        };
        worker.postMessage({firmwareUrl,firmwareSha256,wasmUrl,romUrl,jit,seconds:15});
      }));
    } finally { clearTimeout(timer); worker.terminate(); }
  }
  return runs;
}
```

Use the matching public firmware URL/hash above, plus your same-origin WASM and
ROM URLs. Close the task-owned browser session and local server afterward.
The measured script accepted a broader 64-character hash syntax; the committed
probe narrows this to lowercase hexadecimal. The timed execution code is unchanged.

## Curation and checks

Only hashes, counts, timings, software/hardware configuration and boolean guest
markers are retained. Raw UART text can contain firmware-generated identifiers
and was never written to disk; framebuffer and PCM bytes remain transient worker
memory. No microphone audio, account details or external service responses were
collected. Browser session details and local absolute paths are omitted; this
does not affect timing or output-hash comparison. Results are a new summary, not
a sanitization of a previously published receipt.

Validation: four real-browser arms completed; `node --check
tools/audio-runtime-probe.mjs`, `git diff --check`, and the repository evidence
privacy check pass. The task-owned browser was closed and its exit verified.

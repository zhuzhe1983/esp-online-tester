# Writable device Flash

The emulator now keeps a device's own Flash writes separately from firmware downloads. Guest NVS, FAT/wear-levelling and other Flash writes survive software reset, host reset and reloading the same device and firmware. In browsers with usable IndexedDB, saved pages also survive a page refresh. Firmware downloads may use the separate firmware cache; clearing that cache does not erase device data.

The scope is `productId + board + chip + Flash capacity + SHA-256 of the complete initial Flash array`. The hash is taken after loading the original image, before restoring device data or starting instructions. Erased gaps and capacity are included. A local build directory, manually supplied segments and a merged file with the same resulting bytes therefore use the same scope, independent of file names or URLs. Different products or original Flash contents are isolated.

```js
const state = await espTester.observe();
state.storage; // current actually loaded device, never an uncommitted firmware choice
await espTester.flushDeviceStorage();
await espTester.clearDeviceStorage(); // stops the device, restores the original image, returns ready
await espTester.start();              // explicit boot after clear
```

`storage` reports `scope`, `backend`, `status`, `restored`, `pristine`, `changedPages`, `epoch`, `revision`, `lastSavedAt` and `error`. Subscribe to `{type:'storage', storage}` for changes. `STORAGE_NOT_LOADED` means there is no loaded scope. `memory-only` means writes are retained in the current page session and are not confirmed durable. Database unavailability/quota errors are surfaced; a failed durable clear never claims success. Guest data is not uploaded to a server or added to share URLs.

The WASM ABI exposes read-only Flash pointer/length access. Between guest run calls, an opt-in worker watcher compares 4 KiB pages at approximately 500 ms host intervals. Only changed pages are transferred. Pause, reset, replacement and disposal request a final flush. Page-hide requests a best-effort flush; abrupt process termination, a worker that stops responding, quota exhaustion or browser eviction can still lose unsaved changes. This is not a crash-consistent physical Flash controller or a timing model for Flash wear. Snapshotting does not advance guest time, and no extra work was added to native instruction/tick paths.

A save transaction checks both epoch and revision before writing. Clear creates a new epoch and restores the original image without starting it. BroadcastChannel informs another live tab using the same scope; its stale worker stops with `STORAGE_CONFLICT`. If notifications are unavailable or a tab was suspended, the transaction check still rejects stale writes. Reload the same firmware to acquire the latest stored state. Clearing an active recording/replay first cancels that operation. Shareable recordings require a pristine starting image; existing private device state is not silently erased or embedded in recordings.

Verification uses an internal ESP-IDF 5.5.1 fixture, not a precomputed screen or fake storage response. It writes NVS and a wear-levelled FAT file, calls `esp_restart()`, and reads both back. The real WASM probe also checks host reset, new-runtime restore from an equivalent merged image, conflicting runtime shutdown, clear and reinitialization. A headless Chromium check confirms IndexedDB bytes after refresh and stale-tab rejection after clear. Commands, artifact hashes and results are under [evidence/device-storage](evidence/device-storage/wasm-fixture.json); build instructions are in [the fixture README](../../tools/fixtures/device-storage-firmware/README.md).

The user-supplied private build produced 421 modified Flash pages and reached the observed storage and input initialization markers. No later software-reset/watchdog/panic signature was observed during that run. Pause acknowledgement timed out, so full application compatibility remains unverified. Only counts, artifact/frame/serial hashes and sanitized conclusions are retained in [its receipt](evidence/device-storage/private-build-sanitized.json); private binaries, file names, absolute paths and raw serial were not added to the repository.

# Internal writable-Flash fixture

This ESP-IDF 5.5.1 source exercises NVS, wear-levelled FAT and `esp_restart()` with fixed, nonprivate test data. It is not a product firmware and its binaries are not published to GitHub Pages.

With the ESP-IDF 5.5.1 environment active, from the repository root:

```sh
idf.py -C tools/fixtures/device-storage-firmware -B "$PWD/output/device-storage-build" build
node tools/probe-device-storage.mjs --build output/device-storage-build
```

The first boot writes a fixed configuration file and an NVS marker, commits/unmounts both stores, prints `STORAGE_WRITTEN NVS FAT; SOFTWARE_RESTART`, and invokes `esp_restart()`. The following boot must read the original bytes and print `STORAGE_RESTORED NVS FAT`.

The probe then checks host reset, a new runtime loading the same Flash content in a differently named merged file, another runtime's reaction to clear, and reinitialization after explicitly clearing storage. It uses the actual WASM worker and page-level RAM fallback. Durable IndexedDB and cross-tab notifications are checked separately by `tools/fixtures/device-storage-browser-check.cjs` through the tracked Playwright CLI, on a local repository server.

The default Flash capacity is 8 MB. For a StopWatch UI check, use a separate SDKCONFIG and defaults file with `CONFIG_ESPTOOLPY_FLASHSIZE_16MB=y`; do not reuse or overwrite the existing StopWatch diagnostic binaries.

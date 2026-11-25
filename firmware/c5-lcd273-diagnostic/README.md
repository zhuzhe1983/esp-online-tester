# C5 LCD273 diagnostic 0.1.0

An independently authored MIT test application for the Waveshare ESP32-C5-LCD-2.73. It is **not manufacturer firmware**. Its static 320×320 LVGL screen shows PREV/NEXT/BOOT down/up counts, the most recent observed input edge and TF status. The original manufacturer 02/03 examples remain separate inputs.

The build copies the fixed manufacturer's BSP, local components, partitions, empty main manifest, dependency lock and sdkconfig. BSP, button 4.1.7, adapter 0.5.1 and LVGL 9.5.0 stay unchanged. Only this application's main/CMake/version/license are authored here. Flash remains32MiB, quad80MHz PSRAM and memtest remain enabled, XTAL remains AUTO, and the original PSRAM display buffers remain50 rows/double buffered. Runtime8MiB/48MHz detection is separate evidence.

## Input contract

The fixed BSP creates three active-low GPIO buttons: **PREV2, NEXT3, BOOT28**, using its original5ms button timer and2 debounce ticks. It registers one LVGL ENCODER indev. This application obtains that existing indev and its public read callback; a wrapper calls the original callback first, observes its result and passes all data through unchanged. It creates no additional GPIO button driver.

The pinned adapter's LVGL9 `lvgl_button_read` explicitly supplies `LV_KEY_LEFT/RIGHT/ENTER` and `PRESSED/RELEASED`, rather than only encoder diff. Counts refer to the **debounced single-key state observed through that callback**, not every electrical GPIO edge. LVGL independently consumes the same data to move the real group focus and press the selected button. No host-created frame, synthetic input data or register acknowledgement is involved.

After `C5D1 READY`, press one key at a time, holding down and then releasing for at least100ms of guest time each. BOOT means the running app's GPIO28 button; start this sequence after boot. UART examples have this grammar:

```text
C5D1 KEY seq=1 key=PREV gpio=2 state=DOWN down=1 up=0
C5D1 KEY seq=2 key=PREV gpio=2 state=UP down=1 up=1
```

The global sequence advances only for observed single-key edges, and the corresponding label and recent-event label update in the LVGL callback. A UART marker precedes the completed LCD transfer; advance the guest and inspect the actual framebuffer after it. A label update in source or a queued platform input alone is not execution evidence.

The original adapter prioritizes PREV > NEXT > BOOT and is not a chord-reporting interface. A selected key change while already pressed emits `C5D1 INPUT_AMBIGUOUS`, displays a retry message and does not invent a new physical release/press. Chords and pulses shorter than the debounce/read interval are outside this diagnostic's edge-count contract.

## Shared SPI and TF

The app sets LCD CS8 and TF CS9 high, calls the public BSP bus initializer with `320*50*3=48000` bytes, then calls the unmodified BSP mount. This follows the IDF5.5.3 shared-SPI requirement that the card enter SPI mode before other devices communicate. Only then does it reset the panel through CH32 and start the original display/adapter. LCD and TF share SPI2/SCLK6/MOSI7; TF MISO5 and the two device CS lines remain distinct. The BSP's initialized-bus guard preserves the initial transfer reservation for both devices.

Mount uses the BSP's existing `format_if_mount_failed=false`. After the UI has run for200 guest ms, the app opens only `/sdcard/test.txt` with **rb**. It reads at most128 bytes plus an EOF probe, reports the prefix byte count, EOF/error and `esp_crc32_le(0, prefix, size)`, and compares a complete71-byte file against the original03 example's known payload. It never creates, formats, erases or writes a file. Absence is an ordinary displayed state; other open/read errors remain explicit. A failed mount is displayed and does not become a TF success.

Each observed BOOT release notifies the main task to repeat that same read while the LCD input/UI is active. File I/O stays outside the LVGL task; TF label changes take the adapter lock. Use a clone of the03-produced card when checking the71-byte match and unchanged IMG hash. This application does not reproduce03's write test.

## Rebuild and inspection

Use the existing verified source inputs from the [IDF553 reference receipt](../../docs/evidence/c5-lcd273-idf553-examples-2026-11-25/README.md): vendor418de818f4f40c5a04c4292dbe293bfaa1351068, IDF2c211b236707889e8400c4dc5644dd5c4ee071e0, GCC esp-14.2.0_20251107 and the original15 registry pins. Supply their paths; the helper never installs global tools. It copies the component cache and creates private tool state/TMPDIR under a fresh ignored target output, leaving the reference SDK, compiler, Python, source and package cache read only.

CMake/Ninja may be executable names on the invoking PATH or explicit absolute/relative paths. The helper resolves them to real absolute executable files before creating outputs or adding its Ninja wrapper to PATH. Explicit relative paths use the invoking working directory; directories, missing files and non-executable files are rejected.

```sh
python3 -B tools/c5-lcd273-diagnostic-build.py build \
  --reference-inputs "$REFERENCE_INPUTS" --idf-path "$IDF553" \
  --components-root "$LOCKED_COMPONENTS" --python-root "$PRIVATE_IDF_PYTHON_ROOT" \
  --gcc-bin "$GCC_BIN" --cmake "$CMAKE" --ninja "$NINJA" \
  --jobs 8 --output "$FRESH_DIAGNOSTIC_DIR"
python3 -B tools/c5-lcd273-diagnostic-build.py inspect --output "$FRESH_DIAGNOSTIC_DIR"
```

The original empty `main/idf_component.yml` must be present: omitting it changes the manifest hash and can trigger a new dependency solution. The helper preserves it and refuses any config/lock drift. All existing outputs, including failed attempts, are preserved; use a fresh output name. Outer jobs8/nested bootloader jobs1 are build flags, not a strict total concurrency guarantee or performance result.

Outputs include `build/merged.bin`, `build/c5_lcd273_diagnostic.{bin,elf,map}` and raw per-phase logs. Image/ELF/config/source identity is checked statically; this helper never runs an emulator, benchmark, browser, flash or monitor. Source license is MIT; unchanged upstream BSP/SDK/component files retain their own licenses. See the separate diagnostic evidence receipt for artifact hashes and current validation limits.

# CoreMatrix internal-system firmware check

These fixtures run the unmodified official UIFlow 2.5.4 image through the genuine ESP32-C61 ROM. They upload a test application through USB raw REPL into an isolated in-memory machine. They do not install a replacement firmware, edit `boot.py`, emulate Python results, or modify the supplied `.bin` file.

Build the integrated C61 crate with the current stable toolchain. The compiled `esp32c61` and `esp_soc` libraries must come from the same release build. Assign their `.rlib` paths to `C61_RLIB` and `SOC_RLIB`, and their shared dependency directory to `AUDIT_DEPS`. A fresh task-specific `CARGO_TARGET_DIR` avoids stale artifacts.

```sh
cargo build --release -p esp32c61
rustc --edition=2021 -O -C lto=fat \
  -L "dependency=$AUDIT_DEPS" \
  --extern "esp32c61=$C61_RLIB" --extern "esp_soc=$SOC_RLIB" \
  tools/fixtures/corematrix-uiflow/system-audit.rs -o "$AUDIT_BINARY"
"$AUDIT_BINARY" "$C61_ROM_ELF" "$ORIGINAL_COREMATRIX_BIN" \
  tools/fixtures/corematrix-uiflow/system-write.py \
  tools/fixtures/corematrix-uiflow/system-read.py
```

Check the original input hashes against `docs/platform/evidence/corematrix-system-20261106/receipt.json` before running. `-C lto=fat` consumes the release libraries' LLVM bitcode with rustc's matching LLVM; directly passing those bitcode objects to an older system linker can fail before execution.

The test writes 16,420 deterministic bytes to a unique temporary file under `/flash`, flushes, renames, syncs, and verifies every byte. It checks the guest RTC and elapsed ticks across a 1,250 ms guest delay, scans the real G0/G1 hardware I²C bus, and calls `machine.reset()`. The harness requires an actual `SwReset`, resets the CPU/peripherals through `Machine::reboot`, boots the ROM again, reads every file byte back, verifies RTC date retention and the hard-reset cause, then removes the task file. No SD card or external network is required.

The boot budget is 100 million instructions. Source is sent in 64-byte chunks with one million instructions between chunks. Each executed program receives up to 500 million instructions; the first returns early on `SwReset`. The original distribution's already-documented missing `M5Things` module remains visible at startup and is not stubbed. Application assertions are evaluated by the guest MicroPython interpreter. An unhandled guest assertion, missing result, wrong reset path, or CPU exception fails the harness.

This test covers guest Flash retention within one machine across a hardware reset. Browser IndexedDB persistence, abrupt write interruption, chip deep/light sleep, RTC alarm wake, SNTP, battery drain and oscillator accuracy require separate evidence.

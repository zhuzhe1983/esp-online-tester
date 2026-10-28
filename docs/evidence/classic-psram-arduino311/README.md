# Stock Arduino-ESP32 3.1.1 compatibility

Current integrated base: upstream main `77e43c20a70054ac84d1724d757fa9ce12668151`.
Memory-miss implementation: `9823ed9a978c0fc798ffebc322662624566758f5`.
Current checks, final disassembly pair, archive manifest and model limits: [consolidated receipt](../classic-psram-followup/README.md).

[Public pinned recipe](../../fixtures/classic-psram-arduino311/README.md) reuses
the Arduino3 probe/assertions without changing SDK routines or return values.
Run the same external `classic_psram_arduino3` harness with
ARDUINO3_FIRMWARE_DIR pointing to its 3.1.1 output, and ESP32SIM_ROM_DIR set.
Both present and absent cases must pass; the present guest repeats on a fresh
machine. It retains exact 4 MiB heap/getter, 4192124 free payload, true separate
heap registration, two bytewise 1 MiB hashes, duplicate/retry outcomes, zero stubs
and zero reboots. This is a bounded local external-fixture qualification, not
all Arduino3 versions or CI firmware. Pinned input hashes/current checks are
consolidated. Historical corrupted absent-ID and byte-read probes failed the
named runtime assertions, not compilation.

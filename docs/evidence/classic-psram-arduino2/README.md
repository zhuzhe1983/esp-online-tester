# Stock Arduino-ESP32 2.0.17 qualification

Current integrated base: upstream main `77e43c20a70054ac84d1724d757fa9ce12668151`.
Memory-miss implementation: `9823ed9a978c0fc798ffebc322662624566758f5`.
Current checks, final disassembly pair, archive manifest and model limits: [consolidated receipt](../classic-psram-followup/README.md).

[Public fixture and pinned recipe](../../fixtures/classic-psram-arduino2/README.md).
Stock dual-core ESP32-CAM, IDF v4.4.7-dirty, 80/80 MHz, BOOT_INIT=n,
light heap poisoning. Set ARDUINO2_FIRMWARE_DIR and ESP32SIM_ROM_DIR; run
`cargo +1.99.0 test --release -p esp32sim --test classic_psram_arduino2 -- --ignored`.
Firmware is an external local input, not a CI asset. No SDK patch, startup stub,
guest cache/MMU preparation, return interception or reboot retry is used.

The real SDK test validates 131072 stride-32 words over both halves before heap
registration. Two genuine 1 MiB bytewise passes return zero errors and FNV-1a
20c19dc5 / 9dce9dc5, free after each returns to the initial value; fresh machines
repeat the result. Absent ID is ffffffff, found/size/heap/free are zero and
allocation is unavailable, not an internal-RAM fallback.

Exact accounting: physical=4194304, inclusive-end heap=4194303;
Arduino getter/free payload=4192123. Weak himem reservation returns zero and
external BSS/noinit ranges are empty. Payload subtracts heap header 20,
TLSF 2148 (36 + 16*4 + 16*32*4) and light poisoning 12 bytes.
Sources: ESP-IDF v4.4.7 `components/esp_hw_support/port/esp32/spiram.c:192-209,254-306`,
`components/heap/heap_caps.c:505-514`, `multi_heap.c:80-86,109-132,390-408`,
`heap_tlsf.h:62-108`, `heap_tlsf.c:473-489`, `multi_heap_poisoning.c:51-61,369-386`;
Arduino 2.0.17 `cores/esp32/Esp.cpp:getPsramSize` and `esp32-hal-psram.c`.
This qualifies free followed by allocation, not realloc or himem APIs.

Historical emulator guest mutations killed 7/9; slot-offset/page-shift survived
self-consistent guest addressing. Native mapping tests independently killed both.
The original normal-only prerequisite's memory-test failure is retained as a
negative in the archive; low/high support was the required model change.
Build-path-dependent ELF/application hashes are not claimed reproducible across
unrelated directories. Firmware hashes used for the current rerun are consolidated.

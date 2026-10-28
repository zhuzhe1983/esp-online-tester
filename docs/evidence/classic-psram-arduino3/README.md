# Stock Arduino-ESP32 3.0.7 qualification

Current integrated base: upstream main `77e43c20a70054ac84d1724d757fa9ce12668151`.
Memory-miss implementation: `9823ed9a978c0fc798ffebc322662624566758f5`.
Current checks, final disassembly pair, archive manifest and model limits: [consolidated receipt](../classic-psram-followup/README.md).

[Public fixture and pinned recipe](../../fixtures/classic-psram-arduino3/README.md).
IDF v5.1.4-972-g632e0c2a9f-dirty, stock dual-core 80/80 MHz, BOOT_INIT=n,
light heap poisoning. Set ARDUINO3_FIRMWARE_DIR and ESP32SIM_ROM_DIR; run
`cargo +1.99.0 test --release -p esp32sim --test classic_psram_arduino3 -- --ignored`.
These are local external pinned images, not CI firmware. The public v5.1.4 source
is corroboration, not a substitution for the bundled dirty SDK archive identity.

Separate startup and heap-add are observed at executed instruction entries.
Present entry counts for psramInit/esp_psram_init/esp_psram_extram_test/
psramAddToHeap/esp_psram_extram_add_to_heap_allocator are [2,2,1,1,1];
absent counts [2,1,0,2,0]. Duplicate SDK init returns 259; no stubs or retries.
The real test checks 131072 stride-32 words across both halves. Physical, getter
and heap total are 4194304; free payload 4192124. IDF5 uses end=start+size,
unlike IDF4's inclusive end. Free payload subtracts header 20, TLSF 2148 and
poisoning 12 bytes. Two 1 MiB bytewise passes return zero errors and hashes
20c19dc5 / 9dce9dc5; freeing restores initial payload. Absent cannot allocate.

Sources: Arduino 3.0.7 `cores/esp32/Esp.cpp:149-154`, `esp32-hal-psram.c:44-105`;
ESP-IDF v5.1.4 `components/esp_psram/esp_psram.c:99-114,119-150,196-228,285-336`,
`components/heap/heap_caps.c:516-535`, `multi_heap.c:76-82,129-151`,
`multi_heap_poisoning.c:49-62`; espressif/tlsf
`d2e28f872472ffc6a704faae65ddee1f24e2dfba`, `tlsf_common.h:90-123`, `tlsf.c:598-614`.
Historical emulator guest probes killed 7/9; slot/shift survivors are disclosed,
not counted as kills. Separate guest-library state probes killed 4/4. Firmware
hashes and current gate outcomes are consolidated; no fresh SDK rebuild claim.

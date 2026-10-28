# Classic PSRAM: standalone ESP-IDF 4.4.7 allocator fixture (P4)

Current integrated base: upstream main `77e43c20a70054ac84d1724d757fa9ce12668151`.
Memory-miss implementation: `9823ed9a978c0fc798ffebc322662624566758f5`.
Current qualification: [integrated receipt](../../evidence/classic-psram-followup/README.md).
This fixture qualifies a deliberately standalone, unicore normal-cache profile,
not stock Arduino, dual-core low/high cache addressing, a camera, or WASM.
No emulator implementation, guest SDK function or allocator symbol is patched.
The harness only chooses host presence before ROM boot and loads flash inputs.

## Inputs and build

ESP-IDF v4.4.7, 38eeba213aa695aabfd6d89aa9f5078dbe5a94c3; GCC 8.4.0,
crosstool-NG esp-2021r2-patch5. The multi-platform official container is pinned
by immutable index digest in recipe.sh. Actual qualification uses linux/arm64.
An x86-64 host can select IDF_PLATFORM=linux/amd64; that combination is not
qualified here. Emulating that old x86 compiler on ARM produced compiler
segmentation faults, so the real ARM toolchain was used instead.

    bash docs/fixtures/classic-psram-idf/recipe.sh /absolute/new/output

The output directory must not exist. The recipe runs entirely inside it, with
four build jobs and nice priority. SOURCE_DATE_EPOCH=1609459200 and
CONFIG_APP_COMPILE_TIME_DATE=n eliminate build timestamps. Fixed container paths
make debug paths stable. Two clean builds in distinct host directories produced
identical bootloader, partition and app bytes. Provenance includes the exact
SDK configuration hash, contributed archive hashes and linked guest symbols.
The generated sdkconfig is retained beside provenance.json.

Committed CI inputs: web/wasm/fw/public/classic-psram-idf-{bootloader,ptable,app}.bin;
NOTICE and exact SDK licence texts accompany them. The mask ROM is fetched by
tools/fetch-demo-assets.sh --no-linux, not redistributed by this fixture.

## Profiles

Executed: BOOT_INIT=y, MEMTEST=y, capability-only allocator, unicore,
80 MHz flash/80 MHz RAM, explicit 32-Mbit device, no himem reservation,
no external BSS/noinit. This is the normal single-core cache mode that P3
qualifies. It does not hide a substitution for stock Arduino's dual-core SDK.

Manual profile (identified, NOT executed): BOOT_INIT=n. After startup and before
any external allocation, call the unmodified esp_spiram_init(),
esp_spiram_init_cache(), esp_spiram_test(), esp_spiram_add_to_heapalloc() in that
order, checking the result of each non-void API. Startup then does not perform
boot registration. Arduino 2.0.17's genuine manual sequence is a separate P5
qualification. Historically, its stock low/high cache mode hit the normal-only
P3 unsupported-mode gate; that negative is not the current integrated result.
See the current [low/high qualification](../../evidence/classic-psram-lowhigh/README.md).
The initial 40/40 profile also historically rejected the ID and is not called
passing by this 80/80 fixture. Current 40 MHz delay-clock startup is qualified
separately by the [DCLK/MicroPython contract](../../evidence/classic-psram-dclk/README.md);
it does not retroactively turn the earlier failed profile into a pass.

## Runtime contract

    ESP32SIM_ROM_DIR="$PWD/web/wasm/fw" CARGO_TARGET_DIR=TARGET \
      nice -n 10 cargo +1.99.0 test --release -p esp32sim \
      --test classic_psram_idf -- --ignored --nocapture

The CI tests require only the fetched ECO3 ROM, with no locally installed SDK.
They run ROM -> bootloader -> unmodified SDK startup -> app_main. The host never
writes PSRAM, cache controls or MMU mappings. The test checks all 128 PRO table
entries; held APP's table remains invalid. A repeated fresh-machine run must
have byte-identical serial. The same firmware with no PSRAM must reject the ID
and abort before memory testing, allocator registration or app_main.

Passing guest milestones:

- SDK PSRAM initialization and its own stride-32-byte memory test pass.
- Physical device 4194304 bytes; usable configured mapping 4194304 bytes.
- Registered SPIRAM heap 4194303 bytes, free 4192135 bytes before allocation.
- 1048576-byte SPIRAM|8BIT allocation at 0x3f80087c. Every byte is written/read;
  the allocation touches 33 distinct 32-KiB pages. Pattern is (i*13)^(i>>15),
  truncated to a byte; zero readback errors, FNV-1a 678f9dc5.
- SPIRAM|DMA is rejected, a plain DMA request is internal, and a 5-MiB external
  request is rejected. All successful allocations are freed.

The one-byte heap difference is genuine IDF accounting, NOT a device-size fix:
v4.4.7 components/esp_hw_support/port/esp32/spiram.c:207-209 passes
LOW + usable - 1 as the region end. components/heap/heap_caps_init.c:215-218
stores that end and registers end-start bytes; heap_caps.c:505-514 reports
end-start. No Arduino getter or SDK range is changed. Free space is smaller
again due to allocator metadata. These are three distinct measurements.

The host budget is 300 million scheduling units in 100000-unit chunks, stopping
on the guest result or reboot request. This is a bounded correctness test, not
a performance measurement. No idle/tick path or existing golden is changed.

[Full checks, semantic mutations and limits](../../evidence/classic-psram-idf/README.md).
Run `python3 docs/fixtures/classic-psram-idf/validate.py` to verify the committed
recipe source, effective SDK configuration and firmware input hashes.

## CPU comparison

PENDING. No timing performed. This commit adds only tests and fixture inputs.

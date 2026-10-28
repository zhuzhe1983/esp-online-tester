# Arduino-ESP32 3.1.1 PSRAM compatibility

This is a bounded classic ESP32-CAM profile, not a claim for every Arduino 3.x
release, every board or the S3. It reuses the exact public P6 sketch and host
assertions from `../classic-psram-arduino3/`; no guest SDK routines are intercepted,
no emulator behavior or public API is changed, and no alternate unicore SDK is used.

    bash docs/fixtures/classic-psram-arduino311/recipe.sh /absolute/output
    ESP32SIM_ROM_DIR="$PWD/web/wasm/fw" ARDUINO3_FIRMWARE_DIR=/absolute/output/fw-p6 \
      nice -n 10 cargo +1.99.0 test --release -p esp32sim \
      --test classic_psram_arduino3 -- --ignored --nocapture --test-threads=1

The recipe uses an isolated install with an explicitly existing config. Stock
Arduino 3.1.1 selects `idf-release_v5.3-cfea4f7c-v1`, not vanilla IDF v5.3.2.
The actual binary SDK archive is:

https://github.com/espressif/esp32-arduino-lib-builder/releases/download/idf-release_v5.3/esp32-arduino-libs-idf-release_v5.3-cfea4f7c-v1.zip

SHA-256: `1099291229a9be453c771c5867b8487a0266db9246b672417337b8f511d7f820`
(341118284 bytes). This was checked against the actual installed download.
`provenance.py` retains the effective config, linked archives, symbols and firmware
hashes; its archive hash is an expected identifier, not a substitute for hashing
the downloaded archive.

Observed guest version: `v5.3.2-282-gcfea4f7c98-dirty`. Stock BOOT_INIT is off,
UNICORE is off, PSRAM is 80 MHz, and heap poisoning is light. The same P6 contract
passes unchanged: 4194304-byte registered heap, 4192124-byte free payload, two
1048576-byte allocations at 0x3f800884, readback errors zero, FNV-1a 20c19dc5 and
9dce9dc5, with both frees restoring 4192124 bytes. Duplicate SDK init returns 259.
The real init/memory-test/heap-add symbols execute; passive call counts are
[2,2,1,1,1]. A repeated fresh boot produces identical serial.

With no host PSRAM the ID is ffffffff, getters and external heap/free are zero,
large allocation is unavailable, Arduino init/add retries return false, and call
counts are [2,1,0,2,0]. No internal-RAM fallback is accepted.

The pre-existing A30 test comments cite IDF 5.1.4. A31 retains the same checked
classic low/high cache bits, 128 PRO/APP table mappings, and stride-32 memory-test
contract. This receipt does not relabel A30 source citations as IDF 5.3 evidence.
The scope report's vanilla I53 comparison is source-only: no locally executed
vanilla-I53 baseline exists in this accepted stack. Runtime equivalence with that
unavailable baseline is not claimed; the unchanged P6 contract is the comparator.

No firmware binaries are added to CI by this extension. These remain explicitly
external tests with a public, pinned build recipe. No CPU table was measured.

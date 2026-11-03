# Physical-flash security preflight

`assertFlashSecurity(loader, chip)` is a read-only guard to call after chip detection and before any Flash erase/write. It does not authorize a port, replace product/image validation, write eFuses, or enable encrypted flashing. Tests use loaders and register responses supplied by the test; no hardware validation is claimed.

## Classic ESP32

Classic ESP32 does not provide the security-info command used by newer chips. The guard reads only `EFUSE_BLK0_RDATA0_REG` (`0x3ff5a000`) and `EFUSE_BLK0_RDATA6_REG` (`0x3ff5a018`):

| Read word | Bits | Meaning and refusal condition |
| --- | --- | --- |
| 0 | 26:20 | `FLASH_CRYPT_CNT`: refuse any nonzero value |
| 0 | 27 | `UART_DOWNLOAD_DIS`: refuse when set |
| 6 | 4, 5 | `ABS_DONE_0`, `ABS_DONE_1`: refuse either secure-boot bit |
| 6 | 7, 8, 9 | `DISABLE_DL_ENCRYPT`, `DISABLE_DL_DECRYPT`, `DISABLE_DL_CACHE`: refuse any download restriction |

The counter policy deliberately refuses even-popcount development states, although active Flash encryption is defined by an odd number of set bits. Secure Boot V2 and UART download disable apply to revision 3 and newer; the guard conservatively refuses those bits even without obtaining a revision. Other eFuse fields, such as JTAG disable alone, do not imply one of these refusal conditions. No encryption keys or device identifiers are read.

Definitions checked on 2026-11-03 against these pinned primary sources:

- [ESP-IDF v4.4.8 ESP32 register header](https://github.com/espressif/esp-idf/blob/v4.4.8/components/soc/esp32/include/soc/efuse_reg.h): read words, encryption counter, secure-boot and UART encryption/decryption/cache masks.
- [ESP-IDF v4.4.8 ESP32 eFuse table](https://github.com/espressif/esp-idf/blob/v4.4.8/components/efuse/esp32/esp_efuse_table.csv): names and meanings, including `UART_DOWNLOAD_DIS` at bit 27. The older generated register header omits this named bit; the table includes it.
- [ESP-IDF v5.5.2 ESP32 register header](https://github.com/espressif/esp-idf/blob/v5.5.2/components/soc/esp32/register/soc/efuse_reg.h): same offsets/masks; it also names bit 27 explicitly.
- [esptool v5.3.0 ESP32 target](https://github.com/espressif/esptool/blob/v5.3.0/esptool/targets/esp32.py): independently matches the base address, secure-boot masks, encryption counter and manual-download encryption bit. The implementation is not copied from esptool.

The hardware definitions agree across the IDF 4.4 and 5.5 sources; they do not depend on the firmware being written. `readReg` may return signed int32 values in vendored esptool-js 0.7.0, so the guard validates then normalizes them to unsigned words. Missing values, strings, fractional numbers, out-of-range values and read errors cannot become an implicit zero.

## ESP32-S3, C3 and C6

These targets retain `getSecurityInfo()`. Both `SECURE_BOOT_EN` and `SECURE_DOWNLOAD_ENABLE` must be explicit booleans and `flashCryptCnt` must be an unsigned byte. A set flag or nonzero counter is refused; malformed/missing fields are refused separately. Raw flags, when supplied, must agree with the two parsed flags. This follows the existing ordinary-unsecured-device-only policy rather than attempting security-mode workarounds.

Run `node --test tools/platform-flash-security-test.mjs` for all 127 classic encryption states, individual protection bits, signed register transport values, malformed/failed reads, modern reply validation and chip isolation. Integration still needs to run this guard before Flash writes and retain cancellation/connection cleanup.

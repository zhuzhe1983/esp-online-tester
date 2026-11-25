# C5 MP Flash image validation

This is host-side, read-only image validation. It neither executes a guest nor establishes factory firmware compatibility. The public C5 ROM gate is unchanged. Original third-party binaries remain caller-supplied and are not committed.

`esp32c5::image::parse` retains the strict SRAM app-direct contract: only HP/LP SRAM, an entry inside a loaded segment, zero checksum padding, checked XOR/SHA footer and an erased tail. The only change to its existing helper is the `pub(crate)` SHA visibility required by the pinned MP ROM loader.

The additional interfaces are:

- `parse_flash(data) -> ValidatedImage { image, image_bytes }`: one C5 chip23/revision100 ESP image, supporting the C5 HP/LP and shared mapped-I/D windows, documented non-loaded address0/address4 records, zero-length records, checked bounds/overlap/entry, XOR and optional simple SHA. The suffix after the footer must be erased FF. Standalone parsing does not guess its Flash placement.
- `validate_merged(data, flash_bytes, partition_table_offset) -> FlashLayout { bootloader_offset, partition_table_offset, apps }`: primary bootloader at0x2000; a caller-supplied, sector-aligned table offset at0x8000orabove; bounded32-byte records, optional verified MD5 and required terminator; physical partition bounds/alignment/overlap and unique app subtypes; complete populated applications at their declared offsets. Missing or erased OTA slots are allowed, and at least one populated app is required. `apps` lists populated images without choosing an active OTA slot.

All validation precedes caller mutation. Single-image tail checks and merged image-prefix checks are separate: the app at0x20000 is not treated as data following a bootloader's single-image footer, and data partitions do not need to appear in a shortened merged BIN. Caller restore pages remain raw storage operations, outside these image APIs.

## Sources and format decisions

All ESP-IDF sources use **v5.5.3 commit `2c211b236707889e8400c4dc5644dd5c4ee071e0`**, matching the LCD2.73 original image. [sources.json](sources.json) records each public URL, final URL, fetch time, byte count, content type and SHA-256.

| Rule | Primary source |
| --- | --- |
| C5 chip23; 24-byte image /8-byte segment headers; full revision fields; simple hash footer | [esp_app_format.h](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/bootloader_support/include/esp_app_format.h), [bootloader_common_loader.c](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/bootloader_support/src/bootloader_common_loader.c) |
| Checksum padding is hashed but need not be zero; addresses0/4 are non-loaded; mapped physical/virtual offsets must agree | [esp_image_format.c](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/bootloader_support/src/esp_image_format.c), `process_checksum`, `should_load`, `verify_segment_header` |
| HP384KiB, LP16KiB, mapped42000000..44000000, fixed64KiB page contract | [C5 soc.h](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/include/soc/soc.h), [ext_mem_defs.h](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/include/soc/ext_mem_defs.h) |
| C5 supports32-bit Flash mapping, so the legacy16MiB segment cap does not apply | [C5 soc_caps.h](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/include/soc/soc_caps.h), `SOC_SPI_MEM_SUPPORT_CACHE_32BIT_ADDR_MAP`; `esp_image_format.c` |
| Primary bootloader0x2000; configured table offset | [bootloader Kconfig](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/bootloader/Kconfig.projbuild), [partition-table Kconfig](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/partition_table/Kconfig.projbuild) |
| Partition magic50AA /MD5 EBEB, 32-byte entries,0xC00 maximum including terminator, checksum at+16 | [esp_flash_partitions.h](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/bootloader_support/include/esp_flash_partitions.h), [flash_partitions.c](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/bootloader_support/src/flash_partitions.c) |
| App offset64KiB, other offset4KiB, ordinary unsigned app size4KiB alignment | [gen_esp32part.py](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/partition_table/gen_esp32part.py) |
| Private cold MD5 helper, checked against independent vectors | [RFC1321 section3](https://www.rfc-editor.org/rfc/rfc1321.txt) |

The new Flash parser follows the loader's nonzero checksum-padding rule; SRAM parsing deliberately keeps its existing stricter rule. The merged host contract additionally rejects records after a table MD5, so no later partition escapes that checksum, and rejects partition records in the reserved primary bootloader/table area. This is not recovery/OTA-bootloader/table support. Data/custom partition payloads remain opaque. The 32 MiB cap is the current C5 runtime's mapped/backing capacity; supplied physical capacity can be smaller. Secure-boot signatures, encryption, eFuse policy, rollback policy, boot-time stack/data reservations, actual MMU programming, selected OTA image and peripheral execution are outside this validation result.

## Verification

Toolchain: Rust stable1.100.0 (`0ef1bc386`,2026-11-09); `rustup check` reported it current. Builds use one job and the isolated `target/c5-images-lane` directory. No firmware build, browser run, CPU benchmark, golden update or broad workspace test was performed.

The focused unit cases cover valid SRAM/LP/mapped/padding combinations, unchanged SRAM rejection boundaries, nonzero checksum padding included in SHA, truncation/corrupt checksum/digest/chip/revision/header, address overflow/overlap/entry/alignment, a mapped payload above16MiB, app0x20000 and0x30000, configured table offsets, empty/populated OTA slots, MD5/terminator/alignment/capacity/partition corruption and physical mapped offsets. The original-image case is named `external_*` and ignored by default; it fails with a message naming `ESP32SIM_C5_LCD273_FACTORY` when that required input is absent.

Commands and final results are recorded in [checks.json](checks.json). They validate structure and integrity only; the external check does not run the original firmware.

The caller-supplied manufacturer artifact is [FactoryOnly_20260724.bin](https://raw.githubusercontent.com/waveshareteam/ESP32-C5-LCD-2.73/418de818f4f40c5a04c4292dbe293bfaa1351068/firmware/ESP32-C5-LCD-2.73_FactoryOnly_20260724.bin), 3,346,992 bytes, SHA-256 `cf10a4358d03898458c16390f3f16ceb0306befe2260160de0919d755b920942`. Expected layout: bootloader0x2000, table0x8000, populated factory app0x20000/entry408002b0, image length311230hex, declared factory partition size4e2000hex, and unprovided OTA0x510000. It validates as Flash while remaining rejected by SRAM app-direct.

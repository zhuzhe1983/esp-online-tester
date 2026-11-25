# C5 eFuse digital read controller

This block adds the C5 digital read/reload path required by the real MP rev100 ROM. It is independent of the shared C3/C6 eFuse model: C5 has its controller at `0x600b4800`, CMD at offset`0x1d8`, and its revision fields at`0x4c`. This isolated change does not wire the bus or establish a complete ROM/factory boot.

All register values use ESP-IDF **v5.5.4**, commit `735507283d5b2f9fb363a1901172dbd9e847945d`, matching the factory image's IDF version. Primary sources:

- [C5 register definitions](https://github.com/espressif/esp-idf/blob/735507283d5b2f9fb363a1901172dbd9e847945d/components/soc/esp32c5/register/soc/efuse_reg.h), including controller reset values, field masks, read-only/SC/WT access types.
- [C5 logical fuse CSV](https://github.com/espressif/esp-idf/blob/735507283d5b2f9fb363a1901172dbd9e847945d/components/efuse/esp32c5/esp_efuse_table.csv), for revision, MAC, crystal and security fields.
- [C5 low-level driver](https://github.com/espressif/esp-idf/blob/735507283d5b2f9fb363a1901172dbd9e847945d/components/hal/esp32c5/include/hal/efuse_ll.h) and [read-operation HAL](https://github.com/espressif/esp-idf/blob/735507283d5b2f9fb363a1901172dbd9e847945d/components/hal/esp32c5/efuse_hal.c).
- [C5 TRM v1.1](https://documentation.espressif.com/esp32-c5_technical_reference_manual_en.pdf), §7.3.1.2 read protection, §7.3.3 and Table7.3-4, printed pp260–261: bank layout, reset reload, manual read, stable shadows, error reports and completion.

The logical profile is named `virtual_mp_rev100_48mhz_profile`. It has wafer major1/minor0, an odd-parity 48MHz SPI-boot crystal selection and a deterministic locally administered synthetic MAC. It does not copy an actual chip identity. Revision-disable bits are clear. Flash/PSRAM capacity/vendor, calibration, secure boot, encryption, key purposes and user/key data are zero/unprovisioned. These zero fields must not be described as a physical HF4 fuse dump or as evidence of peripheral/security support; board flash/PSRAM capacity remains a separate configuration.

OTP data is immutable decoded logical data, with valid coding as the profile's input contract. No programming API, raw parity bytes, analog calibration, error injection or RS correction implementation is exposed. Accordingly the read-error reports are zero for this valid logical profile; the model does not claim to diagnose or repair corrupt physical codewords.

The constructor performs the documented reset preload into separate read shadows. A manual read requires CONF=`0x5aa5`, then READ_CMD bit0 at`0x1d8`. It copies every bank, applies each RD_DIS bit to its corresponding block4–10, and only then clears the command and sets sticky READ_DONE. CMD's independent block selector is R/W. A write with no read trigger does not complete a read. READ_DONE is masked by INT_ENA and cleared only by its write-one trigger in INT_CLR; no PGM_DONE is synthesized. `irq()` exposes the level to the C5 bus owner (source14 in the same-version interrupt table).

| Offsets | Supported behavior |
| --- | --- |
| `0x2c..0x178`, aligned | Read-only shadows: block0/1 six words each; blocks2–10 eight words each. |
| `0x17c..0x194` | Read-only error reports for the valid logical profile. |
| `0x198` | DATE reset37818960, writable low28 bits. |
| `0x1c8` | Clock-control readback; memory power-down rejects. No clock/analog timing model. |
| `0x1cc`, `0x1d0` | Read/reset opcode; ECDSA selectors must remain zero. Other operation/key modes reject. |
| `0x1d4`, `0x1d8` | Idle after the completed functional read; exact command path above. |
| `0x1dc..0x1e8` | RAW/ST/ENA/CLR interrupt semantics, including read-only status and WT clear. |
| `0x1ec..0x1fc` | DAC and read/program timing configuration with C5 reset values/masks; UPDATE self-clears synchronously. RS bypass rejects. |

The read finishes synchronously, with no busy interval visible to firmware. It performs real logical data movement before reporting completion; no physical APB, voltage, programming, sleep or clock-domain latency is claimed. Timing register readback configures the functional block without enabling burns.

PGM commands, write opcode`0x5a5a`, PGM staging offsets, APB2OTP/direct-code access, key-manager/ECDSA selection, RS bypass, forced SRAM power-down, invalid blocks, reserved command bits, unknown offsets and unaligned accesses reject. Rejection happens before the command changes shadows or completion state. Writes to modeled read-only banks/status do not modify them.

Six focused tests cover exact profile and reset data, every bank's reload, snapshot stability, read-only preservation, all seven read-protection bits, sticky/masked/WT completion, timing masks and atomic rejection. Their final results and the source hashes are in [checks.json](checks.json). Initial direct Clippy reported two test-only range loops; both were corrected. No full workspace check, bus IRQ-delivery test, ROM execution or performance benchmark was run in this isolated lane.

Reproduction from a checkout:

```sh
CARGO_TARGET_DIR=target/c5-efuse-lane CARGO_BUILD_JOBS=1 cargo test -p esp32c5 --lib efuse::tests
clippy-driver --edition=2021 --test esp32c5/src/efuse.rs -D warnings -o target/c5-efuse-lane/efuse-lint-tests
```

The standalone Clippy invocation checks the owned module/test harness. Full-crate linting is an integration gate after the private module is wired into C5Bus; no dead-code allowance is added to production code. Source citations and a synthetic identity are retained; private paths and raw tool output are omitted from committed evidence.

# C5 XMC SFDP read window

The supplied unchanged LCD273 run stopped at `4080f804`, SPI1 USR START to `60003000=00040000`, after26,951,310 instructions/26,951,296 nominal cycles and3415 UART bytes. It had not reached app_main or LCD output. The next read-only MSPI probe resolves the actual transaction: CS0 flash, SPI STR Mode0, opcode5a, address000032 with24 address bits,8 dummy clocks,1 MISO byte and no MOSI, at80MHz. Two earlier9f requests at the same PC completed and returned JEDEC bytes20/40/19. The previous whitelist rejected5a. [Checks](checks.json) retain both native capture hashes and a pure pre-fix regression that independently reproduces the rejection.

This is the next CORE `init_flash` after brownout. The original wrapper at4200084a calls4206f802 then4206f632 and checks the return. The original `spi_flash_chip_generic_get_caps` at40817632 has the manufacturer20 branch, constructs address32 at4081766e/72 and command5a at4081767c/80, calls `common_command` at40817690, then tests the returned bit3 at40817692..9a. Fixed IDF5.5.3/5.5.4 source is identical: that bit distinguishes some XMC-D parts and enables suspend/32-bit-address capability flags. The original sdkconfig has AUTO_SUSPEND and FORCE_ENABLE_XMC_C_SUSPEND off. This read is a digital capability query, not a calibration command.

The current virtual reply20/40/19 and32MiB capacity match the compatible XM25QH256C reference already used for idle Resume. Its fixed vendor Rev2.1 datasheet §7.2.44, printed pages85/86, explicitly publishes these two DWORDs as Advanced Information:

| SFDP byte address | 30 | 31 | 32 | 33 | 34 | 35 | 36 | 37 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Published value | e5 | 20 | f3 | ff | ff | ff | ff | 0f |

DWORD1 is fff320e5; density is0fffffff bits-minus-one, or32MiB. At byte32, bit3/DTR is0. Returningf3 therefore preserves the C-series branch; it does not impersonate an XMC-D device or force a capability check to succeed. This virtual-compatible identity is not a claim about the physical Flash manufacturer inside the Waveshare device. [Sources](sources.json) pin the vendor PDF, exact SDK files, original firmware/config and typed instruction listings.

Only `mspi.rs` adds a read-only SFDP window30..37. A request must fit completely within those eight published bytes and match the current derived JEDEC20/40/19 and exact32MiB array. Existing clock/gate/CS/STR validators apply. This subset additionally requires the actual Mode0 USER shape, CS0 only, command8/address24/dummy8, byte MISO in the low buffer, serial frequency no higher than80MHz, active power and no BUSY/SUS/pending operation. Other ranges, capacities and unqualified phases/states reject before data or end events. SPI0, NAND, calibration, QPI/DTR Flash, held-CS and unknown command engines are not added. Existing CS1/no-CS routing remains intact; an unselected flash never leaks SFDP data.

A valid read copies only requested bytes into the controller RX buffer and raises the existing USR end bits after that copy. It preserves untouched RX tail bytes, Flash/PSRAM arrays, status, WEL, QE/protection bits, RD_STATUS latch and MMU. No status/program/erase command is added or requalified. A delivered valid SFDP command cancels Reset Enable according to the vendor reset sequence; rejected requests and no-CS transactions do not. Deep-power-down, BUSY or suspended SFDP behavior remains Unsupported in this bounded subset. The model remains synchronous and makes no physical SPI timing or new write-protection accuracy claim.

The independent harness imports the actual `mspi.rs`, `mspi_regs.rs` and `quad_psram.rs`, linking a caller-supplied prebuilt `esp_periph` artifact. Compile references retain four APIs used by Bus/Soc, without executing them or adding lint allowances. It runs21 cases: all17 existing owner tests unchanged and4 new cases covering the original probe, published bytes/multi-byte ranges, preserved state/tail,26 rejection combinations plus wrong capacity, and delivered-command Reset Enable behavior. All21 pass with no ignored/filtered cases; `clippy-driver -D warnings` passes. No Cargo/dependency build, emulator/native guest, firmware or WASM build, benchmark, browser action or shared Bus/Soc edit is part of this change. Actual factory continuation and eventual LCD qualification remain separate integration work.

Reproduce with stable Rust and dependency search directories matching the supplied prebuilt artifact:

```sh
rustc +stable --edition=2021 --test tools/c5-mspi-sfdp-harness.rs \
  --extern esp_periph="$ESP_PERIPH_RLIB" -L dependency="$ESP_RLIB_DIR" \
  -o "$C5_SFDP_TEST_BIN"
"$C5_SFDP_TEST_BIN" --include-ignored --skip external_ --test-threads=1
```

Repeat `-L dependency=...` for transitive artifact directories. Invoke the same test source and artifact arguments with the stable `clippy-driver`, adding `-D warnings`. The receipt records the exact linked artifact hash and toolchain; raw paths, complete UART/peripheral streams, original binary and full disassemblies remain ignored.

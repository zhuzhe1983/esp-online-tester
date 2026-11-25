# C5 bootloader analog startup controls

This private module implements the original LCD273 bootloader's first analog setup, after the actual rev100 ROM loaded its three SRAM segments and jumped to `0x4084bbaa`. The measured baseline is 422509 instructions, 229 UART bytes, and a read fault at `0x4084bc3a` / PCR `0x60096174`. Application entry, LCD pixels and SD commands are still absent. This module receipt does not claim a new full-ROM result before bus integration.

## Sources and observed instructions

The firmware identifies IDF v5.5.3, commit `2c211b236707889e8400c4dc5644dd5c4ee071e0`. The fourteen compared headers and driver/bootloader sources also match v5.5.4, commit `735507283d5b2f9fb363a1901172dbd9e847945d`, byte for byte. Hashes and fixed URLs are in [checks.json](checks.json).

- [C5 bootloader hardware initialization](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/bootloader_support/src/esp32c5/bootloader_esp32c5.c) matches factory instructions `0x4084bc3a..0x4084bc84`: clear PCR bypass bit1, set MODEM clock/force bit2, select clock bit12, clear LP_ANA FIB bit1, enable BOD mode1 bit31.
- [C5 bootloader power-glitch setup](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/bootloader_support/src/esp32c5/bootloader_soc.c), compiled at `0x40850312`, releases/powers the SAR I2C interface, performs three masked byte updates, gives software control of four detectors, then enables those four LP detector controls.
- [C5 HP REGI2C patch driver](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/esp_rom/patches/esp_rom_hp_regi2c_esp32c5.c), matching compiled helpers `0x4084ecec` and `0x4084ee28`, supplies the actual slave/address/data/WR/BUSY encoding and SAR route/master selection. The generated master header gives only coarse control fields; no C6 alias is inferred.
- [C5 TRM v1.1](https://documentation.espressif.com/esp32-c5_technical_reference_manual_en.pdf), sections9.1 and21.3, defines reset scopes and FIB/SW detector permissions. The TRM memory table lists `I2C_ANA_MST` at `0x600b2400`; fixed C5 headers and the original factory code instead use `0x600af800`. Only the witnessed HP address is implemented. The TRM calls the BOD permission the “first bit”; the C5 brownout LL and actual instructions use bit1. The ECO2 header additionally names clock-glitch controls; these remain unsupported.

## API and modeled effects

`C5BootAnalog::virtual_usb3v3()` creates an explicit stable USB3V3 board profile. `read(address)` and `write(address,value)` use absolute addresses. Unsupported access returns `None`/`false`; invalid writes preserve all state.

| Address | Supported state and effect |
| --- | --- |
| `0x60096174` | PCR system-bus reset bypass bit1, reset2; APM/TEE bypass bit0 rejects |
| `0x600af018`, `0x600af01c` | MODEM master clock enable/force bit2, reset0; either supplies the functional engine gate |
| `0x600a9c04` | Master clock select bit12; preserves full documented reset `0x00200002`; no PLL lock/frequency claim |
| `0x600b0158` | PMU SAR interface reset-release26/power27; full reset `0x08000000`; both required for transfer |
| `0x600b2c04` | BOD mode1 enable31, reset0 |
| `0x600b2c08` | Four voltage-glitch detector enables30:27, reset0 |
| `0x600b2c0c` | FIB software permission/force controls1..5; other full-word reset ones preserved |
| `0x600af800`, `0x600af804` | Two synchronous controller command/result words; real selected SAR transfers; BUSY25 is RO |
| `0x600af81c` | Header reset `0x2d` or actual driver SAR route `0x00fffdff`; other target routes reject |
| `0x600af820` | SAR master selection bit11; full header reset4; set selects CTRL0, clear selects CTRL1 |

Only slave `0x69` / register10 is modeled. [SAR definitions](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/include/soc/regi2c_saradc.h) name bits0..3 as the PERIF, VDDPST, XTAL and PLL VDET enables. They do not publish that byte's POR value. This model explicitly configures the virtual byte to `0x0f`, including upper nibble0. It is neither a measured silicon value nor a claimed hardware default. Writes changing the unmodeled upper nibble reject.

Each accepted command really reads or writes that byte. The guest's masked updates produce `0x0f -> 0x0e -> 0x0a -> 0x02`, using three reads and three writes; the host does not preseed the final byte. BUSY clears only after the synchronous target operation. Status reads do not advance or create transfers. No wire timing, contention or calibrated master clock is claimed. `report()` exposes the virtual profile, actual transfer counts and effective detector permissions. The stable supply generates no fabricated brownout/glitch events, interrupt or reset.

`reset_domains(hp,lp,analog)` applies caller-supplied reset domains and returns whether the previous bypass policy requests system-bus reset. CPU reset passes all false; Core resets HP controls/controller; System also resets LP controls; Chip also restores the analog profile. The present modeled bus has no retained arbitration state. Digital resets and interface reset/power gating preserve the virtual analog byte; only analog reset restores it. This is an explicit functional retention contract, not a claim about undocumented internal analog reset latches. Transfer counters persist across digital resets and clear on analog reset. APM/TEE retention and security enforcement are not implemented here.

## Checks and limits

Five focused tests pass, plus standalone Clippy with warnings denied. They cover the exact bootloader sequence, real masked transfers/readback/counters, both master choices, clock/power/reset gates, transactional invalid requests, FIB permissions, stable supply, and reset retention. No allowances, shared engine, bus/soc edit, broad suite or benchmark was added. Parent integration must wire the absolute addresses and run the original ROM again.

Other internal slaves/registers, PLL self-calibration/status, ADC/TSENS conversion, regulator tuning, I2C bursts/timing, power-down of unrelated RF blocks, BOD mode0/IRQs, physical supply transients and clock-glitch detection remain unsupported. Unknown routes and reserved controls fault rather than store arbitrary register RAM. In particular, master `+0x18` is unsupported: no `CAL_DONE` value is supplied to advance PLL startup.

Raw fixed sources, original typed disassembly and the native baseline remain in the ignored common input/research cache. The committed receipt retains public source hashes and aggregate results only; local identities, absolute workspace paths and raw build logs are omitted.

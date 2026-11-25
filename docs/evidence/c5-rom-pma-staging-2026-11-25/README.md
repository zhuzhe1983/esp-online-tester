# C5 ROM PMA address staging

The checked policy's first original-ROM run stopped at `4003ab62`, before UART output: `CSRRW x0,0xbdc,x15`, instruction word `bdc79073`, with `x15=140007ff`. The integration capture records 5,938 instructions, 5,888 nominal cycles, illegal-instruction cause2, no Bus first fault and zero pixels. The pre-write snapshot is epoch1/generation1, all PMA cfg80000000/addr0/POR tags set and all PMP zero. [Checks and capture hashes](checks.json) preserve this negative result. The original capture does not pin the integration source revision, so it establishes this boundary only.

The source at helper commit `845ca6696e61c48b6b7bc590ea8857acc6ff0caf` rejected every nonzero POR address write. The standalone regression reproduces `PorAddressWrite(0)` before this fix. The earlier [helper receipt](../c5-memory-policy-2026-11-25/README.md) and its hashes remain unchanged.

The narrow change accepts a physical30-bit PMA address request while preserving the raw POR cfg, A2 and POR tag. The reset-only128-byte unqualified window moves to `[(addr<<2)&~127, start+128)`. Every intersecting CPU access remains Unsupported. A nonzero staged POR address has Unsupported architectural readback; only the raw diagnostic snapshot exposes the request. A real supported cfg write clears the POR tag and selects OFF/TOR/NAPOT. Newly requested A2 remains rejected transactionally. Locks, guarded Machine context and PMP are unchanged.

This is a bounded reset-only functional contract. TRM1.1 register2.82 gives A reset2/NA4, while section2.9.2.2 excludes NA4 and specifies minimum128-byte granularity. Register2.83 marks the address R/W. The window retains this unresolved condition locally; it does not claim full NA4, WARL or silicon reset behavior. A PMA no-match still requires existing PMP, Bus and APM checks. PMA applies to Machine accesses too.

The pinned rev100 ROM's `riscv_pma_init_cfg` at `4003ab56` statically contains these address-before-cfg pairs. Each cfg is NAPOT/TYPE1 with LOCK0; each pair is followed by14 calls that compare the precise address and cfg readbacks, resetting on mismatch.

| Entry | Address write PC / word | Raw address | Cfg write PC / word | Cfg | Explicit NAPOT interval / permission |
| --- | --- | --- | --- | --- | --- |
| 12 | 4003ab62 / bdc79073 | 140007ff | 4003ab6e / bcc49073 | c0000019 | 50000000..50004000 / RW |
| 13 | 4003abb0 / bdd79073 | 1000ffff | 4003abb8 / bcd79073 | c0000015 | 40000000..40080000 / RX |
| 14 | 4003abfa / bde79073 | 1020ffff | 4003abfe / bce49073 | c0000019 | 40800000..40880000 / RW |
| 15 | 4003ac40 / bdf79073 | 10bfffff | 4003ac46 / bcf41073 | c0000011 | 42000000..44000000 / R |

Address1000ffff while A2 has the unqualified window4003ff80..40040000. The later A3 cfg produces the ROM-wide interval. The staging request alone does not select that later interval. Tests query the subsequent ROM instruction PC under the intermediate policy and separately assert that its unqualified window remains blocked. Physical ROM/SRAM bounds still belong to Bus even after the coarse explicit descriptor permits an access.

The static fixture also retains `riscv_pma_flash_boot_cfg` at4003ac88 (entry14 RWX, entry15 RX), `riscv_pma_direct_boot_cfg` at4003ad26 (entry15 RX), `riscv_pma_ram_boot_cfg` at4003ad3c (entries12/14 RWX), and `riscv_pma_deep_sleep_wakeup_x_enable` at4003ad62 (entry12 RWX). All20 PMA write instructions found in the pinned `.text` listing are represented. These are request fixtures; no claim is made that any of these later instructions executed in the negative capture. Their unlocked configuration can be replaced by the original app's64 PMA writes, and later ROM requests cannot unlock the app's sticky locks without core reset.

[Sources](sources.json) pin the ROM ELF/listing and reuse the IDF5.5.3/5.5.4/TRM source receipt. The ordinary tests require no local ROM or firmware files. Independent release tests pass24 cases, including all22 canonical contract tests under `--include-ignored --skip external_` with zero filtered tests. Standalone all-targets Clippy passes with warnings denied; metadata reports one package and zero dependencies. The pure helper/harness were the only Rust build scope. Full shared integration, actual ROM readback observations, app PMA continuation, native/WASM execution and LCD acceptance remain separate checks. No binary, full disassembly, raw capture or personal paths are committed.

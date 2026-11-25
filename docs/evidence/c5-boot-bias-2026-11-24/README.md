# C5 boot bias configuration

The unchanged LCD2.73 bootloader reaches `PC0x4084c990`, reading `PMU_HP_ACTIVE_HP_REGULATOR0` at `0x600b0028`, after its actual SAR/DIG_REG transactions. This receipt audits that boundary and the following four configuration writes. It does not claim that this module has executed in the original firmware, that PLL initialization has run, or that application/LCD output works.

Sources are fixed ESP-IDF v5.5.3 `2c211b236707889e8400c4dc5644dd5c4ee071e0`, compared byte-for-byte with v5.5.4 `735507283d5b2f9fb363a1901172dbd9e847945d`. `sources.json` records both original/final URLs, UTC fetch times, media types, byte counts and SHA-256 values. The C5-specific PMU register/struct headers and original typed guest instructions take precedence over the stale C6 heading in the C5 LL header. No C6 register layout is substituted.

Nine of the ten source pairs are identical. The exception is `pmu_pvt.c`:v5.5.4 adds a50microsecond ROM delay immediately after enabling the PVT timer, while v5.5.3 does not. This lies inside the rejected PVT calibration/handoff path; it does not change the supported PMU target fields or selector interpretation. No v5.5.4 PVT timing is imposed on the original v5.5.3 guest.

The complete named-field reset words are:

| Register | Address | Fixed-header reset word | Supported fields |
| --- | --- | --- | --- |
| HP_ACTIVE_HP_REGULATOR0 | `0x600b0028` | `0xc667f180` | DBIAS target31:27; PMU selector14 stays1 |
| HP_MODEM_HP_REGULATOR0 | `0x600b005c` | `0xc6678000` | DBIAS target31:27 |
| HP_SLEEP_LP_REGULATOR0 | `0x600b009c` | `0xc6600000` | DBIAS target31:27 |

Other sleep, power, driver and reserved fields retain their full header defaults. Changes outside the supported targets/selector and writes to `DIG_DBIAS_INIT` bit3 reject before any state changes. Clearing selector14 requests PVT ownership and also rejects until a real PVT engine exists. RO writes are ignored, preserving the actual selected target codes. Only the three exact word addresses exist in this module; neighboring PMU clocks, analog I2C, power controls and PVT do not alias it.

`rtc_clk_init.c` sets selector14, then writes the HP_ACTIVE, HP_MODEM and LP_ACTIVE targets. The original PCs are `0x4084c998`, `0x4084c9aa`, `0x4084c9ba` and `0x4084c9c6`. In `pmu_param.c`, `get_act_hp_dbias` and `get_act_lp_dbias` default to28. They use a nonzero eFuse calibration only for supported chip/block revisions, add19 and clamp to31. The existing unprovisioned virtual eFuse therefore makes the guest choose28; this module never seeds28 or intercepts that guest calculation. Tests replay its actual masked-RMW pattern and independently exercise targets0,3,19,22,28 and31.

RO `HP_DBIAS_VOL[13:9]` and `LP_DBIAS_VOL[8:4]` are bias codes. `pmu_struct.h` marks them valid only in HP_ACTIVE. `pmu_pvt.c` reads them when leaving PVT, stores those codes into the HP_ACTIVE/LP_ACTIVE configuration, then sets selector14 to return ownership to PMU. In the bounded awake HP_ACTIVE/PMU-selected profile, RO codes mirror the actually selected configuration: HP from `+0x28`, LP from `+0x9c`. The LP struct is `lp_sys[PMU_MODE_LP_ACTIVE]` even though the register is historically named HP_SLEEP_LP_REGULATOR0. MODEM's `+0x5c` target remains stored inactive configuration and cannot change the active RO HP code.

This mirror is an explicit functional configuration policy. It does not model internal regulator voltage, PVT measurement, analog settling, readiness or voltage-dependent faults. The stable external USB3V3 profile supplies no calibrated internal voltage. Header reset code24 is preserved as a reset configuration, and RO codes change when the guest writes different targets; they are never forced to a permanently expected24.

TRM1.1 register13.9 reserves most bias fields, showing only the HP regulator XPD reset, while the fixed C5 IDF header names those bias fields with explicit defaults. Register13.31 also reserves named bias fields retained by IDF. This disagreement is preserved; the header-backed values are not claimed to be a measured silicon POR. The public sources do not specify precise live-code propagation/settling latency or a per-bias-register reset-retention map. The declared functional reset contract preserves CPU/Core resets and restores defaults only for an explicit LP or analog reset; this is not a claim of independently verified silicon retention.

Checks use one build job and an isolated task target directory:

- `cargo test -p esp32c5 --lib boot_bias::tests`:6 passed, covering complete reset words, actual guest RMWs, independent active/inactive targets, RO write resistance, transactional rejection, reset domains and exact-address boundaries.
- Focused Clippy uses warnings denied except `dead_code` for parallel production APIs awaiting bus integration. No source allowance conceals the missing wiring; final integration must run warnings-denied Clippy without the exemption.
- Source pairs were fetched and hashed; evidence text and JSON were manually checked for personal information. Privacy/diff checks are recorded with the delivery result.

Bus routing, reset dispatch, firmware execution and final integration checks are separate integration work.

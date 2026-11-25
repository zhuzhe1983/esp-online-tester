# C5 APM register automatic clock modes

This change adds six exact APM/TEE register-clock configuration words to the existing `BootApm` owner. Bit0=0 enables automatic clock gating; bit0=1 keeps the register clock always on. Both values preserve the existing permission/filter/world decisions. They do not disable protection, supply a new CPU world, stop MMU/PMA checking or create an APM security engine. No Bus/Soc/lib/BootClock/shared code, firmware byte or host bootstrap grant changes.

The factory uses IDF v5.5.3 `2c211b236707889e8400c4dc5644dd5c4ee071e0`; the six headers and C5 APM LL are byte-identical in v5.5.4 `735507283d5b2f9fb363a1901172dbd9e847945d`. [sources.json](sources.json) retains the fixed public URLs, original capture metadata and verified hashes. The source declarations set the complete named-field POR to1. These are pinned SDK defaults, not measured silicon readbacks.

| Absolute address | Fixed C5 register | Header line | Functional reset owner | Original read/store PCs |
| --- | --- | --- | --- | --- |
| `0x60098ff8` | [TEE_CLOCK_GATE](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/register/soc/tee_reg.h#L3523) | 3523 | HP | `0x42000f1c/0x42000f20` |
| `0x600b34f8` | [LP_TEE_CLOCK_GATE](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/register/soc/lp_tee_reg.h#L1146) | 1146 | LP | `0x42000f2a/0x42000f2e` |
| `0x600997f8` | [HP_APM_CLOCK_GATE](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/register/soc/hp_apm_reg.h#L2086) | 2086 | HP | `0x42000f38/0x42000f3c` |
| `0x6009a7f8` | [CPU_APM_CLOCK_GATE](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/register/soc/cpu_apm_reg.h#L1250) | 1250 | HP | `0x42000f46/0x42000f4a` |
| `0x600b38ec` | [LP_APM_CLOCK_GATE](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/register/soc/lp_apm_reg.h#L1022) | 1022 | LP | `0x42001072/0x42001076` |
| `0x600998dc` | [LP_APM0_CLOCK_GATE](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/register/soc/lp_apm0_reg.h#L942) | 942 | HP bridge | `0x42001080/0x42001084` |

The [fixed APM LL](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/esp32c5/include/hal/apm_ll.h#L97) clears the named bit to enable automatic gating and sets it to force always-on, independently of its filter helpers. The corresponding six calls occur in [C5 clk.c](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/esp_system/port/soc/esp32c5/clk.c#L270). Original typed application instructions at the listed PCs load the actual word, AND with -2 and store it back. [checks.json](checks.json) records the typed source hash and exact addresses. The module tests replay these six read/CLR0/write operations; they do not claim the CPU or unchanged factory has executed the tail after integration.

The decoder now owns exactly26 words: the previous20 filter/world targets plus these6 clock words, with no address-range fallback. `contains`, `read(address,cpu_machine)` and `write(address,value,cpu_machine)` keep their interfaces. All accesses still require the caller's actual constrained machine context. A non-machine caller, unknown address or clock value with reserved bits set rejects before any state/diagnostic mutation. Other APM fields still accept only their original literal-zero teardown paths. No default Machine=true, saved-MPP inference or host-assisted initialization is introduced.

`all_filters_disabled`, `all_masters_tee_selected`, `master_tee_selected` and `access_supported` remain independent of clock modes. The existing original filter/TEE counters retain their `4/15/1` meaning. A separate saturating `clock_writes` counts accepted clock configuration writes, including repeated values. Reports expose all six mode latches in table order and the independent count; reads and reports do not capture, grant permissions or create work.

The functional reset contract follows the existing owners: CPU reset preserves all six; HP resets HP_TEE, HP_APM, CPU_APM and the LP_APM0 bridge to1; LP resets LP_TEE and LP_APM to1; Chip resets all and clears diagnostics. HP/LP preserve aggregate write history. LP_APM0 follows the existing `0x600998c4` HP-domain owner despite its LP name. Public sources do not establish every per-flop reset-retention detail, so this is a declared functional subset rather than silicon retention/timing verification. Raw/profile-off disposal remains the parent Bus's existing responsibility.

The original ten module tests retain their behaviors and assertions. One address-enumeration count changes mechanically from18 to21 because three newly owned clock words lie inside that original scan range; the other three are outside. No rejection assertion was removed or changed to conditional success. Five additional tests cover the actual six CLR0 sequence, 0/1 switches in reset/partial/full-teardown states, every CPU/master/DMA support decision, transactionally rejected context/reserved/unknown requests, reset domains, exact26-word ownership and pure repeated reads. [checks.json](checks.json) records the prior/current ten-test hashes and normalized equality apart from that single count.

Only standalone actual-module checks ran:15 tests PASS, Clippy warnings denied PASS, no allowances. Reproduce without Cargo:

```sh
python3 docs/evidence/c5-apm-auto-clock-2026-11-25/run-owned.py \
  --repo "$REPO" --output-dir "$TASK_TMP"
```

No full Cargo, native firmware, WASM, browser or benchmark run is included. No claim of hardware automatic-gating latency, active REE filtering, PMA/CPU transition accuracy, app_main/LCD/buttons/TF or complete factory acceptance follows from these module checks. Parent integration must preserve its strict-ROM and pending-reset boundaries, then run the unchanged original image separately.

# C5 system timer source contract

This private block uses ESP-IDF v5.5.4 at `735507283d5b2f9fb363a1901172dbd9e847945d`, matching the unchanged Waveshare factory image. It implements system-timer clock, counter/snapshot/load, comparator, interrupt and alarm ETM-source behavior; it does not establish complete ROM/factory execution.

Sources read before implementation:

- [C5 register definitions](https://github.com/espressif/esp-idf/blob/735507283d5b2f9fb363a1901172dbd9e847945d/components/soc/esp32c5/register/soc/systimer_reg.h): exact offsets, widths, reset values and access/WT types.
- [C5 PCR definitions](https://github.com/espressif/esp-idf/blob/735507283d5b2f9fb363a1901172dbd9e847945d/components/soc/esp32c5/register/soc/pcr_reg.h): APB gate/reset/ready at`0x6009606c`, functional XTAL/RC_FAST source/gate at`0x60096070`.
- [C5 interrupt table](https://github.com/espressif/esp-idf/blob/735507283d5b2f9fb363a1901172dbd9e847945d/components/soc/esp32c5/include/soc/interrupts.h): evaluated enum sources **61/62/63**. These are not C6's57/58/59.
- [C5 `soc_caps.h`](https://github.com/espressif/esp-idf/blob/735507283d5b2f9fb363a1901172dbd9e847945d/components/soc/esp32c5/include/soc/soc_caps.h), [C5 low-level timer driver](https://github.com/espressif/esp-idf/blob/735507283d5b2f9fb363a1901172dbd9e847945d/components/hal/esp32c5/include/hal/systimer_ll.h) and [generic HAL](https://github.com/espressif/esp-idf/blob/735507283d5b2f9fb363a1901172dbd9e847945d/components/hal/systimer_hal.c): ordinary IDF command sequence, explicit LOAD/UPDATE synchronization, ETM-source enable in HAL initialization.
- [C5 TRM v1.1](https://documentation.espressif.com/esp32-c5_technical_reference_manual_en.pdf), chapter14, printed pp662–669:16MHz average timer clock,52-bit units/targets,26-bit periods, snapshot validity, synchronized loads, missed-alarm compensation, real target readback and level interrupts.

The base is`0x6000a000`. Source-backed reset values include CONF=`0x46000000`, DATE=36774432, no valid snapshots, no alarm interrupts, PCR APB enabled/ready and functional XTAL clock enabled. Both40MHz and48MHz XTAL yield an average16MHz timer count under the chapter14 contract. The model uses one timer tick per15 nominal240MHz CPU cycles; it does not model individual divider-edge jitter or dynamic CPU frequency.

The shared engine was audited before deciding to implement a C5-private controller. Its broadly similar field layout does not satisfy this contract: VALUE_VALID is always set, zero LOAD writes act, staged and active alarm values are shared, REAL_TARGET in period mode is not maintained as the next alarm, and its plain integer overdue comparison does not preserve52-bit rollover. No shared engine/core/SoC changes are included here.

The approved ETM scope is source generation only: alarms increment bounded aggregate diagnostic counters when ETM_EN is set. This allows the HAL's unconditional source enable without claiming an ETM fabric, route or consumer. ETM routing MMIO remains unsupported. Clock-source RC_FAST also remains unsupported and rejects; exact PCR gates and reset are modeled locally.

The stable private integration API is `new(cycle)`, `contains(address)`, `read(address, cycle)`, `write(address, value, cycle)`, `pump(cycle)`, absolute `deadline`, `sources()` and diagnostic `etm_events()`. Addresses are absolute; offsets are not accepted as aliases. The bus should route the system-timer block and the two exact PCR registers to this owner, expose the three IRQ levels and aggregate ETM counts, and pump only on access or a reached deadline. When no comparator is armed, no deadline is published; enabled counters still catch up lazily on access.

Counter LOAD values and comparator target/period values are staged until their respective WT load command is1. A zero trigger write has no effect. Counter UPDATE actually copies the current count into stable HI/LO snapshots before asserting VALUE_VALID; validity starts clear. The model completes these crossing-domain commands synchronously rather than inventing an acknowledgement without copying data. REAL_TARGET is independent of staged values and tracks the next periodic alarm. Mode/unit or work-enable activation starts the configured periodic interval at the current selected counter; changing only the staged period does not affect the running interval before COMP_LOAD.

Comparisons use52-bit modulo arithmetic. Missed-alarm compensation follows the strict `tc-tt < 2^51-1` boundary in TRM Table14.5-2; the exact boundary waits through rollover. This is a source-derived functional interpretation, not a silicon measurement. Advancing a long elapsed interval counts every periodic alarm arithmetically, retains the target grid and raises one sticky IRQ level. Manual counter jumps coalesce already-skipped periods into one immediate comparison and advance the real target to the next future grid point; they do not synthesize ETM events in time that never elapsed. This coalescing and synchronous command completion do not claim cycle-accurate silicon behavior.

PCR APB gating blocks successful timer-register transactions while the functional clock can continue counting. Functional gating freezes count-clock progress and clears the deadline until re-enabled. Holding reset actually clears counters, staged/active values, snapshots and interrupt registers; READY is low while reset is held and high after that reset work has completed. Host ETM observation totals are deliberately retained across peripheral resets. RC_FAST writes reject before changing clock selection. CONF register autogating/force bits do not act as counter enables; the documented UNIT_WORK and PCR functional gate control counting.

The13 focused tests cover reset/snapshot coherence, fractional-clock conservation, staged52-bit loads, one-shot and all three C5 IRQ sources, both counter selections, committed alarm/period values, periodic catch-up, ETM source enable/disable/saturation, W1TC, manual-jump event accounting, wrap/strict-half-range behavior, PCR gates/reset, unknown/unaligned access and zero-period rejection. `checks.json` records source hashes, commands and results. Initial standalone Clippy found the test module before the implementation; it was moved to the required end of the file and the final warnings-denied check passed.

Limits: no dynamic CPU clocks, fractional XTAL-divider edge jitter, RC_FAST calibration, actual CPU/debug stall input, ETM fabric/consumer, voltage/power sequencing or physical crossing-domain latency. Zero-period active engines reject. Absolute deadlines beyond the remaining u64 CPU-cycle range are unrepresentable and omitted; normal modulo counter advancement remains bounded. No shared scheduler, bus or SoC interface is changed by this commit. No full workspace checks, ROM/factory execution or performance benchmarks were run in this isolated lane.

Reproduction:

```sh
CARGO_TARGET_DIR=target/c5-systimer-lane CARGO_BUILD_JOBS=1 cargo test -p esp32c5 --lib systimer::tests -- --nocapture
clippy-driver --edition=2021 --test esp32c5/src/systimer.rs -D warnings -o target/c5-systimer-lane/systimer-lint-tests
```

Standalone Clippy checks the owned module/test harness. Full-crate linting follows bus wiring, since the private module is unused in this isolated branch. Committed evidence retains source facts and aggregate checks and omits private paths/raw compiler logs.

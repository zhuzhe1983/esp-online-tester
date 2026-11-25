# C5 ordinary TG1 interrupt watchdog

The strict ROM profile can now execute the unchanged factory's statically audited TG1 interrupt-watchdog sequence. This receipt is a controller result: it does not establish that the factory has executed this path, started FreeRTOS, reached app_main, or produced LCD output. No firmware bytes, ROM instruction handling, Bus/Soc code or host feed shortcut changes here.

Only `esp32c5/src/timg_wdt.rs` and this evidence directory change. The isolated baseline is `5ae6002bc21b190bd23f01d274094d258931f184`. The previous TG0 cold-flashboot evidence remains unchanged. A later ordinary TG0 task-watchdog path is a separately audited follow-up; this commit still rejects ordinary TG0 enable.

## Fixed sources and actual call contract

[source-pins.json](source-pins.json) records both fixed IDF v5.5.3 `2c211b236707889e8400c4dc5644dd5c4ee071e0` and v5.5.4 `735507283d5b2f9fb363a1901172dbd9e847945d` URLs, bytes and hashes. All used C5 register/LL/HAL/IWDT/tick files are identical between these revisions. The entire RISC-V vectors file differs only in a P4 FPU revision guard; its relevant C5 IRQ/panic branch is unchanged. No whole-file identity is claimed for vectors.S. TRM1.1 sections16.2.2.1–4 define ticks, stage progression and write protection; Table9.1-1 defines simultaneous-reset priority.

The unchanged typed factory calls `esp_int_wdt_init=4206f9b4`. If the guest peripheral refcount is zero, `4206f9da/e2/e8` enables/resets/releases TG1 APB at PCR `60096060`, then `4206f9f6` clears TG1 FLASHBOOT14. BootClock owns this APB/reset word and supplies actual observations; this module owns the separate WDT functional-clock word `60096068`. Clearing general timer gates `60096058/64` or LP pad-clock output does not disable MWDT source edges.

`4206fa18` calls original ROM `wdt_hal_init=40020ff2` with MWDT1 instance2, prescaler24000, interrupt=true. The ROM chooses base60009000, unlocks, disables EN and all four stage actions, clears IRQ bit2 at+7c, enables it at+70, writes reset-length encodings7/7, commits prescaler24000, and locks. Its four stage-off updates, two reset-length updates and prescaler update are seven actual UPDATEs. **This ROM branch has no PCR+68 writes**. The fixed C HAL has extra source/gate writes, but those must not be fabricated as ROM transactions. The parent must supply the actual existing source0/gate1 state.

The next original calls configure stage0 hold10000/actionINT1 and stage1 hold10000/action3, each with a real UPDATE. Together with initialization there are nine updates before EN. `40021322` writes FEED=1; `4002132c` directly sets EN31, with no following UPDATE. The active snapshot is `CONFIG0=b81f8000`, `CONFIG1=5dc00000`, holds10000/10000. EN is therefore a live control; the other configuration fields remain shadowed until UPDATE. Staging actions without committing them and then setting EN rejects rather than activating stale/default actions.

The typed CPU init registers guest tick_hook40802bf6 through42120fbe. Actual SYSTIMER target0/source61 ISR4080c4aa calls xPortSysTickHandler4080c47a, then esp_vApplicationTickHook4080ae52, which calls the real callback table at40839d84. The callback unlocks, writes stage0=(600,INT), UPDATEs, writes stage1=(1200,action3), UPDATEs, writes FEED through ROM40021388, and locks. The controller never performs a host-side periodic feed. Interrupt delivery or a queued tick without execution of these guest writes does not feed it.

An earlier ignored static 38-row list omitted the repeated runtime KEY-unlock row. It remains retained as a static-audit negative result, never a native trace. The new actual-module fixtures include the initial30 writes and all9 runtime writes (KEY, three stage0 operations, three stage1 operations, FEED, lock). Their times are synthetic boundary samples, not observed factory timestamps.

## Executed functional subset

Ordinary operation requires the already explicit strict cold-ROM profile, TG1 source0, observed48MHz or explicitly unavailable None, positive prescaler and stage0/1 holds, actions `[INT, Core, OFF, OFF]`, and FLASHBOOT0. Unknown active clock selectors, conflicting known frequencies, CPU-reset actions, active later stages and zero holds reject. Inactive named configuration retains the previous readable-field contract. No second automatic TG1 flashboot latch is introduced.

The selector1/2 disagreement remains explicit: fixed C5 PCR header/TRM says RC_FAST/PLL80 while fixed mwdt_ll programs PLL80/RC_FAST. This commit executes only the unambiguous selector0 XTAL48M. The LL's legacy “500 ticks/us” and 80MHz comments are not used as arithmetic: 48MHz/24000 is 2000 ticks/s, **500 microseconds per tick**.

The scheduler stays nominal240MHz. With divider phase0 at FEED:

| Stage holds | IRQ from last FEED | Core reset from last FEED |
| --- | --- | --- |
| 10000 /10000 | 1200000000 cycles (5s) | 2400000000 cycles (10s) |
| 600 /1200 | 72000000 cycles (300ms) | 216000000 cycles (900ms) |

The second hold is additional stage1 time. It is not an absolute600ms reset deadline from the last FEED. Nonzero divider phase shortens the first partial tick; FEED resets count/stage and preserves that phase. DIVCNT_RST explicitly clears the divider phase. UPDATE commits shadow fields without feeding; a shorter already-crossed hold triggers at the commit boundary, never at an invented past timestamp. An inactive divider change with a saved fraction rejects until the guest issues DIVCNT_RST, so no fractional progress is silently discarded. These are declared functional phase rules, not measured electrical synchronization.

PCR functional gate22 pauses count and phase; APB gate blocks MMIO while the independent WDT still counts. Explicit source None pauses ordinary TG1 edges and48MHz restoration resumes the saved phase, with old-rate time settled before the boundary. Conflicting known rates fail explicitly. The original cold TG0 None behavior remains its sticky unsupported-clock fault. Unknown clocks never fall back to a readiness flag or nominal XTAL.

TG1 stage0 expiration sets RAW2, enters stage1 and clears stage count. Source60 is the true RAW2&ENA2 level, independent of APB or CPU masking. Late unmask exposes the existing latch. FEED, UPDATE and CPU interrupt acceptance do not clear RAW. ENA+70, RAW+74, ST+78 and CLR+7c are exact shared IRQ words: RAW named write1 clears, ST is read-only, CLR is W1C/read0. General-timer bit0 and reserved writes reject. The sourced initialization writes occur unlocked; locked shared-IRQ write protection is not independently settled, so that extra write path rejects explicitly. Existing locked dedicated configuration/FEED writes retain their documented ignored-write behavior.

Stage1 action3 is SDK `RESET_SYSTEM` but explicitly excludes RTC. On C5 it is **HP/Core reset cause08**. LP and analog domains are retained. CPU-only resets retain TG1 state; HP reset restores both TIMG groups and their declared PCR controls without rearming cold flashboot. Group reset clears only its group and does not reset TG0 RTC calibration. Chip reset creates a fresh selectable cold epoch. Physical reset pulse widths are unmodeled; reset-length encoding7 does not prove a3.2us waveform at48MHz.

## Integration interface and chronology

`take_reset_event() -> Option<(u8,u64)>` returns cause07 (existing cold TG0) or08 (ordinary TG1) with the exact nominal event timestamp. The old `take_core_reset` is now test-only. Production Bus must replace its two old calls and retain the actual cause. `source() -> (usize,bool)` returns `(60, level)`. `deadline()` is the earliest real event across cold TG0 and ordinary TG1, including the stage0 IRQ.

Advancement first limits both engines to their earliest possible reset timestamp, then advances through at most the two reachable ordinary stages. No per-tick loop or elapsed-time queue exists. A pending or consumed reset is a dispatch barrier: late feed, source/gate changes and further pump calls cannot erase it or advance another stage. U128 intermediates and checked timestamp addition bound maximum holds and u64 time; an event beyond the representable timeline has no deadline rather than wrapping.

Parent arbitration must preview actual LP+TIMG controllers, choose earliest time, use TRM priority only at equal times, and commit the real controllers only to that boundary. Within this scope SWD12 > RWDT System10 > RWDT Core09 > cold MWDT0 Core07 > ordinary MWDT1 Core08 > RWDT CPU0d. Applying HP reset at reset_at must retain and then catch up independent LP state. The standalone cross-module fixtures exercise this public interface; they do not replace Bus/Soc integration tests or native proof.

## Validation and retained negative results

Reproduce with no Cargo/shared build:

```sh
python3 docs/evidence/c5-ordinary-mwdt1-2026-11-25/run-owned-checks.py --output "$TASK_TMP/final-checks"
```

The harness imports the actual TIMG, LP and RWDT modules. All65 tests pass:25 TIMG (the original14 plus11 focused additions),37 unchanged LP tests, and3 joint-controller fixtures. Stable Rust1.100.0, edition2021, warnings-denied clippy-driver passes with no lint allowances. The original14 test bodies and expectations are preserved except one necessary address substitution: TG1 IRQ+7c is newly supported, so its unknown-address negative now uses still-unsupported TG0 IRQ+7c. No golden, default console literal or inactive/cold progression was regenerated.

New checks cover the exact ROM init/update/direct-enable order, original5s+5s holds, actual tick reconfiguration without implicit feed,300ms+600ms timing, raw/mask/clear and late-unmask levels, feed/divider fraction, source loss/restoration, independent gates, strict transactional rejection, maximum positive holds, u64 overflow, chunk equality, shorter-hold commit, reset barriers/domains, TG0/TG1 earlier/equal/later reset races and LP retention. Joint fixtures check LP Core210M truncates TG1 before216M, TG1 Core216M retains later SWD228M expiry, and cause08 outranks coincident LP CPU0d while LP Core09 outranks08.

The first focused run passed23/24: the maximum-hold expected IRQ integer was transcribed as1407353803969125. Independent Python arithmetic `4294967295 * 65535 * 5` confirms1407353408389125; the expected IRQ/reset values were corrected without an implementation change for that failure. The final65-test run and precise hashes are in [checks.json](checks.json). Old source/firmware receipts remain unchanged.

No workspace/native/WASM build, browser run, CPU benchmark or original-factory execution was performed. Parent owns shared integration and acceptance. The evidence omits private captures, home paths, process/session inventories and unrelated commands; original hashes remain in source/input receipts. No physical clock accuracy, full MWDT stage-loop support, ordinary TG0 support or complete product acceptance is claimed.

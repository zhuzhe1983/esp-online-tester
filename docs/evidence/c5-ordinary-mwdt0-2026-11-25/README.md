# C5 TG0 task watchdog on the shared ordinary engine

This follow-up to `3ddfd6393b13db89f12e92bed7327298e9ce289a` adds the statically required TG0 task-watchdog path to the same ordinary timer engine. It changes only `timg_wdt.rs` and this new evidence directory. Previous TG1, cold-flashboot and other receipts remain unchanged. No factory/native, FreeRTOS, app_main or LCD execution is claimed here.

The additional fixed primary sources are C5 IDF v5.5.3 `2c211b236707889e8400c4dc5644dd5c4ee071e0` and v5.5.4 `735507283d5b2f9fb363a1901172dbd9e847945d`: [task_wdt_impl_timergroup.c](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/esp_system/task_wdt/task_wdt_impl_timergroup.c),8481 bytes/SHA `48c0370e20e4cfd469f2e15cc893320c350b22d364b90b23d0b81feb4ef269fd`, and [task_wdt.c](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/esp_system/task_wdt/task_wdt.c),29231 bytes/SHA `b885a3115fdaba43e3beca3461a35ef443e75c6517c7626389c309b1178c5450`. Both files are byte-identical between the two fixed revisions. [source-pins.json](source-pins.json) preserves the existing paired capture timestamps/URLs/hashes; no replacement fetch time is invented. The inherited C5 headers, LL, ROM HAL and TRM pins remain in the [TG1 receipt](../c5-ordinary-mwdt1-2026-11-25/README.md). In particular the selector1/2 conflict is unresolved; only selector0 XTAL48M executes.

## Original static path, separate from execution

The unchanged typed main_task constructs `{timeout_ms=5000,idle_core_mask=1,trigger_panic=false}` at `42121c4e..5c`, calls `esp_task_wdt_init=4206feb0` at42121c5e, and calls app_main4200aebe only later at42121cac/cb0 after successful initialization. The selected implementation is TG0, not the alternative esp_timer implementation:

| Original PC | Operation |
| --- | --- |
| `4206ffbc →4207036a` | Allocate the task-watchdog controller |
| `4207038e` | Interrupt allocator source58, flags0, callback4207021e; CPU line remains guest-allocated |
| `420703ac..c4` | Conditional first-acquire APB enable/reset/release at PCR60096054 |
| `420703ce` and following store | Read TG0 CONFIG0 at60008048 and clear FLASHBOOT14 after group reset |
| `420703e2..f4` | Original ROM HAL init, context40839dc4, instance1=MWDT0, prescaler24000, interrupt=true |
| `40021138/13e` | ROM reads TG0 IRQ CLR6000807c, then writes read-value OR4 |
| `40021140/146` | ROM reads TG0 IRQ ENA60008070, then writes bit2 enabled |
| `42070404..14`, `42070418..28` | Stage0 hold10000/actionINT1; stage1 hold20000/action3; each real UPDATE, then lock. Allocation still leaves EN=0 |
| `4206ffe0 →42070482` | Restart after a subscribed idle entry exists: unlock; ROM enable FEED40021322 then direct EN4002132c; an additional real FEED via420704a4; lock |

The exact TG0 word decoder covers CONFIG0..5+48..5c, FEED+60, KEY+64, IRQ+70/+74/+78/+7c and independent WDT clock PCR6009605c. The general timer source57, general timer counters, calibration+68/+6c/+80, version+f8 and other adjacent words are not claimed. TG1 retains source60; TG0 gets the actual source58 WDT level. No new PCR+5c/+68 transaction is invented inside ROM initialization.

`trigger_panic=false` changes guest timeout handling; it does not disable the hardware INT or second-stage action. On real TWDT IRQ, guest callback4207021e calls timeout_triggered42070454, whose typed calls unlock, original ROM handle_intr, then lock. ROM handle_intr40021366/68 writes FEED before4002136a/70 reads/writes CLR|4. Thus the real handler explicitly feeds and clears RAW separately. The idle hook and task-entry `has_reset` bookkeeping eventually call the actual feed implementation only when the guest's required subscribed entries have reset. The emulator never invents those writes or feeds merely because an idle task exists.

Factory/ROM/typed inputs are the unchanged hashes in the TG1 receipt: factory `cf10a4358d03898458c16390f3f16ceb0306befe2260160de0919d755b920942`, ROM ELF `49693d745eb4b4e6e6c77be4f5213765d545d8c7a1d9742a8a619bf596c4e27a`, typed app seg2 `3e23c10184628e7c5dcea63972581d1683ba76345ad250af08dbb22c43646a45`. These are static call/parameter checks, not a native receipt.

## Handoff, common engine and boundaries

TG0 ordinary timing and shared IRQ access require the existing explicit strict profile **and a committed guest FLASHBOOT disable while its cold latch was live**. Staging FLASHBOOT0 without UPDATE does not grant this; nor does resetting the group, which withdraws the cold latch without pretending the guest disabled it. The successful handoff marker survives subsequent group/HP reset so the later true TG0 reset/init can proceed. Chip reset and raw-profile deselection withdraw it; a fresh cold epoch must establish its own disable. This is an explicit strict platform qualification, not a claim about a hidden silicon latch.

Two fixed-size `Ordinary` states call the same `advance_ordinary(index, elapsed)` function. They share the exact TG1 source0/positive-prescale/positive-hold `[INT, Core, OFF, OFF]` contract, shadow/UPDATE/liveEN rules, FEED/divider phase, IRQ raw/mask/clear, locked-write boundaries, unavailable-source pause and reset barrier. There is no copied TG0 counter algorithm, host feed or unconditional success. Unsupported active actions, later stages, zero holds and selector1..3 reject transactionally. General-timer bit0/reserved writes reject. Qualified original shared IRQ writes occur unlocked; uncertain locked shared IRQ writes explicitly reject.

With source48MHz/prescale24000, one tick is500 microseconds. From a phase0 FEED, TG0 holds10000/20000 produce IRQ after5s (1200000000 nominal240MHz cycles) and HP/Core cause07 after a further10s, totaling15s (3600000000 cycles). These are additional stage times, not an absolute10s reset from FEED. The new fixture allocates at cycle8 and restarts at10, observes no pre-enable deadline, two real restart FEEDs, IRQ at1200000010 and Core07 at3600000010. Its times are independent test inputs, not factory timestamps or physical pulse measurements.

Source gate, APB gate and group reset remain independent by group. Ordinary source loss preserves fractional progress;48MHz restore resumes it after old-source time was settled. CPU reset retains both ordinary states. HP reset clears configuration/raw/active timers and restores the declared WDT PCR defaults; the established qualification does not autonomously restart a counter. Original cold TG0 counts, None clock-fault policy, special flashboot Core07 and clear+UPDATE disable are retained. Default/raw reports remain the prior literals until ordinary operation actually starts.

Production now uses `sources() -> [(usize,bool);2]`, returning fixed `(58,level)` then `(60,level)`. The former TG1-only `source()` wrapper is test-only. `take_reset_event() -> Option<(u8,u64)>` still returns Core07/Core08. `deadline()` includes both real IRQ events and each controller's reset event. Parent Bus must connect both sources and retain cause/time in its existing preview/commit/reset pipeline.

Advancement chooses the earliest reset across cold TG0 and both ordinary instances before updating any real state. Same-time TG0 Core07 wins TG1 Core08; an earlier08 still wins later07. At most two reachable stages per ordinary instance are processed, independent of elapsed time. Pending and consumed events remain dispatch barriers. Parent LP/TIMG arbitration retains TRM priority only at equal times: LP Core09 outranks07, while07 outranks LP CPU0d. Applying a Core reset must not drop a later retained LP event.

## Owned checks and limitations

```sh
python3 docs/evidence/c5-ordinary-mwdt0-2026-11-25/run-owned-checks.py --output "$TASK_TMP/final-checks"
```

All73 tests pass:31 actual TIMG tests,37 unchanged LP tests, and5 joint-controller fixtures. Stable Rust1.100.0/edition2021 warnings-denied clippy-driver passes without allowances. The previous65-test checks remain passing; the six new TIMG fixtures cover real allocate/restart timing, handoff denial/retention, two ordinary sources and early/equal/late reset races, the actual handler FEED+CLR order, independent TG0 gates/source restoration/group reset, strict invalid controls and bounded chunk equality. Two additional actual-module LP fixtures check earlier LP Core truncation, equal-time Core07/LP priorities and preservation of a later LP event.

The first mechanical generic regression passed24/25: the old unknown-address fixture still named the newly modeled TG0 IRQ words. Those four negative addresses were replaced with unowned+6c/+80/+84/+88; explicit handoff/APB/key/reserved negatives cover the new words. No tick/count/reset assertion was relaxed. TG1 fixture state access changed from one state to array[1] without changing its numeric expectations. Earlier receipts, including their negative outcomes, remain byte-identical; [checks.json](checks.json) records their hashes.

No full Cargo/native/WASM build, browser run, benchmark or original-factory run was performed by this lane. Bus/Soc glue, interrupt allocation/delivery, actual guest feeds and factory acceptance remain parent integration work. Physical pulse widths, source synchronization and unsupported ordinary stage loops are not claimed. This evidence contains compact source/command/hash/numeric facts, with no raw private capture, home path, process or session inventory.

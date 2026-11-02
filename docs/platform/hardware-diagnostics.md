# StopWatch hardware checks

The workbench and standalone player show **硬件检查** above serial output.
Checks observe executed I²C transactions and display commands, not filenames
or console keywords. `observe().hardwareDiagnostics` and the
`hardware-diagnostics` event expose the same bounded snapshot. Unsupported
boards return `null`, never a compatibility pass.

| Observation | Result and suggested action |
| --- | --- |
| PM1 I²C clock exceeds its register 0x09 setting | Warning retains the count and last mismatch even after a later speed change. Start at 100 kHz and change master/device settings coherently. |
| PM1 0x49 bit7 is set | Warn that firmware requested a download-button lock; omit this from routine startup. |
| A set download-lock bit is cleared | Warn that this model cannot validate physical PM1 unlocking. |
| PM1 LED_EN is clear | Information, not a failure: factory firmware also deliberately disables the indicator. |
| Pixel memory exists while sleep/display/power gates hide it | Show a blank visible screen and explain that GRAM content is not proof of emitted light. Preloading before display-on is legal. |
| Pixels precede an explicit CO5300 0x53 write | Suggest checking brightness initialization. Conflicting manufacturer defaults prevent calling this a guaranteed failure. |
| Pixel data arrives in a non-RGB565 format | Warn that the model's two-byte decoder is outside its supported format. |

PM1 register 0x09 bit4 selects 100/400 kHz; a slower master is not reported as
a mismatch. Nominal rates come from existing S3/C3/C6 clock/divider and SCL
period registers after address and physical-route matching. A 1% comparison
margin avoids divider rounding noise, **not** a measured electrical tolerance.
Classic and GPIO bit-bang clocks are unknown; unknown observations do not reuse
stale rates or produce rate warnings. Address-alias/general-call behavior stays
on its existing path.

## Display memory and visible output

StopWatch opts into CO5300 state observation. Sleep-in (0x10), display-off
(0x28), and explicitly enabled brightness control (0x53 bit5) with zero
brightness (0x51) hide visible output. Sleep-out (0x11), display-on (0x29),
and nonzero explicitly enabled brightness restore it without another pixel
write. Command changes update the publication version and release retained
presentation frames. Physical reset/power gating still resets the panel.
`gram()` remains a separate debug view of raw pixel memory.

Other boards keep their previous shared-CO5300 behavior. No analog luminance,
dimming transitions, invented wake/settle delays, or additional pixel formats
are claimed.

## Evidence and limits

The motivating private application used 400 kHz for immediate PM1 access. On
hardware, its first checked read failed and `ESP_ERROR_CHECK` aborted before
display initialization. A repair combining 100 kHz, wake handling, bounded
retries and transfer spacing booted; subsequent changes restored LED/display/
button use. These combined changes do not isolate speed or one omitted display
command as the sole physical cause. Device flash had already matched the
original image: successful flashing did not imply successful startup.

PM1/IOE still ACK without a sleep/wake model. The simulator does not reproduce
that original panic, certify retry handling, model analog power, or determine
that every warned transfer must fail. Warnings expose the configuration even
when firmware keeps running. No finding means only that these checks found
nothing.

The manufacturer PM1 library refuses clearing a set DL_LOCK and says power
cycling is required. The register table labels it read/write and retained on
system reset/shutdown; enforcement inside the PM1 was not independently
established. Clear attempts are therefore recorded without inventing sticky
latch behavior. New simulator instances reset model state; that is not proof
that resetting an ESP clears a physical PM1 lock.

Observation runs at I²C address phases and existing host publication points,
not a new CPU tick loop. The worker samples at most every 250 ms while running,
refreshes at pause/stop, and emits changed snapshots. It injects no UART text
and clears previous-session state when changing firmware.

## Sources

- [M5PM1 at 315d2899](https://github.com/m5stack/M5PM1/tree/315d2899c863ae34b59197392b9f4c59490751ef): SPD, LED_EN, DL_LOCK and `setDownloadLock`.
- [StopWatch power wiring](https://docs.m5stack.com/en/arduino/stopwatch/m5pm1_m5ioe1): PM1 remains powered in L0 with a battery present.
- [Official StopWatch HAL at 81099c78](https://github.com/m5stack/M5StopWatch-UserDemo/tree/81099c784991dc77f522de1a5bf62ec5de9ae0b5/main/hal): IOE pins and explicit CO5300 initialization.
- [CO5300 V0.01, 2023-07-24](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1242/CO5300_Datasheet.pdf), SHA-256 `2ad0461aec31aa046e2661f52f43e9c36b36218c58c53f07217c73bef7d710c6`: pages 131/143 give Sleep In, conflicting with page 145. Page 186 gives both 0x28 and 0x00 as WRCTRLD defaults, retained as unknown until written. Pages 155/157, 176 and 184 define display gates, COLMOD and zero brightness.

## Reproduction

Portable tests cover controller timing/routing, PM1 writes, split display
transactions, visible output versus GRAM, cold ABI buffers, and UI/API lifecycle.
The unsafe legacy sequence stays in tests as an explicit negative example.
`examples/platform-stopwatch` uses corrected initialization and remains an
internal fixture, not a complete hardware qualification.

Run `tools/probe-hardware-diagnostics.mjs <cases.json> <receipt.json>` for actual
platform Worker/WASM execution. A case names `id` and `mergedPath`, `buildPath`,
or a runtime firmware `source`; optional `expectCodes`, `absentCodes`,
`noWarnings`, and `seconds` assert outcomes. It checks visible pixels and saves
hashes and bounded observations. It does not flash USB hardware or publish
private firmware, serial captures, server addresses or device identifiers.

Results, input hashes, browser checks and CPU samples are retained in the
[verification receipt](evidence/stopwatch-hardware-diagnostics/README.md).

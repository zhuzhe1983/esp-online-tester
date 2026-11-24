# PCF85063A guest-clock calendar model

`esp_soc::devices::pcf85063` implements an opt-in calendar subset of NXP's
PCF85063A. This reconstruction uses the actual PDF retrieved on 2026-11-23:
**Rev. 7, 30 March 2018**, obtained from
[Waveshare's NXP datasheet copy](https://files.waveshare.com/wiki/common/PCF85063A.pdf).
It does not label that file as a later revision. Exact original bytes, hashes,
retrieval times and the relevant public Waveshare driver members are retained
in the [source manifest](../research/tca9554-pcf85063-sources/manifest.json).

## Clock, register and calendar behavior

Create `Pcf85063State::new(guest_clock_hz)`, share it through an
`Arc<Mutex<_>>`, and attach `Pcf85063::new(state)` at the board's actual SDA/SCL
pads and address `51h`. Advance its clock with `state.advance_to(guest_cycle)`.
The model never uses host wall time. I2C wrappers **must forward `stop()`**;
otherwise the documented calendar-access lock will not be released.

The 18-register pointer increments and wraps from `11h` to `00h`. POR uses
Table 8 exactly: Saturday 2000-01-01 at 00:00:00, with oscillator-stop flag OS
set. This is an invalid initial time, not a claim about the user's real date.
Seconds, minutes, hours, dates, weekdays, months and the two-digit year advance
in BCD. Both 12-hour and 24-hour modes, leap years including year 00, and the
99-to-00 rollover are represented. Invalid guest-written calendar values remain
visible instead of being silently normalized into an invented valid date.

STOP freezes the calendar. Releasing STOP selects the documented lower restart
phase, 65/128 second, rounded up to a guest cycle; subsequent ticks are one
second apart. Unknown POR subsecond phase is represented by a deterministic
one-second delay to the first tick. Neither choice simulates crystal jitter
or analog oscillator startup.

An I2C access blocks calendar increments through repeated START. At most one
pending increment is retained and applied at STOP, following section 8.4.
Long blocked accesses therefore do not fabricate a catch-up clock. OS is cleared
by a zero write; writing one preserves its current state instead of fabricating
a new oscillator failure.

## Exact reset and unsupported features

Software reset is the exact `58h` command to register `00h`, as specified in
section 8.2.1.3. It restores Table 8 and returns the address pointer to `00h`.
The archived Waveshare `PCF85063_Reset` helper constructs `11h`; that value
does not match the primary reset definition and is rejected. The driver's
ordinary `01h` capacitor-selection initialization is supported. Factory behavior
must be tested without making the incorrect reset sequence succeed.

RAM, capacitor selection, disabled-alarm storage and disabled-timer settings
retain their defined values. The implementation does not claim alarm matching,
minute interrupts, countdown timing, calibration pulses, external test clocks,
CLKOUT waveforms or physical INT output. Writes enabling those unimplemented
engines NACK. Offset is supported only at its neutral zero value. Existing boards
and other RTC models are unchanged.

## Fresh verification

Six protocol tests cover Table 8 and OS, guest-time progression, STOP and its
restart boundary, BCD and leap-day rollover, 12-hour AM/PM transitions, repeated
START and STOP, one pending tick, pointer wrap, exact reset, unsupported writes,
invalid dates and bounded large time jumps. Exact fresh commands/results and
source hashes are recorded in the
[verification manifest](../evidence/tca9554-pcf85063-2026-11-23/manifest.json).
No lost pre-restoration log is used as a new acceptance result.

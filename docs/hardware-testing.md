# Testing against real hardware

Every chip is checked against hardware, by different methods:

| | ESP32-S3 | ESP32-C3 | ESP32-C6 |
| --- | --- | --- | --- |
| Method | JTAG lock-step | console diff | console diff |
| Reported result | 8000 steps after timing-loop resynchronization | 205/208 lines identical | 203/204 lines identical |
| Reported full-listing decoder checks | 977 k instructions, 0 mismatches | 161 k instructions, 0 mismatches | 126 k instructions, 0 mismatches |

These full-listing and hardware counts describe earlier manual runs. CI runs the checked-in sampled
decoder corpora and emulator golden outputs; it excludes the external full-listing tests. The
Xtensa objdump test excludes PIE operand comparisons. Hardware trace comparison normalizes window
exceptions and resynchronizes delay loops, so it is not uninterrupted cycle-by-cycle agreement. See
the [test scope](../tests/README.md), [CI workflow](../.github/workflows/ci.yml),
[Xtensa decoder test](../xtensa-lx7/tests/objdump_diff.rs) and [hardware comparator](../hw/compare.py).

## ESP32-C3 and ESP32-C6: console diff

The C3 and C6 are checked by diffing a captured console against the emulator running the same
binaries:

- ESP32-C3: `hw/c3-hello-world-real.txt`, a physical C3 module. 205 of 208 console lines are
  identical over three boot cycles; the difference is the ROM's `Saved PC:` line. See
  [esp32c3.md](esp32c3.md).
- ESP32-C6: `hw/c6-hello-world-real.txt`, a Waveshare ESP32-C6-LCD-1.47. 203 of 204 lines are
  identical over three boot cycles, including the ROM's `Saved PC:` after `esp_restart()`; the one
  difference is that line on the first boot. See [esp32c6.md](esp32c6.md).

Those comparisons found five emulator bugs each, including a device-command race and a cycle
counter that did not restart at reset. A board-free test would have caught neither.

## ESP32-S3: JTAG lock-step

```sh
DIFF_DIR=hw/<board> FLASH_MB=8 hw/difftest.sh 3000
```

The scripts read efuses and strapping pins from the attached chip over JTAG, then step it and the
emulator in lock-step on the same flash dump (`flash-8M.bin` if present). Atech board, 2026-08-25:
3000 steps from reset, 0 divergences.

Any ESP32-S3 board on USB works: its built-in USB-Serial/JTAG carries both the console and JTAG.
The flow is scripted:

```sh
# one-time: dump the board's bootloader/partition table/app start (esptool) into hw/flash-0-1M.bin
hw/difftest.sh 8000                 # reset, then single-step 8000 instructions on the chip and in the emulator, diff
hw/difftest-at.sh 403c8948 6000     # run both to a PC (here the 2nd-stage bootloader entry), then step 6000 and diff
```

`difftest*.sh` read the chip's efuses and strapping pins over JTAG, take the peripheral reset
state from `$DIFF_DIR/reset-regs.txt` (dumped over JTAG at `reset halt`), and start the emulator
from the same image and state. Everything read from the chip (efuses, traces, logs) goes to
`$DIFF_OUT`, a fresh temporary directory unless set, because the efuses hold the chip's MAC
address; the scripts print where. They stop rather than kill an openocd that is already running.

`hw/compare.py` diffs `pc a0..a15 ps windowbase` per instruction. It:

- masks `PS.INTLEVEL`, which is forced during single-step;
- hides window-exception handlers, which the debugger steps over atomically;
- resynchronises across CCOUNT-timed delay loops, which iterate a different number of times when
  each step takes milliseconds.

Result so far: the ROM reset path (8000 steps) and 3000+ steps of the IDF 5.5 bootloader run with
zero PC divergence. The remaining register differences are RTC-domain values that depend on the
previous boot.

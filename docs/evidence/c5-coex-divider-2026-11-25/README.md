# C5 COEX LP divider configuration

Starting at `abb80621`, this change retains the complete named COEX selector and
12-bit `DIV_NUM` field at `0x600af008`. It accepts the original factory's requested
`0x314` word and exposes its conditional pre-functional-gate rate without creating
a COEX timer, RF/PHY readiness, interrupts, or elapsed-time progress.

## Fixed sources and actual boundary

The fixed IDF revisions are 5.5.3 `2c211b236707889e8400c4dc5644dd5c4ee071e0`
and 5.5.4 `735507283d5b2f9fb363a1901172dbd9e847945d`.
[Source pins](source-pins.json) record full URLs, byte counts and SHA-256 hashes.
The register header, LL header and common HAL header are identical between these
revisions. The complete `modem_clock.c` files differ; the relevant select/deselect
functions were compared separately and are identical. The 5.5.4 changes concern
device reference-count policy and `with_refcnt` fields.

- The [5.5.3 register header](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/include/modem/modem_lpcon_reg.h)
  and [5.5.4 register header](https://github.com/espressif/esp-idf/blob/735507283d5b2f9fb363a1901172dbd9e847945d/components/soc/esp32c5/include/modem/modem_lpcon_reg.h)
  name `DIV_NUM[15:4]` with reset zero, and four source bits in `[3:0]`.
- The [5.5.3 LL](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/esp32c5/include/hal/modem_lpcon_ll.h)
  and [5.5.4 LL](https://github.com/espressif/esp-idf/blob/735507283d5b2f9fb363a1901172dbd9e847945d/components/hal/esp32c5/include/hal/modem_lpcon_ll.h)
  set and get the raw encoded `DIV_NUM`; that API does not subtract one.
- The [5.5.3 modem clock code](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/esp_hw_support/modem_clock.c)
  and [5.5.4 modem clock code](https://github.com/espressif/esp-idf/blob/735507283d5b2f9fb363a1901172dbd9e847945d/components/esp_hw_support/modem_clock.c)
  deselect all COEX sources, select a source, and program its divider. Deselecting
  clears source bits and retains the divider. Functional COEX gate operations
  are separate TODOs in these source functions.

The retained original typed input has SHA-256
`3e23c10184628e7c5dcea63972581d1683ba76345ad250af08dbb22c43646a45`.
Its caller sets selector `4` and true divisor `50` at `0x420b866a..0x420b8670`.
The setter at `0x420b8824` subtracts one at `0x420b884a`, writes the selector at
`0x420b8852`, then RMWs the divider at `0x420b8864`. Its requested value is
`(49 << 4) | 4 = 0x314`. The conversion routine separately extracts all 12 divider
bits at `0x420b8784/0x420b8786` and adds one on the XTAL branch at `0x420b8798`
and RC_FAST branch at `0x420b87ee`. That routine converts a timebase; its result
is not a measured hardware clock rate. Its nominal 20 MHz RC_FAST constant does
not override the model's caller-supplied 17.5 MHz observation.

The parent's unmodified native run stopped at the second store, with
27,103,952 steps, 27,103,936 nominal cycles, 3,955 UART bytes and cause 7.
The accepted first store left COEX `0x4`, raw XTAL `48,000,000` Hz and
BLE/COEX/WIFI write counts `4/5/10`. The failed `0x314` write had not executed.
The log hash and compact parameters are retained in [checks.json](checks.json);
the raw log remains outside Git as
`.git/c5-rom-factory-20261124/native-machine-lcd273-keymgr-modem-integration.log`.
The successful new divider write is currently a component fixture; a subsequent
native run is required to establish further factory execution.

## Bounded contract

COEX retains a `u32` word, accepts every named 12-bit divider, and permits either
no source or exactly one supported source bit: RC_SLOW `1`, RC_FAST `2`, XTAL `4`.
BLE RTC and WIFI retain the existing divider-zero boundary. Multiple source bits,
the shared-32k selector `8`, bits `[31:16]`, neighboring words and inaccessible APB
writes reject atomically. Reject/`None` denotes an unsupported model boundary;
it is not a claimed hardware abort. A disabled source can retain any divider.

`selected_source_hz` keeps its original raw-source meaning. Inputs remain the
strict caller-supplied virtual observations: RC_SLOW 136,000 Hz, RC_FAST
17,500,000 Hz and XTAL 48,000,000 Hz. Missing, zero or different inputs have no
fallback. APB access does not imply a functional COEX clock gate.

`selected_rate_ratio` returns the exact tuple `(source_hz, DIV_NUM + 1)` before
the unmodeled functional gate. `selected_clock_hz` returns an integer only when
division is exact. For `0x314`, the raw source remains 48 MHz and the divided
candidate is exactly `48,000,000 / 50 = 960,000` Hz. For `0xfff4`, the ratio is
`48,000,000 / 4096` and the integer getter returns `None`. The ratio preserves
fractional information. No rate observation establishes consumer progress.

The existing declared functional reset contract is retained: CPU/LP retain,
HP/analog clear all three words and their counters. This is a simulator reset
profile, not a per-flop silicon retention assertion. Source observations are
computed on demand and never cached. Divider-zero reports remain byte-identical;
nonzero COEX reports append the divider and candidate-rate limitation.

## Checks and retained negative results

The actual module passed 14 standalone tests, including all seven prior tests
and seven new focused tests. One old rejection fixture was narrowed for the
newly supported COEX words `0x10`, `0x11`, `0xfff0`; BLE/WIFI still reject those
words. All previous multi-source, 32k and reserved-bit negatives remain.
The new tests cover the two original RMW stores, divider-preserving deselection,
maximum 4095, exact and fractional rates, unknown inputs, APB atomicity, declared
resets, counter saturation and unchanged default reports. Stable strict
`clippy-driver -D warnings` passed without lint allowances.

Run the owned checks with a caller-chosen output directory:

```sh
python3 docs/evidence/c5-coex-divider-2026-11-25/run-owned-checks.py --output "$TASK_TMP/final-checks"
```

No shared Cargo, native, WASM, browser or benchmark run was performed for this
patch. Guessed `register/modem` header and common `components/hal/modem_clock_hal.c`
paths returned 404; their failed lookups remain in the ignored source receipts.
The correct register path is `components/soc/esp32c5/include/modem/`.
No complete-file identity is claimed for the two `modem_clock.c` versions.

The committed evidence contains hashes, numeric samples and source references;
it omits raw UART/log streams, local paths and process/session details. This
curation leaves the stated module/configuration conclusions intact and does not
establish RF behavior, oscillator accuracy or later factory startup.

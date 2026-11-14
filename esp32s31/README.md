# ESP32-S31 runtime and diagnostics

`esp32s31::machine` / `SocBus` is the native runtime imported from upstream. It uses
shared `DeviceSet` peripherals, the S31 ISA extensions and the Mosaico board model.
The browser product catalog does not gain an S31 runtime merely by compiling this crate.

`esp32s31::bringup::Bringup` is the retained strict diagnostic runner. The root
`Bringup`, `Stop`, `Run` and `PcWatch` exports remain aliases for existing callers.
The `bringup` example and all earlier guest fixtures select this namespace explicitly.
`ReadImage`, `Rates`, `Drive` and reset result values retain shared value types.

The paths intentionally have different contracts:

| Native runtime | Strict diagnostics |
| --- | --- |
| Unclaimed peripheral MMIO retains written words and records them in `unmodelled`. | Unclaimed MMIO faults and records the first guest access. |
| Model refusals outside APM/PMS may read zero or discard writes, recorded in `refused`. | Unsupported operations remain explicit faults. |
| Current PLL, eFuse power-on, cache-freeze, flash and GPIO behavior. | Earlier bounded-model behavior used by the retained diagnostics. |
| S31 Zb/Zc/PIE subset, hardware loops, decoded blocks and default FPU memory reason. | Original base-ISA interpreter profile and explicit FPU reason compatibility switch. |

APM/PMS access failures and out-of-range memory still fault on the native path.
Storage fallbacks and counted refusals are compatibility behavior, not evidence that
an unmodelled peripheral function executed. Native debug reporting exposes the counts.

Only modules whose diagnostic semantics differ are retained under `src/bringup/`.
Unchanged peripherals and value types are shared with the native implementation.
Changes to the native path must not silently change the historical fixture contract.
The boundary test `native_storage_fallback_does_not_weaken_diagnostic_faults` checks
that storage fallback and clock-gated UART writes remain distinct.

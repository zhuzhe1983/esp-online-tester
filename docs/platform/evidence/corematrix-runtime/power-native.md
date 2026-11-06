# CoreMatrix original-firmware power and motion wake

The external regression `external_corematrix_uiflow_power_key_voltage_and_motion_wake`
executes the unmodified official UIFlow 2.5.4 image from the C61 mask ROM.
It loads Python through USB Serial/JTAG raw REPL into RAM; it does not replace
`boot.py`, patch firmware bytes, inject voltage return values, or set CPU reset
from the test to simulate a wake event. The test calls `Machine::reboot()` only
after the actual PM1 → board → C61 chain has requested `Stop::SwReset`.

## Inputs and reproduction

Use the firmware and ROM hashes listed in [the native boot receipt](c61-native.md).
The [numeric receipt](power-native.json) also records both input hashes, the
test/guest-script hashes, source revisions, runtime configuration and results.
The test is ignored and named `external_*` because the official firmware must
be supplied by the caller. It is not silently included in ordinary CI runs.

```sh
ESP32SIM_ROM_DIR="$ROM_DIR" \
ESP32SIM_COREMATRIX_FIRMWARE="$COREMATRIX_BIN" \
CARGO_BUILD_JOBS=2 \
cargo test --release -p esp32c61 --test corematrix_power_firmware -- --ignored --nocapture
cargo clippy -p esp32c61 --all-targets -- -D warnings
```

The firmware pins M5Unified `b67ea7bd2d187d3fa8719edf1b915fd34a128c0d`.
The CoreMatrix branch in
[`Power_Class.inl`](https://github.com/m5stack/M5Unified/blob/b67ea7bd2d187d3fa8719edf1b915fd34a128c0d/src/utility/Power_Class.inl)
initializes the PM1, maps keys to GPIO0/1/2, makes GPIO3 the ESP GPIO2 IRQ,
and configures GPIO4 as the BMI270 INT1 input. `_batteryPresent()` requires
two stable samples at least 1250 ms apart before accepting the initial nominal
3900 mV as a battery. The low-level
[`M5PM1_Class.inl`](https://github.com/m5stack/M5Unified/blob/b67ea7bd2d187d3fa8719edf1b915fd34a128c0d/src/utility/power/M5PM1_Class.inl)
reads the read-only voltage pairs through I²C. Exact retrieval metadata is in
[power-sources.json](power-sources.json).

The RAM fixture `tools/fixtures/corematrix-uiflow/power-wake.py` configures
the standard Bosch any-motion feature through the guest's `machine.I2C`,
using the [pinned Bosch sources and datasheet](../corematrix-imu/sources.json).
PM1 GPIO4 falling-edge wake, `HOLD_CFG` LDO bit5, `SYS_CMD=0xa1`, and
`WAKE_SRC` bit5 follow the [M5PM1 v1.9 register manual](../../../m5-i2c.md).
The board connection and independent L1 supply follow the C158 v0.7 schematic
in [the electrical source receipt](../corematrix-peripherals/sources.json).

## Observed behavior

| Check | Result |
| --- | --- |
| `M5.Power` voltage samples, 1300 ms apart | Battery `-1`, `-1`, then `3900` mV; VBUS `5000` mV and external output `True` for all three. The initial unknown result is retained. |
| Physical power double click | Guest PM1 configuration reads `[42,0]`; two 20 ms clicks with 70 ms between them turn off the controller. |
| Powered-off CPU | Across 500 ms, PC and retired instruction count remain unchanged: `732000000` before and after. |
| Physical single press while off | PM1 requests reset; releasing the same press does not enqueue another reset. The mask ROM and original UIFlow REPL both boot again. |
| Motion with L1 hold disabled | A raw X acceleration change from `0` to `0.5g` does not wake the CPU over 150 ms. PC and retired count stay unchanged. |
| Motion with L1 hold enabled | The same stimulus causes INT1 → PM1 GPIO4 wake after `13945112` guest cycles (`87.15695` ms). No sleeping-PC instruction retires before reset. |
| Firmware after motion wake | The original image boots again; guest I²C reads `WAKE_SRC=32`, the GPIO3/4 wake pair containing the board's GPIO4 wire. |
| Whole run | Four complete ROM/application boots, three requested resets, zero CPU exceptions. |

The configured duration is five qualifying samples on the 50 Hz motion engine.
The stimulus can occur anywhere between sample deadlines, so the digital model's
observed delay lies in 80–100 ms; the measured 87.15695 ms is not a calibrated
physical sensor latency. The test checks that no wake occurs by 80 ms and that
the actual reset arrives by 100 ms.

The first attempted motion configuration did not wake: the original
`M5.begin()` sets `INT_MAP_DATA` (`0x58`) to `0xff`, so data-ready had already
asserted the shared INT1 line before sleep. The fixture now explicitly unmaps
data-ready and checks that PM1 GPIO4 is high before enabling the sleep test.
This is necessary guest interrupt configuration; the emulator was not changed
to manufacture an extra edge. The failed attempt is retained here rather than
treated as an independent hardware success.

## Limits

This is native digital emulation of one pinned original firmware. It does not
establish silicon timing, analog rail transients, battery chemistry, calibrated
voltage measurement, optical behavior, every BMI270 feature, or full UIFlow
cloud support. The distribution's `M5Things` startup import error remains as
documented in the boot receipt. No physical board was flashed or tested.

The committed receipt contains selected output values and hashes, not a raw
console transcript, local user paths, host identities or process inventories.
The commands regenerate all assertions; no performance comparison is claimed.

# FT3168 single-contact digital report model

`esp_soc::devices::ft3168::{Ft3168, Ft3168State}` is an opt-in model of the
FT3168 point report and the configuration transactions actually issued by the
official FT5x06 driver used in Waveshare's AMOLED 2.06 BSP. The chip identity
remains FT3168; the driver family name does not establish another physical part.

## API and observable behavior

Create shared state with `Ft3168State::new(cpu_hz)` and an I²C device with
`Ft3168::new(state.clone())`. The board supplies its independently verified I²C
address/pads, power rail, reset pin and interrupt pin. `hardware_at(cycle,
powered, reset_released)` applies those digital conditions after advancing the
state; `touch_at(cycle, x, y, down)` replaces one host contact. Backwards time is
rejected. `irq_high()` and `next_deadline()` expose the host-input pulse policy.

The readable 00h–08h window contains mode/gesture zero, one contact count, 12-bit
X/Y coordinates, down/move/up event bits and contact ID zero. The report is
latched at a read START so a host update cannot tear an in-progress burst.
Weight and area are zero placeholders, not measurements of contact pressure or
shape. There is no multi-touch, gesture recognition or analog detection engine.

The nine explicitly used configuration registers (80h–85h and 87h–89h) retain
guest-written values and read them back. Each accepts one parameter. Initial
values are unknown, so an initial read rejects; a convenient fabricated reset
value is not supplied. The driver's initialization values are 70, 60, 16, 60,
10, 20, 2, 12 and 40 in that order. They are digital configuration only and do
not change the host's injected position or imply simulated analog thresholds.

Report registers are read-only. Unknown addresses, identity/firmware queries,
86h operating mode and calibration/sleep commands reject. This unsupported
access policy is an emulator boundary, not a claim about physical NACK timing.

Both power loss and asserted reset invalidate contact/configuration and any old
transaction pointer. FT3168 Table 3-5 specifies at least 70 ms before point
reporting after reset; reports remain empty until that many guest cycles have
elapsed. Configuration transactions are allowed before point-ready. A changed
host contact produces a falling interrupt pulse lasting one guest millisecond,
delayed to point-ready if necessary. That pulse length is an explicit digital
input policy, not measured physical scanning behavior. The datasheet's minimum
5 ms reset-low width, supply ramps and analog reset settling are not simulated.

## Sources and evidence boundary

- [FocalTech FT3168 datasheet, official Waveshare linked copy](https://files.waveshare.com/wiki/common/FT3168.pdf): electrical supply ranges and Table 3-5 reset/report timing. The document does not publish a complete register identity map; no identity value is inferred from it.
- [Espressif FT5x06 1.1.1 component](https://components.espressif.com/components/espressif/esp_lcd_touch_ft5x06/versions/1.1.1): exact archive is tied to `espressif/esp-bsp@684b63a9e3dc7ba7445a031d9e598fea2457f664`. Its driver/header define the 38h address, count/coordinate reads and the nine initialization writes. These accesses are the implemented compatible subset, not proof of every generic FT5x06 feature on FT3168.
- [Waveshare AMOLED 2.06 fixed BSP](https://github.com/waveshareteam/ESP32-S3-Touch-AMOLED-2.06/tree/3de8684cc3b27a8b6aa920157ecf399287528181/examples/factory/components/waveshare__esp32_s3_touch_amoled_2_06): independently identifies the physical FT3168 and selects the compatible driver. Board voltage, routing and original firmware checks belong to its separate adapter.

[The source manifest](../research/ft3168-sources/manifest.json) retains original
retrieval timestamps, exact byte counts, SHA-256 and deterministic gzip archive
hashes. The component ZIP's member hashes distinguish original archive bytes
from extracted files. Firmware BINs are not included.

Three protocol tests exercise unknown identity/default rejection, stored
configuration, latched coordinate/event reports, point-ready deadlines,
interrupt duration and reset/power invalidation. These device tests do not
establish a particular board's original-firmware boot or UI interaction.

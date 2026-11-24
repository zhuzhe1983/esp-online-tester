# AXP2101 digital power and control subset

The shared `esp_soc::devices::axp2101` model implements observable digital power
configuration. It is opt-in; existing LilyGO private power models and existing
boards are unchanged. It does not simulate battery charging or regulator analog
behavior. New boards must independently establish the actual AXP address, I²C
pads and the connection between each output rail and the peripheral supply.

## API and declared supply scenario

```rust
use std::sync::{Arc, Mutex};
use esp_soc::devices::axp2101::{Axp2101, Axp2101Power, PowerRail};

let power = Arc::new(Mutex::new(Axp2101Power::default()));
let chip = Axp2101::new(power.clone());
let requested_mv = power.lock().unwrap().output_mv(PowerRail::Dcdc1);
```

`output_mv` returns the requested digital voltage only when the corresponding
enable bit is set, the system is on, and that voltage code is implemented.
`None` means disabled, reserved or unsupported; it does not assert that an
unknown physical output is zero volts. A board may require the documented
nominal supply to accept LCD/touch transactions. `system_on` exposes the PMU's
digital on/off state. `irq_asserted` combines the real pending-latch and enable
registers; the initial scenario has no generated interrupt events.

The explicit virtual supply scenario is ideal 5 V USB, no cell battery and no
backup battery. Status 00h reports VBUS good and no battery. Status 01h reports
standby and not charging, with the system-on bit following actual model state.
Enabling the cell charger never produces charge-progress or completion events
without a battery. The model does not invent a battery percentage. A4h remains
its published reset latch; it is not a valid state-of-charge measurement.

The initial rail configuration is the **reference startup preset in standard
X-Powers Table 6-1**, not a readout of any particular board's programmable EFUSE.
DCDC1–4 and ALDO1/ALDO3/CPUSLDO start enabled with the table's voltages. The other
implemented LDOs start disabled. The 300 mA charge-current register preset comes
from Table 6-2. EFUSE-only fields lacking a published reference remain unknown
until the guest explicitly writes the register; an initial read is rejected.
This affects 22h, 25h, 27h, 50h, 58h and 69h and deliberately does not invent
board-specific power-button/TS/CHGLED configuration.

## Implemented digital behavior

| Function | Behavior and boundary |
| --- | --- |
| I²C | One-byte register pointer, bounded sequential reads/writes. Pointer setup accepts only declared registers. Unknown addresses and unsupported writes reject; a failed transfer cannot continue modifying another register. |
| Identity | 03h = 4Ah comes from the fixed official Waveshare-bundled XPowers header and initialization check. The public X-Powers register list omits this register; it is not attributed to that table. |
| Output control | 80h, 90h and 91h enable the corresponding implemented rails. DCDC1–4, ALDO1–4, BLDO1–2, CPUSLDO and DLDO1–2 each use their own voltage register and bit. |
| PWM/DVM settings | 81h includes DCDC1 forced PWM at bit 2. Settings read back; no switching waveform, ramp or settling time is claimed. |
| Reset | 10h bit 1 self-clears and resets only the documented System Reset fields. POR-only buffers, ADC configuration, charge limits and applicable IRQ latches survive. Construction supplies a fresh power-on reference state. |
| Soft off | 10h bit 0 self-clears, records software power-off in 21h, clears system-on status and makes all modeled output rails unavailable. No host CPU reset/power callback or analog startup interval is implemented. |
| Interrupt control | 40h–42h enable bits and 48h–4Ah write-one-to-clear pending latches. Writing ones cannot create an interrupt. The per-bit reset domains are retained. No battery/temperature/charger event generator is implied. |
| ADC | Only the declared ideal VBUS input is supplied: when channel 2 is enabled, reading 38h then 39h yields the documented 1 mV/LSB code 5000. High-byte read latches the pair. This is a digital injection policy with no conversion delay or accuracy claim. |
| Other ADC channels | Disabled channels expose their reset latch. Enabled battery, TS, VSYS and die-temperature channels reject reads because their analog inputs/conversion behavior are not modeled. |
| Charger configuration | Documented current, voltage, threshold and control fields retain values and respect writable masks. They never advance a charging state machine. Invalid/reserved current and voltage encodings reject. |
| Unsupported engines | Watchdog enable/clear, gauge reset/load, sleep/wakeup, fast-start enable, JEITA enable, ADC debug mux and backup-battery charging reject instead of self-completing. |

The model is not a 256-byte RAM device. Reserved addresses do not acknowledge;
read-only registers cannot be mutated, and documented read-only/reserved bits
of writable registers are preserved/masked. The stricter rejection of unsupported
writes is an emulator boundary, not a claim about the physical chip's ACK timing.

## Source differences retained

1. The standard English V1.0 PDF describes five DCDCs and a DCDC3 high-voltage
   range; the separately titled **SWcharge** V1.0 PDF explicitly describes four
   DCDCs and restricts DCDC3 to 1.54 V. These are not treated as interchangeable
   product specifications. This initial shared subset does not implement DCDC5
   or DCDC3 codes above 87. Their outputs remain unknown, not silently redirected.
2. Standard page 53 labels 90h bit 4 as ALDO1 a second time. A visual inspection
   confirms that this is in the source PDF, not an OCR error. SWcharge page 42
   and the fixed bundled XPowers `enableBLDO1` implementation identify bit 4 as
   BLDO1; the model uses that mapping independently of ALDO1 bit 0.
3. The standard CPUSLDO/DLDO2 table states 0.50–1.40 V at 50 mV/step but labels
   binary 10011 as 1.40 V, an inconsistent endpoint. Codes 0–18 have the stated
   linear meaning; code 19 is deliberately unknown. DLDO1 is limited to the
   unambiguous 0–28 / 0.50–3.30 V codes. No source conflict is hidden by clamping.
4. The standard BATFET-control and die-temperature-control tables contain the
   same digital fields. They are retained as separate configuration registers;
   neither is used to fabricate a temperature, protection event or analog BATFET
   state. The bundled driver also treats them as separate registers.

## Primary sources and provenance

Exact fetched bytes, original retrieval times, SHA-256, fixed Git blob identities
where applicable, and deterministic gzip archive hashes are retained in
[`../research/axp2101-sources/manifest.json`](../research/axp2101-sources/manifest.json).
The `sha256` field refers to the decompressed original file, not the gzip wrapper.

- [X-Powers AXP2101 standard English V1.0, fixed official Waveshare copy](https://raw.githubusercontent.com/waveshareteam/ESP32-S3-Touch-AMOLED-2.16/e8e5d132978b768cce18901602b89fdb318ad41a/examples/arduino/libraries/XPowersLib/datasheet/AXP2101_Datasheet_V1.0_en.pdf): Tables 6-1/6-2, §6.3, §6.10 and §6.13.2 register permissions, codes and reset domains.
- [X-Powers AXP2101 SWcharge V1.0, official linked copy](https://files.waveshare.com/wiki/common/X-power-AXP2101_SWcharge_V1.0.pdf): title/features identify the switch-charge variant; p42 resolves BLDO1's enable bit. Variant differences are limitations, not assumed board identity.
- [Fixed official-bundled XPowers register constants](https://raw.githubusercontent.com/waveshareteam/ESP32-S3-Touch-AMOLED-2.16/e8e5d132978b768cce18901602b89fdb318ad41a/examples/arduino/libraries/XPowersLib/src/REG/AXP2101Constants.h) and [driver](https://raw.githubusercontent.com/waveshareteam/ESP32-S3-Touch-AMOLED-2.16/e8e5d132978b768cce18901602b89fdb318ad41a/examples/arduino/libraries/XPowersLib/src/XPowersAXP2101.tpp): 34h seven-bit address, 03h/4Ah identity check, buffer/IRQ multi-byte accesses and separate BLDO1/ALDO1 controls. The board must still bind its own real physical pads and address.

## Fresh verification on 2026-11-23

Five protocol tests passed on the newly restored workspace: output enable and
voltage boundaries including forced PWM; System Reset versus POR and soft-off;
IRQ masks/W1C/reset domains; honest supply/ADC/charging behavior; and bounded
transactions with read-only/unknown-EFUSE/unknown-address negative cases.
`cargo clippy -p esp-soc --all-targets -- -D warnings` also passed.
The source-hash-bound record is
[`../evidence/axp2101-2026-11-23/verification.json`](../evidence/axp2101-2026-11-23/verification.json).

These are device-protocol checks. A particular board's original-firmware boot,
display and interaction are separate acceptance steps and are not claimed here.

# StampFly VL53L3CX digital model

The model implements the register/interrupt subset used by the unchanged StampFly ST driver. It does not execute the sensor's private MCU or simulate optics, vehicle motion, collisions or environmental noise. Board integration and execution of the ESP32 firmware are separate checks; the reference-driver test below is not an ESP32 boot result.

## Pinned sources

- [M5StampFly source, commit 2b96752c72c465c63b260996046c71c7675f81b2](https://github.com/m5stack/M5StampFly/tree/2b96752c72c465c63b260996046c71c7675f81b2): `src/tof.cpp` (two XSHUTs, front address change, medium/long modes and 33000us budget), `src/sensor.cpp` (ten bottom IRQ measurements).
- The same revision's `lib/vl53l3c/vl53lx_def.h`: ST API 1.2.8, revision 2578. `vl53lx_register_map.h`, `vl53lx_register_settings.h` and `vl53lx_ll_device.h` define addresses, masks and 1200us boot delay. `vl53lx_nvm.{c,h}` defines power/read sequencing and 50us/5us waits. `vl53lx_api_core.c` defines the phasecal patch and raw histogram decoding. `vl53lx_api_preset_modes.c` maps low-ambient bin sequences to registers 0x0064–0x0069. `vl53lx_core.c`, `vl53lx_core_support.c` and `vl53lx_tuning_parm_defaults.h` define timeout conversion, phase-to-distance arithmetic and nominal histogram gain 1987.
- [ST VL53L3CX datasheet](https://www.st.com/resource/en/datasheet/vl53l3cx.pdf), [UM2778](https://www.st.com/resource/en/user_manual/um2778-vl53l3cx-timeofflight-ranging-module-with-multi-object-detection-stmicroelectronics.pdf): public product and API context. Device identification EA/AA/10 is the public VL53L3CX identity also reported in [ST's VL53L3CX measurement discussion](https://community.st.com/imaging-sensors-49/vl53l3cx-measurement-never-ready-132925); it is not a copied per-device identifier.

The verifier checks an aggregate SHA-256 over all 76 `.c`/`.h` files in the pinned driver's directory (sorted filename, NUL, contents, NUL): `5fe2ab98f3879e6de905bf89a7d18233a8d23386c8202f6e25354d3c597fb49f`. The repository contains no downloaded firmware, ST driver copy, physical-device capture or serial number.

## Implemented behavior

- Independent shared states and physical bus pins; 16-bit big-endian register pointers, sequential access, dynamic 7-bit address at register 0x0001. XSHUT clears volatile state and restores address 0x29. The board must initially release XSHUT and advance guest cycles; the constructor starts disabled.
- Firmware boot status becomes ready after 1200us of guest time. Software reset cancels pending work. Power-off does not acknowledge I2C.
- NVM READN protocol with power-up and trigger delays. Calibration is explicitly synthetic: zero offsets/xtalk, nominal oscillator 0xBCCC (ST's published fallback), all SPADs enabled. Untriggered NVM output is 0xFF; this is a model policy, not a silicon measurement.
- Recognized ST phasecal patch payload, histogram single-shot/back-to-back modes, timing from programmed timeout/VCSEL period, active-low/high IRQ, interrupt clear/rearm, stream-counter rollover 255→128, and grouped-parameter ID. Missing/unrecognized patches, timed mode and malformed timing do not produce measurements.
- A completed result remains latched until interrupt clear. The next window begins on clear; this handshaked scheduling is a deterministic subset, not a claim about every private MCU pipeline delay.
- Host input `None` gives flat ambient bins and no target. `Some(mm)` accepts finite 0–4000mm and produces one ideal triangular periodic pulse, using the programmed even/odd bin sequences and VCSEL periods. Outputs are raw 24-bin data, including the split last-bin byte and phase calibration. The unchanged ST host algorithm performs target detection, histogram merging, phase unwrapping and distance calculation. A new input does not retroactively change a latched sample; host histogram history can delay stable new output.

The pulse strength, fixed low ambient, effective SPAD count and nominal calibration are idealizations. Arbitrary bin sequences, calibration procedures, alternative private patches, multi-target optics, reflectance/ambient inputs, multi-zone behavior, autonomous timed ranging and full silicon register semantics are outside this subset. Unknown writable registers retain bytes, not an invented device operation. Initial ST results may have no-wrap/invalid-phase status before the host has both even and odd samples.

## Reproduce and results

Supply an unmodified local archive/checkout of the pinned public source:

```sh
python3 docs/platform/evidence/stampfly-tof/verify.py /path/to/M5StampFly
```

The verifier compiles the Rust model and its unit tests directly, then compiles the exact ST C driver with only its OS/I2C platform layer replaced. It requires Python 3, `rustc` and a C compiler; it downloads nothing and removes temporary build artifacts. Six Rust tests cover XSHUT/boot/address isolation, time boundaries/IRQ polarity and latched data, stop/reset/unsupported modes, NVM timing, malformed guest inputs/pointer wrap, and stream rollover.

Reference-driver results: 20 scenarios / 516 frames pass. Both medium and long modes initialize with the factory sequence; each checks readiness before/after its deadline, low-level stream/GPH consistency, and stop. Static inputs include no target and 100, 250, 500, 1000, 1500, 2000, 3000, 4000mm. After two warm-up frames:

| Input mm | Medium output mm | Long output mm |
| --- | --- | --- |
| 100 | 99 | 99 |
| 250 | 257 | 257 |
| 500 | 496 | 496 |
| 1000 | 1007 | 1007 |
| 1500 | 1493 | 1493 |
| 2000 | 1995–2004 | 2004 |
| 3000 | 2967–2986 | 2997 |
| 4000 | 3963–3983 | 3993 |

No-target scenarios return zero objects. The 260-frame run verifies rollover; the 40-frame run changes 500→1000mm→no target and verifies settled outputs. These finite samples establish digital compatibility and bounded error for this ideal pulse, not a hardware accuracy specification. The public `GetMultiRangingData` implementation overwrites an internal error return, so the harness also calls the low-level stream/GPH checker rather than relying on its API return alone. Standalone Clippy with warnings denied passes; integration workspace/WASM checks remain the integration owner's responsibility.

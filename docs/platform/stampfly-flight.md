# StampFly flight environment

`stampfly-rigid-body-v1` is an explicit, uncalibrated flight environment for the two StampFly boards. It consumes the four **guest-routed LEDC PWM outputs** and supplies ideal BMI270 and VL53L3CX environmental inputs through the existing physical SPI/I2C devices. It never writes motor duty, firmware memory, guest control targets, or synthetic serial messages. A READY/ACK or the factory's “Enjoy Flight!” is not evidence of takeoff; assess actual guest sensor reads, motor outputs, height and contact together.

## API and lifecycle

```js
await api.pause();                    // if already running
await api.configureFlight({enabled:true});
await api.start();                    // explicitly boot again after a mode change
const state = await api.readFlight(); // also observe().flight
const unsubscribe = api.subscribe(event => {
  if (event.type === 'flight') console.log(event.flight);
});
```

`configureFlight` accepts exactly `{enabled:boolean}` on StampFly and StampFly v1.1. It rejects running/loading devices. Changing the value with loaded firmware recreates the guest at the ground in `ready`, preserves persisted Flash, releases inputs, and requires an explicit start. An unchanged setting is idempotent. Configuration before loading is allowed. `reset()` keeps the setting and starts a new ground state; pause/resume freezes/continues both guest and plant. Firmware replacement starts at ground; product configuration uses the new scene setting. Dispose clears the state. Disabled `readFlight()`/`observe().flight` is `null`. `setImu()` is rejected while flight owns sensor input. `observe().imu` reports `source:flight`, the actual raw inputs and guest time; its generic display-orientation quaternion is `null` (use the explicitly FRD/NED flight quaternion instead). Before the first flight sample its arrays are `null`.

Portable scene configuration is optional:

```json
{"flight":{"enabled":true,"model":"stampfly-rigid-body-v1"}}
```

Only that configuration is shared. A share contains no pose, accumulated motor speed, telemetry, UART commands or pending control mode. A shared enabled scene boots at ground with the chosen firmware; it does not replay a takeoff. Loading the demo firmware alone does not enable the plant. Flight recording/replay rejects with `FLIGHT_RECORDING_UNSUPPORTED`, because the current semantic recorder does not record UART commands. The embed bridge exposes `configureFlight` and `readFlight` with its existing parent-origin/session checks.

The snapshot contains `enabled`, `model`, `calibrated:false`, `source:'guest-pwm-rigid-body'`, `epoch`, `cycle`, `guestTime`, `worldFrame:'NED'`, `bodyFrame:'FRD'`, `positionM`, `velocityMps`, `quaternionXyzw`, `angularVelocityDpsFRD`, `heightM`, `contact`, `motors` and `sensors`. Quaternion maps body to world. Initial position is `[0,0,0]`; height is COM displacement above its initial resting height. `motors` are FL/G5, FR/G42, RL/G10, RR/G41, each with observed `duty`/`frequencyHz` and modeled `omegaRadS`/`thrustN`. These speeds and forces are model results, not physical measurements. `sensors` has raw BMI `rawAccelG`/`rawGyroDps` and mounted-ray `bottomDistanceM`/`frontDistanceM` (`null` means no target).

The existing `motors` event remains a PWM-only observation with `flightSimulated:false`; the separate `flight` event identifies the optional plant. No pose is inferred from PWM-only snapshots. Snapshot publication is wall-time throttled; integration never is. Epoch increments per newly created flight worker. Stale workers, epochs and backward cycles are rejected by the runtime.

## Time and causality

The disabled worker calls the original WASM run export directly and constructs no plant. The cold Board methods are appended to preserve existing hot vtable offsets; there is no new bus/peripheral tick dispatch.

Enabled execution stops at nominal 1 ms guest-cycle boundaries. The plant advances through the elapsed interval using **previously observed** PWM, then samples new routed PWM for the next interval and supplies sensor inputs. The rigid body and motor ODE use fixed 250 µs steps. Instruction execution can overshoot a requested boundary; a completed physics step can trail the observed cycle by less than 250 µs. There is therefore bounded sample/hold delay, not cycle-exact electromechanical coupling. Host wall time neither advances a paused plant nor substitutes a control signal. Repeated same-time observation cannot move it.

The BMI input is specific acceleration `R⁻¹(a_world − gravity)/g` and body angular velocity. Sensor/FRD mounting uses `FRD = [rawY, rawX, −rawZ]`, also for gyro; supported rest reads raw Z = +1 g. The existing BMI range selection, SPI chip select, configuration/ready behavior and quantization still apply. ToF input changes only the ideal target of the raw histogram generator; firmware must start conversion and process the unchanged ST driver, including invalid startup frames and history filtering. Sensor supply, XSHUT, IRQ and physical routes remain authoritative.

## Source and approximations

Propulsion constants and the complete 101-point `R_out(D)` table are adapted from [M5Fly-kanazawa/stampfly_ecosystem](https://github.com/M5Fly-kanazawa/stampfly_ecosystem/tree/ac0ed6f67d609a6f7e989f61be0a6d1e0e311906), fixed revision `ac0ed6f67d609a6f7e989f61be0a6d1e0e311906`:

- [Physical parameters](https://github.com/M5Fly-kanazawa/stampfly_ecosystem/blob/ac0ed6f67d609a6f7e989f61be0a6d1e0e311906/control/models/stampfly_physical.yaml): mass 0.037 kg, inertia `[9.16e-6,1.33e-5,2.04e-5] kg m²`, arm offsets ±0.023 m.
- [Generated coupled propulsion data](https://github.com/M5Fly-kanazawa/stampfly_ecosystem/blob/ac0ed6f67d609a6f7e989f61be0a6d1e0e311906/simulator/sils/plant/generated_params.hpp): complete `bench_2026_10_15` set, not mixed with the historical linear-duty motor model.
- [Plant](https://github.com/M5Fly-kanazawa/stampfly_ecosystem/blob/ac0ed6f67d609a6f7e989f61be0a6d1e0e311906/simulator/sils/plant/plant.cpp): motor equations and 0.05 m reliable-close-range constant.
- [License](../../web/wasm/stampfly-ecosystem-LICENSE.txt): MIT, Copyright (c) 2026 Kouhei Ito. The license is distributed with the browser data.

Motor current is `(V − Ke ω)/R_out(D)`, or zero below duty 0.01. Rotor acceleration follows `J ω̇ = Kt I − Cq ω² − Kt(I0a + I0b ω²)`. Thrust is `0.976 Ct ω²`. Voltage is fixed at 4.0 V to match this board's existing ideal INA3221 input. Steady level hover from this parameter set is duty **0.6124589856**, before guest PWM quantization. Motor lag, reaction torque and quaternion rigid-body motion are integrated; duty does not directly set pose.

Uncalibrated scene assumptions are kept in `stampfly-parameters.js`: flat floor; no wind; constant battery; linear drag 0.015 N/(m/s), angular drag 2e-6 N m/(rad/s); four ideal support points `(±23,±23,+40) mm` relative to COM. Inelastic contact uses projected normal impulses and approximate viscous ground friction. Bottom sensor mount is `(0,0,+5) mm`, front `(25,0,0) mm`; these are explicit approximations, not recovered component placements or CAD collision shapes. Floor is 40 mm below initial COM, so bottom mounted-ray distance initially equals 35 mm.

Each beam intersects the floor from its transformed mount. A hit requires its down direction cosine ≥0.5 and distance **50–3000 mm**. This 50 mm cutoff is a **conservative scene visibility rule**, not the ST driver's hard limit: the unchanged ST harness can produce a valid approximately 42 mm reading from an ideal 35 mm input. It is not clamped to 50 mm. The model's ground target is consequently absent until it rises about 15 mm; tilted, upside-down, out-of-range or upward-facing beams can be absent. The front beam can see the floor only when physically aimed down enough. No wall target is invented.

The scene and sensor models are not validated against either hardware revision for actual flight. No optical flow, magnetic heading, barometer, realistic optics, aerodynamics/ground effect, mesh collisions, battery sag or ESP-NOW link is added. Attitude/height control must run in guest firmware. Without optical flow or another horizontal observation there is no claim of stable horizontal position hold. Original factory defaults and their radio limitation remain separate from the UART demonstration.

## Focused checks

`node --test tools/platform-flight-test.mjs tools/platform-flight-runtime-test.mjs` covers ground support, motor lag, torque signs, deterministic subdivision, sensor coordinate mapping, mounted rays/invalid visibility, lifecycle, malformed/stale messages, disabled path and recording boundaries. The transport tests use an explicitly synthetic worker only to test API validation; they are not firmware execution evidence. Native `board::stampfly::tests` covers finite atomic input, no motor writes, physical SPI/I2C routing and unchanged default output. Real worker/firmware receipts are required separately for takeoff/hover claims.

Timing history: EX117 concerns presentation cadence; EX248 fixes S3 FPU reset. This optional guest-cycle plant is a different mechanism and reuses both existing behaviors without changing the execution core. CPU comparisons and real closed-loop receipts belong to the integration experiment, not these unit checks.

The [real runtime lifecycle receipt](evidence/stampfly-flight/runtime-lifecycle.json) uses the earlier UART demo image (`dba1e1b…`), retained as historical evidence, through full ROM on WASM `4d8d85eb…`. Both SKUs produced fresh guest MOTOR acknowledgements, routed 0.65 duty, about 0.053 m height and mounted-ray distance about 0.088 m. STOP produced zero PWM followed by rotor coast, gravity and ground contact; pause, reset and disabling were checked in the same sessions. All six workers were disposed. This is a manual PWM/plant check, not an AUTO hover result. Reproduce with `tools/probe-stampfly-flight-runtime.mjs`; it requires caller-supplied integration root, demo artifact, expected WASM SHA and receipt path.

The [original factory off-mode regression](evidence/stampfly-flight/factory-disabled.json) reboots both exact official images through ROM and verifies fresh G0-triggered software reset and second initialization in the same worker. The [legacy peripheral fixture](evidence/stampfly-flight/legacy-fixture-disabled.json) separately verifies physical routes, three RGB outputs, four PWM channels, input, pause and reset while flight remains disabled. [Provenance](evidence/stampfly-flight/runtime-provenance.json) retains source hashes and a first probe failure caused by requiring the combined host/guest console to be empty; the corrected assertion checks the actual guest UART0 suffix. No firmware or runtime behavior was changed for that correction.

The current public image is `b5f9665e…` (349328 bytes), built from source revision `58824a07`. Its [final closed-loop receipt](evidence/stampfly-uart-demo/flight-final.json) verifies both SKUs through takeoff, height targets, combined roll/pitch/yaw targets, return to level and landing with guest STOP and exact zero PWM. Separate checks cover 0.10/1.50m height bounds and explicitly test-only ToF target loss after established hover. The [final manifest regression](evidence/stampfly-uart-demo/manifest-final.json) repeats motor, RGB, stop, pause and reset through the current public manifests. Historical dba1 motor receipts do not certify its failed attitude controller; the [retained negative](evidence/stampfly-uart-demo/control-negative-01.json) and [source/rebuild record](evidence/stampfly-uart-demo/README.md) distinguish the revisions.

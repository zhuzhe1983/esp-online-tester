# StampFly motor observation

The `stampfly` and `stampfly-v11` boards publish four routed guest PWM outputs. `observe().motors` and the `motors` event expose `{id, gpio, duty, frequencyHz}`. Duty is a normalized electrical output, not a motor-speed measurement. The front end never writes motor output from user input, serial logs, IMU input, or model animation.

| ID | GPIO |
| --- | --- |
| `front-left` | 5 |
| `front-right` | 42 |
| `rear-left` | 10 |
| `rear-right` | 41 |

The board's motion snapshot contract is:

```json
{
  "type": "stampfly-motors",
  "version": 1,
  "cycle": 240000000,
  "clockHz": 240000000,
  "model": "latched-pwm",
  "flightSimulated": false,
  "motors": [
    {"id": "front-left", "gpio": 5, "duty": 0.25, "frequencyHz": 150000},
    {"id": "front-right", "gpio": 42, "duty": 0.5, "frequencyHz": 150000},
    {"id": "rear-left", "gpio": 10, "duty": 0.75, "frequencyHz": 150000},
    {"id": "rear-right", "gpio": 41, "duty": 1, "frequencyHz": 150000}
  ]
}
```

The worker adds `t: "motion"`. `clockHz` describes the board's monotonic 240 MHz cycle clock; each `frequencyHz` describes its own PWM carrier. `version` changes with output state; `cycle` continues as the guest executes. The public `motorOutput` object retains this metadata and adds `guestTime = cycle / clockHz`. Invalid shapes, wiring, ranges, clocks and stale observations are ignored. Observations from unrelated boards are ignored. Reset, worker replacement and product switching clear the state and its monotonic history; a new worker may start again at version/cycle zero. Pause retains the observed duty values.

Model metadata declares `presentation.rotors` as `{id, parts, axis, pivotMm, direction}` objects. `parts` lists exact direct-child mesh names, `axis` is the local rotation axis, `pivotMm` is the original assembled pivot, and `direction` is `1` or `-1`. Grouping preserves the source mesh geometry and its initial world transform. Missing meshes or duplicate ownership reject the declaration.

Rotor angle advances only when another accepted observation advances guest time while running. The preceding duty scales an arbitrary visual rate of 18 radians per guest second. There is no wall-clock spin loop, flight pose, calibrated RPM, thrust, aerodynamic response, or real-hardware validation. Pausing freezes the presentation, resuming establishes a fresh time baseline, and clearing observations restores the original assembly. Both versions expose explicit host-fed BMI270 virtual inputs through `setImu`; scene-to-sensor mounting is unverified and scene-sourced input is rejected.

## Regression checks

`node --test tools/platform-motor-test.mjs tools/platform-imu-test.mjs tools/platform-stackchan-runtime-test.mjs tools/platform-servo-view-test.mjs tools/platform-manufacturer-model-test.mjs` passes 21 tests. They cover observation isolation, wrong boards, malformed values, stale workers, pause/reset, explicit IMU delivery, rotor pivots, and worker publication. These fixtures do not execute StampFly firmware.

Serve the repository root and open `tools/fixtures/stampfly-motor-view.html` headlessly through the Playwright wrapper. This retained fixture uses the actual manufacturer model loader and scene view with simple synthetic geometry. Capture `#mount` before advancing, after one advance, after pause plus advance, and after reset. The first and advanced images must differ; the paused image must equal the advanced image; the reset image must equal the first image. This verifies rendering transport, not product geometry or firmware behavior. Actual StampFly firmware acceptance must be recorded separately.

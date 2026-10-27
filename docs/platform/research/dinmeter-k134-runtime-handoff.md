# DinMeter K134 runtime research checkpoint

Status: **research only; no runtime implemented or verified by this checkpoint**.
Base: `7d03d2409912516030412d715454da8dd8e4bc93`.
Scope: original **K134**, excluding **K134-V11**. The catalog's unavailable status is unchanged.

## Sources already fetched

GET receipts (URL, resolved URL, media type, UTC timestamp, byte count and SHA-256) are retained in [dinmeter-k134-runtime-sources.json](dinmeter-k134-runtime-sources.json). Raw downloads are disposable inputs in `output/dinmeter-sources/`, excluded from Git. Re-fetch using each receipt's URL and compare its hash if those inputs are absent.

- [Original product documentation](https://docs.m5stack.com/en/core/DinMeter).
- [DIN_Meter_v1.0 schematic](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/500/DIN_Meter_v1.0.pdf), one page. Downloaded and text inspected; **visual connectivity inspection remains pending**.
- M5DinMeter commit `8ccbad7297beb82a140f812e5606e060d8a5e0dc`: `src/M5DinMeter.cpp`, `src/M5DinMeter.h`, `src/utility/Encoder.cpp`, `src/utility/Encoder.h`, `examples/Basic/display/display.ino`.
- Additional driver references already present in the AtomS3/AtomS3R evidence chain: M5GFX `65c816a38386062d2c3dc47d41237a9083acbc75` and M5Unified `368ad60807161625aa65baad37a4a3af2a7eccea`. Retrieve their relevant files and record new receipts before implementation.

## Documented connections

| Function | Original K134 connection | Implementation boundary |
| --- | --- | --- |
| MCU | Stamp-S3 / ESP32-S3FN8, 8 MB flash | PSRAM is not documented; confirm package facts before setting 0 MB |
| LCD | ST7789V2, native 135 × 240 | Catalog/model use physical landscape 240 × 135 |
| LCD SPI | MOSI5, SCLK6, CS7, DC4, RESET8 | Use physical SPI routes, not a generic board alias |
| Backlight | GPIO9 | Determine how existing LEDC route observation can represent PWM without adding generic per-tick work |
| Encoder | A41, B40, both pulled high | Reuse existing queued quadrature, after confirming physical direction |
| Encoder push | GPIO42, active low | Independent button, not touchscreen input |
| Internal I2C | SDA11, SCL12 | RTC8563 remains unimplemented unless added and checked explicitly |
| Buzzer | GPIO3 | Audio may be an explicit first-pass omission |
| Battery hold | GPIO46 | USB-powered scope is possible; do not pretend battery latch/charger behavior is implemented |
| Grove A | GND, 5V, G13, G15 | Keep existing source-backed metadata and wire ordering |
| Grove B | GND, 5V, G2, G1 | Same |

Product documentation states that battery-only operation requires GPIO46 HIGH to retain power after wake. USB power changes that behavior. The original has ST7789V2/Stamp-S3; K134-V11 uses ST7789P3/Stamp-S3A and must remain a separate profile.

M5GFX's original DinMeter configuration uses 240 × 320 ST7789 GRAM, panel width135/height240, offsets52/40 and offset_rotation2. **The fixed physical landscape crop/rotation is not yet established.** Derive it from the official panel transform and factory display orientation, then verify corner colors in a real guest fixture. Do not copy Cardputer's crop without checking the different rotation offset.

## Encoder behavior and shared contract

`M5DinMeter.h` constructs `ENCODER(41,40)`. `Encoder.h` stores pin1 in bit0 and pin2 in bit1, initially11. Its transition table decrements on `11 → 10 → 00 → 01 → 11`, totaling -4. Existing `Machine` clockwise scheduling emits CLK LOW, DT LOW, CLK HIGH, DT HIGH; if CLK=A41 and DT=B40 this yields that -4 count. **The electrical table alone does not establish the hardware's physical clockwise sense.** Confirm it from manufacturer evidence or retain this direction limitation explicitly; do not reverse pins just to force a positive guest count.

The concurrent Dial implementation owns common encoder API, renderer, schema, replay and exporter changes. Proposed integration contract:

```js
api.input({ type: 'encoder', id: 'encoder', steps: 1 });
```

- `steps` is a nonzero integer from -64 to64; positive means clockwise viewed from the front.
- Runtime capability: `runtime.encoder: true`.
- Product control: `{id:'encoder', type:'encoder', name:'Rotary encoder', pins:{a:41,b:40}, detents:16}`. **16 is the proposed UI contract, not a verified K134 mechanical detent count.** Confirm or omit that product fact.
- Separate ordinary push control remains `encoder-button` / GPIO42.
- CAD selected part and emitted part use `encoderId:'encoder'`; shared renderer handles the blue rotation highlight and angular drag. Push uses `controlId:'encoder-button'`.
- Existing machine transport `{t:'knob',d:N}` queues 2 ms between phases and 16 ms per detent. No new hot-path scheduler is required.

## Next implementation steps

1. Inspect the schematic visually, fetch immutable M5GFX/M5Unified/factory references, settle screen orientation and encoder phase direction. Check current open PRs before adding shared pieces.
2. Implement a distinct `dinmeter` Rust board with physical SPI pins, documented DCS dimensions/ID, button42 and encoder41/40. Declare power/RTC/buzzer/ADC limits honestly; do not enable K134-V11 by association.
3. Register the board through WASM/worker/schema and update only K134 catalog, knowledge and model metadata. Coordinate shared encoder work instead of duplicating it.
4. Add an internal source-built ESP-IDF fixture and real-WASM probe: distinguish corner colors, button press/release, both encoder directions and queued detents as observed by guest code. Keep binaries out of published assets.
5. Set the existing `encoder-knob` CAD component's button/encoder identifiers in both model and model-plan. Its current bounds are `[8.239498,-6.010504,9.2]` to `[20.260502,6.010504,15.700001]`, axis Z. The glass/LCD area is not a touch input.
6. Run the public `tools/rebuild-product-model.py` scratch rebuild and preserve existing failed envelope/photo comparisons and their timestamps. This task has not remeasured the model or fixed its known visual limitations.
7. Run required Clippy, Rust, Node, catalog, real browser and evidence checks; benchmark only if generic bus/per-tick machinery changes. Browser work should use a task-owned headless wrapper session and port8094, then close both session and server.

No source-built firmware, WASM rebuild, browser session, server, runtime test or implementation change was produced in this checkpoint.

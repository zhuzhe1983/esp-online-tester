# Five M5Stack S3 product corrections, 2026-11-26

| Product | Corrected behavior | Boundary |
| --- | --- | --- |
| Capsule K129 | G42 wake and G0 boot; G21 hardware-RMT RGB; BMI270 at **0x69**, BM8563 at 0x51 on SDA8/SCL10; actual wake-cap mesh is a button, separate from shell appearance | Virtual always-on supply. HOLD46, RTC alarms/wake, microphone, IR, buzzer and SD transport absent |
| Capsule v1.1 K129-V11 | Separate board identity and the same documented host wiring; Stamp-S3A RGB supply additionally requires driven-high G38 | Does not inherit the older Capsule's firmware identity; analog power and wake circuit absent |
| AtomS3U K125 | G41 button and G35 hardware-RMT WS2812; source-photo-bounded front flexure hit surface | PDM microphone G38/G39, IR G12, USB-OTG and external electrical routing absent. 8 MiB is the explicit simulation configuration matching the catalog's official 8MB UIFlow target; not a hardware measurement |
| Cardputer Mesh Kit K152 | Exact, manufacturer-documented **Cardputer-Adv host** reuses its board implementation, TCA8418 input and 56 independently clickable original CAD keys | Cap SX1262 LoRa and GPS expansion unimplemented. Product/firmware/storage identity stays K152; official firmware unexecuted |
| Xiaozhi Card Kit K146 | Corrects `touch:false` to its documented FT6336U capacitive touch; records SDA2/SCL1/IRQ16/reset47 and G21 wake button | Keeps `runtime.board:null`, `status:unavailable`: this factual correction does not create an EPD, power or touch runtime |

Official HTML pages and five schematics were actually fetched; byte counts, SHA-256, final URL and original UTC receipts are in `source-receipts.json`. Historical source records remain unchanged. Raw marketing pages, firmware binaries and personal publisher metadata are not republished. `firmware-survey.json` records the per-product factory/UIFlow/Xiaozhi investigation and existing fixed entries. No binary was downloaded or run for this report.

K152's keyboard carrier contains 39,180 source triangles. Its 56 caps each contain 250 triangles above the same documented source height threshold; the remaining 25,180 triangles stay in the carrier. The final model preserves every original triangle position exactly once, with normals regenerated from the pinned source/exporter. The normalized position-triangle multiset digest is recorded in `k152-keyboard-partition.json`; `model-plan.json` retains native STL source-face ranges for public regeneration; these are deliberately mapped independently of the old exported binary face order. The keyboard host mapping follows the documented Adv/TCA8418 host, including the explicit platform convention that host AltRight represents Fn.

Validation commands:

```sh
node tools/platform-m5-s3-products-test.mjs
node tools/platform-knowledge-test.mjs
python tools/validate-platform-catalog.py
python tools/rebuild-product-model.py build m5stack-k152 --scratch /tmp/m5-k152-rebuild
node tools/check-evidence-privacy.mjs
```

The mesh test loads all five actual local models, verifies texture/geometry bytes against their manifests, tests the nearest visible ray hit for all 56 K152 keys and the AtomS3U flexure, and checks key material isolation under gold and translucent-purple shells. Image decoding is stubbed; these Node checks do not claim GPU/browser, physical dimensions or official-firmware acceptance. Prior photo comparisons remain historical and do not certify the changed model. Final model hashes and observed check results are recorded separately in `verification.json`.

Rust tests in `capsule.rs` cover the exact I2C route/address, shared-controller calendar advancement, wrong address isolation, v1.1 RGB power loss/recovery, invalid IMU input and absent sibling peripherals. `atoms3u.rs` covers correct/wrong RGB pins, GRB bytes, short frames and absence of fake display/audio peripherals. They must run against the integrated final code before publishing; this lane does not claim an unobserved Rust or browser pass.

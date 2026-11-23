# SenseCAP Watcher XiaoZhi recovery execution

The independent XiaoZhi commercial edition now has a pinned official recovery
image and a real platform-worker execution receipt. It reaches the Wi-Fi
configuration page; GPIO encoder input changes the guest's volume, and a short
knob press enters the guest's `audio_testing` state. These are partial display
and input results, not cloud voice, microphone, Himax, account or hardware
certification.

## Exact image and hardware configuration

The manufacturer [recovery guide](https://wiki.seeedstudio.com/visual_trigger_and_ai_flash/)
links the [English merged image](https://files.seeedstudio.com/wiki/Watcher_Agent/SenseCraftAI/merged-binary-en.bin).
The fetched image contains `xiaozhi 2.0.4`, ESP-IDF `v5.5.1`, chip ID 9:

| Item | Observed value |
| --- | --- |
| Download size | 13,628,728 bytes |
| SHA-256 | `329fd7ad510ab9bce012b7ed3ff138d6223f675016373be8e66e6e6357122961` |
| Full image load address | `0x0` |
| Partition table | `0x8000` |
| Application | `0x200000` |
| Declared flash / PSRAM | 32 MiB / 8 MiB |
| Display | 412 × 412, SPD2010 QSPI |
| Encoder / press | GPIO41 / GPIO42; expander P0.3 |

The AI Vision edition has a different recovery layout. The XiaoZhi image's
device identity area is blank; the platform does not fabricate identity keys.
The recovery version is not evidence of the exact software shipped in every
manufacturing batch. The catalog therefore preserves the factory investigation
separately from the hash-pinned recovery project used as its explicit default.

## Polled encoder timing

The unchanged firmware reports `espressif/knob` version 1.0.1. Its
[fixed Kconfig](https://github.com/espressif/esp-iot-solution/blob/63e9a518c823a9912bb90610891407496abb4e8c/components/knob/Kconfig)
defaults to a 3 ms scan and two debounce samples. The matching
[decoder](https://github.com/espressif/esp-iot-solution/blob/63e9a518c823a9912bb90610891407496abb4e8c/components/knob/iot_knob.c)
reads real GPIO levels. The upstream [Watcher application](https://github.com/78/xiaozhi-esp32/blob/v2.0.4/main/boards/sensecap-watcher/sensecap_watcher.cc)
registers volume callbacks; this source tag is a review reference, not an
assertion that it reproduces the manufacturer's binary byte for byte.

The old 2 ms host phase could disappear between polling/debounce samples. An
actual WASM run showed no volume callback. The page title scrolls by itself,
so its changing pixels did not establish accepted rotation.

`BoardModel::encoder_phase_ms()` now supplies a 20 ms slow-turn stimulus for
Watcher, shared by browser messages and native `knob` scripts. Other boards
retain 2 ms. No firmware state, volume value or display text is injected.
The queue remains bounded at 64 cycles / 256 GPIO edges per message. Including
the inter-cycle gap, Watcher's maximum message can occupy 10.24 guest seconds;
it is no longer described as one second. The chosen duration is an input
policy, not a measured mechanical detent period.

## Final actual worker observations

Production WASM SHA-256:
`1002b80d7336ee0e610eedad8b9688fc5389bb1bcaeadf48c02d279069181a24`
(6,026,506 bytes). The [receipt](evidence/seeed-xiaozhi-20261122/receipt.json)
records the unchanged firmware, exact source hashes, worker/probe hashes,
ROM boot, guest instruction counts, framebuffer hashes and input times.

| Action | Guest result |
| --- | --- |
| Boot for 15 guest seconds | Wi-Fi configuration page on the real 412 × 412 framebuffer |
| Positive four-edge encoder cycle | Volume 70 → 65 → 60 |
| Negative four-edge encoder cycle | Volume 60 → 65 → 70 |
| Knob press for 0.2 guest seconds, release, settle 1 second | Application enters `audio_testing`; no additional pixel change is required for that conclusion |

This decoder produces two callbacks per four-edge cycle. Mechanical detents
per revolution are not calibrated. The worker combines USB/UART logs, so the
curated receipt collapses consecutive duplicate complete markers and ignores
fragmented state prefixes. The separate native UART-only cross-check observes
the same four volume transitions.

The configuration frame includes a generated hotspot identifier. Public
evidence keeps exact pixel counts and RGBA hashes, without publishing the
frame or raw serial stream. [Before/after and native evidence](evidence/seeed-xiaozhi-20261122/before-and-native.json)
also retains the unsuccessful 2 ms observation and the hash of an invalid
16 MiB native invocation. The latter was a harness configuration error: the
guest rejected a flash size smaller than its binary header. The corrected
32 MiB invocation booted normally; the public catalog already used 32 MiB.

An earlier native report read stale cached backlight output before refreshing
the board. The CLI now refreshes latched outputs before printing its report,
matching screenshot capture and the worker. Actual LEDC output was present;
no unsupported hardware-fade behavior was added to make the page appear.

## Reproduction and limits

```sh
tools/wasm-build.sh
ESP32SIM_ROM_DIR=target/roms node tools/probe-seeed-runtime.mjs \
  --product seeed-sensecap-watcher-xiaozhi --seconds 15 \
  --inputs docs/platform/evidence/seeed-xiaozhi-20261122/inputs.json \
  --output target/seeed-xiaozhi
cargo test -p esp32s3 --test host_input
```

For offline reproduction, pass `--local-map` with an investigator-supplied
mapping of the exact public image URL to a downloaded file, plus
`--local-only`. The platform still verifies the catalog SHA. Source HTTP
receipts are in [sources.json](evidence/seeed-xiaozhi-20261122/sources.json).

The GPIO/I²C display-input subset does not establish microphone sampling,
speaker sound, acoustic echo cancellation, Himax inference, cloud enrollment,
real Wi-Fi radio behavior, battery life, hardware flashing or full touchscreen
functionality for this firmware. `audio_testing` is an observed application
state only. No physical device was used.

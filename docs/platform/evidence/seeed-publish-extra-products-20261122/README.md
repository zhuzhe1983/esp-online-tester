# Final independent-product execution observations

Three separate catalog products were booted with the production WASM below on
2026-11-22. All three probe processes exited zero. The original firmware bytes
were not patched; each run used the real platform worker, the ESP32-S3 ROM boot
path, `wasm-jit`, and no application-direct boot or guest stubs. The detailed
identities, input events, actual guest times, pixel hashes, serial hashes and
bounded conclusions are in [receipts.json](receipts.json).

These are fresh observations for this artifact. Historical captures are used
only for explicit byte comparisons. This evidence does not upgrade the products
from partial support to full hardware compatibility.

## Artifact identity

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `web/wasm/esp32sim.wasm` | 6,056,179 | `a3f0c90c47f9403225084c7138a3ec6c6b7ab5d2d9a0d1e6d3db8938a7379a75` |
| `web/wasm/worker.js` | 31,922 | `f2903f82105003582d2ccde163c3dbfbc1c61d980c542fd9c0a5a69803af960d` |
| `tools/probe-seeed-runtime.mjs` | 9,819 | `8cf822899f24ebff4a6f5295dee5f2c66f19fd32b67f05e6acedfa8518b34429` |

Both Indicator variants use the original Seeed
[factory v1.0.0 merged image](https://github.com/Seeed-Solution/SenseCAP_Indicator_ESP32/releases/download/v1.0.0/Default_Factory_Firmware_ESP32-S3_v1.0.0.bin):
2,036,000 bytes, SHA-256
`749c9b77222405e2b2ffc1a08f3d51a0752e762e10b6bba6c426f5440d638cf2`.
Its app descriptor reports `terminal_demo-v1.0.0-11-g82ddd9`. Each independent
board receives an 8 MiB flash image. The source image is shared, while each
product has its own board and manifest identity.

The Watcher Xiaozhi product uses Seeed's
[English merged image](https://files.seeedstudio.com/wiki/Watcher_Agent/SenseCraftAI/merged-binary-en.bin):
13,628,728 bytes, SHA-256
`329fd7ad510ab9bce012b7ed3ff138d6223f675016373be8e66e6e6357122961`.
Its app descriptor reports Xiaozhi `2.0.4`; the runtime flash size is 32 MiB.
The ROM ELF is 949,552 bytes, SHA-256
`c0ce0f338d1de1bdc6efbef1591779a2a42c1ab7d759d3c6ae8ae63a7dd34cfd`.
All three distinct firmware/ROM source files were read again and checked against
these hashes during curation.

## Observed results

| Product | Final guest time | Actual guest frames reported | Result |
| --- | ---: | ---: | --- |
| `seeed-sensecap-indicator-d1l` | 9.06 s | 567 | Wi-Fi list; touch at `(200,136)` opens the password keyboard. 205,305 pixels change. |
| `seeed-sensecap-indicator-d1pro` | 9.04 s | 566 | The same visible factory workflow, independently booted on its own board. 205,305 pixels change. |
| `seeed-sensecap-watcher-xiaozhi` | 17.12 s | 1,045 | Wi-Fi configuration page; encoder volume transitions in both directions; short press reaches `audio_testing`. |

For each Indicator, the touch is held for 0.2 guest seconds and followed by a
0.8-second settle request. Both the 480 × 480 boot capture and the subsequent
keyboard capture are byte-identical to their corresponding historical runs on
WASM `f3ad2ef68a5e758c9e19d2cb237ff297a2bac557ad22e94f5c41296abdefacc0`.
Each comparison covers all 921,600 RGBA bytes. The boot hash is
`f474347c2d4da91954933422d9c10bd29f002d0375262fa15fc794bd04a6b66d`;
the keyboard hash is
`7f7e7e30c3bf2d0d368e05d358413f3c9527089e2c83d7c02f05d7d852eb0b24`.
The synthetic network fixture supplies the access-point list; this is not an
internet association or cloud-service test.

The radio remains **held in reset** in both Indicator factory runs. At boot and
after the touch, it reports `powered=true`, `resetHigh=false`, `busy=true`,
`config=null`, mode 32, and zero IRQ, transmit, receive, timeout, rejected-command
and dropped-transmit counters. These states match the earlier runs apart from
guest timestamps. The original factory UI therefore does not verify LoRa
initialization or packets. The separate Meshtastic product's results are not
substituted for either factory run.

For Xiaozhi, the clockwise request produces guest volume transitions
**70 → 65 → 60**, and the counterclockwise request produces **60 → 65 → 70**.
The requests drive the modeled encoder's GPIO phases; they do not set a firmware
volume variable. A 0.2-second knob press with a 1-second settle request then
produces the guest state `audio_testing`. The last capture changes zero pixels
from the preceding one: the state marker, rather than a fabricated visual
transition, is the evidence for the short press.

These numeric transitions and the state marker were extracted again from the
current serial files. Consecutive duplicate matches in the mixed console stream
were collapsed; duplicate text is not used to estimate callback counts. The
ordered transitions match the historical run on WASM
`1002b80d7336ee0e610eedad8b9688fc5389bb1bcaeadf48c02d279069181a24`.
Each of the three input-result captures is also byte-identical to its historical
counterpart across all 678,976 RGBA bytes. A request with `steps=1` is not claimed
to change volume by only five: the actual guest reports two successive changes.
This verifies the observed digital control/state path, not microphone capture,
acoustic playback, a cloud conversation or Wi-Fi association.

The Xiaozhi **boot captures are not byte-identical**. The current capture at
15.00 guest seconds differs from the earlier 15.06-second capture by 3,741
pixels, all within `[52,53,361,82]` (exclusive upper bounds). Private visual
inspection shows the same Wi-Fi configuration page with a different title-text
position. All pixels outside that rectangle are identical. This bounded
observation is retained rather than replacing it with an exact boot-frame pass.

## Verification and reproduction

Curation read and SHA-256 checked all 16 current/historical RGBA files, all 16
per-capture serial files, and the six final serial files. RGBA sizes equal
`width × height × 4`. Previous-capture pixel counts were recomputed using whole
four-byte pixels. Comparisons used no resampling, rotation, channel tolerance or
image correction. The current/historical source artifacts, initial-flash
identities, manifest hashes and requested input payloads are equal for each
product. Returned guest input times, instruction counts and frame counts are
recorded separately; this is not an equal-work performance benchmark.

The committed input fixtures below are byte-identical to those used in these
runs. Supply `SEEED_SOURCE_MAP` as a local URL-to-file map for the exact pinned
artifacts in the receipt; local cache paths are deliberately not published.
Install the ROMs and build the platform as described in the repository setup
instructions, then run from the repository root:

```sh
node tools/probe-seeed-runtime.mjs \
  --product seeed-sensecap-indicator-d1l --seconds 8 \
  --local-map "$SEEED_SOURCE_MAP" --local-only \
  --inputs docs/platform/evidence/seeed-final-wasm-20261122/inputs/seeed-sensecap-indicator.json \
  --network-json docs/platform/evidence/seeed-final-wasm-20261122/inputs/indicator-network.json \
  --output /tmp/seeed-publish-check/d1l

node tools/probe-seeed-runtime.mjs \
  --product seeed-sensecap-indicator-d1pro --seconds 8 \
  --local-map "$SEEED_SOURCE_MAP" --local-only \
  --inputs docs/platform/evidence/seeed-final-wasm-20261122/inputs/seeed-sensecap-indicator.json \
  --network-json docs/platform/evidence/seeed-final-wasm-20261122/inputs/indicator-network.json \
  --output /tmp/seeed-publish-check/d1pro

node tools/probe-seeed-runtime.mjs \
  --product seeed-sensecap-watcher-xiaozhi --seconds 15 \
  --local-map "$SEEED_SOURCE_MAP" --local-only \
  --inputs docs/platform/evidence/seeed-xiaozhi-20261122/inputs.json \
  --output /tmp/seeed-publish-check/xiaozhi
```

Asynchronous capture polling can overshoot the requested guest time; the receipt
retains each actual capture time. No fresh emulator, native build or benchmark
was run during this docs-only verification. Raw serial, network state, storage
keys and device identifiers are omitted. No new PNG or raw framebuffer is
committed; original receipt and serial hashes preserve provenance without
publishing those bodies.

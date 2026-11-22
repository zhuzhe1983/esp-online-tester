# Meshtastic final WASM: stable pixels, incomplete application progress

The unmodified SenseCAP Indicator Meshtastic recovery image now produces a
stable startup framebuffer in two independent runs. All nine saved captures
are equal byte for byte. The application still has no demonstrated exit from
the startup screen through 20.36 guest seconds, and the requested short press
has no observed guest handling marker. Compatibility remains **partial**.

[receipts.json](receipts.json) retains the original receipt, firmware, ROM,
runtime, frame and serial hashes, input acknowledgements, radio snapshots and
the limits of each conclusion. It records the final production WASM, not a
mocked worker or a patched guest image.

## Two timing windows

Both runs execute the actual platform worker through Node `worker_threads`,
boot the same official ROM and three-part recovery image, and use an explicit
digital peer. Their start states are independent. The peer fixture is identical
and supplies a four-byte synthetic payload with an explicitly matching radio
configuration; this is not evidence of a valid decoded Meshtastic message.

| Observation | Long run, late peer | Independent early-peer control |
| --- | --- | --- |
| Schedule | Boot to 8 s; capture at 10, 12, 14 and 16 s; peer plus 3 s; press for requested 0.2 s, release plus 1 s | Boot to 8 s; peer plus 3 s |
| Final guest time | 20.36 s | 11.08 s |
| Guest instructions | 5,241,358,298 | 2,057,647,331 |
| Produced / saved frames | 719 / 7 | 327 / 2 |
| Peer accepted at guest time | 16.0228897 s | 8.019260920833334 s |
| Model receive count | 0 to 1 | 0 to 1 |
| Guest expander ISR and P3-high log markers | Present | Present |
| Subsequent RadioIf receive markers | Absent | Region-unset rejection, then receive-noise marker |
| SX1262 IRQ after peer | `0x02`, still pending after the button observation | `0x00` |
| Final serial size | 16,871 bytes | 16,974 bytes |

The late peer does reach the guest's IO-expander interrupt path. The missing
later RadioIf markers and still-pending RX IRQ establish a narrower failure
than a missing interrupt. The early run reaches the RadioIf receive path and
logs `lora rx disabled: Region unset`. Its IRQ is no longer pending. This early
result does not erase the late run's incomplete processing or the longer
application stall.

The [radio model](../../../../esp-soc/src/devices/sx1262.rs) copies scheduled
packet bytes into its RX buffer before incrementing `rxCount` and raising
RX_DONE. The observed counter therefore establishes model-side FIFO delivery.
Neither receipt contains a guest FIFO-read transaction trace, so guest payload
read completion is not established. Radio parameters are digital fixture
inputs; `frequency` is the SX1262 register word, and `physicalRf` is false.

## Frame boundary result

Each saved capture contains 480 by 480 RGBA pixels, or 921,600 bytes, with SHA-256:

```text
6e589868a0731cee1ed42cf7a00f15756b4a8a418cf14d9b49646abb2345be78
```

The full byte arrays were compared across both runs. Every capture has 228,791
black pixels and 1,609 foreground pixels of RGB565 `0x6752`. The previous
byte-swapped foreground and half-pixel boundary colors are absent in these
samples. Only the nine saved captures were compared; this is not an assertion
that every one of the 1,046 produced frames was separately inspected.

The earlier 370-frame run remains a
[preserved failure](../seeed-lcd-vsync-20261122/boundary-before-probe.json).
Its complete RGB565 streams differed by odd cyclic byte offsets.
The [boundary repair](../seeed-lcd-vsync-20261122/boundary-review.md) prevents a
DMA service from consuming next-frame bytes beyond its 34-byte FIFO lookahead,
retains fractional time credit and consumes whole RGB565 pixels. The final
captures validate the byte path with the original guest's ISR. They do not
change the separate [software rotation evidence](../../seeed-meshtastic-display-review.md)
or establish physical panel orientation.

## Remaining application limits

Neither run contains the boot-screen exit marker. The long run queues the
GPIO38 button press and release through the public input API, but produces no
press marker, no new serial bytes after the peer capture and no framebuffer
change. Queued inputs alone do not verify guest button handling or navigation.

Both runs log NimBLE initialization. No PC or stack samples were collected for
these runs; that last startup message does not identify the blocking location
or establish a Bluetooth fault. No BLE bypass, prefilled region, alternate
image, pixel recoloring or host-side rotation was applied to obtain these
results. Full Meshtastic application operation, decoded mesh delivery and
physical RF remain unverified.

## Runtime and provenance

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Production WASM | 6,056,179 | `a3f0c90c47f9403225084c7138a3ec6c6b7ab5d2d9a0d1e6d3db8938a7379a75` |
| Platform worker | 31,922 | `f2903f82105003582d2ccde163c3dbfbc1c61d980c542fd9c0a5a69803af960d` |
| Probe | 9,819 | `8cf822899f24ebff4a6f5295dee5f2c66f19fd32b67f05e6acedfa8518b34429` |

The recovery release is `v2.5.11.8e2a3e5`; the exact factory shipping build is
unknown. The [pinned recovery layout](../../research/seeed-meshtastic-2511-layout-2026-11-22.json)
and receipts retain the ZIP, individual entries and flash offsets. Each run
reads the 62,741,576-byte ZIP once. A local URL map supplies the previously
downloaded, hash-checked source bytes. The probe's `.invalid` origin is locally
intercepted; these runs do not test live source availability, browser CORS,
DOM or WebGL. Both producer-reported process exits are zero, and both original
JSON receipts independently report `observed` without an error.

Curation rechecked runtime hashes, original receipt and serial hashes,
cumulative serial prefixes, every saved frame hash, complete frame equality,
palette counts and matching peer configuration. It did not execute a new
emulator run or build. Raw serial, screenshots, RGBA files, payload/address
bytes, device labels, local paths and storage identifiers are omitted. Original
hashes continue to identify original bytes, not redacted substitutes. The
privacy check supplements this manual review:

```sh
node tools/check-evidence-privacy.mjs docs/platform/evidence/seeed-meshtastic-final-20261122
git diff --cached --check
```

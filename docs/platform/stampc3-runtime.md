# Stamp-C3 runtime

C056-B uses `stampc3`, 4 MiB Flash, no PSRAM. Its only exposed control is `button` (keyboard A), the active-low S2 switch on G3. G2 RMT output feeds one 24-bit GRB SK6812. G8/G9 are released high as shown by the official pull-ups; G3 uses an ideal released-high input matching the official driver's internal pull-up. S1 is an EN reset switch, so it is not mapped to the user-button event.

The official schematic and STAMP-C3 commit `148c263fddcb7b61fef20b0cf6de8ab8c3475f73` establish these connections. The Arduino folder is named `LED_RGBW`, but its actual configuration is `TYPE_GRB` / 24 bits. The manufacturer IDF example names IDF `v4.4.1-472-gc9140caf8c`; our independent fixture uses the public IDF 5.5.1 GPIO and RMT APIs. These are separate programs and toolchains.

A fresh actual page-worker/WASM run of the internal fixture boots through the C3 ROM, ignores a white frame on G4, then emits red on G2. Press/release of G3 changes actual guest output to green, blue, off and red. Platform reset while held releases the input, clears old RGB output, ignores G4 again and produces a fresh red frame. Unknown controls are rejected; there is no framebuffer. This tests digital routing and guest execution, not complete firmware compatibility. See [source receipts](evidence/stamp-controllers/c056-b-sources.json) and [execution receipt](evidence/stamp-controllers/c056-b-runtime.json).

No complete original factory binary or exact UIFlow1/UIFlow2/Xiaozhi target was located. The catalog offers the manufacturer's source as a reference; it does not offer the internal test firmware. No default executable firmware is invented. EN reset, CH9102 USB transport/autoreset, power/analog circuits, software GPIO LED decoding, physical networks and external interfaces remain outside this subset. No physical hardware test was performed.

Reproduce by building `tools/fixtures/stampc3-firmware` with ESP-IDF 5.5.1 and local ROM/WASM assets, then run:

```sh
node tools/probe-stampc3.mjs --build <fixture-build-directory> --product m5stack-c056-b
```

The harness creates and disposes the real platform worker, checks the exact catalog board before loading, and records hashes of every input image and the WASM artifact. Generated binaries stay outside Git and the public firmware catalog. Physical 3D hit-surface checks accompany the completed model separately.

## Stamp-C3 Mate K056

The official K056 page explicitly includes one Stamp-C3 plus loose headers/connectors and repeats G3 button/G2 SK6812. It links the identical schematic. K056 therefore uses `stampc3` with a separate catalog identity; loose kit contents do not imply connected electrical accessories. A fresh K056-scoped worker run repeats the GPIO/RMT/reset checks: [receipt](evidence/stamp-controllers/k056-runtime.json). Its old `button-a` catalog ID is deliberately normalized to `button`, matching the physical front control; EN remains separate and unbound.

## Stamp-C3U C122-B

C122-B uses distinct board `stampc3u`: S2 is G9 with R6 10k pull-up, and its single U3 SK6812 remains on G2. G8 has R2 10k pull-up. S1 remains EN. The C3U schematic routes native USB D−/D+ to G18/G19 and omits CH9102; this subset does not claim USB transport or flashing. Its current page IO list says G22, which conflicts with the C3 pin range and schematic; no such GPIO is invented. [Source receipt](evidence/stamp-controllers/c122-b-sources.json).

The independently compiled C3U fixture explicitly selects G9 and reports `STAMPC3_PINS 9 2`; the probe checks it against the selected `stampc3u` board. It repeats wrong-pin, real guest RGB, press/release and reset checks in [the C122-B receipt](evidence/stamp-controllers/c122-b-runtime.json). Source-only firmware references are shared only because the official source explicitly distinguishes C3/C3U, not because their names are similar.

## Stamp-C3U Mate K122

The K122 product page explicitly supplies one Stamp-C3U and links the identical C3U schematic. It uses the same G9/G2 board with an independent product and [fresh K122 execution receipt](evidence/stamp-controllers/k122-runtime.json). `button-user` is normalized to `button` for the real front user-key region; EN is not a GPIO action. Loose connectors and headers remain separate kit contents.

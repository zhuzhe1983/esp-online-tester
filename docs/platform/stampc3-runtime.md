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

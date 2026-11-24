# Waveshare ESP32-C6-Zero: two exact sale SKUs, two sourced capacity profiles

Official SKU27035 is the bare board; SKU26976 has pre-soldered headers. Current manufacturer overview explicitly describes C6FH8/8MiB; the retained manufacturer schematic explicitly labels C6FH4/4MiB. The fragment therefore exposes separate `waveshare-c6-zero` and `waveshare-c6-zero-fh4` board profiles, each with the two real header options. Four product profiles do not create four sale SKUs. No V1/V2 sale name, revision date or hardware release date is inferred.

Both profiles use the source BOOT GPIO9 and WS2812 RGB GPIO8. Unlike Zero-B, this Zero does not acquire a GPIO7 RF switch. No screen, touch device, TF socket, external I2C device or PSRAM is invented. Package/eFuse capacity fields reflect FH8 or FH4 explicitly; embedded flash vendor remains unknown. Full application, wireless, USB and analogue power behavior are outside acceptance.

The original manufacturer ZIP provides complete native assemblies:116 components for the bare board and118 for pre-soldered headers. Every component and source material distinction is retained at original millimetre scale. The native18.0173×23.5173-mm PCB envelope is not forcibly resized to the published18×23.5-mm dimension. Native CAD/electrical revision matching remains unverified, particularly the flash capacity not distinguishable from package geometry.

The official linked C6 Zero Basic Kit repository was inspected at fixed commit067435640535d90ce9c1fe0c046ba36a12886974:353 tree entries, noBIN or mechanical source. The BLEAssist download contains an Android APK. No original full ESP image/default is invented. Exact tree/README/example source receipts are retained separately from board firmware identity. UIFlow/Xiaozhi investigations use the already retained fixed upstream trees.

## Verification

- Native board regressions verify both exact FH capacities and package fields, BOOT high/pressed-low reads, source RGB GRB order, wrong-pin rejection, no RF switch on Zero, and absence of screen/touch/I2C/SD.
- A clearly labelled SRAM regression fixture executes actual RV32 instructions through the C6 bus: GPIO matrix/IO_MUX stores, RMT channel0 memory/start, UART0 output and BOOT read. It verifies GPIO8 emits RGB and GPIO10 does not. This fixture is repository test code, not manufacturer firmware.
- Both model files pass the production Three.js loader, finite geometry, artifact hashes, native PCB dimensions and material isolation. Both actual four-view plots were opened and inspected. These CPU geometry checks do not claim browser WebGL or physical fit acceptance.
- Four actual knowledge files equal the fragment records and pass the current schema and actual `validateKnowledge` checks. The fragment has two exact sale SKUs, four product profiles, two board profiles and two models.

Source receipts: `docs/platform/research/waveshare-c6-final-basic-sources/esp32-c6-zero-manifest.json` plus the two model source manifests. Full raw manufacturer sources remain distinct from derived inventories and reconstructed metadata.

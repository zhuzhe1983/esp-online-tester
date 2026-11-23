# LILYGO adaptation source and chronology audit — 2026-11-22

## Scope and ordering

This audit covers the 127 LILYGO product identities already present in the catalog. Five had partial board runtimes at the input revision and 122 had unavailable runtimes. This is a source inventory; it does not change any runtime support claim.

Devices are ordered newest first. A dated hardware revision in the manufacturer’s version-history table takes precedence over an official storefront publication timestamp. The original hardware version, hardware revision, and software-preloaded editions remain distinct. A new documentation commit or a V2 evaluation preview is never treated as the release date of an existing V1 model.

Only 19 entries have an applicable day-level hardware date in the inspected official documents. 105 use **storefront publication proxies**, which may reflect relisting. Three dates remain unknown. These proxies are useful for a provisional work queue, but they are not verified retail launch dates. Several official revision tables say January 1; their day-level precision remains unconfirmed and the audit lowers confidence explicitly.

## Source coverage

- 125 exact storefront product handles found in the official public JSON inventory; both remaining individual product JSON endpoints returned HTTP 404.
- 78 product documentation mappings inspected at a single pinned official documentation commit. Its object type and actual tree SHA were independently checked through the Git commit API; the normalized tree-list snapshot alone is not used to infer either identity.
- The two retired storefront identities now have exact front/back photo composites recovered from the pinned manufacturer Spark repository. The supplemental audit resolves a named T5 V2.3.1 DEPG0213BN display and the T3-S3 Meshtastic OLED through explicit manufacturer descriptions and bundled drivers. It enables no board runtime and assigns no sibling launch dates.
- The JSON preserves public source URLs, redirect targets, UTC retrieval times, byte counts and SHA-256; Markdown content additionally checks the Git blob identity.
- Model and photo links are marked `linked` until retrieved and validated by the model workflow. The two supplemental photos and the T5 reference STEP have byte receipts; the STEP has no exact revision label or CAD-specific license. Discovery of a model directory does not establish an exact licensed assembly.
- Every item includes a factory-firmware investigation status. A link to an official firmware directory is not a verified complete image, hash, factory identity, or successful boot. No third-party firmware binary is downloaded or committed by this audit.

## Important identity and source conflicts

| Product | Finding | Required treatment |
| --- | --- | --- |
| T-ETH Lite | Store page offers ESP32/RTL8201 and ESP32-S3/W5500; catalog identifies S3. | S3 documentation mapped explicitly; do not merge pin tables or claim the classic board is equivalent. |
| T-Display S3 Pro / External | Original Pro wiki mentions 2.2-inch ST7789V2; storefront says 2.33-inch ST7796, both 222×480. | Resolve controller and geometry against the exact source header and revision; retain the External listing’s separate identity. |
| T-Beam 1W | Storefront MCU text contains `ESP32-53FN8` and an unrelated malformed Mini-Epaper link. | Prefer the correct LoRa-Series hardware files; do not silently copy obvious page-template mistakes into a runtime. |
| T-Display P4 | V1.0 version-history date is 2025-06-13; storefront was published later. | Keep V1.0 date; a newer V2 preview is a separate revision, not a replacement date. |
| T5 V2.3.1 / T3-S3 Meshtastic | Exact storefront handles are absent from the current inventory. Exact retained Spark photos show both faces; T5's old page redirects to V2.4. | Keep original identities and unknown dates. Use exact retained photos; do not replace V2.3.1 with current V2.4 pictures or inherit sibling dates. |
| T5 V2.3.1 DEPG0213BN | Spark says SSD1680 212×104; its explicitly named manufacturer-bundled driver declares visible 122×250 and RAM width 128. | Scope the debug panel to DEPG0213BN at 122×250. Preserve the conflict and alternative-panel uncertainty. |
| T8-C3 | The former named alias pointed to an ESP32-WROVER T8 wiki with 8 MB PSRAM. | Removed the sibling alias, its dimensions, photos and memory candidates; exact C3 V1.1 uses 4 MB flash and no PSRAM. |
| T-Pico2350 V1.2 | Wiki gives 2024-01-01; all exact schematic sheets are dated 2025-01-16. Shared storefront includes older Pico C3/Pro. | Exact release remains unknown. The C6 runtime preserves the physical RP2350 + ESP32-C6 identity and models only the C6 coprocessor. |
| T-Lite W5500 | Catalog C3 label conflicts with manufacturer classic ESP32 schematic, GPIO21/22/23 and ESP32 Dev Module instructions. | Correct physical MCU to classic ESP32; 128×64 SSD1306 evidence is handed to the separate runtime lane. |

The detailed evidence, exact URLs and retrieval hashes for these two recovered products are in [lilygo-model-gap-sources-2026-11-22.json](lilygo-model-gap-sources-2026-11-22.json). The audit generator merges this curated supplement without activating a runtime. Exact RISC-V board source conflicts, the Watch Keyboard C3 blocker, and the T-Pico date correction are retained in [lilygo-riscv-additional-profiles-2026-11-22.json](lilygo-riscv-additional-profiles-2026-11-22.json).

## Newest-first inventory

`hardware` means a manufacturer-documented version date, not independent retail-launch verification. `storefront` means a low-confidence publication proxy. `unknown` sorts after dated entries.

| Rank | Catalog product | Date | Date basis | Prior board runtime |
| ---: | --- | --- | --- | --- |
| 1 | [LILYGO T7-C5](https://lilygo.cc/products/t7-c5) | 2026-01-28 | storefront | unavailable |
| 2 | [LILYGO T-Dongle C5](https://lilygo.cc/products/t-dongle-c5) | 2026-01-28 | storefront | unavailable |
| 3 | [LILYGO T-Display S3 Pro External](https://lilygo.cc/products/t-display-s3-pro-external) | 2025-11-28 | storefront | unavailable |
| 4 | [LILYGO T-Lora Pager Meshtastic](https://lilygo.cc/products/t-lora-pager-meshtastic) | 2025-10-10 | storefront | unavailable |
| 5 | [LILYGO T-Deck Pro Meshtastic](https://lilygo.cc/products/t-deck-pro-meshtastic) | 2025-08-19 | storefront | unavailable |
| 6 | [LILYGO T-Embed CC1101 Plus](https://lilygo.cc/products/t-embed-cc1101-plus) | 2025-08-08 | storefront | unavailable |
| 7 | [LILYGO T-Beam BPF](https://lilygo.cc/products/t-beam-bpf) | 2025-07-22 | storefront | unavailable |
| 8 | [LILYGO T3 S3 V1.3](https://lilygo.cc/products/t3-s3-v1-3) | 2025-07-18 | storefront | unavailable |
| 9 | [LILYGO T-Watch Ultra](https://lilygo.cc/products/t-watch-ultra) | 2025-07-18 | storefront | unavailable |
| 10 | [LILYGO T-Display Bar](https://lilygo.cc/products/t-display-bar) | 2025-07-18 | storefront | unavailable |
| 11 | [LILYGO T-2CAN](https://lilygo.cc/products/t-2can) | 2025-07-18 | storefront | unavailable |
| 12 | [LILYGO T-Radar](https://lilygo.cc/products/t-radar) | 2025-06-24 | storefront | unavailable |
| 13 | [LILYGO T-Display P4](https://lilygo.cc/products/t-display-p4) | 2025-06-13 | hardware | unavailable |
| 14 | [LILYGO T-Embed SI4732](https://lilygo.cc/products/t-embed-si4732) | 2025-05-12 | storefront | unavailable |
| 15 | [LILYGO T-Deck Meshtastic](https://lilygo.cc/products/t-deck-meshtastic) | 2025-05-05 | storefront | unavailable |
| 16 | [LILYGO T-Beam Meshtastic](https://lilygo.cc/products/t-beam-meshtastic) | 2025-05-05 | storefront | unavailable |
| 17 | [LILYGO T-Deck Plus Meshtastic](https://lilygo.cc/products/t-deck-plus-meshtastic) | 2025-04-22 | storefront | unavailable |
| 18 | [LILYGO T-Beam SUPREME Meshtastic](https://lilygo.cc/products/t-beam-supreme-meshtastic) | 2025-04-22 | storefront | unavailable |
| 19 | [LILYGO T-Deck Pro](https://lilygo.cc/products/t-deck-pro) | 2025-03-26 | storefront | unavailable |
| 20 | [LILYGO T-Lora Dual](https://lilygo.cc/products/t-lora-dual) | 2025-03-15 | hardware | unavailable |
| 21 | [LILYGO T-Connect Pro](https://lilygo.cc/products/t-connect-pro) | 2025-02-28 | storefront | unavailable |
| 22 | [LILYGO T7-C6](https://lilygo.cc/products/t7-c6) | 2024-12-16 | storefront | unavailable |
| 23 | [LILYGO T-Deck Plus](https://lilygo.cc/products/t-deck-plus-1) | 2024-12-09 | storefront | unavailable |
| 24 | [LILYGO T3 TCXO](https://lilygo.cc/products/t3-tcxo) | 2024-11-27 | storefront | unavailable |
| 25 | [LILYGO T5 E-Paper S3 Pro](https://lilygo.cc/products/t5-e-paper-s3-pro) | 2024-11-14 | storefront | unavailable |
| 26 | [LILYGO T-Bao](https://lilygo.cc/products/t-bao) | 2024-11-13 | storefront | unavailable |
| 27 | [LILYGO T3-S3 MVSR](https://lilygo.cc/products/t3-s3-mvsr) | 2024-11-06 | hardware | unavailable |
| 28 | [LILYGO T-Keyboard S3 Pro](https://lilygo.cc/products/t-keyboard-s3-pro) | 2024-09-28 | storefront | unavailable |
| 29 | [LILYGO T-Connect](https://lilygo.cc/products/t-connect) | 2024-08-12 | storefront | unavailable |
| 30 | [LILYGO T-Lora Pager](https://lilygo.cc/products/t-lora-pager) | 2024-08-05 | hardware | unavailable |
| 31 | [LILYGO T-TWR REV2.1](https://lilygo.cc/products/t-twr-rev2-1) | 2024-07-29 | storefront | unavailable |
| 32 | [LILYGO T-Embed CC1101 · V1.0](https://lilygo.cc/products/t-embed-cc1101) | 2024-07-29 | hardware | partial |
| 33 | [LILYGO T-ETH ELite](https://lilygo.cc/products/t-eth-elite-1) | 2024-07-25 | storefront | unavailable |
| 34 | [LILYGO T-Knob](https://lilygo.cc/products/t-knob) | 2024-07-13 | storefront | unavailable |
| 35 | [LILYGO T-Beam 1W](https://lilygo.cc/products/t-beam-1w) | 2024-06-15 | hardware | unavailable |
| 36 | [LILYGO T-Energy S3](https://lilygo.cc/products/t-energy-s3) | 2024-06-13 | storefront | unavailable |
| 37 | [LILYGO T-Glass](https://lilygo.cc/products/t-glass) | 2024-06-04 | storefront | unavailable |
| 38 | [LILYGO T-Display S3 AMOLED (DS)](https://lilygo.cc/products/t-display-s3-amoled-1-64) | 2024-05-20 | hardware | unavailable |
| 39 | [LILYGO T-SIM7670G S3](https://lilygo.cc/products/t-sim-7670g-s3) | 2024-04-12 | storefront | unavailable |
| 40 | [LILYGO T3 S3 LR1121](https://lilygo.cc/products/t3-s3-lr1121) | 2024-03-25 | hardware | unavailable |
| 41 | [LILYGO T-Lora C6](https://lilygo.cc/products/t-lora-c6) | 2024-03-15 | hardware | unavailable |
| 42 | [LILYGO T-Micro32 S3](https://lilygo.cc/products/t-micro32-s3) | 2024-03-11 | storefront | unavailable |
| 43 | [LILYGO T-Encoder Pro](https://lilygo.cc/products/t-encoder-pro) | 2024-02-02 | hardware | unavailable |
| 44 | [LILYGO T-Panel S3](https://lilygo.cc/products/t-panel-s3) | 2024-01-22 | storefront | unavailable |
| 45 | [LILYGO T-Watch S3 Plus](https://lilygo.cc/products/t-watch-s3-plus) | 2024-01-01 | hardware | unavailable |
| 46 | [LILYGO T-Display S3 Pro LR1121](https://lilygo.cc/products/t-display-s3-pro-lr1121) | 2024-01-01 | hardware | unavailable |
| 47 | [LILYGO T-Display S3 AMOLED Plus](https://lilygo.cc/products/t-display-s3-amoled-plus) | 2024-01-01 | hardware | unavailable |
| 48 | [LILYGO T-Circle](https://lilygo.cc/products/t-circle-s3) | 2024-01-01 | hardware | unavailable |
| 49 | [LILYGO T-QT C6](https://lilygo.cc/products/t-qt-c6) | 2023-12-20 | hardware | unavailable |
| 50 | [LILYGO T3 S3 E-paper](https://lilygo.cc/products/ts-s3-epaper) | 2023-12-16 | hardware | unavailable |
| 51 | [LILYGO ESP32 S2 WOOR](https://lilygo.cc/products/esp32-s2-woor) | 2023-11-25 | storefront | unavailable |
| 52 | [LILYGO ESP32 S2](https://lilygo.cc/products/esp32-s2) | 2023-11-25 | storefront | unavailable |
| 53 | [LILYGO LoRa V1.3](https://lilygo.cc/products/lora-v1-3) | 2023-11-18 | storefront | unavailable |
| 54 | [LILYGO T-RSS3](https://lilygo.cc/products/t-rs-s3) | 2023-11-14 | storefront | unavailable |
| 55 | [LILYGO T-TV](https://lilygo.cc/products/t-tv) | 2023-11-13 | storefront | unavailable |
| 56 | [LILYGO T-Mini E paper S3](https://lilygo.cc/products/t-mini-epaper-s3) | 2023-11-10 | hardware | unavailable |
| 57 | [LILYGO T4 S3](https://lilygo.cc/products/t4-s3) | 2023-11-03 | storefront | unavailable |
| 58 | [LILYGO T-Display S3 Long](https://lilygo.cc/products/t-display-s3-long) | 2023-10-27 | storefront | unavailable |
| 59 | [LILYGO T-Camera Plus S3](https://lilygo.cc/products/t-camera-plus-s3) | 2023-10-23 | hardware | unavailable |
| 60 | [LILYGO T-POE Pro](https://lilygo.cc/products/t-poe-pro) | 2023-10-19 | storefront | unavailable |
| 61 | [LILYGO T-Beam SoftRF](https://lilygo.cc/products/t-beam-softrf) | 2023-10-13 | storefront | unavailable |
| 62 | [LILYGO T-ETH-Lite](https://lilygo.cc/products/t-eth-lite) | 2023-09-09 | storefront | unavailable |
| 63 | [LILYGO TTGO T-Display · 4MB](https://lilygo.cc/products/t-display) | 2023-08-04 | storefront | partial |
| 64 | [LILYGO T-Display S3 Pro](https://lilygo.cc/products/t-display-s3-pro) | 2023-08-01 | hardware | unavailable |
| 65 | [LILYGO T-Beam SUPREME](https://lilygo.cc/products/t-beam-supreme) | 2023-08-01 | storefront | unavailable |
| 66 | [LILYGO T-Beam Helium](https://lilygo.cc/products/helium-t-beam) | 2023-07-25 | storefront | unavailable |
| 67 | [LILYGO T-Vending](https://lilygo.cc/products/t-vending) | 2023-06-23 | storefront | unavailable |
| 68 | [LILYGO T-TWR Plus](https://lilygo.cc/products/t-twr-plus) | 2023-06-16 | storefront | unavailable |
| 69 | [LILYGO T-Watch S3 · SX1262](https://lilygo.cc/products/t-watch-s3) | 2023-05-31 | storefront | unavailable |
| 70 | [LILYGO T-Display keyboard](https://lilygo.cc/products/t-display-keyboard) | 2023-05-26 | storefront | unavailable |
| 71 | [LILYGO T-Deck · SX1262](https://lilygo.cc/products/t-deck) | 2023-05-26 | storefront | partial |
| 72 | [LILYGO T-A7608](https://lilygo.cc/products/t-a7608e-h) | 2023-05-24 | storefront | unavailable |
| 73 | [LILYGO T-Relay S3](https://lilygo.cc/products/t-relay-s3) | 2023-05-13 | storefront | unavailable |
| 74 | [LILYGO T-Track](https://lilygo.cc/products/t-track) | 2023-04-28 | storefront | unavailable |
| 75 | [LILYGO T3 S3](https://lilygo.cc/products/t3s3-v1-0) | 2023-04-18 | storefront | unavailable |
| 76 | [LILYGO T-RGB](https://lilygo.cc/products/t-rgb) | 2023-04-17 | storefront | unavailable |
| 77 | [LILYGO T-FPGA](https://lilygo.cc/products/t-fpga) | 2023-04-14 | storefront | unavailable |
| 78 | [LILYGO T-Display S3 AMOLED](https://lilygo.cc/products/t-display-s3-amoled) | 2023-04-14 | storefront | unavailable |
| 79 | [LILYGO T-SIM7080G S3](https://lilygo.cc/products/t-sim7080-s3) | 2023-02-09 | storefront | unavailable |
| 80 | [LILYGO T-Touch Bar](https://lilygo.cc/products/t-touch-bar) | 2023-02-02 | storefront | unavailable |
| 81 | [LILYGO T-HMI](https://lilygo.cc/products/t-hmi) | 2023-02-02 | storefront | unavailable |
| 82 | [LILYGO T-TWR/ SOLD OUT](https://lilygo.cc/products/t-twr) | 2023-01-12 | storefront | unavailable |
| 83 | [LILYGO T-Camera S3](https://lilygo.cc/products/t-camera-s3) | 2022-12-28 | storefront | unavailable |
| 84 | [LILYGO T-A7670E/G/SA R2](https://lilygo.cc/products/t-sim-a7670e) | 2022-12-02 | storefront | unavailable |
| 85 | [LILYGO Watch-Keyboard-C3 V1.0](https://lilygo.cc/products/watch-keyboard-c3-v1-0) | 2022-11-25 | storefront | unavailable |
| 86 | [LILYGO T5-2.13inch E-paper](https://lilygo.cc/products/t5-2-13inch-e-paper) | 2022-11-25 | storefront | unavailable |
| 87 | [LILYGO T-CAN485](https://lilygo.cc/products/t-can485) | 2022-11-23 | storefront | unavailable |
| 88 | [LILYGO T-SIM7000E](https://lilygo.cc/products/t-sim7600e) | 2022-11-22 | storefront | unavailable |
| 89 | [LILYGO T7 V1.3 MINI 32 ESP32](https://lilygo.cc/products/t7-v1-3-mini-32-esp32) | 2022-11-16 | storefront | unavailable |
| 90 | [LILYGO T5s 2.7 inch E-Paper](https://lilygo.cc/products/t5s-2-7inch-e-paper) | 2022-11-16 | storefront | unavailable |
| 91 | [LILYGO T-Wrist E-paper 1.54 Inch Display](https://lilygo.cc/products/t-wrist-e-paper-1-54-inch-display) | 2022-11-16 | storefront | unavailable |
| 92 | [LILYGO T-Weigh](https://lilygo.cc/products/t-weigh) | 2022-11-16 | storefront | unavailable |
| 93 | [LILYGO T-Watch 2021](https://lilygo.cc/products/t-watch-2021) | 2022-11-16 | storefront | unavailable |
| 94 | [LILYGO T-PCIE](https://lilygo.cc/products/a-t-pcie) | 2022-11-16 | storefront | unavailable |
| 95 | [LILYGO T-Display AMOLED Lite](https://lilygo.cc/products/t-display-amoled-lite) | 2022-11-12 | storefront | unavailable |
| 96 | [LILYGO T7 S3](https://lilygo.cc/products/t7-s3) | 2022-11-03 | storefront | unavailable |
| 97 | [LILYGO T-SIMCAM](https://lilygo.cc/products/t-simcam) | 2022-10-25 | storefront | unavailable |
| 98 | [LILYGO T-Relay 5V 8 Channel Relay](https://lilygo.cc/products/t-relay-5v-8-channel-relay) | 2022-10-25 | storefront | unavailable |
| 99 | [LILYGO T-Lite W5500](https://lilygo.cc/products/t-lite-w5500) | 2022-10-25 | storefront | unavailable |
| 100 | [LILYGO T-Lion](https://lilygo.cc/products/t-lion) | 2022-10-25 | storefront | unavailable |
| 101 | [LILYGO T-Journal](https://lilygo.cc/products/t-journal) | 2022-10-25 | storefront | unavailable |
| 102 | [LILYGO Mini E-Paper](https://lilygo.cc/products/mini-e-paper-core) | 2022-10-25 | storefront | unavailable |
| 103 | [LILYGO ESP32 OLED V3.0](https://lilygo.cc/products/esp32-oled-v3-0) | 2022-10-25 | storefront | unavailable |
| 104 | [LILYGO T-Dongle S3](https://lilygo.cc/products/t-dongle-s3) | 2022-10-20 | storefront | unavailable |
| 105 | [LILYGO T5 e-Paper](https://lilygo.cc/products/t5-4-7-inch-e-paper-v2-3) | 2022-10-07 | storefront | unavailable |
| 106 | [LILYGO T-Embed](https://lilygo.cc/products/t-embed) | 2022-09-27 | storefront | unavailable |
| 107 | [LILYGO T-QT Pro](https://lilygo.cc/products/t-qt-pro) | 2022-09-21 | storefront | unavailable |
| 108 | [LILYGO T-SIM7600E-L1](https://lilygo.cc/products/t-sim7600e-l1) | 2022-09-19 | storefront | unavailable |
| 109 | [LILYGO T-SIM7600E/G/SA-H](https://lilygo.cc/products/t-sim7600) | 2022-09-19 | storefront | unavailable |
| 110 | [LILYGO T-Higrow](https://lilygo.cc/products/t-higrow) | 2022-09-19 | storefront | unavailable |
| 111 | [LILYGO T-Encoder](https://lilygo.cc/products/t-encoder-esp32) | 2022-09-13 | storefront | unavailable |
| 112 | [LILYGO T-01 C3](https://lilygo.cc/products/t-01c3) | 2022-09-06 | storefront | unavailable |
| 113 | [LILYGO T-Internet-COM](https://lilygo.cc/products/t-internet-com) | 2022-08-24 | storefront | unavailable |
| 114 | [LILYGO LoRa32 V1.0 Kit/ Sold Out](https://lilygo.cc/products/lora-v1-0-kit) | 2022-08-24 | storefront | unavailable |
| 115 | [LILYGO T8](https://lilygo.cc/products/t8-c3) | 2022-08-17 | storefront | unavailable |
| 116 | [LILYGO T-SIM7070G](https://lilygo.cc/products/t-sim7070g) | 2022-08-17 | storefront | unavailable |
| 117 | [LILYGO T-Zigbee](https://lilygo.cc/products/t-zigbee-esp32-c3-tlsr8258) | 2022-08-15 | storefront | unavailable |
| 118 | [LILYGO T-Relay](https://lilygo.cc/products/t-relay) | 2022-08-12 | storefront | unavailable |
| 119 | [LILYGO T-SIM7000G](https://lilygo.cc/products/t-sim7000g) | 2022-07-25 | storefront | unavailable |
| 120 | [LILYGO T-Call](https://lilygo.cc/products/t-call-v1-4) | 2022-07-22 | storefront | unavailable |
| 121 | [LILYGO T-Dongle](https://lilygo.cc/products/t-dongle-esp32-s2-1-14-inch-lcd-board) | 2022-07-18 | storefront | unavailable |
| 122 | [LILYGO T-Beam V1.2 · SX1276](https://lilygo.cc/products/t-beam) | 2022-07-18 | storefront | partial |
| 123 | [LILYGO T3 LoRa32 V1.6.1](https://lilygo.cc/products/lora3) | 2022-07-18 | storefront | unavailable |
| 124 | [LILYGO T-Display-S3 · 非触摸版](https://lilygo.cc/products/t-display-s3) | 2022-07-15 | storefront | partial |
| 125 | [LILYGO T5 V2.3.1](https://lilygo.cc/products/t5-v2-3-1) | unknown | unknown | unavailable |
| 126 | [LILYGO T3 S3 Meshtastic](https://lilygo.cc/products/t3-s3-meshtastic) | unknown | unknown | unavailable |
| 127 | [LILYGO T-Pico](https://lilygo.cc/products/t-pico) | unknown | unknown | unavailable |

## Reproduction

```sh
python3 tools/audit-lilygo-sources.py --fetch --cache target/lilygo-audit
```

The command reads the current catalog and retrieves public manufacturer metadata and a pinned Markdown documentation snapshot. It writes the JSON research report only, never modifies catalogs, and never downloads firmware. New upstream snapshots can legitimately change the report. The retained source receipts identify the exact snapshot used here.

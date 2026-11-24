# P4 WIFI6 Touch LCD 3.5: separate V1.3 and V3.2 factory runs

Both exact profiles have 16 MiB Flash and 32 MiB PSRAM. The manufacturer's pinned firmware
README associates `Factory_20261119` with V1.3 and `Factory_20261105` with V3.2. The newer
filename is therefore not the newer silicon revision. Original headers and complete source
receipts are retained in `docs/research/waveshare-p4-lcd35-sources`.

Each five-second native run uses its original ROM revision and untouched factory image.
Both display **ESP32-P4 / LOADING / Starting Wi-Fi...**. Their screenshots, UART logs, and
SHA-256/command receipts are retained in the capture archive. V1.3 reaches the missing C6 SDIO initialization
failure at about 3327 ms; V3.2 at about 3159 ms. Each then raises an instruction-access fault
and reboots. These are reproducible failure boundaries, not complete factory acceptance.

The SDMMC counters in these logs belong to the missing C6 link and do not prove TF mounting.
The TF supply is Q1/AO3401, GPIO45 active low, with a physical pull-down shown in the retained
schematic crop. The board model gates software-controlled TF access; passive analog behavior
and the complete physical GPIO routing remain outside its verified subset. The simulation
uses its existing 360 MHz virtual clock; the vendor's physical clock specification is not
silently treated as the same thing.

BOOT uses GPIO35. Battery, power key, radio, and analog audio are not reported as verified.
`simulationVerified` remains false for these two factory catalog entries.

## Raw capture retention

Raw UART, traces, frame buffers and PNGs referenced by the receipts are retained in
`waveshare-runtime-captures-20261123-batch3.zip` with their original directory paths.
See `docs/evidence/waveshare-integration-2026-11-23/capture-manifest.json` for byte sizes,
SHA-256 values, the owner-access recovery link and recorded privacy substitutions.
This development snapshot has not published the captures or application to GitHub.

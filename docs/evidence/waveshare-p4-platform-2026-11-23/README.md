# P4 platform profiles: bounded acceptance

This batch registers 17 distinct module/carrier configurations. Capacity, connector options,
BOOT, codec control, TF routing, and TF power are sourced separately in
`docs/research/waveshare-p4-platform-profiles.json` and its source archive. Bare modules do not
gain the peripherals of a carrier. All configurations remain `partial`.

The production Worker/WASM test in `tools/platform-waveshare-test.mjs` boots the original IDF
hello application on the seven module/core profiles without a board factory binary. Its
memory-negative cases reject capacities that do not match the selected product.

`factory-runs.json` records ten other profiles, each with the original factory image, ROM,
executable SHA-256, complete command, and two-second guest limit. The runs reach application
startup; most stop progressing while configuring DSI. WIFI6-DB reports SPIFFS error -10025
and restarts. NANO-WIFI6-DB reaches the JD9365 panel setup. None of those runs demonstrates
DSI pixels, CSI capture, C6 radio, Ethernet traffic, analog sound, or a complete factory UI.
Process exit zero means the requested bounded run ended; it is not a factory success claim.

The first exploratory logs are retained separately from the per-profile runs. Source and
runtime evidence can therefore be reviewed without substituting a later passing diagnostic
for the original failure.

## Raw capture retention

Raw UART, traces, frame buffers and PNGs referenced by the receipts are retained in
`waveshare-runtime-captures-20261123-batch3.zip` with their original directory paths.
See `docs/evidence/waveshare-integration-2026-11-23/capture-manifest.json` for byte sizes,
SHA-256 values, the owner-access recovery link and recorded privacy substitutions.
This development snapshot has not published the captures or application to GitHub.

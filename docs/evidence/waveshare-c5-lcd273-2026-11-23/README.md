# C5 LCD2.73 native acceptance

The project SRAM diagnostic completed226,000,000 instructions with zero
exceptions/resets. It exercised all8 MiB PSRAM,307,200-byte RGB666 GDMA frames,
CH32V006 digital reset/PWM, QMI identity, three GPIO keys and SPI SD block
preservation. PREV and NEXT each changed26,752 pixels; BOOT changed73,728.
Quiet frames changed zero pixels, and each release restored the exact baseline.
The whole8 MiB nonzero card sample was unchanged after the guest's save/test/
restore workload. The test does not establish a filesystem mount.

`inputs-and-hashes.json` records exact inputs/source hashes, native execution,
53 passing C5 release tests, denied-warnings Clippy, timing boundaries and raw
artifact hashes. Wall intervals include compilation/shared-host scheduling;
there is no CPU performance claim. An initial Clippy spacing error and its
line-break-only correction are retained; final checks passed.

`factory-validation.json` parses the unchanged manufacturer's image and records
its failure with the real ROM:6831 steps, PMU600b00e4 read at PC4001df26,
zero UART/display output. Factory acceptance is false. The diagnostic App and
factory have different offsets and independent roles.

Raw UART, logs, PNGs and RGB565 frames are retained under ignored
`target/c5-lcd273-2026-11-23/`. They are omitted from Git under the evidence
retention policy. No external archive upload is claimed and the only copies
have not been removed. `reproduce.py` regenerates and verifies the measured
frame/UART/execution/board-report hashes; optional ROM/factory arguments verify
the preserved startup boundary.

```sh
python3 docs/evidence/waveshare-c5-lcd273-2026-11-23/reproduce.py \
  --output target/c5-lcd273-reproduced
```

Physical display calibration, MV1/TE/scroll/readback, SHTC3/charging/power key,
radio/USB/OTA and public Worker/browser execution are outside this native claim.

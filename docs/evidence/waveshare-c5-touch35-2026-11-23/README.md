# C5 Touch LCD3.5 — fresh native evidence

The source-wired `waveshare-c5-touch35` profile passed the project's original
SRAM App diagnostic with32MiB flash/8MiB PSRAM. The guest writes/reads all8MiB
PSRAM and displays its320×480 RGB565 framebuffer through real GDMA/SPI2.
FT6336 touch changes1024pixels and BOOT changes119808pixels; the quiet control
changes0 and both releases restore every pixel. The complete8MiB nonzero
SD image is byte-identical after the guest saves, tests, restores and rereads
LBA32. This is raw SPI block access, not a filesystem mount or interrupted-run
transaction guarantee. There were170250000instructions and0exceptions/resets.

The final package gate passed50tests including the real-ROM boundary test;
all-target release Clippy passed with warnings denied. These are targeted C5
checks. They are not a full-workspace or CPU-performance gate.

The unmodified manufacturer32MiB factory image has valid checksums/digests
and chip/revision headers, but actual ROM execution still faults at6831steps
reading PMU600b00e4 from PC4001df26. No UART or screen is produced and the
public C5 loader does not yet accept that flash boot path. Do not interpret
the successful SRAM diagnostic as factory verification.

## Reproduce

From a checkout with Rust installed:

```sh
python docs/evidence/waveshare-c5-touch35-2026-11-23/reproduce.py
```

Optionally pass `--rom /path/to/esp32c5_rev100_rom.elf --factory /path/to/the.bin`
to reproduce the separate failure boundary. The script verifies both input
hashes before running them. It regenerates frames/UART under ignored `target/`
and verifies deterministic raw outputs against the compact record.

## Retention and limits

[`inputs-and-hashes.json`](inputs-and-hashes.json) contains compact results,
source/input hashes, environment information and raw artifact hashes. Each
evidence file is below50KB. Complete raw captures are retained in the working
checkout at ignored `target/c5-touch35-2026-11-23/`; the earlier independent
run has also been kept. No release/archive upload is claimed or required to
run the reproducer. Root integration should preserve those captures in its
recovery package before clearing the local output. Git contains no raw UART,
PNG, framebuffer arrays or embedded logs in this evidence batch.

Nominal48MHz crystal,24MHz SPI and400kHz I²C are guest configuration values.
Wall intervals include compilation and shared-host scheduling and are not a
performance comparison. Physical color/scan calibration, manufacturer GUI,
RF, audio, battery, camera, SHTC3 and PWR key input are not validated.

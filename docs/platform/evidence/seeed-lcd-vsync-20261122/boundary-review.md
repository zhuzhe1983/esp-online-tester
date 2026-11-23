# RGB DMA must stop at the VSYNC byte boundary

## Preserved failure

The first VSYNC repair enabled real RGB output, but did not establish correct
display behavior. The production WASM
`abb53cb6a9f768aa811b48aa89ccacb2774d5207e33388a116e352cd1972c995`
produced 370 frames during the original Meshtastic probe through 12.1 guest
seconds. Its three captured images drifted horizontally and changed color.
[boundary-before-probe.json](boundary-before-probe.json) retains exact image
hashes and non-identifying capture metrics from that unsuccessful probe.

The independent [source and byte review](../../seeed-meshtastic-display-review.md)
establishes that the intended foreground is RGB565 `0x6752`. The other capture's
`0x5267`, `0x0067` and `0x5200` are consistent with a one-byte phase error across
foreground/black boundaries. More strongly, all 460,800 RGB565 bytes of the boot
capture equal a 27-byte cyclic shift of the peer capture; the button capture
equals a -157-byte shift. These observations do not justify rotating, recoloring
or cropping the output. The fixed guest drawing code also explains its software
rotation independently of the transport problem.

Baseline `972577650202ffa5e062ac02ee50b0ccdced5433` consumed the entire due batch,
even if it crossed the frame end. Its complete-frame loop then left extra bytes
in `lcd_line`, while its DMA FIFO retained another 34 bytes. The guest's ISR
resets GDMA and skips only the known 34 prefetched bytes. Already consumed
next-frame bytes therefore caused duplication and phase drift. The original
tests used batches ending exactly at the frame boundary and missed this case.

The new 513-byte regression was run against that baseline and failed at
`VSYNC must not consume a byte from the next frame`. Its source then passes with
the repair. Both logs are retained in [boundary-checks.txt](boundary-checks.txt).

## Byte and time accounting

`lcd_acc` is now `u128` byte-cycle credit. For a scan with `B` bytes and `T` CPU
cycles, an elapsed interval `c` earns `c * B` credit; scheduling `n` output bytes
spends `n * T`. All products, sums and divisions use the wide representation.
Credit is bounded by the current frame's remaining byte count before conversion
to `usize`. RGB565 consumes complete two-byte pixels. The service threshold is
the smaller of 512 bytes and the current frame's remainder.

The same bounded byte count controls both prefetch and consumption. A service
can publish at most one frame, leaves no next-frame bytes in `lcd_line`, and
retains only the original 34-byte FIFO lookahead. There is no board-name or
interrupt-enable branch in this data path. The guest can run its actual ISR;
without an ISR, the same ring continues in byte order on subsequent services.
There is no wait for interrupt acknowledgement and no invented porch interval.

Unused credit survives the boundary. A large elapsed interval can therefore be
drained on later services without charging elapsed time twice. A single
`u64::MAX` interval fits the `u128` product exactly, and still allocates/services
at most one configured frame plus FIFO lookahead. Only repeated host intervals
beyond the entire `u128` credit range saturate instead of wrapping. The new
test checks the exact retained credit and a subsequent zero-cycle service.
Production LCD activity already uses the existing bounded device-tick cadence;
the repair does not enlarge scheduler quanta.

START, RESET and a starved VSYNC clear credit using its new units. A completed
frame sets the scan-clock remainder from retained equivalent cycles modulo `T`,
instead of discarding that remainder. The model continues to spread active-byte
consumption over the configured total horizontal/vertical period. It does not
resolve each porch or pixel-clock edge. ISR latency and the first recovered
scan must still be assessed with actual firmware output.

This **intentionally changes** the old batch-rounded RGB timing: fractional
credit is no longer lost, half-pixel service is disallowed, and a VSYNC service
does not cross into the next frame. The initial repair's claim that all ordinary
RGB interrupt times remain unchanged is superseded. Existing goldens must be
compared in integration; none was regenerated here. The `u128` field also changes
S3 layout, so an unchanged disabled-path early return is not a performance claim.

## Targeted validation

Three new regressions cover the 513-byte boundary, 1,800 irregular ticks with
and without actual GDMA reset/skip-34 restart writes, and exact large-interval
credit with bounded output. The ISR case includes elapsed cycles between its
register writes. It compares every byte of every delivered frame and each FIFO
prefix; the no-ISR case follows the same producer logic. Both retain the expected
frame count from elapsed time. Existing starved-clock, reset, short-frame,
descriptor-failure and RGB565 prefetch tests remain active.

[boundary-checks.json](boundary-checks.json) records the failing baseline and
passing candidate commands, code hashes and reviewed logs. These are digital
tests. The final production WASM, original-firmware captures, LCD-4B/PaperS3
regressions, full workspace gates and common-path CPU comparison remain separate
integration checks. No firmware bytes, board color mapping or catalogue status
were changed in this targeted checkpoint, and no golden file was regenerated
then. The later integration observed and intentionally revised four `panel_sid`
goldens after an independent frame/audio/counter comparison; see the
[golden review](../seeed-panel-golden-20261122/README.md) and
[completed integration gates](../seeed-final-integration-20261122/README.md).

The exact [three-file Rust patch](boundary-source.patch), with its
[byte/hash receipt](boundary-source.json), is retained separately from private
integration ancestry. It makes the before/after source change reviewable without
publishing superseded original CAD or photo metadata from that ancestry.
It uses zero-context unified-diff serialization to avoid a context-only trailing
space in the evidence file. Apply or reverse with `git apply --unidiff-zero`
only after checking the three source-file SHA-256 values in the receipt.
An isolated apply/reverse round trip matched every recorded before/after byte;
the original context-patch hash is retained as provenance.

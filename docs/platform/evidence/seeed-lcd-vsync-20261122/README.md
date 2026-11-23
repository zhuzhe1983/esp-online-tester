# S3 RGB VSYNC while DMA is starved

**Follow-up:** [the frame-boundary repair](boundary-review.md) records the actual
WASM byte-phase failure and supersedes the complete-frame timing-preservation
claim in this initial repair. The original checks and observations below remain
historical evidence.

This is a timing correctness repair for the existing S3 LCD_CAM engine. It does
not supply an image, change the guest firmware, fix the guest's first DMA link,
or broaden descriptor acceptance. The Meshtastic Indicator image remains an
unmodified official `2.5.11.8e2a3e5` image. Actual product output after this repair
must be established by the final native/WASM probes, separately from these tests.

## Observed bootstrap

The four six-second captures are reduced to relevant register and memory words in
[registers.json](registers.json). Original capture byte counts and SHA-256 values
are retained; paths, device identifiers, unrelated serial output and host timing
are omitted. These are observations from the pre-repair runtime, not results
from the changed implementation.

LCD_USER had START, RGB565 and continuous output set. The active dimensions were
480 by 480; horizontal/vertical totals were 548 by 518. LCD_CLOCK selected a
12 MHz pixel clock, giving 5,677,280 CPU cycles per scan at the simulator's
240 MHz clock. The enabled LCD interrupt was VSYNC, but RAW was `2` and ST was
`0`. GDMA channel zero selected LCD trigger 5 and had stopped: OUT_INT_RAW `9`,
OUT_STATE `0`, current descriptor `0`.

The original OUT_LINK still selected the address of a pointer field,
`0x3fcb30a4`. Its three words were `[0x3fcb3270, 0x3fcb31cc, 0]`. Interpreting
those words as a descriptor yields size 624, length 3251, an unset owner bit,
an i80 object as the buffer and a null next link. These are decoded observations;
CHECK_OWNER was not enabled, and no claim is made that the hardware must reject
this combination. The existing RGB walker had consumed this interpretation and
stopped at its null link. Its validation rules are unchanged by this repair.

The valid first descriptor at `0x3fcb3270` has size and length 4032, a DMA owner
bit, a PSRAM buffer and a next descriptor. A **separate**, already constructed
restart descriptor at `0x3fcb3098` has size and length 3998, its buffer advanced
by 34 bytes, and the same next descriptor. `0x3fcb31cc` is the i80 object, not
the restart descriptor.

The fixed LovyanGFX source candidate first writes the address of `_dmadesc` to
OUT_LINK. Its VSYNC ISR resets GDMA and selects `_dmadesc_restart`. The observed
object layout, descriptor values and 34-byte adjustment match that sequence.
However, the fixed Meshtastic `platformio.ini` requests LovyanGFX `#develop`.
Revision `894e4340c94d78b1fb56d180185f773cc1f45099` is a release-date ancestor,
**not a proven dependency lock for the released binary**.

## Source basis

[sources.json](sources.json) contains original-body hashes, sizes, fetch times,
URLs and connector Git blob hashes where available. All seven retained source
receipts were checked against the actual downloaded bytes. The failed attempt
to retrieve an IDF v4.4.7 RGB driver at the queried path remains recorded as HTTP
404; it is not used as evidence.

ESP-IDF v4.4.7 `lcd_ll.h` and `lcd_cam_reg.h` define VSYNC bit 0, START bit 27,
RGB mode bit 31, and NEXT_FRAME bit 25. The low-level implementation maps clock
selector 2 to PLL240M. That implementation agrees with the observed divider
and the LovyanGFX source; the old `lcd_cam_reg.h` selector comment is inconsistent
and is not used to select a clock frequency.

Espressif's fixed v5.0.6 LCD documentation explains that S3 can continue output
with unavailable DMA data and that DMA restart occurs at a later VSYNC. This is
hardware-behavior documentation from a later IDF version, not a claim that this
binary uses IDF 5.0.6. The binary's IDF version is 4.4.7. The important distinction
is that a scan-timing interrupt does not depend on receiving a complete pixel
buffer.

## Model change and limits

The existing `lcd_running()` early return remains the first operation in
`dma_lcd_step`. Clocked RGB work additionally requires the existing system clock
gate, LCD_CLK_EN and a nonzero LCD clock source. A scan accumulator advances only
inside this active path; its period comes from the existing timing registers.
Clock gating pauses that accumulator. START transitions and RESET clear it.

For uninterrupted valid DMA, frame byte collection, its 512-byte service
threshold, 34-byte FIFO lookahead, frame counter, publication and VSYNC point
remain on the original complete-frame path. Completing a frame synchronizes the
new scan accumulator to that existing event. The new timer does not add another
interrupt to the same complete frame.

When data runs out, incomplete scan bytes are discarded. Later bytes from that
scan are consumed without publishing an assembled image. The clock boundary
raises VSYNC and clears any unserved pixel debt, allowing guest code to choose
and restart its own DMA chain. A starved one-shot scan clears START. No dummy
pixels are invented, no black frame is reported as a successful render, and
GDMA is never automatically restarted by the emulator. A complete frame that
was received before starvation is still delivered.

This retains the existing approximate, batched scan model. It does not model
each porch or pixel-clock edge, calibrate physical underflow values, or prove
the first recovered scan's alignment on a device. In particular, a restart that
skips 34 bytes after an entirely empty FIFO still requires actual output review;
the source's normal prefetch assumption is not manufactured by filling a FIFO.

Two fields are declared at the end of `LcdCam`. This avoids intentionally moving
existing declarations, but Rust layout and generated code still need integration
review. The unchanged disabled-path early return is not a host-performance
result. No timing benchmark, no-regression claim or golden regeneration is part
of this change.

## Checks and experiment relationship

The final targeted bus suite covers six new cases: clock-only VSYNC without a
frame; clock gates/START/RESET/one-shot; a complete circular DMA frame and a
complete finite frame; RGB565 restart after exactly 34 prefetched bytes; dropping
an incomplete scan before explicit guest DMA restart; and elapsed-cycle
remainder arithmetic. The RGB565 restart test compares every byte of both
frames and both FIFO prefixes, with the actual GDMA reset and restart register
writes. Existing malformed/terminal DMA and interrupt-rescan tests also run.
Three old synthetic DMA tests now explicitly enable the physical LCD clock;
their expected results and steps are unchanged.

Commands, results and reviewed logs are in [checks.json](checks.json) and
[checks.txt](checks.txt). Native product boot, final WASM, full workspace gates
and the required common-path CPU comparison remain integration work.

This follows [EX212](../../../experiments.md#ex212)'s distinction between a sync
clock and DMA capture, but concerns the LCD output timing engine and a different
guest driver. [EX134](../../../experiments.md#ex134) / EX157's existing active
LCD cadence is unchanged. [EX201](../../../experiments.md#ex201)'s PaperS3 cold
START callback is unchanged. [EX246-LILYGO](../../../experiments.md#ex246-lilygo)'s
descriptor-failure boundary is retained; this repair does not bypass ownership
or rewrite descriptors.

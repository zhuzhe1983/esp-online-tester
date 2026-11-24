# AXS15231B source-defined RAMWRC continuation

The retained manufacturer driver (fixed revision and SHA in the manifest)
implements `panel_axs15231b_draw_bitmap`: every vertical chunk sets CASET;
in QSPI mode it omits RASET, sends RAMWR2Ch when y_start==0 and RAMWRC3Ch
for later chunks. The shared DCS pixel engine therefore accepts3Ch data
without resetting the existing Y cursor. CASET restarts X only. A new2Ch
still restarts the full window and increments the frame counter.

The five native tests include a non-symmetric two-row chunk example plus
fresh RAMWR restart. This bounded shared command support does not declare
that all DCS-family vendor registers, AXS analog initialization, QSPI timing,
optical calibration or any complete factory application are implemented.
The board-level adapter independently validates source-specific bus pins,
SPI mode3 and actual quad data phases.

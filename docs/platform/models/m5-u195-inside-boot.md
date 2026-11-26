# Gateway H2 U195 internal BOOT

Exact-SKU naked-board `photo-3` clearly shows a black circular tact actuator and its metal body. The official BOOT procedure and the existing schematic-backed GPIO9 catalog control establish its function. The exterior photographs and original enclosure STL do not establish an external movable cap. The native case remains unchanged.

Only this local switch body/cap is reconstructed. Its placement is deliberately approximate. A three-point affine registration uses the visible PCB upper corners and Grove center:

| Feature | Source pixel | Approximate model XY, mm |
| --- | --- | --- |
| PCB upper left | 207, 282 | -10, 20 |
| PCB upper right | 421, 194 | 10, 20 |
| Grove center | 475, 563 | 0, -24 |
| Black cap center | 287, 413 | -8.83, 1.57, estimated result |

The chosen cap center is (-8.8, 1.6) mm with ±2 mm XY uncertainty. These reference PCB coordinates are clearance/photo estimates, not an independently measured PCB drawing. The metal body envelope, height and seating at the existing estimated PCB face remain display estimates (Z/height ±0.8 mm). The original UNKNOWN board envelope and all other populated electronics remain unresolved; no full layout, pads, traces or hidden linkage are inferred.

The actual production picker rejects BOOT through the closed native enclosure from six axes and an oblique direction. After production inspection separates the covers, 25 oblique rays through the real gap reach the visible internal cap. The cap/body retain their own materials with original, gold and translucent-purple shells. The closed translucent recoloring still blocks input; visual transparency does not turn the case into an input bypass.

The existing BOOT9 API/panel input remains available. That electrical simulator input does not imply that a person can press BOOT through the assembled enclosure or establish physical service access. The default picking audit intentionally remains `mapped-but-occluded`; inspection-only success is reported separately.

The offline plan rebuild preserves the original native geometry bytes and reproduces the added derived pieces. Three small CPU views inspect only this opened SKU, with an explicitly estimated switch on the UNKNOWN board. Source/model hashes, actual closed/open ray results, the independent picking audit and CPU receipts are in `../evidence/m5-control-picking-20261126/u195-inside-boot.json`. No firmware rerun, physical calibration or factory acceptance is claimed.

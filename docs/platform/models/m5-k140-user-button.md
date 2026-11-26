# SwitchC6 K140 side User button

The previous exterior approximation omitted the real orange side actuator and its opening. The official orthographic drawing, exact-SKU `photo-3`, and the uncased assembly photograph establish a separate actuator on the long +Y wall near the board seam. The printed BUTTON arrow in `photo-9` supports identification; the printed word is never a hit target.

The independent drawing is retained as `button-dimension-drawing.pdf` with source SHA256 `012b64ee588a4ba4d2fb48e3a1889826ed56a31d11bed894dbcaaa4526131558`. Its 54 mm housing width constrains the photograph scale. The source views give approximately 8×4 mm for the visible cap. The new local cap and matching wall opening are estimates: about ±0.6 mm for placement/outline, displayed depth 0.8 mm ±0.5 mm. They are derived geometry, not manufacturer CAD. The internal tact mechanism/linkage remains unknown.

The wall is actually cut around the cap; a visible orange mesh carries the existing `button` / GPIO7 identity. The front and opposite walls continue to block it from their respective directions. The front U-notch, already present in the paper's source-photo alpha, is also cut into the approximate case. Paper pixels, DIN parts, terminal levers and the UNKNOWN populated-board policy remain unchanged.

The production picker passes 35 side-cap rays and 35 rays through the real opening with the cap hidden. Front/opposite-wall rejection, real source-image alpha, U-notch clearance and orange-material isolation under original/gold/purple shell presets pass. A separate full-loader audit finds the physical button. The offline plan rebuild produces identical final geometry and metadata values.

Only K140 was rendered: before/after original side view, corrected gold/purple side views, and the corrected front notch at 480 pixels / 16 samples using the existing Blender CPU path. The camera is a studio view, not a calibrated photo overlay; hidden populated electronics are still UNKNOWN. Source hashes, model/plan/geometry hashes, actual test results and render receipts are in `../evidence/m5-control-picking-20261126/k140-button.json`.

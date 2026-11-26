# Source-bound native cover buttons

Five previously unmapped controls now have an invisible input region on their retained manufacturer molding. The visible native triangles remain whole. Photograph identity and GPIO identity remain specific to each SKU.

| Product | Control | Source surface | Photograph |
| --- | --- | --- | --- |
| U121 PoE CAM | button / GPIO37 | shell-front side flexure | photo-side |
| U121-B PoE CAM-W | button / GPIO37 | shell-front side flexure | photo-side |
| U121-B-V11 PoE CAM-W v1.1 | button / GPIO37 | shell-front side flexure | photo-side |
| U149 Thermal2 | button / GPIO39 | shell-front lower solid flexure | photo-0, explicitly annotated BUTTON G39 |
| U213 PoE-P4 | user / GPIO45 | case-front side flexure | photo-11; official KEY_USER pin table |

The regions cover conservative flat portions of the real caps. U149's RGB aperture is excluded. A 0.012 mm outward displacement separates the picking planes numerically; it does not claim physical stock thickness. Each region moves with the same production inspection group as its native cover.

`platform-m5-native-cover-buttons-test.mjs` uses the production picker and original image alpha. For 81 points per region it first hides the region and requires the nearest native molding face, matching orientation and a gap below 0.04 mm. Restoring the region must yield the correct control. Opaque foreground rejection, original/gold/purple shell presets and actual inspection movement are checked. No ray is routed through an opaque case.

The five plans rebuild offline with unchanged geometry bytes and the new presentation values. Actual production CPU exports before and after are identical in every field except the model metadata hash, including geometry, transforms, UVs, source textures, materials and gold/purple variants. Thus the existing visual evidence remains applicable without rerendering. Full source and metadata hashes, native face indices, ray results and reproduction commands are in `../evidence/m5-control-picking-20261126/five-native-covers.json`.

This establishes source-bound 3D input placement. It does not rerun firmware, establish physical calibration or broaden board-runtime acceptance.

# K136 VAMeter G0/BOOT source decision

**Physical G0 exists. Its molded Stamp-S3 cap is identified in the native K136
geometry, facing -Z. The appropriate 3D interaction is inspection-only.**
It must not be classified as a nonexistent physical button or merely a board
test point. Default assembled visibility and reachability are separate from
electrical/API support. MIX/ISO is a separate supply-selection switch and the
thumbwheel press is a separate G2 input.

Exact-SKU source checks:

- The [official product page](https://docs.m5stack.com/en/core/VAMeter) identifies
  Stamp-S3, lists Boot in the user interface, and instructs holding G0 during
  power-on. The [restore guide](https://docs.m5stack.com/en/guide/vameter/restore_factory)
  gives the same G0/USB procedure.
- Official gallery `4.webp` labels the menu input `BtnG0/BOOT`; `7.webp` shows
  the real Stamp module and its connected molded tongue after the Base is
  removed. Gallery `16.webp` and the download GIF provide the USB-side context.
  The receipt binds their original source URLs and bytes. No image was edited.
- Official firmware `hal_config.h` at
  `5e8f847bbc771da2d354be7472c6345acb5655ac` assigns the side button G0 and
  encoder A/S/B to G3/G2/G1 independently.
- M5_Hardware at `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109` has only the
  source STL under `Products/K136_VAMeter`; current master has the same
  directory tree. `KiCad/Projects` contains ExtPort, not a VAMeter board.
  The retained Stamp-S3 KiCad STEP and independently read Stamp-S3-SMD footprint
  corroborate module geometry. The footprint references a module STEP with
  +1.1 mm Z offset; it does not provide a K136 board netlist or button-cap net.

The native cover registration checks all 24 proper signed axis rotations using
every unique vertex: 1,574 from the exact Stamp-S3 cover against 1,600 in K136
component 0. The unique good solution, in current S007 model coordinates, is:

```text
rotation = diag(1, -1, -1)
translation_mm = [-0.0784478188, -1.8184561729, 7.8999992609]
```

Nearest-vertex RMS is 0.013724 mm; median is approximately 0.000015 mm and
90th percentile approximately 0.000016 mm. The next-best rotation has RMS
1.129 mm. This identifies the actual reversed module orientation using full
geometry, beyond matching outer dimensions. Different tessellation of a few
edges accounts for the retained residual; no geometry is fitted or changed.

Use K136's own flat tongue area: center `[3.4,-3.6,5.53]`, input footprint
`[2.4,2.2]` mm, outward normal `[0,0,-1]`. Five center/inset-corner samples lie
on native triangles 1175, 1020 and 1193 at Z=5.55000019 mm, about 0.02000019 mm
from the input plane. Their exact vertices and barycentric containment are
retained. The larger S007 interaction rectangle is not transferred wholesale:
its transformed left corners extend onto a deeper hinge surface.

The source decision is closed. Production closest-surface picking and visible
equivalence for the separately added inspection mapping are independent
evidence; they must keep the real shell and UNKNOWN board as occluders. This
study changes no model, populated PCB, runtime or firmware behavior and claims
no GPU, guest or physical hardware measurement.

Reproduce with the retained exact source assets and independently obtained
gallery/footprint files:

```sh
python tools/audit-vameter-boot-source.py --source-root "$SOURCE" --photo-root "$PHOTOS" --footprint /tmp/Stamp-S3-SMD.kicad_mod --out /tmp/vameter-boot-source.json
```

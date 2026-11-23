# Exploded model view

The workbench and embedded player place the screen capture button to the right of **3D / 2D**. In 3D, models with valid inspection groups get a **爆炸图 / Exploded view** slider to the left of the shell settings. The slider shows 0% for the assembled model and 100% for the declared viewing offsets. Intermediate values help reveal covered parts and `UNKNOWN` surfaces.

Add the same declaration to the reproducible model plan and exported `model.json`:

```json
{
  "presentation": {
    "inspectionGroups": [
      {
        "id": "cover",
        "parts": ["cover", "printed-film", "cover-screw", "cover-key:button"],
        "offsetMm": [0, 0, 12],
        "sourceIds": ["manufacturer-cover", "product-photo-top"],
        "notes": "The source cover, printed sheet and screw head form this viewing group. The +Z clearance is a viewing offset, not a measured disassembly travel."
      }
    ]
  }
}
```

Each `sourceIds` entry must exist in `model.provenance.sources`. Record the basis for membership, the direction and any estimated clearance in `notes`; do not invent hidden relationships. `offsetMm` uses the exported model's local millimetre axes, before the viewer rotates the whole product. It is added to each member's exact assembled position. Existing rotations, scale, vertices and material objects remain unchanged. The array defines flat groups; it does not infer a hierarchy or calculate radial offsets.

`parts` contains exact, unique names of direct children in the loaded model. These include source part names, derived part IDs and named controls such as `cover-key:button`. Keep a cover's paper, visible hardware and cover-mounted hit regions in its group. A button physically mounted to the PCB remains with the PCB. The loader automatically includes each listed part's generated photographic projection surface; do not list a `-photo-surface` twin separately. Groups cannot claim the same object twice. Nested rotor/servo pieces must not be claimed separately from their top-level assembly.

A group can additionally declare `hideParts: ["optical-flow-harness"]` for fixed parts that should disappear only in the separated viewing arrangement. Use this for a documented cable that cannot follow the chosen viewing offset; do not stretch it or imply a simulated disconnection. Each name follows the same direct-child and unique-ownership rules as `parts`, with a maximum of 128 hidden parts. A hidden part cannot also belong to a moving group. Its registered photo companions hide with it. Expansion changes only visibility; reassembly restores the original visibility, including parts that were already hidden. Record the viewing-only reason in the group's `notes` and retain the actual assembled wire geometry.

At most 32 groups and 128 declared parts per group are supported. Offsets contain three finite values, each within ±5000 mm, with at least one nonzero component. A group requires a nonempty ID, source references and a note. Validation completes before any transform changes. An invalid declaration leaves the assembled model available, disables inspection and records `group.userData.inspectionError` plus a console diagnostic. Missing declarations add no control and do not create product inputs.

The slider scales each offset from the saved assembled position. Setting it to 0%, resetting the view, changing product, switching to 2D or loading or resetting firmware restores assembly. Appearance changes preserve the separation and keep each part's material. Shared scenes open assembled and retain ordinary appearance settings. The slider belongs to the viewer and has no scene, recording or automation field. The viewer releases held inputs before moving the hit meshes.

Attaching external accessories restores assembly and disables the slider while they are connected. Remove them to adjust the exploded view.

For renderer integrations, `createSceneView` exposes `getInspection()` with `available`, `expanded`, `progress` and `blockedByAccessories`. Use `setInspectionProgress(value)` with a value from 0 to 1. The existing `setInspection(boolean)` sets one of the two endpoints. `onInspectionChange` also reports state after model loading, reset and product replacement.

Validate with `node --test tools/platform-model-inspection-test.mjs` and a real headless browser. Check intermediate offsets, whole-sheet support, photo twins, hit meshes, readable `UNKNOWN`, exact restoration, reset and product replacement, appearance, attached accessories and both public pages. Use the actual product geometry for appearance claims.

The reproducible public UI fixture is [`tools/fixtures/model-inspection-browser-check.js`](../../tools/fixtures/model-inspection-browser-check.js). Open the caller-supplied platform URL with the Playwright skill wrapper, create `output/playwright`, then pass that file's function to `run-code`. It temporarily routes S007 metadata, exercises both public pages and removes its route and child page in `finally`. Close the task-owned wrapper session afterwards. The [browser receipt](evidence/model-inspection/browser.json) records 20 passing checks, screenshots and source hashes; individual products still require their own source and `UNKNOWN` readability review.

The [review-fix receipt](evidence/model-inspection/review-fixes.json) records the later 21-check public flow and camera regressions. Run `tools/fixtures/model-inspection-camera-check.js` through the same wrapper to measure projected bounds after both `setOrientation` and a real pointer orbit, including the maximum declared offset and restoration. Separated views reframe during rotation and extend the camera depth range when necessary; assembled views keep their ordinary camera behavior.

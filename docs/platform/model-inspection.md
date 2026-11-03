# Declared model inspection

The workbench and embedded player show **分开查看 / Inspect parts** only when the loaded model declares valid inspection groups. The button moves those parts to a documented viewing arrangement so that covered `UNKNOWN` surfaces can be inspected. It does not imply complete internal CAD, a physical disassembly sequence, or firmware support.

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

At most 32 groups and 128 declared parts per group are supported. Offsets contain three finite values, each within ±5000 mm, with at least one nonzero component. A group requires a nonempty ID, source references and a note. Validation completes before any transform changes. An invalid declaration leaves the assembled model available, disables inspection and records `group.userData.inspectionError` plus a console diagnostic. Missing declarations add no control and do not create product inputs.

Inspection is temporary viewer state. Repeated toggles apply offsets from the saved assembled positions, so they do not accumulate. **恢复装配 / Reassemble**, **恢复视角 / Reset view**, product changes, switching to 2D and a firmware load/reset return to assembly. Appearance changes preserve the inspection view and the existing material boundaries. Shared/exported scenes preserve their ordinary appearance settings and open assembled; inspection is absent from the scene schema, recordings and automation/iframe RPC. The view does not pause, start, reset or feed a sensor merely to separate parts. A human toggle releases any held model input before moving its hit mesh.

Attaching external accessories returns to assembly and disables inspection until they are removed. The current cable renderer does not claim that connected cables follow arbitrary separation offsets. The disabled button explains this boundary.

For renderer integrations, `createSceneView` exposes `getInspection()` (`available`, `expanded`, `blockedByAccessories`) and `setInspection(boolean)`. `onInspectionChange` reports availability and state changes after model loading, reset and product replacement. These methods belong to the viewer, not `window.espTester`.

Validate with `node --test tools/platform-model-inspection-test.mjs` and a real headless browser. Check whole-sheet support, photo twins, hit meshes, readable `UNKNOWN`, exact restoration after repeated toggles, reset/product replacement, appearance, the disabled accessory case and both public pages. A synthetic declaration used to exercise the viewer is a UI fixture, not product adaptation evidence.

The reproducible public UI fixture is [`tools/fixtures/model-inspection-browser-check.js`](../../tools/fixtures/model-inspection-browser-check.js). Open the caller-supplied platform URL with the Playwright skill wrapper, create `output/playwright`, then pass that file's function to `run-code`. It temporarily routes S007 metadata, exercises both public pages and removes its route and child page in `finally`. Close the task-owned wrapper session afterwards. The [browser receipt](evidence/model-inspection/browser.json) records 20 passing checks, screenshots and source hashes; individual products still require their own source and `UNKNOWN` readability review.

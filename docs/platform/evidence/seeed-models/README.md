# Seeed display model evidence

This is the historical first-stage snapshot for the original 13 Seeed devices. Its measured sizes, hashes and review images remain preserved. The current 48-model dataset and subsequent Watcher surface correction are documented in the [expanded model report](../seeed-expanded-models/README.md); subsequent integrated gates are recorded in the [final integration report](../seeed-final-integration-20261122/README.md).

This batch provides 13 local, reproducible 3D previews: 11 entries based on official CAD, one dimensioned photo reconstruction, and one explicitly simplified bare-screen debugging model. Product capabilities and firmware execution are assessed separately by the platform integration work.

The rendered review sheets in this directory are **software mesh renders with a synthetic framebuffer pattern**. They are not browser screenshots or firmware screenshots. They check gross shape, front/back orientation, native assembly placement and screen visibility. They do not establish WebGL behavior, hardware operation, mechanical fit or a calibrated photo-overlay pass.

## Inventory

Geometry sizes below are decimal MB. Body dimensions exclude deployed stands and optional cables where noted in the individual plan. For the two procedural fallbacks they are construction inputs or an estimated debugging layout, not independent CAD measurements.

| Product | Actual geometry method | Geometry MB | Body width × height × depth, mm | Front/back/angle review |
| --- | --- | ---: | --- | --- |
| `seeed-reterminal-d1001` | manufacturer-cad | 5.199 | 201.400 × 136.400 × 25.600 | [review](seeed-reterminal-d1001-review.png) |
| `seeed-reterminal-e1001` | manufacturer-cad | 2.718 | 175.000 × 120.400 × 17.000 | [review](seeed-reterminal-e1001-review.png) |
| `seeed-reterminal-e1002` | manufacturer-cad | 2.718 | 175.000 × 120.400 × 17.000 | [review](seeed-reterminal-e1002-review.png) |
| `seeed-reterminal-e1003` | manufacturer-cad | 9.693 | 185.100 × 223.700 × 18.201 | [review](seeed-reterminal-e1003-review.png) |
| `seeed-reterminal-e1004` | photo-dimension-reconstruction | 0.007 | 311.000 × 376.000 × 40.000 | [review](seeed-reterminal-e1004-review.png) |
| `seeed-reterminal-sticky` | manufacturer-cad | 8.918 | 105.820 × 65.440 × 7.602 | [review](seeed-reterminal-sticky-review.png) |
| `seeed-sensecap-indicator` | manufacturer-cad | 10.549 | 92.139 × 96.076 × 18.716 | [review](seeed-sensecap-indicator-review.png) |
| `seeed-sensecap-watcher` | manufacturer-cad | 8.318 | 65.000 × 68.953 × 20.200 | [review](seeed-sensecap-watcher-review.png) |
| `seeed-trmnl-75-diy` | screen-debug | 0.004 | 212.500 × 127.000 × 1.800 | [review](seeed-trmnl-75-diy-review.png) |
| `seeed-xiao-epaper-75` | manufacturer-cad | 1.413 | 178.200 × 130.500 × 20.250 | [review](seeed-xiao-epaper-75-review.png) |
| `seeed-xiao-ips-096` | manufacturer-cad | 3.718 | 18.953 × 43.653 × 10.680 | [review](seeed-xiao-ips-096-review.png) |
| `seeed-xiao-ips-114` | manufacturer-cad | 4.280 | 26.150 × 48.094 × 10.780 | [review](seeed-xiao-ips-114-review.png) |
| `seeed-xiao-ips-147` | manufacturer-cad | 4.660 | 26.150 × 51.394 × 10.520 | [review](seeed-xiao-ips-147-review.png) |

Total geometry payload: **62.196 MB** across all 13 devices. The normal model loader fetches metadata and geometry, not the retained source CAD archives or reference photographs. The catalogue uses the existing `manufacturer-cad` loader contract for all these files; `model.json.provenance.kind` records the actual acquisition/reconstruction method and must be used when describing provenance.

## Sources and reproduction

Each device directory under `web/platform/assets/seeed-*` contains `model-plan.json`, `source-manifest.json`, `model.json`, `geometry.bin`, `build-check.json`, retained reference material and `SOURCE-RIGHTS.txt`. Source receipts contain the original URL, resolved URL, actual UTC acquisition time, byte count and SHA-256. A merely linked specification is labelled `linked`, not represented as a raw download. The XIAO 7.5-inch STEP parts are labelled as extracted members of the retained official ZIP rather than independently fetched files.

The Watcher source was originally acquired from the manufacturer's main branch. Its retained bytes were additionally verified against the official GitHub blob metadata at commit `0dbd7ac5a0bedb2de51fdb7bb2064a9001e9d273`, reproducing Git blob SHA-1 `708241d7ad4e29663ec0363e1305f2d633a72fea`. The original acquisition receipt is preserved, alongside the immutable reference and the actual metadata download receipts.

Install the repository's `tools/requirements-model.txt` dependencies, then run from the repository root:

```sh
python3 tools/build-seeed-models.py --output /tmp/seeed-rebuilt
python3 docs/platform/evidence/seeed-models/render-review.py
python3 docs/platform/evidence/seeed-models/verify-models.py --catalog web/platform/catalog/products.json
```

The builder is offline and validates retained source hashes before opening them. CAD is transformed only by an explicit rigid matrix; scale is always one. Selected native solids, orphan shells and faces retain their assembly positions. The normal mesh tolerance is 0.12 mm / 0.35 rad; Watcher and Indicator use 0.15 mm / 0.4 rad. Small copper, silkscreen and package detail is omitted only where the plan lists the exact original component indices. Tessellation settings are not presented as an independently measured manufacturing tolerance.

`metadata-privacy.json` documents the minimal removal of unrelated personal fields from one GitHub metadata response and editing-history paths from two reference images. Original HTTP receipt fields stay unchanged; a separate `retained` object identifies each sanitized file used by the offline builder. The image codestreams outside XMP and their decoded RGBA hashes are unchanged, as are all source CAD and generated geometry. The original and retained identities must not be interchanged when reproducing an HTTP download or an offline build.

`reproduction.json` records independent byte-identical rebuilds of the IPS 1.47-inch model and both procedural fallback methods, including geometry, metadata and build-check JSON. The other exports have source/hash/geometry checks; that receipt does not claim every CAD export was independently repeated.

## Source selection decisions

- The three IPS STEP assemblies explicitly identify their XIAO ESP32S3 Plus and matching display boards. They are not substituted RP2350 electronics. Optional unplugged battery wiring and duplicate socket instances are omitted where specified. The live planes fit the native windows; 1.14-inch and 0.96-inch panes are inset slightly to preserve framebuffer pixel aspect instead of stretching the CAD.
- E1001 and E1002 share an official STEP assembly containing distinct, explicitly named 7.5-inch and 7.3-inch front-cover alternatives. Each product selects its corresponding cover. Two exact duplicate instances are omitted; the manufacturer stand stays in its native deployed pose.
- E1003 and D1001 retain their original major case, board, display and deployed-stand positions. Display mounting planes are separate runtime surfaces. E1003 active size derives conservatively from the published 10.3-inch diagonal and 3:4 pixel aspect; it is not claimed as an independently verified panel drawing dimension.
- Sticky is rigidly rotated into a landscape debugging view for the 800×480 raw framebuffer; official photographs show portrait handheld use. CAD geometry is not resized. The function block diagram is correctly classified as a hardware diagram, not a physical reference photograph.
- Watcher uses the later native assembly. The source also contains an earlier assembly, repeated TF-socket surfaces, an overlapping socket alternative and unplaced camera/title geometry, including an outlier near native Z = −826 mm. Those source indices are retained in the omission record. The selected case, camera, board, wheel and stand are not relocated.
- Indicator retains its deployed stand and omits the alternate folded stand instance. Source-overlapping connector representations are disclosed; the preview does not certify a specific sensor or connector hardware revision.
- XIAO 7.5-inch has official enclosure, stand and button CAD only. A separately positioned press board from the archive is retained as source but omitted from the preview because its assembled location is unverified. Its internal board is a visibly labelled `UNKNOWN` placeholder, with a separate display inspection group.
- E1004 has no official CAD in the consulted resources. Published 311×376×40 mm portrait dimensions anchor a procedural frame, mat and rear enclosure derived from official front/back photographs. Mat opening, electronics-cover location, visible fastener heads and USB-C location are estimates. The internal board is `UNKNOWN`; no hidden circuitry, mounting-hole pattern or screw threads are claimed.
- TRMNL 7.5-inch DIY is a bare-screen kit. Its debug model has an estimated panel border/flex and a separate `UNKNOWN` controller silhouette. It does not borrow the XIAO cased product's enclosure. The side-by-side inspection layout is explicitly not a claimed assembled device.

## Checks and limitations

`checks.json` validates all source and generated artifact hashes, finite six-float geometry, normalized normals, part bounds, unique part/source identifiers, inspection references, rigid transforms and front-facing screen sample visibility. A 5×5 ray grid is used within rectangular panes; the Watcher samples are restricted to its circular pane. The static catalogue check confirms model URLs and the presence/type of every bound 3D button ID. It records catalogue buttons without a verified 3D binding rather than fabricating physical locations.

The static review found and corrected a duplicate Sticky PCB part name; the native tiny board detail and main board now have separate identifiers. Sticky's STEP `POWER_BUTTON_1` is bound to AI/Power only because the official hardware table explicitly gives that shared identity on GPIO4. The other two side-button meshes remain present without `controlId` until their Up/Down physical order is verified. E1001/E1002 hotspots use the official key identities and native cap positions: key2, key1, then green key0 from left to right along the top edge.

The Watcher wheel is bound to the `knob` press action through the manufacturer's PCA9535 P03 button definition; rotation remains a separate encoder action. The Indicator's native top button is bound to `button` through the manufacturer's active-low GPIO38 definition. `control-bindings.json` records the fixed source identities, original component indices, and byte-identical geometry hashes before and after regenerating both models from their updated plans. These bindings do not add geometry or claim that a click supplies encoder rotation. The BSP gzip archives retain the original manufacturer bytes and notices, with separate original and archive hashes.

All 13 meshes were visually inspected from the front, rear and an oblique view using the saved software renders. This review and its ray checks remain independent of any browser acceptance performed later. Transparent-material behavior, live touch coordinates, deployed-site loading and real firmware behavior require their respective integration checks. Official dimensions are compared where available; revision/rounding differences remain visible in each `build-check.json`. No calibrated three-view photo alignment or physical hardware fit has been completed, and neither is reported as passing.

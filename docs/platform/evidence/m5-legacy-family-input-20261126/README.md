# Immutable legacy family-CAD metadata input

The K022/K006/K007 recipes originally pinned K001 `model.json` SHA-256 `bdcf623c5956ab615752680bb2f8f7b84faa73612bdd260f0e4d2cb3dd3d26f3` from commit `81bc93c0333a9fd13c16565b245b3d1c0d7ce237`. The current original Basic rear closure/paper correction changes that metadata to `0e84d0acbe4294100a93446c924fde4acd5102aaeca83035b5788ac997e88249`; it does not change its native `geometry.bin` (`0f7484955cd052e105dfd16ad129734ae86b643a49c53c45fcc13f97aaf983ec`). Accepting the new metadata hash as the old source would blur that distinction.

Each of the three child plans now retains a5122-byte `family-k001-model.json.gz`, an exact compressed copy of the original24013-byte metadata. Both compressed and original bytes are pinned. `familyCad.inputs.model.json` keeps its original SHA. The builder reads this immutable snapshot and continues checking the existing geometry/STL against their original hashes. No Git executable, historical checkout or network is needed at build time. The snapshot is a recipe input beside the child plan; it is not a new visual model artifact and is not inserted into the existing output artifact map.

The builder consumes only `parts`, `presentation.screen`, `presentation.buttons` and the `usb-c`/`grove` entries of `derivedParts`. That complete data view is equal between old and current K001 metadata; its sorted compact-JSON SHA-256 is `47fd59147ab1cba38ebd3414e81fafff69cb222778642605a631b943f4498e2f`. The current K001-specific rear paper and closure derived parts are consequently retained on K001 and are not silently inherited by older products.

The [receipt](receipt.json) records the original/new child plan hashes and unchanged current model/geometry hashes. The actual public offline builder regenerated all five current legacy models byte-for-byte, including the source-traced K016-B glyph and29mm K016-T. Loader/material/inspection and every native vertex's rigid-translation checks also passed. Corrupted compressed metadata, a mismatched original metadata pin and a mismatched geometry pin were each rejected. No target `model.json`, `geometry.bin`, source picture or visible appearance changed in this repair.

After integrating the plans and snapshots, the ordinary regression command needs no special environment:

```sh
node --test tools/platform-m5-legacy-kit-model-test.mjs
```

To read assets from a separate immutable working tree without copying large inputs, the builder also accepts `--source-root <repository>` and optional `--plan <plan.json>`. The Node check's optional `M5_LEGACY_SOURCE_ROOT` and `M5_LEGACY_PLAN_ROOT` support that same independent validation. Missing overrides fall back to the source repository's current plan. These options do not disable any fixed-input hash check.

# Per-SKU M5Stack runtime evidence coverage

The machine-readable [matrix](evidence/m5-runtime-coverage-final-20261126/matrix.json) freezes source revision `02d56152` and contains all141 controller identities. It preserves the original101 partial entries and the40 continuation entries using catalog revision `fbe068e3`, independently of their current status. All141 declared chip/memory profiles construct through the real020 WASM ABI; all141 own knowledge records are present and match their catalog identity. Every SKU now has an exact-product execution reference rather than only a shared-board, model or status claim.

| Coverage dimension | Recorded result |
| --- | --- |
| Original/continuation cohorts | 101/40 |
| Real declared-profile constructors | 141 constructed,0 rejected,0 unconfigured |
| Exact-SKU functional execution references | 141; no missing functional anchor |
| Dedicated machine-readable firmware investigations | 106 |
| Other firmware identity/investigation provenance | 25 knowledge-source investigations,10 catalog identity records; no factory run is inferred |
| Physical or complete factory application acceptance | No new claim from this matrix |

`tools/audit-m5-runtime-coverage.mjs` uses the unchanged central020 WASM (`0cc18d96042c0b2707afbe3b80102cabcbdd6868fccfcc84504cabed8b3179a2`) to construct each configured SKU with its own declared Flash/PSRAM profile. It does not load firmware. Existing execution references are curated separately in `tools/fixtures/m5-runtime-coverage-anchors.json`, with SHA-256 for every referenced receipt. The report keeps exact-product runs, shared-board runs, direct native board tests, narrative-only runtime reports and missing references distinct. A historical receipt is not silently upgraded to an020 rerun.

Each row includes knowledge identity/hash, firmware candidates and roles, retained firmware investigations, runtime evidence, limits and explicit gaps. Matching an image hash to an execution receipt provides a place to inspect the run; it does not establish successful factory application acceptance. Catalog `partial`, firmware `simulationVerified`, model validation and constructor success are not used as functional pass criteria. A missing dedicated survey reference can coexist with catalog firmware identity or knowledge investigation and is reported at that narrower scope. The35 entries without a separate dedicated survey JSON remain named in the matrix; for example S012 retains the explicit historical empty-M5Burner-category source and source-only/no-fixed-image investigation, rather than being misreported as a located factory binary.

The final closure adds20 independently product-scoped Worker020 diagnostics: the11 [additional controllers](m5-coverage-worker-20261126.md), eight Basic/Gray/FIRE variants and K152 [Cardputer Mesh host](evidence/m5-basic-mesh-runtime-20261126/README.md). These retain positive input consumption, actual guest output, wrong physical routes and new-guest public reset results. Previous native/Worker receipts remain individually hash-pinned. Each run's tested digital subset and omissions remain attached to its own SKU; the141 references do not imply that all peripherals or all original applications execute.

Reproduce after integrating the intended catalog and receipts:

```sh
node tools/audit-m5-runtime-coverage.mjs \
  --source-root <repository> --revision HEAD \
  --wasm <immutable-central020.wasm> \
  --out docs/platform/evidence/m5-runtime-coverage-final-20261126/matrix.json \
  --require-all-construct
```

The source revision is frozen before reading catalog/knowledge/evidence. The script makes no catalog or runtime changes, performs no network downloads and invokes no Cargo or rendering process. `--require-all-construct` returns nonzero while any SKU has no configured board or its real constructor rejects the declared profile. Functional and firmware gaps remain visible even when all constructors pass.

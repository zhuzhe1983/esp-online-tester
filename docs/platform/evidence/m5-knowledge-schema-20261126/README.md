# M5 knowledge Schema alignment

All141 M5 product knowledge records validate against the committed Draft2020-12 Schema, including calendar/timezone assertions. The original253 structural errors are resolved. An additional72 errors appear when genuinely asserting URI formats against original manufacturer URL spellings; the provenance locator contract below preserves those original paths instead of rewriting source history.

Only12 knowledge files change, with40 additive fields. Every pre-existing JSON value and array position survives. The source URLs, fetched/checked/original timestamps, hashes, revision strings, firmware investigations, keyboard maps, facts and unknowns are unchanged. The machine proof covers all3730 M5 source records,416 unknown entries and9755 source references.

| Contract | Explicit treatment |
| --- | --- |
| Product/catalog/device IDs | Original lowercase-and-hyphen `id` definition retained exactly. Nested catalog-role and firmware IDs also use it. |
| Source/asset IDs and source refs | Separate `sourceId` namespace allows ASCII letter case, digits, dots, underscores and hyphens; maximum160 characters, no slashes or URLs. Real examples include `runtime-v11-M5Dial-h`, `capture-camera_pins-h`, `shared-runtime-paper-M5GFX.cpp` and `K014-B-gallery-0`. No existing ID is renamed. |
| Source HTTPS provenance locator | Only source/asset URL fields permit exact publisher path spelling, including literal spaces in `M5StampS3 PIN1.27` and `AtomS3 Lite`. Scheme/host are constrained; credentials, host spaces and control characters are rejected. HTTP request parsers encode path spaces. Catalog and firmware links retain the separate URI contract. |
| Original acquisition times | `originalFetchedAt` preserves full precision and original timezone offsets. `checkedAt` accepts an actual date or timestamp; it does not establish a successful fetch. Canonical `fetchedAt` and all original fetch/failed/link rules remain in place. |
| Source revision | Preserve either a nonempty original revision string, including a Git SHA or `Rev5; 2/2015`, or a structured object with explicitly typed Git, repository, document, container and acquisition fields. No unrestricted object is added. |
| Embedded acquisition | Explicit method, container source ID/SHA, embedded name, checksum algorithm, declared/verified checksum and decoder fields. The checker verifies container references/hashes and checksum equality. |
| Surveys/catalog roles/keyboard mapping | Every existing extension is declared with named typed fields and closed nested objects. Product/firmware IDs remain strict. Keyboard row/column/GPIO/scan-index consistency and source refs are checked independently. |
| Unknown reasons | The existing `reason` string is accepted on facts; `unknowns` still requires `basis: unknown` and `value: null`. |
| Legacy asset kind | Retain `photo` or `manufacturer-photo` alongside the required normalized role, rather than deleting historical classification. |

Actual missing metadata is repaired as follows:

| Missing field | Added value | Evidence boundary |
| --- | --- | --- |
| Eight revision release dates | `null` | No release date is inferred from a fetch/check time, version label or page update. |
| K005 revision local ID | `flash-old-versus-current` | Stable local identifier for the existing comparison; original `revision: old unspecified` text is retained. |
| K005 revision basis | `official` | The existing official fact `spec-0-3` says `16MB (Old version 4MB)` and cites the same fetched `official-docs` source. No revision of the user's hardware is inferred. |
| Fifteen photo asset IDs | Existing `sourceId` | These IDs are unique within each record. |
| Fifteen photo asset roles | `photo` | Taken directly from each original `kind: photo`; the original kind remains. |

`record-additions.json` identifies every added field and its reason. `validation.json` records all141 old/new raw record SHA-256 hashes, canonical source/fact/unknown hashes, per-record reference counts and an assertion that every old value survives. `baseline-structural-errors.json` preserves the original253 structural errors; `baseline-errors.json` additionally asserts URI/date-time formats.

The check also runs the exact unmodified `tools/platform-knowledge-test.mjs` from baseline commit `710a6911dc2343b9a097403d5338f4ed7e0b19f7`, against all736 current indexed records and matching baseline catalogs. All736 pass that separate JS validation. This does not mean all736 pass JSON Schema:595 non-M5 records retain3021 pre-existing Schema errors outside this task. All420 previously Schema-valid non-M5 records remain valid;31 additional non-M5 records now validate because the shared source/provenance types also cover their existing metadata. Non-M5 record bytes are unchanged. No factory firmware or hardware validation is added.

The negative cases reject relaxed catalog/device IDs, source path IDs, arbitrary extensions, invalid dates/types, false linked-source fetch receipts, concrete unknown values, missing basis/asset roles, invalid acquisition metadata, unrecognized nested survey fields, malformed keyboard GPIOs and dangling source references. Date-time assertions use the standard library in addition to explicit Schema syntax patterns; optional `jsonschema` format plugins are not required for the recorded proof.

Reproduce from this commit, with the pinned baseline available locally:

```sh
python docs/platform/evidence/m5-knowledge-schema-20261126/check.py --repo . --output /tmp/m5-knowledge-validation.json
```

The checker performs no network requests, downloads, model loading, firmware execution or source-tree mutation. Its temporary JS view contains only knowledge records, baseline catalogs and the unchanged validator source.

## Validating current data after independent source updates

The default mode remains the frozen lossless comparison above. It deliberately rejects unrelated changes since710a, including later correct source revisions, facts, unknown entries or `updatedAt` changes. The original `validation.json`, `baseline-errors.json`, `baseline-structural-errors.json` and `record-additions.json` remain immutable evidence of the original repair; their checker is also preserved in commit `35f93da41b3dc44332bd15b4e62a7f1c57789ee2`.

For current integration data, run:

```sh
python docs/platform/evidence/m5-knowledge-schema-20261126/check.py --validate-current --repo . --output /tmp/m5-knowledge-current.json
```

This mode reads the current Schema, catalogs, index and knowledge records. It asserts all141 M5 catalog/index identities are covered, the same strict Schema/date/time rules, source references, embedded-container hashes/checksums, keyboard relations and23 negative cases. The unchanged current JS validator runs against a disposable byte snapshot of the same736 captured records and current catalogs. No pinned baseline history is needed for this mode.

Its report explicitly says `mode: validate-current` and `losslessBaselineComparisonPerformed: false`. It records current source commits and record hashes, and uses “validated” rather than “unchanged” counts. It makes no attribution of concurrent source/fact changes to the Schema repair and writes no baseline-error files. Attempting to target one of the four frozen evidence files is rejected before writing.

`current-validation.json` records the actual Root integration run after the independent Tough0c source update:141 M5 records pass with3733 source records,417 unknown entries,9767 source references,23 rejected negative cases and the736-record JS pass. The historical3730/416/9755 lossless counts remain in the original receipt. Neither mode claims the remaining3021 non-M5 current Schema errors are fixed.

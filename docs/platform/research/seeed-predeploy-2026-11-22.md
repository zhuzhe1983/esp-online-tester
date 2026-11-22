# Expanded Seeed catalog: release metadata predeploy review

The release snapshot contains **149 watch entries** covering **55 upstream repositories**. Its compact JSON is **1,435,970 bytes**, below the **2,097,152-byte** deployment budget. This synchronization changed no firmware binary URL, hash, version, flash layout or product default. It downloaded release metadata only and executed no firmware.

## Source mapping and bounded metadata

Seventeen explicitly selected EE board/panel configurations track the same official OSHW release, whose latest asset list contains 381 entries. Saving the complete list in every reference entry would produce a 6,130,046-byte snapshot. Each of these 17 entries now has an anchored `metadataAssetNamePattern` selecting exactly its four board/panel BIN names. The release identity, dates and original asset count remain available as metadata.

This field is allowed only when `compatibility.status` is `reference-only` and there are no automatic `catalogTargets`. Both the JavaScript validator and JSON schema enforce that boundary. Failed refreshes compact the retained historical metadata using the same rule, while preserving the original observation time. The selector cannot silently change a firmware, choose a neighboring panel, or turn a source mapping into an execution claim.

The 48 Seeed catalog identities comprise 23 independent devices or bare boards and 25 explicit configurations of four real configurable kit SKUs. They do not represent 48 distinct manufacturer SKUs. Shared display circuitry is explicitly sourced; each local manifest must match its product's declared runtime board. New commercial editions retain their own SKU facts and do not inherit another edition's simulation or hardware validation. Model provenance separately identifies official family CAD, PCB/photo reconstruction, dimensioned screen debug components and UNKNOWN internal boards.

## Observed requests and retained failures

The live run took place from **2026-11-22T09:33:11.665Z** to **2026-11-22T09:35:07.125Z**. It issued **155 requests: 152 returned HTTP 200 and three returned HTTP 404**. Those three missing `releases/latest` resources were:

- `Seeed-Projects/Display-Gadgets` — affects the three IPS watch entries; their fixed-commit factory archives remain independently available.
- `Seeed-Studio/reTerminal-D1001`.
- `Seeed-Studio/OSHW-SenseCAP-Watcher`.

Seven other entries received responses that did not yield the configured release metadata: `m5stack-dial-v11-uiflow2`, `lilygo-factory-s3`, `lilygo-factory-original`, `lilygo-factory-beam`, `lilygo-factory-deck`, `lilygo-factory-embed`, and `m5-official-stampc3-examples`. Their error records remain visible. In total, 137 entries contain a release and 12 contain an error. A successful HTTP request alone is not a successful configured release match.

The 17 new reference entries were initially seeded from the same upstream's existing observation at **2026-11-22T07:57:30.504Z**, then checked in this live run. The synchronizer's existing same-day policy retains the earlier `checkedAt` when the result is unchanged; the request receipts preserve the actual later access times. Earlier standalone HTTP 403 attempts remain in the product research records. Neither an old observation nor this mixed-result synchronization is described as universal current availability.

## Pinned recovery source and runtime boundaries

The separate Indicator Meshtastic recovery work added the official `2.5.11.8e2a3e5` ESP32-S3 archive, **62,741,576 bytes**, SHA-256 `62c02b3636e448e4d16e33488d5ec3a29fb0fca439d10a3bda2d649bde5f95d0`. Its three exact entries retain their own hashes and partition offsets. This is one additional pinned official recovery source alongside 32 factory-image-pinned products. The guide names factory version 2.5.11, but does not establish the exact shipping build suffix or manufacturing-batch image hash. The factory reference remains separate; the recovery is a trial with `simulationVerified: false`.

The archive has 56 members with 99,880,682 declared expanded bytes. The loader keeps three independent limits: 64 MiB downloaded ZIP, at most 64 members, and at most 128 MiB declared expanded data. It inflates only the selected member, checks its board/member size limit, CRC and SHA, and retains the existing duplicate-name, path, encryption, symlink and ZIP64 rejection rules. One verified container may be reused within a single load; member cache identities remain separate. The exact layout and upstream helper-script defect are recorded in [the recovery layout](seeed-meshtastic-2511-layout-2026-11-22.json).

This metadata review does not claim a displayed Meshtastic UI, working radio, or real hardware validation. Runtime receipts are maintained separately and preserve unsuccessful frame checkpoints.

## Verification and evidence retention

Replaying the exact recorded HTTP response bodies through the current complete sync pipeline returned **`changed: []`**. This is a reproducibility check, not another live network observation. The report retained 176 configured update decisions, made zero updates, resolved one download metadata entry and left 71 requiring verification. No firmware binaries were downloaded by this synchronization and no firmware was executed.

Before changing the shared metadata filter, a successful read-only connected GitHub request for this repository's open pull requests returned an empty list. An earlier unauthenticated HTTP 403 was not interpreted as proof of no open work.

The [machine-readable audit](seeed-predeploy-2026-11-22.json) preserves the request URLs, statuses, UTC times, original byte counts/hashes, errors, snapshot hash, replay result and completed validation commands. The [retained audit harness](seeed-predeploy-sync-audit-2026-11-22.mjs) invokes the existing synchronizer and writes proposed outputs to an explicitly supplied temporary directory for review. Raw API response bodies are not committed because they include unrelated author and uploader records. The normalized snapshot contains only the release metadata required by the platform; its bytes are not represented as original HTTP bodies.

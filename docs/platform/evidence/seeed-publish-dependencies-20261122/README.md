# Seeed publication dependency review — 2026-11-22

Read-only review of unpublished checkpoint `972577650202ffa5e062ac02ee50b0ccdced5433` (tree `37e5327ae0362a09008c00c059bb1958414c115e`). [review.json](review.json) records source SHA-256 hashes, Git tree/blob identities, inventory definitions and retained privacy hashes. No additional confirmed defect was found in this scope.

## Checked scope

| Check | Result |
| --- | --- |
| Manufacturer model manifests | 152 |
| Actual loader dependencies | 295 references / 282 unique tracked files; none missing or excluded |
| Additional conservative stager metadata | 125 top-level texture references; 420 total references / 284 unique tracked files |
| Existing downloadable STL exceptions | Two exact paths retained, 1,224,768 bytes total |
| Added or changed tracked files since the stated base | 877; none over 100 MiB; largest 49,950,481 bytes |
| Tracked Web symlinks | 0 |
| Tracked generated skill copies | 0 in each generated destination |
| Sanitized source metadata / photos | Three retained hashes match their privacy receipt; original HTTP hashes remain distinct |

Pages builds and fetches its generated dependencies, then stages outside the source and Rust cache. The upload uses that same fresh directory. The stager guards loader dependencies and rejects actual output above 1,000,000,000 bytes. This review read the workflow, stager and regression tests; it did not execute a new staging copy or rerun those tests.

The catalog SHA-256 is `addcdc576e36db375d5b884eee0aeb6d7b651b500e4ec28c4216435e3e290600`: 48 Seeed identities comprise 23 independent products or commercial variants and 25 explicit board/panel configurations. The actual-loader count excludes top-level `textures[]` metadata, which the stager additionally checks conservatively.

## Rechecking the inventory

Use the recorded source hashes and directory tree identities to establish the same inputs. The JSON defines the sorted, tab-separated manifests and their SHA-256 digests. Enumerate tracked model JSON files, resolve `binary`, `parts[].texture.file`, and label/photo-surface `derivedParts[].texture` relative to each model; add `textures[].file` only for the conservative stager total. Compare each dependency with `git ls-tree -r -l` and its working file size. The changed-file inventory uses `git diff --name-only --diff-filter=ACMR` from the recorded base. These are lightweight file checks, not model rendering or firmware execution.

## Packaging remains pending

The intended clean release commit has sole parent `573f5a6c1bcc6dbf0388383d178a3935ec8ddcce`, which contains no Seeed asset paths. The clean commit and bundle had **not been created or verified** when this receipt was written. Their entries are `not_run`, with the exact remaining ancestry/tree/ref checks listed in the JSON. Export only the sanitized release ref; the private checkpoint ID above documents the review and does not require its old ancestors to be retained.

This receipt does not validate later runtime changes, GitHub deployment, browser WebGL, physical hardware or radio operation. The [final integration record](../seeed-final-integration-20261122/README.md) owns those later results. No CAD, geometry, reference-image pixels or existing measurement was changed for this review.

## Actual local Pages artifact

The later [artifact-check.json](artifact-check.json) records a read-only check of the generated Pages directory: **2,209 files / 661,894,046 bytes**, below the 1,000,000,000-byte limit. Its production WASM is `a3f0c90c47f9403225084c7138a3ec6c6b7ab5d2d9a0d1e6d3db8938a7379a75` (6,056,179 bytes). This is a separate check of real staged files; the earlier source-review receipt remains unchanged.

- All 152 model manifests and 295 actual-loader references (282 unique files) match the current source SHA-256 values. All 48 Seeed model/geometry pairs are present, totaling 121,033,296 geometry bytes. The conservative top-level texture declarations also match.
- Both complete skill copies match their canonical directories (4 and 5 files). All 47 workflow-pinned resources match their expected hashes, including eight ROMs, 30 P4/C6 images, Linux, MicroPython, xterm and Pocket assets; all 58 vendor files match the source.
- All 181 excluded source archives and seven diagnostic fixtures are absent. The two existing STL downloads remain present and byte-identical (1,224,768 bytes). No unexpected source file is missing, and no extra files exist beyond the nine canonical skill copies.
- Five cache URLs have exactly the expected relative targets and the local `ba18bcea` token. The two stamped files equal the workflow substitutions applied to the unchanged sources. No private asset-map filenames or the documented map shapes were found among 805 parsed JSON files.

The check hashed 556 staged files totaling 485,556,243 bytes and recorded reproducible inventory digests. It did not alter the source or artifact, rerender models, execute firmware, launch a browser or upload/deploy anything. A later clean-commit token replacement was not performed and would change the two stamped file hashes; this receipt certifies the observed local artifact only.

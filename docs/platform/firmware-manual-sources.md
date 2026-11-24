# Static manufacturer distribution sources

A public product page, static ZIP, or Drive download can establish a pinned firmware source without establishing a release API. Use `metadataProvider: "manual"` in its watch entry. Keep the real HTTPS document in `metadataUrl` and `releaseUrl`; `upstreamRepo` may be `null` or an explicit identity such as `manufacturer:Waveshare/ESP32-S3-LCD-3.16`. Do not infer a GitHub repository from a product name.

`monitoringNote` states what was reviewed and why a future revision needs manual review. `checkedAt` remains the actual source review time. Manual entries cannot carry automatic `catalogTargets`, an automatic asset-name selector, or supplementary API metadata. A source can still provide a separately pinned catalog `trial`; manual update monitoring does not remove that firmware or alter its compatibility evidence.

Metadata synchronization makes no network request for a manual entry, records `manual-review-required`, and does not renew its review timestamp. An earlier API snapshot cannot be promoted into a current manual release. Catalog synchronization retains the existing firmware and reports the same review requirement. Public checks explain the manual source and link to its document; they do not claim a new version, probe an invented API, or create a broken-link issue solely because no API exists.

Validation: `node --test tools/platform-manual-watchlist-test.mjs tools/platform-firmware-sync-test.mjs` covers zero network requests, stable source dates, rejection of automatic targets, retention of the pinned trial, and public manual-review reporting. `node tools/platform-watchlist-test.mjs` retains existing GitHub and manufacturer API behavior.

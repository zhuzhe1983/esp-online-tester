# AtomU official UIFlow metadata correction

K117 AtomU now has two exact official development-firmware reference entries and two metadata watches. Both remain `status: reference`, `source: null`, `storage: remote-links-only`; watch compatibility is `reference-only`, with empty assets and catalog targets. The product remains runtime unavailable and no factory default or factory identity is changed.

| Channel | Official project | Latest public/published version | Fixed version ID |
| --- | --- | --- | --- |
| UIFlow2.0 AtomU | 2099000000000000001 | v0.1 | 2117190240810143746 |
| UIFlow_AtomU | 1897200000000001759 | v1.15.5 | 2117073971620450306 |

Both project names and their official descriptions explicitly identify AtomU. `sourceType` is `OFFICIAL`; the selected versions are `PUBLIC`, `PUBLISHED`, current, and listed for device1897100000000000003. That device is the shared `ATOM_SERIES` category: its identifier alone cannot establish a target match, and this correction is assigned only to K117. UIFlow1's latest filename contains `lite`; no Lite product inherits the AtomU project mapping from that filename.

UIFlow2's actual published `versionName` is v0.1. Its filename contains a separate v2.5.4 build string. Both are preserved without replacing the release version with the filename's build version. UIFlow2 has one public release; the UIFlow1 endpoint returned63 public/published versions. Selection uses official publication time among releases listing the same device, rather than arbitrary filename ordering.

Only four JSON GET requests were made: the two exact project endpoints and their `versions?scope=PUBLIC` lists. No download endpoint was requested and no BIN was downloaded, mirrored, inspected or executed. Binary hash, boot layout, target capacity, peripheral behavior, simulation compatibility and hardware results remain unverified. These are development firmware references, not evidence identifying the product's factory image.

`docs/platform/evidence/m5stack-k117-uiflow-20261126/` retains original URL/final URL, UTC acquisition, raw byte count and SHA-256, plus selected public project/release/device fields. Each sanitized file has a separate hash linked to its raw digest. Developer/account/contact data, statistics and viewer permissions/session fields are omitted. The UIFlow2 project supplies its upstream source URL; UIFlow1 supplies none. The existing UIFlow family repository label in the watch is not proof of a version-specific source-code build.

Catalog schema, actual knowledge validation, watchlist validation, catalog loading/selection and firmware policy/target checks were run. No model, runtime or product-definition change is included. The investigation corrects the older tutorial-only UIFlow discovery result; its factory and execution uncertainties remain valid.

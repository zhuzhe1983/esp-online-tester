# Tough K034: virtual shortcuts, real touchscreen

The original audit reported three missing physical button mappings. These are
platform `btn-a/b/c` aliases that synthesize CHSC6540 contacts at (54,228),
(160,228), and (266,228). They are now explicitly `physical:false`,
`spatialInputMode:touchscreen-shortcut`, with simulation/platform provenance.
API and keyboard input remain available; spatial input uses the existing real
LCD touchscreen at those same native coordinates. No fixed key plane, cap or
other hardware geometry is added. Model/plan bytes and every artifact are
unchanged from `3bc45f158dc857a8aa968c2e7b795c1448831c62`.

Source correction: the rectangles in pinned M5Tough `M5Button.h` lines 268–273
are label examples for older GPIO-button M5Stack devices. They do not establish
Tough touch-key positions. Its earlier prose mentions front touch buttons, but
the same revision's `M5Tough.h` declares no BtnA/B/C members. The incorrect
rectangle attribution is removed from knowledge. A separate one-line source
comment correction (`9e0a3391`) changes no non-comment line or line count.

Independently read M5Unified revision
`96576a14247ddc935149145aa6b51943464120f1` implements configurable software
buttons for Tough. Its routing uses `raw.y >= 240 - height` and
`(raw.x * 614) >> 16`. The header initializes height to zero. For the platform's
y=228 contacts, firmware must configure a height of at least 12 pixels; no
default fixed front-key region is asserted. Knowledge retains pinned source
hashes and the remaining physical-position uncertainty.

The receipt loads the unchanged real model using production code. All three
pixel-center rays hit `live-display`, have no button control ID, recover the
expected native pixel coordinates using scene-view's UV conversion, and route
touch down/up through the production pointer router. The new audit category is
`virtual-only-with-real-touch-surface`; these do not count as physical button
clicks. The original misclassification receipt remains available.

Reproduce, optionally checking separately obtained pinned upstream source files:

```sh
node tools/verify-m5-tough-touch-shortcuts.mjs --source-root "$SOURCE" --overlay-root "$OVERLAY" --baseline 3bc45f158dc857a8aa968c2e7b795c1448831c62 --upstream-inl /tmp/M5Unified-96576a14.inl --upstream-hpp /tmp/M5Unified-96576a14.hpp --out /tmp/tough-touch-shortcuts.json
```

Validation: production knowledge reference checks pass for the 736 existing
records and this updated K034 record; catalog JSON Schema accepts the three
virtual controls. The full K034 knowledge schema has the same two pre-existing
errors as baseline: `httpStatus` on existing firmware sources 60/61 is not a
declared schema property. No new knowledge schema error is introduced.
The comparison is bound to the baseline schema hash; a separate integration
follow-up declares the existing HTTP-status field without deleting evidence.

This is host-side surface/input evidence, not guest firmware execution, a new
CPU render, browser GPU validation or a physical hardware measurement. The
fixed geometry of manufacturer front keys remains unverified. Upstream source
files can be recovered from their pinned GitHub URLs in the knowledge record.

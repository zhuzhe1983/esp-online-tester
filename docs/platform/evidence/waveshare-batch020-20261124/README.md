# Batch020 — five C6/C3/H2 listing families

This incremental release selects C6 Pico, C6 Dev Kit N8/N16, C3 Zero,
H2 Zero and H2 Dev Kit N4:12 exact sale SKUs/product profiles and10 mechanical
assets. The two Dev Kit memory options are separate electrical profiles within
one storefront family. Source, variant and native mechanical evidence is in
`docs/evidence/waveshare-c6-final-basic-2026-11-23/`; H2's original source and
image audits are in `docs/evidence/waveshare-h2-sram-2026-11-23/`.

## Independent H2 execution

H2 is now a separate RV32IMAC SoC/backend, not an alias of C6. The CLI
`--chip h2` and platform/production Worker use its own4MiB/0PSRAM machine,
320KiB HP SRAM and4KiB LP SRAM ranges, physical GPIO restrictions, UART0 and
bounded RMT implementation. Explicit SRAM app-direct loading is the supported
entry. There is no substituted C6 ROM, JIT backend, mapped Flash execution,
radio, SD, audio, USB, persistent device storage or fabricated interrupt route.

`tools/fixtures/waveshare-h2-core-probe.mjs` is a **project-authored diagnostic**,
not manufacturer firmware. Its real RV32 instructions select H2's32MHz RMT
clock and GPIO8 output, emit a known RGB value, print UART0 text and repeatedly
read BOOT GPIO9. The public runtime tests execute this on all four exact H2
sale options, press/release BOOT, reset and reload the same image, and confirm
that no ROM was fetched. CLI tests independently execute the retained native
diagnostic on both board identities and reject ROM boot and incorrect memory.

The platform accepts only chip16,4MiB physical Flash and SRAM-contained
segments/entry. H2 wafer revision is unverified, so revision-restricted images
are rejected; unrestricted maximum0/65535 is permitted. Header capacity,
segment ranges, overlap, checksum and appended SHA remain enforced. This
does not make the18 audited original H2 example images executable: their
full Flash/ESP-IDF boot paths are unsupported. The original catalogue entry
remains reference-only with no runnable source, and no authored probe is
presented as a factory replacement.

## C6/C3 and source boundaries

C6 Pico retains its actual TCA9554 on SDA22/SCL23/address0x20; expander outputs
are not fictional MCU pins or IRQs. C6 Dev Kit N8/N16 preserve external Flash
identities and capacities. C3 Zero uses its own C3FH4/4MiB core and GPIO10 RGB;
the S3-target image found in the related Basic Kit repository was rejected for
C3 after chip-ID validation. No matching complete image was found for the
remaining basic boards in the retained bounded source search; this is not a
claim that none exists elsewhere.

## Integrated verification

-44 meaningful release native checks passed:8 H2,2 CLI,28 C6 and6 C3.
-The release WASM build passed.8 public-runtime tests passed, covering H2's
  real Worker execution, existing S2/C5 SRAM regressions and all registered
  Waveshare chip/memory/revision boundaries.
-Catalogue schema/identity validation passed. Knowledge validation covered
  all574 indexed records; the combined knowledge/manual-watchlist run passed
  all6 Node test entries.
-All10 models passed the actual production Three.js loader, dimensions,
  constructed UNKNOWN labels and relevant CPU geometry checks. Root inspected
  all10 four-view plots; exact model/image SHA values are in
  `visual-review.json`. Native N8 CAD is unscaled; N16 revision matching is
  explicitly unverified. Photo-only boards retain UNKNOWN on unmeasured faces.

The first CLI attempt's concurrent fat-LTO processes were killed; a serial
release build and both CLI tests then passed. The retained final logs and
WASM hash are indexed in `checks.json`. These checks are not browser DOM/WebGL,
full manufacturer application, real hardware or mechanical fit acceptance.

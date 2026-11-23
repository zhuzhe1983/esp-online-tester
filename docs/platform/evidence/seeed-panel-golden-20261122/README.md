# Intentional panel SID golden revision for RGB timing

Only the four `panel_sid` golden files are revised. The adopted shared RGB fix
retains fractional byte-cycle credit, consumes complete RGB565 pixels and stops
at the current frame boundary. This intentionally changes the old rounded batch
timing. The implementation and its negative/positive boundary tests are described
in [the RGB boundary review](../seeed-lcd-vsync-20261122/boundary-review.md).

Before changing expectations, the exact seven-second `panel_sid` scenario was
run once with the original main executable and once with the frozen candidate.
The original executable matched **all four original goldens**. The candidate
changed all four outputs, while the complete final frame remained identical and
all overlapping audio samples matched after an exact one-sample displacement.
These observations support adopting the deliberate digital timing change rather
than retaining the defective FIFO accounting to preserve old expectations.

This is a correctness change, **not a CPU optimization or physical timing
calibration**. No physical IRQ timestamps were measured. The comparison does not
establish a separate causal trace for every changed interrupt count. Golden
checks remain exact; no comparison tolerance or test bypass was introduced.

[receipts.json](receipts.json) contains executable/input identities, exact
argument arrays, old/new file hashes, raw/normalized log hashes, all count
changes, and the sample/pixel comparison results. The earlier failing full gate
remains in
[the integration evidence](../seeed-final-integration-20261122/pre-golden-review-gates.json);
it is not overwritten or retrospectively described as passing.

## Frozen executables and inputs

| Executable | Bytes | SHA-256 |
| --- | ---: | --- |
| Original main `573f5a6c1bcc6dbf0388383d178a3935ec8ddcce` | 34,395,672 | `22954f6f7ef68a1d12e76a07ed3494c25e9cc2c231781ea912d479af8f3a87be` |
| Candidate | 35,168,624 | `cf80f0cd643e3cfaea707c44c629f6cdd133bbf9a3b61b35ab0b69e317230111` |

The candidate Rust source checkpoint is
`819989b129de90f0cc19bf6d83b3e88a7d53d4b9`, an unpublished integration checkpoint.
Executable and input hashes, rather than an unpublished commit name alone,
identify the measurement. Both binaries and every input/golden file retained
their frozen hashes across both runs. Both processes exited zero at
7.000 guest seconds / 1,680,000,000 guest cycles.

| Input | Bytes | SHA-256 |
| --- | ---: | --- |
| `esp32s3_rev0_rom.elf` | 949,552 | `c0ce0f338d1de1bdc6efbef1591779a2a42c1ab7d759d3c6ae8ae63a7dd34cfd` |
| `panel-bootloader.bin` | 22,272 | `f804e8ef245d68ee0932650062e8f4d6b14df22d61fb39ba50f5e33977127801` |
| `panel-ptable.bin` | 3,072 | `fb462bc675bf6928afe8de87977f72c7dfce38491d32aa89a9204b6d4d5b1066` |
| `panel-demo.bin` | 1,651,104 | `8094e4be95f8d92c7ce41fe1acbbecbcdb3b31725fc292a1eef63cfcc22cf5a0` |
| `energydata.json` | 2,846 | `aa8d5326406a83aa254d953d16f18f644c0112d523ef2e2e66b3ab1add4ff0cb` |
| `panel-sid.txt` | 477 | `bb6e0b1f1d5945265f0310f2a40b59b03602b257b412f52f649fc2c66823f420` |

The five non-ROM inputs are under `web/wasm/fw/public/`. From the repository
root, each arm used the following argument sequence, with a fresh output
directory and the corresponding executable:

```sh
mkdir -p "$PANEL_REVIEW_OUT"
"$PANEL_REVIEW_BIN" \
  --rom "$ESP32SIM_ROM_DIR/esp32s3_rev0_rom.elf" \
  --board waveshare-lcd4b --boot rom --no-dump \
  --flash-mb 16 --psram-mb 8 --console usb \
  --bootloader web/wasm/fw/public/panel-bootloader.bin \
  --ptable web/wasm/fw/public/panel-ptable.bin \
  --app web/wasm/fw/public/panel-demo.bin \
  --flash-at 0x610000=web/wasm/fw/public/energydata.json \
  --script web/wasm/fw/public/panel-sid.txt \
  --wav "$PANEL_REVIEW_OUT/panel.wav" --max-seconds 7 \
  --tft-png "$PANEL_REVIEW_OUT/panel.png" \
  > "$PANEL_REVIEW_OUT/stdout.txt" 2> "$PANEL_REVIEW_OUT/stderr.txt"
```

The arguments before `--tft-png` are exactly those in
`cli/tests/goldens.rs::panel_sid`, apart from the output/ROM paths. The optional
PNG is written after stop, IRQ and WAV reporting. Its implementation refreshes
and reads the board display without executing another guest instruction.

## All observed differences

Both console files are 5,126 bytes with 97 CRLF endings and no bare LF. Only byte
offsets 4911 and 5064 (zero based) differ. The demo-mode log timestamp on line 94
changes **1147 → 1146 ms**; the SID playback log on line 97 changes
**5778 → 5777 ms**. Every other console byte is identical. The candidate console
golden is copied with `write_bytes`, preserving those original CRLF bytes.

| Counter | Original | Candidate | Difference |
| --- | ---: | ---: | ---: |
| Exceptions | 569,599 | 569,899 | +300 |
| Interrupts | 47,868 | 47,835 | −33 |
| Core 0 instructions | 264,427,873 | 264,578,203 | +150,330 |
| Core 1 instructions | 134,240,194 | 136,133,761 | +1,893,567 |
| Total instructions | 398,668,067 | 400,711,964 | +2,043,897 |

| IRQ line and source | Original | Candidate | Difference |
| --- | ---: | ---: | ---: |
| core0 int2, source 59 | 1,133 | 1,114 | −19 |
| core0 int3, source 79 | 7,112 | 7,114 | +2 |
| core0 int5, source 57 | 6,448 | 6,407 | −41 |
| core0 int9, source 42 | 6,202 | 6,196 | −6 |
| core0 int12, source 24 | 353 | 350 | −3 |
| core0 int13, source 71 | 17,285 | 17,309 | +24 |
| core0 int17, source 72 | 519 | 513 | −6 |
| core1 int0, source 80 | 1,875 | 1,891 | +16 |
| core1 int1, source 58 | 6,941 | 6,941 | 0 |

The report is derived using the same stop-line event suffix and `  core` lines
as the test. Both runs still report 369 RGB frames and 135,079 I2S output
samples. These are functional observations at equal guest duration; the changed
instruction counts preclude calling this an equal-work CPU comparison.

### Exact audio displacement

Both WAV files contain a 44-byte header and 135,079 signed 16-bit mono samples at
22,050 Hz: 270,202 bytes total. The headers are byte-identical. Their full-file
SHA-256 values are:

| WAV | SHA-256 |
| --- | --- |
| Original | `37b4961b189338e0977f283e2485080f5f8078380fac956f969283cc20ca2171` |
| Candidate | `e4f1ebab78263ffa94ac5182c0778989fd78761929b3b6d4dc02e6bd4fc70a1d` |

For the decoded PCM arrays `B` (original) and `C` (candidate), direct byte
comparison establishes:

```text
C[0:135078] == B[1:135079]
```

All **135,078 overlapping samples / 270,156 bytes** match exactly. The common
slice has SHA-256
`816ccae6fb79a647467fe01a57dbc89a686671001b09ca7861ba888e425ba857`.
There is no resampling, amplitude tolerance or fitted correlation involved.
The first nonzero sample moves from index 106,229 to 106,228. All 28,850
overlapping samples after that onset also match exactly. Without alignment,
15,595 sample positions differ.

The observed displacement is **one sample earlier**, or
`1 / 22050 = 0.0000453514739229` seconds. The omitted leading original sample is
zero; the new trailing candidate sample is **3370**. That final sample lies
outside the original capture window and is not claimed to have a validated
future counterpart. The 1 ms console timestamp change and the roughly 45.35 µs
PCM displacement are separately measured quantities, not equated.

### Identical final display

Both 480 × 480 captures decode to the same 921,600 RGBA bytes, with **zero changed
pixels**. RGBA SHA-256:
`5776d3aadc3153a8cdb06370c7544989f91f355913a92397933381cee98f51a4`.
The PNG files themselves are also identical, SHA-256
`436e648ffc5c11fffc456c0e986a43d47f7c8ed11fbaa4620818815c142735da`.
Visual review confirms the SID player page with the selected tune and playback
controls. No frame rotation, cropping or correction was applied.

## Narrow update and retained evidence

The new expectations are derived solely from the captured candidate: exact
stdout bytes, the test's report extraction, the summed core instruction counts,
and SHA-256 of the complete WAV. No emulator is rerun to generate this update,
and no global `UPDATE_GOLDENS` setting is used.

| Golden file | Original file SHA-256 | New file SHA-256 |
| --- | --- | --- |
| `panel-sid.console.txt` | `942ce18d2c00e8738f36be7ce3a1d318b7ab363dd8fa0090dfc8e147fd0f92d2` | `86f232a489d3b5baf27199339f212b5bf19898b44f0cbbce62a7162c75092b2d` |
| `panel-sid.report.txt` | `f91fbe2c2ef92f876b532b3356621854892c3fa0df573e4f794fc993fe23dd56` | `4c7cd552cfcd1036e2f10cd387ca1a9276248c5751278358e9a8c93675254cb5` |
| `panel-sid.insns` | `61f00f17e882c4e55f21c932cfce58fae830d5c12d84a65cc3e257ab9deb3274` | `81e7c27318b929d9c4523f7e82b287c6b487093371e70c568a67a4d8bc27e117` |
| `panel-sid.wav.sha256` | `d76a1dbdcc7aef23b0c7283d8439262a4948799fa52316c731dc49e431197ebb` | `fd2f084492ced5ca49d077d9b9bcc1bd147f1c58a5b47b8a9e9ee4ee41aeee60` |

The last row hashes the small text golden containing the WAV digest; it is
distinct from the WAV file digest above. The sanitized
[original stderr](baseline.stderr.txt) and [candidate stderr](candidate.stderr.txt)
retain all guest events/counts. Local ROM/output paths use placeholders and
incidental wall-time/throughput fields are omitted. Their original and normalized
hashes are both recorded. Raw WAV/PNG files and original local paths are not
published here.

The captured-byte/receipt audit and evidence privacy check pass. A default
`git diff --check` flags the two retained CRLF endings as trailing whitespace;
that result is recorded. The check passes with the per-command setting
`core.whitespace=blank-at-eol,blank-at-eof,space-before-tab,cr-at-eol`, which keeps
the standard whitespace checks and recognizes the required line endings. No
repository configuration or console encoding was changed to silence the check.

The subsequent exact `panel_sid` test passes with `UPDATE_GOLDENS` unset and
all tracked golden-file hashes unchanged before and after execution; see the
[targeted execution receipt](../seeed-final-integration-20261122/targeted-panel.json).
The complete workspace gate is recorded separately in the
[final integration report](../seeed-final-integration-20261122/README.md).
Neither result is inferred from curation checks or the earlier failed gate.

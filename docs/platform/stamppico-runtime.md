# Stamp-Pico runtime

C050-B uses independent classic ESP32 board `stamppico`, 4 MiB Flash and no PSRAM. The G39 user switch is active-low control `button` (keyboard A), with the schematic's external R3 5.1k pull-up. G0 has its own R5 5.1k download-strap pull-up; EN is separate. LED1 is an SK6812_3535 on G27 with 3.3V supply. Cold board setup selects PICO-D4 package 5 using the existing classic efuse path; this uses the existing ECO3 ROM execution model, not a hardware revision calibration.

Current official UIFlow2 commit `e437118892520ba987c2d930a2428db1cab65ab1` maps M5StampPico to `SK6812(io=27,n=1)`. Its driver `ORDER=(1,0,2,3)` emits GRB. The older `STAMP-PICO@90e28e6` Arduino LED example explicitly chooses RGB. The board decodes raw GRB frames, preserving the older example's R/G discrepancy; it does not guess colors from a firmware filename. Sources and exact hashes are retained in [the C050-B source receipt](evidence/stamp-controllers/c050-b-sources.json).

A fresh actual worker run of the independent IDF 5.5.1 fixture boots through the classic ROM, ignores a white G4 frame, produces red on G27 and reads actual G39 changes. Press/release advances green, blue, off and red. Reset while held clears old RGB, releases G39, ignores G4 again and generates fresh red. Unknown controls are rejected and no framebuffer is invented. [Fixture receipt](evidence/stamp-controllers/c050-b-runtime.json). Rebuild and run:

```sh
idf.py -C tools/fixtures/stamppico-firmware -B /tmp/stamppico-fixture reconfigure
ninja -C /tmp/stamppico-fixture -j2
node tools/probe-stamppico.mjs --build /tmp/stamppico-fixture --product m5stack-c050-b
```

No full factory image was found. The fresh UI defaults to the exact official Burner UIFlow2 v2.5.4 source; UIFlow1 v1.15.5 remains an alternative. Both ordinary download endpoints return HTTP 400 `BURN_CAPTCHA_INVALID`; no binary or hash was obtained from them. A separately listed GitHub v2.5.4 build has publisher digest `a9a76061b62fe99ad0a95f10ca021c9ae8f9ceec8265d2e023b6f97d967e424a`, verified against 4,186,112 downloaded bytes and classic merged-image layout. It is the `e437118` build, not Burner's `509a67aa` build. [Release receipt](evidence/stamp-controllers/pico-github-release.json).

The unmodified GitHub image reaches the MicroPython V2.5.4 / M5STACK Stamp PICO REPL. Its original startup fails with `ImportError: no module named 'M5Things'` in `boot.py`/`startup/headless.py`, and there is no RGB output after 21.04 guest seconds. The same run also logs XTAL estimation and uninitialized I2C warnings. This is partial execution, not full application compatibility; no module stub or guest patch was added. [Original-image receipt](evidence/stamp-controllers/c050-b-uiflow254.json). The probe exits 1 when the expected blue output is absent.

The exact GitHub asset is blocked by browser CORS (missing `Access-Control-Allow-Origin`). Official download plus local file import remains available; a local successful download does not establish browser access. [Transport receipt](evidence/stamp-controllers/pico-browser-sources.json). Firmware stays trial and is never mirrored into Git or the public site.

EN/reset hardware, download cables, USB transport, analog/power, software-GPIO LED waveforms, physical networking and full firmware applications are outside this subset. No physical hardware measurement was performed. LED numeric output may be valid while a paper-covered LED remains visually occluded; no unsupported hole or paper transparency is invented. Physical front clicks are checked separately after the final model is integrated.

## Stamp-Pico Mate K051

The K051 page includes one Stamp-Pico and loose connectors/headers, repeats G39/G27, and links the same schematic and UIFlow1/2 tutorials. It shares the electrical board and exact remote firmware sources with C050-B while retaining its own model/package identity. Its [fresh fixture run](evidence/stamp-controllers/k051-runtime.json) independently verifies the K051 selection and guest GPIO/RMT/reset output. The official image startup failure above is shared-board evidence, not an additional claimed K051 official-image run. Loose kit accessories remain unattached electrically.

## Stamp-Pico DIY Kit K051-B

The DIY Kit official page separately includes the same Stamp-Pico, headers, Grove socket and an ESP32 Downloader. Its G39/G27 pin map, schematic and UIFlow tutorial targets match the module. It uses `stamppico` and the exact shared remote sources but keeps its own SKU. [Fresh K051-B worker evidence](evidence/stamp-controllers/k051-b-runtime.json) verifies guest GPIO/RMT/reset output; neither included tools nor visual insertion imply modeled electrical connections or flashing. The C050-B official-image startup failure is referenced as shared-board evidence, not a separate official-image run for this kit.

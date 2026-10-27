# Stamp-S3A internal GPIO/RMT/supply fixture

This ESP-IDF 5.5.1 test is not manufacturer firmware. Generated binaries remain outside the public catalog and website.

```sh
idf.py -C tools/fixtures/stamps3a-firmware -B "$PWD/output/stamps3a-fixture-build" build
node tools/probe-stamps3a.mjs --build output/stamps3a-fixture-build
```

The official Stamp-S3A v0.3.3 schematic and Arduino quickstart identify G0 (active-low button), G21 (WS2812B GRB) and G38 (AW35122FDR LED power enable). Startup sends white while supply is off, then enables supply and waits before a fresh red frame. Subsequent presses produce green, supply-off plus ignored blue, re-enable with no stale frame before fresh blue, G38 input (off), and output-high red. The probe checks actual guest serial/LED output, release and reset. Board tests also cover input-mode latch-high and unchanged directly powered base Stamp-S3.

UIFlow2 v2.5.3 reports IDF v5.5.4-dirty. This independent source-built fixture does not prove the official application's complete input/network behavior.

# Stamp-S3 internal GPIO/RMT fixture

This is a source-built ESP-IDF 5.5.1 test, not manufacturer firmware. It is not in the public catalog and its generated binaries must not be copied to the website.

With ESP-IDF 5.5.1 activated:

```sh
idf.py -C tools/fixtures/stamps3-firmware -B "$PWD/output/stamps3-fixture-build" build
node tools/probe-stamps3.mjs --build output/stamps3-fixture-build
```

The official Stamp-S3 v0.2 schematic and STAMP-S3 `6b64d431` example identify G0 (active low) and G21 (one WS2812B, GRB). This fixture uses IDF's GPIO/RMT APIs, emits actual GPIO state and starts red. Each fresh press cycles green, blue, off, red. The platform probe checks guest output, release and reset, not just queued input acknowledgements. The official UIFlow2 binary uses IDF `v5.5.4-dirty`; this independently built driver fixture does not establish whole-application compatibility with that firmware.

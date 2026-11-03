# Internal Stamp-Pico protocol fixture

Build with ESP-IDF 5.5.1 for classic ESP32, 4MB Flash / no PSRAM. This independently authored diagnostic is not manufacturer firmware and is not in the public catalog.

```sh
idf.py -C tools/fixtures/stamppico-firmware -B /tmp/stamppico-fixture reconfigure
ninja -C /tmp/stamppico-fixture -j2
node tools/probe-stamppico.mjs --build /tmp/stamppico-fixture --product m5stack-c050-b
```

G39 input uses the documented external R3 5.1k pull-up, not a nonexistent internal pull-up on classic GPIO39. RMT first sends white on wrong G4, then GRB red on actual G27. Every G39 press advances green, blue, off, red. The host verifies actual worker output and releases, no framebuffer, unknown-control rejection and fresh output after reset.

The official 2021 Arduino example selects RGB, while exact UIFlow2 e437118 StampPico mapping and SK6812 driver explicitly emit GRB. This fixture follows the latter and the standard SK6812 framing; older raw RGB frames are not silently reordered. No physical color calibration, download cable, EN/power, USB transport or network behavior is claimed. Generated binaries are local only.

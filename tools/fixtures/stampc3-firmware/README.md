# Stamp-C3 internal GPIO/RMT fixture

Build with ESP-IDF5.5.1 and the C3 target. This independent test exercises the actual public GPIO/RMT drivers, not firmware hooks. It is not factory firmware or an official application, and generated images stay outside Git and the public catalog.

```sh
idf.py -C tools/fixtures/stampc3-firmware -B <temporary-build-directory> reconfigure
ninja -C <temporary-build-directory> -j2
node tools/probe-stampc3.mjs --build <temporary-build-directory> --product m5stack-c056-b
```

The C056-B schematic connects user switch S2 to G3 and U4 SK6812 to G2; EN/S1 is separate. The official STAMP-C3 repository at `148c263fddcb7b61fef20b0cf6de8ab8c3475f73` uses `TYPE_GRB` in its Arduino example and G3 input pull-up in its IDF example (tested there with IDF4.4.1). This fixture uses IDF5.5.1: it first sends a white frame to wrong pin G4, then red to G2; each G3 press cycles green, blue, off and red. The probe checks actual guest GPIO prints and RMT colors, release and platform reset. CH9102 USB bridge, EN reset, analog power, wireless and software bit-banged LEDs are outside this test.

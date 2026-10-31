# Classic camera JPEG fixture

A third build of the same pinned ESP-IDF / public esp32-camera project, using
`main/jpeg.c`. The C8 and lifecycle images and checks are unchanged. The guest
uses the AI-Thinker wiring with `PIXFORMAT_JPEG`, 20 MHz XCLK and
`jpeg_quality = 12`, as common ESP32-CAM sketches do. `jpeg.bin` keeps one
framebuffer in internal DRAM; `jpeg-psram.bin` (`C10_PSRAM=ON`) adds the 4 MiB
external RAM profile (`sdkconfig.psram`) and keeps two framebuffers in PSRAM.
No driver changes, symbols or host stubs.

The host inputs are four baseline JPEG files written by `make-jpeg.py`
(standard library only, independent of the emulator): flat 8 × 8 blocks, unit
quantisation and the ITU-T T.81 Annex K luminance Huffman tables. They are
host data, not an emulator encoder; the emulator delivers them byte for byte.

| file | size | bytes | SHA-256 |
| --- | --- | ---: | --- |
| `vga-a.jpg` | 640 × 480 | 6218 | `f602a49340beba9cbf3c7b8ee37d34db066baf7b7a9f545e52742eeaca576998` |
| `vga-b.jpg` | 640 × 480 | 11400 | `680228c782f1beff1ae24e8349ce9b8e8c14386103a941bf81a417fce720cde4` |
| `qvga-a.jpg` | 320 × 240 | 2489 | `7a7f480aa5bf001fda3760c1c7dda6327295c02ec32939595f5814596f8932d0` |
| `qvga-b.jpg` | 320 × 240 | 2487 | `a711e4317a9af4fd9efcbdf70c412a2d123dc16c6efa5d38b97b65423edec968` |

Use the pinned checkout/install procedure in README.md and lifecycle.md, then,
from the repository root:

```sh
python3 examples/classic-camera/make-jpeg.py "$JPEG_OUT"
cmp "$JPEG_OUT/vga-a.jpg" esp32/tests/fixtures/classic-camera/jpeg/vga-a.jpg  # and the other three
C10_JPEG=ON bash examples/classic-camera/build.sh "$BUILD_A"
C10_JPEG=ON C10_PSRAM=ON bash examples/classic-camera/build.sh "$BUILD_PA"
# Second clean source/output directories, no generated sdkconfig copied:
nice -n 10 idf.py -C "$SOURCE_B" -B "$BUILD_B" -D SDKCONFIG="$BUILD_B/sdkconfig" \
  -D SDKCONFIG_DEFAULTS="$SOURCE_B/sdkconfig.defaults" -D C10_JPEG=ON build
nice -n 10 idf.py -C "$SOURCE_B" -B "$BUILD_PB" -D SDKCONFIG="$BUILD_PB/sdkconfig" \
  -D SDKCONFIG_DEFAULTS="$SOURCE_B/sdkconfig.defaults;$SOURCE_B/sdkconfig.psram" \
  -D C10_JPEG=ON -D C10_PSRAM=ON build
python3 examples/classic-camera/check-builds.py "$BUILD_A" "$BUILD_B" \
  esp32/tests/fixtures/classic-camera jpeg.bin
python3 examples/classic-camera/check-builds.py "$BUILD_PA" "$BUILD_PB" \
  esp32/tests/fixtures/classic-camera jpeg-psram.bin
ESP32SIM_ROM_DIR="$PWD/web/wasm/fw" nice -n 10 cargo +1.99.0 test \
  --release -p esp32 --test camera_jpeg -- --ignored --nocapture
```

Both variants share the committed bootloader and partition table.

## What the test pins

The guest prints, per frame, the phase, width, height, `fb->len`, format, whether
`fb->buf` is in external RAM and the SHA-256 of `fb->len` bytes. The host:

1. supplies `vga-a.jpg` before boot (no sensor size is configured yet);
2. on `C10 REPLACE vga-b` supplies `vga-b.jpg` without stopping the camera;
3. on `C10 QVGA qvga-a` (after the guest's `set_framesize(QVGA)`) checks that
   `vga-a.jpg` is now refused as `SizeMismatch { 640x480, 320x240 }` and that a
   truncated `qvga-a.jpg` is `Malformed`, then supplies `qvga-a.jpg`;
4. on `C10 REPLACE qvga-b` supplies `qvga-b.jpg`.

The guest then deinitializes, captures two raw 160 × 120 RGB565 frames (the C8
oracle hash: the stored JPEG must not leak into raw output), deinitializes again
and captures two QVGA JPEG frames after reinitialization. Every phase must show
exactly two frames with the expected length and hash. After each host change
the guest returns one possibly queued frame first; it must be either the old or
the new JPEG, never anything else, and it is not a freshness oracle.

No JPEG encoding, decoding, quality setting, timing, adjustable FPS or hardware
accuracy claim is made. Raw capture with two framebuffers is outside this fixture.

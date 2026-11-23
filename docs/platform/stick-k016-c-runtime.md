# M5Stack StickC K016-C runtime

The K016-C entry covers the ESP32-PICO-D4 and MPU6886 hardware configuration, with 4 MiB Flash and no PSRAM. The original ESP32 ROM boots M5Stack's official V0.0.1 factory image. The firmware remains a trial. The hardware variant and manufacturer sources are recorded in the [K016-C knowledge record](../../web/platform/knowledge/m5stack-k016-c.json).

## Implemented board interfaces

| Device | Model |
| --- | --- |
| ST7735SV | 80×160 LCD from controller RAM, SCK13/MOSI15/CS5/DC23/RST18. AXP192 LDO3 powers the LCD and LDO2 gates the backlight. |
| Controls | Active-low A/GPIO37, B/GPIO39 and red LED/GPIO10. The power hotspot sends the AXP192 PEK short-press event. |
| Internal I²C | SDA21/SCL22, AXP192 at 0x34, MPU6886 at 0x68 and BM8563 at 0x51. |
| Simulated inputs | The AXP192 starts with a 25 °C and 4.1 V fixture. The BM8563 advances with guest time and starts with an unset date. The factory tilt page uses the IMU acceleration sample. |

Earlier SH200Q variants, raw PDM, physical microphone capture, IR output, RF/GATT, long-press shutdown and physical power behavior remain outside the accepted scope. The case is an estimate measured at 24×48×13 mm against the documented 25.5×48.2×13.7 mm. Button hotspots follow the manufacturer photographs, with the B position estimated within ±4 mm. The internal PCB remains an UNKNOWN placeholder.

## Inputs and reproduction

| Input | Bytes | SHA-256 |
| --- | ---: | --- |
| [Official factory V0.0.1](https://m5burner-cdn.m5stack.com/firmware/691fd90c716a33efe43b85d61c8e89a4.bin) | 1,804,704 | `8bd01902abdc0f0f6bfbf36a563de027d01cc9640e59c30756cc7315b06ccb03` |
| ESP32 rev300 ROM ELF | 857,500 | `920b70635440517866aab2230964a570d2cf2b676658d93c52fbac108c1cca31` |

Save the pinned factory bytes to a local `factory.bin` and check them before running. From the repository root, with Cargo's stable toolchain and an installed version of Playwright and headless Chromium:

```sh
shasum -a 256 factory.bin
tools/fetch-rom-elfs.sh web/wasm/fw
cargo run --release -p esp32 --example stickc_interaction_probe -- \
    factory.bin web/wasm/fw/esp32_rev300_rom.elf target/stickc-native
tools/wasm-build.sh
node tools/fixtures/stickc-factory-browser-check.mjs factory.bin target/stickc-browser
```

The browser check uses the official URL when accessible and records the transport. It falls back to a loopback copy of the matching official bytes if the URL fails. The check records ROM and WASM hashes, guest boot UART messages, visual framebuffer fingerprints and press/release events. `ESP_TEST_PLAYWRIGHT` and `ESP_TEST_CHROME` select an existing Playwright module and Chromium binary if needed.
Append `--extended` to also check two IMU poses, the return to neutral and four logical PCM stimuli in the factory app.

## Recovered native board

The first combined classic core and K016-C snapshot `44fcf723` completed 14.7 guest seconds from the pinned ROM and unchanged factory image. UART reported `SPI_FAST_FLASH_BOOT` and `M5StickC initializing...OK`. The observed 80×160 captures were the MPU6886 idle view, RTC opened with A, the RTC changing from `00:00:12` to `00:00:13`, and the factory microphone view opened with B.

| Capture | Guest seconds | Lit pixels | RGB565 FNV-1a |
| --- | ---: | ---: | --- |
| MPU6886 idle | 12.0 | 1,129 | `87a0b45da40b5155` |
| RTC after GPIO37 | 12.7 | 3,834 | `3cb042004227daad` |
| RTC after a second elapsed | 14.0 | 3,825 | `674c156ff0ef4507` |
| Microphone after GPIO39 | 14.7 | 1,260 | `8414897c067ac40d` |

After the PICO-D4 eFuse correction in `089ff608`, the same native probe passed. The idle frame had 1,131 lit pixels and RGB565 FNV-1a `ffec0934c1d4fce1`. The RTC and microphone fingerprints matched the table.

## Recovered browser build

The combined `cd49997f` build ran in headless Google Chrome 157.0.8103.80 with Playwright 1.60.0. The 6,926,500-byte WASM had SHA-256 `e01badb6b24df012ec97eee06592d5e96d430ef18d497cee8101054efdee8e15`. The production worker loaded the unchanged official image from M5Burner CDN, used the ROM and JIT, and reported `appDirect:false` with zero stubs. UART reported `SPI_FAST_FLASH_BOOT` and `M5StickC initializing...OK`. A was pressed on the 2D model and B on the keyboard; each action recorded both input edges. The RTC changed from `00:00:14` to `00:00:15`.

| 80×160 capture | Lit pixels | PNG SHA-256 |
| --- | ---: | --- |
| MPU6886 idle | 1,237 | `0a733033c67454506b85153a66040e063b2d1fb4cba231bb44d50f4f7397a83f` |
| RTC after A | 3,816 | `ace5cb7e37222bd0c0b4ed29cc3cef9f0b28e5f729051e067fddb279284d73ef` |
| RTC after the tick | 3,843 | `87ba4fbf8b9c25b638a155bb63a2cb5541881aa71d5822fe7f6c70c5ca4a898b` |
| Microphone after B | 1,260 | `7d4c1ef209957fe7c11434a0f31cfae88d4564004379a80fb42db57131932076` |

An extended browser run used the same WASM and the UI snapshot `c06bb9a8`. Two orientation inputs changed the drawn cube and the displayed angles to −30.00°/35.27° and 30.00°/16.78°. Returning to neutral restored 0.00°/0.00°. The microphone page displayed distinct spectra for the positive PCM inputs and a black graph with its header for both negative controls.

| Logical PCM stimulus | Lit LCD pixels | Consumed frames | Underrun frames |
| --- | ---: | ---: | ---: |
| 1 kHz, port 0 | 12,536 | 159,709 | 2,725 |
| 2 kHz, port 0 | 12,593 | 159,709 | 1,623 |
| Silence, port 0 | 1,260 | 159,709 | 2,726 |
| 1 kHz, port 1 | 1,260 | 0 | 0 |

The basic 1,485-byte receipt has SHA-256 `ff252da686015ac09588dbc0e723f34cd8bc555fcd765df7f623394dae632de6`. The extended 3,148-byte receipt has SHA-256 `dc5cabfd3830aebf87b3630fb6f5304976d61f3a82d9c986ae0ce0a1bf303f40`. Playback timing and frame numbers depend on the run.

## November 22 standalone K016-C evidence

The earlier K016-C-only build used a 5,928,264-byte WASM with SHA-256 `4069c28e9df5622e9eba4ffff0fa903be9ff525c49237e15b80631f471820954`. The 125,156-byte browser receipt had SHA-256 `708c940d6f3656417848e816ae17ef0d268b54be8f5d75de00bdc5d3621c7d61`. These are the inputs and outcomes of that build. The summary omits full event streams, session identifiers and local capture paths.

The unchanged image booted through the browser's worker, ROM and JIT with `appDirect:false` and no stubs. At 12 guest seconds the default fixture was on the IMU screen. Two interior tilt inputs changed the factory drawing and values to −30.00°/35.27° and 30.00°/16.78°. The neutral input settled at 0.00°/0.00°. A and B worked through the UI, keyboard and 3D hotspots. The RTC digits advanced and B opened the microphone page. Mini sharing restored the pinned image and controls, and the real cross-origin iframe RPC delivered button edges. Four recorded A/B edges replayed with the same final microphone frame and up to 54.47 ms of timing variation.

| Logical PCM stimulus | Lit LCD pixels | Consumed frames |
| --- | ---: | ---: |
| 1 kHz, port 0 | 12,529 | 159,709 |
| 2 kHz, port 0 | 12,593 | 159,709 |
| Silence, port 0 | 1,260 | 159,709 |
| 1 kHz, opposite port 1 | 1,260 | 0 |

These plots exercised the logical PCM conversion path, with underruns recorded in the positive cases. A separate native wrong-pin control used GPIO35. Additional native negative controls retained ERROR21 at 0 °C and 110 °C and WARNING22 at 3.0 V. Earlier attempts stopped at the factory's ERROR31 or Bluetooth-clock wait, followed by ERROR21 until the temperature ADC model was restored.

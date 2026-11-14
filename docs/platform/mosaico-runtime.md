# ESP-Mosaico V1.2

`espressif-esp-mosaico` uses the production `esp32s31::Machine` through the browser WASM worker, board `mosaico`, 16 MiB NOR Flash and 16 MiB in-package PSRAM. It is not an ESP32-S3/P4 alias or the strict S31 register-diagnostic runner. The chip uses the dual-hart RV32 interpreter. ESP-IDF v6.1 / esptool identify S31 images as chip `0x0020` and place the bootloader at `0x2000`; merged images, separate local segments and local IDF builds retain these offsets and reject another chip or a later required silicon revision.

The catalog entry is specifically CoreBoard V1.2 / BaseBoard V1.2. The official V1.0 pin map differs. Hardware references, timestamps, artifact hashes and the missing official CAD acquisition are in [source evidence](evidence/mosaico-sources/sources.json) and the product knowledge entry. The official display drawing establishes a 38.39 × 38.39 mm active area and 43.3 × 43.3 mm cover glass. The exterior model is photo-derived, with estimated case dimensions and an `UNKNOWN` internal PCB; it is not a manufacturer CAD model or a manufacturing drawing.

## Official firmware and observed behavior

The default recovery image is Espressif's **Mosaico Hello 0.1.3 + Vibe Mode 0.3.0**, published 2026-11-12. This recovery bundle is distinct from the FAQ's preinstalled Mosaico-Claw application.

- [Official recovery page](https://mosaico.espressif.com/firmware-update/)
- [Publisher image](https://mosaico.ai/api/v1/firmware/4/download): 16,777,216 bytes, SHA-256 `7562ff67cad8280fe8fdd06f3be7d6c91f44e5a8cc63fe596721f48da428af9b`.
- S31 mask ROM: `esp32s31_rev0_rom.elf`, SHA-256 `a52ad7513deb656a910a5740125f1cce2c7941f11ce57213b7b43aea93d5ab87` from the existing pinned ROM fetcher.

The unchanged image boots ROM → bootloader → `main_app` at `0x210000`; no stubs, direct-app boot, flash patches, or substituted display frames are used. The actual 480×480 CO5300 framebuffer first presents “Choose Your Language”. A CST9220 touch at `(350,300)` selects Chinese and opens the main screen. An AI/GPIO7 short press enters the firmware's standby UI; a second press wakes it. Serial output independently records `CO5300 brightness set to 20%`, `Module manager stopped`, then `CO5300 brightness set to 80%` and `Wake source=1`. This does not establish calibrated optical brightness or physical low-power operation (`light_sleep=0ms`).

The BOOT/GPIO61 key is connected through the S31 pad model, including the high GPIO bank. Its short press is delivered; no application action was observed for that press in this image. Keyboard A maps to AI and B to BOOT. The SAM8108 physical power key is distinct and has no ordinary GPIO control in this adaptation.

The official download endpoint returned no `Access-Control-Allow-Origin` for this site's origin; an actual local-browser fetch also failed with CORS. Therefore the catalog remains **trial**. If “下载并运行” fails, open the official recovery page, download the complete BIN, select **本地文件**, and choose **加载并运行**. Files stay in the browser. A shared configuration retains the product and pinned remote source, but cannot carry locally selected firmware bytes or make the publisher allow CORS. Receivers may need to download and choose the same BIN. Remote autostart and portable recording replay of this source are not accepted as working. No third-party firmware is mirrored into this site or committed to Git.

## Boundaries

| Area | Scope |
| --- | --- |
| Display and touch | Real CO5300 QSPI/TE path and CST9220 HYN212 report path; official language selection changes guest output. Dynamic 2D/3D surfaces display the same framebuffer. |
| Buttons | AI standby/wake accepted against official guest output. BOOT routes to GPIO61; short-press application effect unobserved. |
| Flash | Existing scoped browser Flash persistence applies. The official firmware's own stored language/settings are preserved; do not clear them to recreate onboarding. |
| NAND | Upstream GP-SPI3 NAND model is present. It is not TF/microSD, and is not persisted across a newly created browser runtime. No SD slot is declared or fabricated. |
| Motion | Native BMI270/BMM150 register subsets initialize. Host `setImu` and model-orientation input are unavailable for this board; no physical sensor calibration or module behavior is claimed. |
| Audio | I²S0/1 logical host PCM and bounded mono output are connected; a cold ABI test checks drain/no-duplicate capture. This is not acceptance of full voice, cloud speech, gain, microphone wiring or acoustic output in the recovery image. |
| Network | Native digital station/AP-link transport is connected to the existing optional Ethernet relay. Physical RF, Internet/cloud operation and automatic provisioning are unverified. S31 SoftAP capability remains false. |
| Other | BLE, camera, left/right modules, full USB hardware behavior, charging and physical power management are not accepted. Xiaozhi v2.6.0 is a separate pinned trial candidate, not verified by the recovery image's results. |

The inherited PSRAM ID fallback and DQS tuning warnings remain visible. The emulator has functional timing and permissive production register fallback; successful boot is not silicon accuracy or arbitrary firmware compatibility.

## Reproduce

```sh
tools/wasm-build.sh
tools/fetch-rom-elfs.sh web/wasm/fw
node --test tools/platform-mosaico-layout-test.mjs
cargo test -p esp32sim-wasm s31_ --lib
node tools/probe-mosaico-factory.mjs /path/to/hello-mosaico-complete_0.1.3.bin output/mosaico
```

The probe insists on the exact image hash, executes the real platform worker, records acknowledged inputs and guest output, saves four real framebuffer PNGs, and closes the worker in `finally`. Its initial state uses isolated in-memory Flash. A user's persisted device must retain existing data and can therefore start directly in its saved language.

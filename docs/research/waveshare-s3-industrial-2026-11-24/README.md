# Waveshare S3 industrial/controller adaptations

The first group covers five storefront families and twelve exact SKU options. It adds source-bounded digital behavior, preserves official factory/demo image identity and integrity, and supplies seven source-backed partial mechanical models. Full original firmware startup and physical hardware validation are **not** claimed.

| Listing | Exact SKUs | Native subset |
| --- | --- | --- |
| ESP32-S3-RS485-WLED | 35580 | ES7210 control registers on SDA15/SCL14; guest GPIO/RMT output observation on the two external strip signals; no strip or RS485 peer invented |
| ESP32-S3-CAN-2CH / U | 35815, 35882 | PCF85063 RTC on SDA39/SCL38, real guest-clock calendar rollover and STOP; CAN/XL2515 peer unmodeled |
| ESP32-S3-RGB-Matrix bare controller | 34422 | Existing exact 32/16 MiB controller profile, RTC/codec control and native SDMMC subset; no included display panel |
| ESP32-S3-CAM-OV5640 / OV3660 / GC2145 / GC0308 | 33699, 33700, 33702, 33701 | Exact common SDA8/SCL7 codec bus and BOOT0/PWR15; no fabricated camera chip ID or frame, no CH32 or camera SD transport |
| ESP32-S3-LR1121 HF/LF, bare/Kit | 34010, 34011, 33928, 33929 | Exact SPI/GPIO connector observation; no LR1121 response, radio peer or IRQ fabricated; physical BOOT is present only with the USB adapter |

## Evidence and limitations

Each family directory retains curated fetched sources with original/final URLs, UTC, bytes, SHA-256, pinned Git tree identity, exact storefront options, firmware investigation and native test receipt. The raw storefront HTML, example Wi-Fi configuration files and third-party firmware bodies are not committed. Official BIN files remain remote links; integrity audits are independent of execution.

Four families have explicitly named manufacturer Factory images. The Matrix manufacturer Test image remains `role: project`: its complete boot layout does not establish factory shipping identity. Its manufacturer-distributed Xiaozhi image is independently pinned, and is not renamed as the latest upstream release. Current UIFlow, Xiaozhi and M5Burner investigations retain both exact targets and negative matches. Xiaozhi camera display builds are not silently mapped to the no-screen camera SKU.

The camera manufacturer's STEP retains all 756 native solids/free shells/free faces, with a rigid viewing rotation only. It does not supply the separate sensor lens/flex assemblies. LR retains all 285 native topology members; the metal RF cover absent from that CAD is a separately documented hollow photo reconstruction. The USB adapter in the Kit has known dimensions and front appearance, an UNKNOWN reverse, and a clearly detached debug placement.

WLED and CAN2 use hollow case walls, separate caps, openings and an UNKNOWN internal PCB. Source dimensions govern the envelope; unmeasured wall thickness, port geometry and hidden mounting details remain estimates. The WLED photograph's body and ear dimension scales are inconsistent, and the measured overlay discrepancy is preserved. Bare Matrix retains its source photograph, independently estimated open USB receptacles and BOOT position, and an UNKNOWN rear. It never gains a panel through the related panel-kit product.

`models/` contains the final production Three.js loader receipt, seven four-view geometry plots, seven source overlays and review limitations. CPU mesh/ray/material checks do not claim browser/WebGL or physical fit acceptance. Every native source and explicit model plan is retained under `web/platform/assets/`; use the repository's ordinary `tools/rebuild-product-model.py` workflow to reproduce it from hash-matched files.

## Reproduce the meaningful checks

```sh
cargo test --release -p esp32s3 --test waveshare_industrial
cargo clippy --release -p esp32s3 --test waveshare_industrial -- -D warnings
node tools/check-waveshare-model-loading.mjs \
  waveshare-s3-rs485-wled-mechanical \
  waveshare-s3-can-2ch-mechanical waveshare-s3-can-2ch-u-mechanical \
  waveshare-s3-camera-controller-mechanical \
  waveshare-s3-lr1121-controller-mechanical waveshare-s3-lr1121-kit-mechanical \
  waveshare-s3-rgb-matrix-controller-mechanical
```

The six Rust tests execute actual LX7 S32I/L32I guest instructions into peripheral MMIO. They check source-exact memory, routed GPIO inputs, I2C addressing and wrong routes, codec reset, RTC boundary/STOP, RMT output routing and LR SPI observation with reset/CS/clock negative cases. They do not write expected peripheral output directly into framebuffer or device state.

Five integration fragments live under `tools/waveshare-batches/s3-industrial/`. Their declared memory/capabilities must enter the shared registry before public WASM trials; old generic capacities must not be inherited. The Matrix board module is identical to the already-reviewed shared implementation in commit `2cb056d4`. Root integration owns final public Worker/WASM checks and sequential five-family patch publication.

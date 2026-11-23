# EE adapter and factory execution evidence

`factory-wasm-checkpoint.json` retains **19 actual production-worker
observations** on the 6,022,516-byte WASM whose SHA-256 is
`dd32a7e3d1b4f72114260e2ffedea002bdcfb33900e74591ddd375b84107dc44`.
This is an executable checkpoint, not a claim that every final deployment
necessarily uses that same binary. Firmware bytes were unchanged and every
run used the real S3 ROM, with no application-direct boot or guest stubs.

## Observed results

| Workload | Guest window | Result |
|---|---|---|
| All 17 published SenseCraft 1.1.5 EE targets | requested 6 s; actual 6.00–6.26 s | Every target reached ROM boot, application startup and electronic-paper initialization. Each had logged seven SD retries and had completed zero display frames. All captures returned `NO_FRAME`. |
| EE04 + 7.5-inch UC8179 | 15 s | All nine SD attempts finished unsuccessfully; application-view initialization and Wi-Fi initialization were reached. Battery reported 63%. Zero frames, `NO_FRAME`. |
| EE05 + 7.5-inch UC8179 | 15 s | The same later startup stages were reached. Battery reported 0%, consistent with firmware driving GPIO6 while hardware ADC_EN is GPIO39. Zero frames, `NO_FRAME`. |

The first sample window was too short to reach the later initialization
stages. It is preserved in full. Extending the two representative boards
resolved that specific uncertainty without declaring a factory UI working.
The final observed Wi-Fi-initialization line does not by itself identify the
exact blocked register or instruction. No default factory screenshot was
produced, including a low-battery warning image. No diagnostic firmware,
synthetic screenshot or altered GPIO mapping replaced these results.

The actual [official SD initialization source](https://github.com/Seeed-Projects/OSHW-reTerminal-Series-E-D/blob/8e03627d41f63b4133e606cb25c5c81b9d251494/examples/official/SenseCraft_HMI/src/boards/common/sd_card.cpp)
tries 1, 2 and 4 MHz, three attempts each. Its fetched timestamp, full hash and
byte count are retained in the JSON. The general firmware SD initialization
does not prove an SD socket or implemented transport on the EE product.
Unmodeled PMIC register reads and the missing initial filesystem contents are
also recorded. The firmware reports a successful filesystem mount after
formatting the erased partition; no filesystem image was inserted.

The 12 MMIO integration tests described in
[the runtime documentation](../../seeed-ee-runtime.md) passed separately.
Those establish the digital controller behavior and input routes, including
visible pixels from guest SPI activation, but they do not promote the 17
default factory images to verified UI compatibility. The 4.26-inch mounting's
complete Y orientation remains uncalibrated.

## Reproduction

Build the production WASM and retrieve the S3 ROM as documented by the
repository. For every requested product use the catalog's unchanged pinned
factory plan:

```sh
ESP32SIM_ROM_DIR=/path/to/official-roms \
  node tools/probe-seeed-runtime.mjs \
  --product seeed-xiao-epaper-ee04-p075-mono \
  --seconds 6 --local-only \
  --local-map /path/to/official-url-to-downloaded-file-map.json \
  --output target/ee04-p075-factory-six-seconds
```

Repeat for the 17 product IDs in `runs` with cohort `initial-6s`. To reproduce
the extended checks, use `--seconds 15` for EE04-P075 and EE05-P075, with fresh
output directories. The map merely makes already downloaded original bytes
available to the real loader; it does not bypass hash verification. Every
required original URL, artifact size/hash and flash offset is in the receipt.
The four offsets are 0x0, 0x8000, 0xe000 and **0x90000**. Map URL queries
exactly, as included in the pinned plans.

Host compilation and other functional probes ran concurrently. No native or
WASM performance conclusion is drawn from these observations. The receipt
retains guest instruction counts and actual virtual time; transient host
speed and wall-time figures are omitted.

## Retention and privacy

Raw receipts and serial logs contain synthetic device/storage identifiers and
are excluded from the repository. Their original SHA-256 values and serial
byte counts remain, together with selected identifier-free startup markers.
There are no omitted framebuffer bytes: every requested capture actually
returned `NO_FRAME`. The source header receipt in `sources.json` pins the
ESP-IDF v4.4.7 register definitions used by the actual factory version.

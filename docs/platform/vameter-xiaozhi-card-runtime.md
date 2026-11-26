# VAMeter K136 and Xiaozhi Card Kit K146

Both exact SKUs now have separate ESP32-S3 board adapters with a tested digital
display/input subset. The factory firmware entries remain **trial**, with
`simulationVerified: false` and `hardwareValidated: false`. These diagnostics do
not establish that either complete factory application boots or works.

## Fixed source contracts

The fetched source files, exact URLs, fetch times, sizes and SHA-256 receipts are
in [sources.json](evidence/vameter-xiaozhi-card-20261126/sources.json). Six small
source files are retained beside the receipt. Their copyright/SPDX notices are
preserved. Existing product CAD and photographs retain their own source and
license records; no new photograph or invented controller identity was used.

| SKU | Fixed public source | Digital adapter |
| --- | --- | --- |
| K136 | `m5stack/VAMeter-Firmware` at `5e8f847bbc771da2d354be7472c6345acb5655ac`, `hal_config.h`, `hal_display.cpp` and `hal_power_monitor.cpp` | `vameter` |
| K146 | `m5stack/xiaozhi-card` at `44aa2f2b91c3e938932bb41f670e984cf752b731`, exact board config and initialization | `xiaozhi-card` |
| K146 EPD driver | That parent tree's `components/esp_epd_gdey027t91` gitlink points to `hlym123/esp_epd_gdey027t91` at `e7e4420392a8cf0927e79ed286f18531a14c433b` | Byte-X / gate-Y 176×264 monochrome command endpoint, without asserting an undocumented controller die |

| Behavior | K136 VAMeter | K146 Xiaozhi Card Kit |
| --- | --- | --- |
| Routed SPI | SCK36, MOSI35, CS37, DC34 | SCK45, MOSI46, CS42, DC41 |
| Display | ST7789 RGB565, source 240×240 zero-offset viewport in 240×320 RAM; sleep/on/reset gates | Official driver command subset: `11`, `44/45`, `4e/4f`, new RAM `24`, then `22/20` refresh |
| Reset / visibility | Actual GPIO33 reset and GPIO38 or routed LEDC backlight on/off | Actual GPIO47 reset shared by EPD and touch; BUSY48 high during the digital refresh deadline; optical image survives reset |
| Inputs | Active-low BOOT0 and encoder press2; A3/B1 quadrature | Wake21; FT6336U common coordinate-register subset at `0x38`, SDA2/SCL1, IRQ16 |
| Memory | 8 MiB Flash; 0 MiB PSRAM is an explicit simulation configuration, not a finding about all physical revisions | Documented 8 MiB Flash / 8 MiB PSRAM and S3-PICO package identity |
| Deliberately absent | INA226 `0x40/0x41`, physical voltage/current circuitry, relay, buzzer, power, network | AW32001 `0x49`, BQ27220 `0x55`, ES8311 `0x18`, audio, SD NAND, modem/network, USB switching and dock behavior |

Unimplemented I²C peripherals NACK instead of returning fabricated measurements,
power state or codec identity. A power controller is not replaced with a GPIO
button. Touch reset must actually be released; incorrect SPI/I²C routes do not
reach the corresponding peripheral.

## Model and interaction

K136 retains the manufacturer mesh and all existing native transforms. Its
previously absent interactive black thumbwheel is now a separate derived mesh
from the exact SKU's retained front photograph
`reference-product-photo-99e842f36d.webp`. Radius 2.1 mm, width 8 mm and center
`[0, -23.1, 18.1]` mm are photo estimates, approximately ±1 mm; they are not native
CAD dimensions. The model plan retains the same derived definition for public
rebuilding. Native `geometry.bin` is unchanged. The offline public rebuild found
all five pinned inputs and retained the same part/derived-part contracts, but
its native geometry byte hash differs. A per-part canonical comparison of
oriented position triangles passes for all ten parts; ordering and normals
remain byte differences. This negative byte-rebuild result is retained rather
than silently replacing the accepted native asset.

The actual wheel's exposed axial end maps to encoder press2, and its rim maps to
the existing quadrature input path. Neither an invisible shell-wide proxy nor a
front-end display animation provides this behavior. Actual model ray tests
check both exposed regions, and customization leaves the wheel's dark material
opaque. Mechanical travel, physical detents and rotation direction are not
calibrated; the catalog's 20 detents only sets the viewer gesture scale.

K146 keeps the source handset/dock assembly and live 176×264 screen. The already
documented FT6336U contact can now reach the guest register model. No guessed 3D
power button or dock control was added.

Full VAMeter mounting/MADCTL rotations and optical brightness have not been
calibrated. K146's 20 ms refresh, 1 ms touch IRQ and short-contact retention across
two register reads are explicit digital-interface policies. They do not model
physical EPD timing, LUTs, temperature, ghosting, analog waveforms, grayscale or
the complete FT6336U identity/gesture register space.

## Factory investigation

The pre-existing exact-SKU factory-first defaults are retained, including their
historical receipt dates. No firmware binary was downloaded or executed in this
change. Fetching current board source does not refresh those older binary
checks or establish compatibility.

| SKU | Retained default | Evidence and present status |
| --- | --- | --- |
| K136 | `m5stack-k136-official-reference`, official `v1.2.2-pre` | Existing M5Burner entry `b181a9558bf3e36e53ce79018cb11805`; fixed CDN URL and publisher SHA-256 remain in the catalog. Prior layout check retained, no factory boot here. |
| K146 | `m5stack-k146-official-reference`, official Xiaozhi `v1.9.4` | Existing M5Burner firmware `1897200000000002463`, device `1897100000000000013`, version `1897300000000005416`; publisher hash unavailable. Existing `BURN_CAPTCHA_INVALID` download-access result remains; no bypass or new binary/layout claim. |

Existing exact factory and Xiaozhi watchlist mappings remain intact. No universal
UIFlow build or sibling firmware is substituted for either default. This source
review did not establish a new exact UIFlow image for either SKU.

## Verification

`esp32s3/tests/vameter_xiaozhi_card.rs` installs authored LX7 `l32i`/`s32i`
instructions in IRAM and runs the CPU through GPIO matrix, SPI3 and I²C0 MMIO.
The tested pixels and key levels are guest-observable results. No diagnostic
UART lines are injected. ESP-IDF v5.5.4 S3 register-source receipts used by the
existing MMIO probes are retained in `web/platform/knowledge/cloudzao-claw-154.json`.

| Check | Result |
| --- | --- |
| `cargo test -p esp32s3 --test vameter_xiaozhi_card` | 3 passed: routed RGB565/BL/reset and real encoder/press reads; 1bpp RAM separated from timed optical refresh/BUSY/reset retention; FT coordinate reads/reset/wrong-route and wake press/release |
| `cargo clippy -p esp32s3 --all-targets -- -D warnings` | Passed |
| New model test plus manufacturer-model and inspection suites | 10 passed, including actual closest wheel surface rays, shell material isolation and rejecting invalid derived control metadata |
| Catalog schema validator | Passed: 714 products, 10 accessories, 654 firmwares, 501 watchlist entries |
| Knowledge validation | Passed: 722 records and source/revision reference checks |
| Config / input / view suites | Passed: 17 config groups, 11 input checks, 4 framebuffer and 2 material-lifecycle checks |
| Generic worker/WASM runtime suite in this worktree | Blocked at initial missing local ROM ELF; no runtime assertion reached. Reuse the integration checkout's built WASM and ROMs for the combined check. |
| Public model rebuild | Offline plan/input hashes passed. Build completed but exact comparison failed on `geometry.bin`; all 10 oriented position-triangle multisets match, part/derived metadata matches, native accepted asset retained. See `k136-rebuild-comparison.json` and `k136-native-position-comparison.json`. |

The final asset/code hashes and retained-source byte checks are recorded in
[verification.json](evidence/vameter-xiaozhi-card-20261126/verification.json).
No physical hardware, whole factory firmware, browser UI, iframe/replay, full
workspace release or new production WASM acceptance is claimed by this report.

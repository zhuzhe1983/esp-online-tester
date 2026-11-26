# Tab5 C145: P4 display runtime remains blocked

Historical baseline audit. The subsequent [ILI9881C + GT911 digital runtime](m5stack-tab5-runtime.md) implements the missing DSI/DMA path and supersedes this document's current-availability conclusion. The baseline hashes, source findings and excluded acceptance claims below remain unchanged.

Audit baseline: `baf3cb90cc3ca4f6a2ce0e1bd57d44fab68722ba`, 2026-11-26 UTC. **C145 remains unavailable and does not count as a completed board adaptation.** The ESP32-P4 is the main controller; the ESP32-C6-MINI-1U is the wireless coprocessor. A C6 runtime, an empty P4 board or a host-generated image cannot execute the Tab5 display path.

This is a static source audit plus isolated tests of existing I2C device code. No Tab5 guest image was executed; there is no measured boot hang, framebuffer, touch-to-guest trace, browser or physical-hardware acceptance. No runtime, catalog availability flag, Rust bus, shared build target or WASM artifact changed in this audit.

## Primary sources and versions

The [official Tab5 documentation](https://docs.m5stack.com/en/core/Tab5) identifies the two MCUs, MIPI-DSI display, I2C pin map and batch-dependent display/touch devices. The audit fetched the actual source bytes listed in [source-receipts.json](evidence/m5stack-tab5-runtime-blockers-20261126/source-receipts.json), with final URL, UTC fetch time, content type, byte count and SHA-256. Links below use fixed software revisions:

| Source | Fixed revision | Relevant files |
| --- | --- | --- |
| [M5Tab5-UserDemo](https://github.com/m5stack/M5Tab5-UserDemo/tree/fe8aa051beb9385468f7e46b909e764169df3f7a) | `fe8aa051beb9385468f7e46b909e764169df3f7a` | `platforms/tab5/dependencies.lock`; `components/m5stack_tab5/{m5stack_tab5.c,include/bsp/{m5stack_tab5.h,display.h}}` under that platform |
| [M5GFX](https://github.com/m5stack/M5GFX/tree/28d47949bdc84f30881623476f555398e2194313) | `28d47949bdc84f30881623476f555398e2194313` | Tab5 board specification; P4 `Bus_DSI`; ILI9881C/ST7121/ST7123 panel and GT911/ST7123 touch drivers |
| [ESP-IDF](https://github.com/espressif/esp-idf/tree/v5.4.2) | `v5.4.2`, matching the UserDemo lock and README | DSI bus, DBI and DPI drivers; P4 DSI host/bridge/PHY LL headers; host/bridge and DW-GDMA register headers; register bases |

The necessary public register addresses, bit definitions and driver sequence were found. The immediate blocker is missing emulator behavior, not an assertion that a complete official register contract is unavailable. The fetched headers do not establish analog PHY behavior, every reserved register or silicon timing.

## Existing implementation versus the required path

| Layer | Baseline evidence | Consequence for Tab5 |
| --- | --- | --- |
| P4 CPU and memory | [lib.rs](../../esp32p4/src/lib.rs), [soc.rs](../../esp32p4/src/soc.rs): P4 revision families, two HP core objects, P4 memory/MMU/interrupt paths | A P4 RV32IMAFC foundation exists. `soc.rs` explicitly leaves Xesppie and hardware loops illegal. This audit did not execute a Tab5 firmware image, check whether it uses these instructions or establish its required virtual wafer revision. |
| P4 digital IO | [periph.rs](../../esp32p4/src/periph.rs) `device_set!`: I2C0 `0x500C4000`, I2C1 `0x500C5000`, GPIO, IO_MUX, LEDC and other endpoints | IO infrastructure is reusable, but Tab5 devices and reset/power nets are not attached to a Tab5 board. |
| Existing P4 display | [board.rs](../../esp32p4/src/board.rs): `WaveshareP4Lcd35`, `spi_tx`, `display` | The implemented display is GPSPI2/ST7796, 320×480. It cannot consume Tab5 MIPI-DSI traffic. |
| Waveshare DSI products | [waveshare_screens.rs](../../esp32p4/src/board/waveshare_screens.rs): 15 profiles, `report()` and `every_profile_uses_exact_input_geometry_without_a_synthetic_dsi_frame` | These provide selected input/IO devices. They explicitly report an unimplemented DSI/DPI framebuffer and assert `display().is_none()`. Their panel dimensions are not evidence of a DSI renderer. |
| DSI host and bridge | No DSI field or mounted `0x500A0` endpoint in P4 `device_set!`; `block_name` only labels the address for diagnostics | No command FIFO, PHY status, packet handling, bridge configuration, scanout or DSI interrupt state machine exists. |
| Display DMA | No mounted DW-GDMA `0x50081` or DMA2D `0x50088` endpoint; [bus.rs](../../esp32p4/src/bus.rs) `spi2_dma_tx` consumes AXI-GDMA for SPI2, with separate AHB-GDMA audio logic | Existing generic AXI/AHB GDMA paths do not implement the DesignWare channel/LLI contract used by the official DPI driver. No DSI pixel sink consumes guest memory. |
| Unhandled MMIO | [mmio.rs](../../esp-periph/src/mmio.rs) `read32`/`write32`; [regram.rs](../../esp-periph/src/regram.rs) | Unmounted block accesses use zero-initialized register RAM with write/readback and no write effects. Naming a block `DSI` is not a device implementation. |
| Tab5 board | P4 `make_board` and the delegated LilyGo/Waveshare constructors contain no C145/Tab5 board | No source-correct internal I2C wiring, reset/power gating, touch interrupt or framebuffer publication exists for C145. |

### Concrete first hardware dependency

ESP-IDF v5.4.2 `components/esp_lcd/dsi/esp_lcd_mipi_dsi_bus.c`, lines 90–99, waits for the PHY PLL to lock and for the enabled lanes to stop. The LL implementation reads hardware-owned status in `DSI_HOST_PHY_STATUS_REG`:

| Address / field | Official contract | Baseline behavior |
| --- | --- | --- |
| DSI host base `0x500A0000` | `reg_base.h`, `DR_REG_DSI_HOST_BASE` | Unmounted block |
| PHY status `0x500A00B0`, bit 0 | PLL lock, read-only | Initially zero register RAM; no lock transition |
| Same status, bits 2, 4, 7 | Clock lane and two data lanes stopped | No lane state transitions |
| Host `+0x6C`, `+0x70`, `+0x74` | Generic packet header, payload and command packet status | No command FIFO or DCS delivery to a panel |
| DSI bridge base `0x500A0800` | DPI enable/configuration, input pixel type, flow control and interrupt state | Same unmounted 4 KiB block as the host; independent host/bridge behavior is absent |
| DW-GDMA base `0x50081000` | Dedicated DesignWare DMA channels and linked-list items | Unmounted block, not the AXI-GDMA at `0x5008A000` |
| DSI bridge pixel memory `0x50105000` | DPI driver's fixed DMA destination | No registered display sink |

**Static deduction:** with the baseline peripheral dispatch and no external test pre-seeding those status registers, the official bus initialization has no modeled hardware action that can satisfy its PHY waits. This is not a captured guest execution failure. Hard-coding the status bits would only pass that first wait; it would not implement command handling or scanout.

The official DPI driver allocates a DW-GDMA channel and link list, points the transfer source at the guest framebuffer and its fixed destination at `MIPI_DSI_BRG_MEM_BASE`, enables the channel, selects video mode and enables bridge DPI output (`esp_lcd_panel_dpi.c`, lines 119–155, 283–331 and 404–459). Its DMA callback handles frame/block progress. Both UserDemo panel paths set `use_dma2d=true`: external draw buffers select the DMA2D copy path. The driver also has a direct-framebuffer/cache-writeback branch; accepting a narrower firmware path requires a trace proving which branch it actually uses, including initialization. The baseline cannot obtain real pixels by attaching the existing SPI panel object.

PHY power and clocks also need explicit behavior: UserDemo requests LDO channel 3 at 2500 mV; the IDF LL writes `HP_SYS_CLKRST` DSI bus, PHY reference/config and DPI clock controls. Existing clock register acceptance alone is not evidence that a PHY state machine or pixel clock runs. No new clock/register bit values are introduced by this report.

## Exact board wiring and batch boundaries

| Function | Tab5 official wiring | Reuse boundary |
| --- | --- | --- |
| Internal I2C | SDA GPIO31, SCL GPIO32 | Do not reuse the Waveshare SDA7/SCL8 attachment. |
| Touch interrupt | GPIO23 in the official pin map | The UserDemo header leaves its touch INT macro unconnected while referring to GPIO23; follow the selected driver's actual polling/interrupt behavior and preserve that distinction. |
| Backlight | GPIO22 | Preserve LEDC/GPIO routing and gate visibility; this is not a pixel-data pin. |
| IO expanders | PI4IOE5V6408 at `0x43` and `0x44` | An existing digital PI4IO implementation can be instantiated twice with separate state. The BSP section title says PI4IOE5V6416; the official hardware pin map and 8-bit register/pin usage identify the actual modeled part. |
| Expander 1 reset nets | P4 LCD_RST, P5 TP_RST, P6 CAM_RST | Here “P4” means expander pin 4, not CPU GPIO4. Reset/power gating must affect the attached devices. |
| LCD reset release | Low output latch; direction output pulls low, then input with pull-up releases reset | `Pi4ioState::drive(4)` becomes `None` at release. Testing only for `Some(true)` would hold the display in reset. The digital pull state must resolve the net; analog voltages are outside the current model. |
| Expander 2 | P0 WLAN_PWR_EN; P3 USB5V_EN; P4 PWROFF_PULSE; P5 nCHG_QC_EN; P6 CHG_STAT_LED; P7 CHG_EN | Do not claim wireless/USB/charging behavior merely because the expander registers accept writes. |

The source has three distinct display paths. This table records the pinned **UserDemo BSP C implementation**, not a mixture of BSP and M5GFX timing defaults. All three use 720×1280 portrait geometry, two DSI lanes, one RGB565 framebuffer and DBI virtual channel 0 with 8-bit commands/parameters.

| Batch path | Touch detection / protocol | BSP DSI lane / DPI clock | BSP timing H: back/sync/front; V: back/sync/front |
| --- | --- | --- | --- |
| ILI9881C + GT911 | GT911 probe at `0x14`; ready/count `0x814E`, coordinates `0x8150`, report acknowledgement | 730 Mbps / 60 MHz | H `140/40/40`; V `20/4/20` |
| ST7121 | `0x55`, register `0x0000` returns firmware version 1 | 965 Mbps / 70 MHz | H `40/2/40`; V `24/20/200` |
| ST7123 | `0x55`, register `0x0000` returns firmware version 3; the BSP also falls back to ST7123 on unrecognized values | 965 Mbps / 70 MHz | H `40/2/40`; V `8/2/220` |

M5GFX at the audited revision uses different default DPI clocks for some panels (ILI9881C and ST7123: 80 MHz); that difference must stay associated with the selected firmware. The BSP detector contains a stale ST7703 log string after the GT911 probe, but selects `BSP_DISPLAY_TYPE_ILI9881C_GT911` and constructs an ILI9881C panel. It is not evidence that Tab5 uses the existing SPI ST7796 panel or that this batch is ST7703.

The reusable [GT911 device](../../esp-periph/src/i2c/gt911.rs) implements a bounded single-contact report/acknowledgement subset. It would need the Tab5 `0x14` address, reset and GPIO31/32 wiring; attaching the Waveshare `0x5D` device is insufficient. It does not cover ST712x. The M5GFX ST7123 touch driver uses 16-bit register addresses, firmware/capability registers, the `0x0010` coordinate-present flag and 7-byte contact records at `0x0014`, with valid bit 7 and the high X bits in the first byte. No such device is implemented in this baseline. Do not identify one protocol as another or fabricate unspecified firmware revision bytes to make automatic detection pass.

## Checks completed and acceptance still required

[recheck.py](evidence/m5stack-tab5-runtime-blockers-20261126/recheck.py) verified all 14 baseline file hashes and all 26 primary-source download hashes. Its `--io-probe` compiled the actual frozen I2C trait and PI4IO/GT911 source files directly with `rustc 1.100.0 (0ef1bc386 2026-11-09)`, without Cargo or the shared build target. Four tests passed: the three existing PI4IO/GT911 device tests and an additional probe that replays the official LCD reset output-low → input-pull-up sequence. This establishes a reusable digital device subset only. It does not execute the P4 I2C master, a Tab5 board or firmware.

Recheck from the repository root with `rustc` on `PATH`:

```sh
python3 docs/platform/evidence/m5stack-tab5-runtime-blockers-20261126/recheck.py --repo . --sources /tmp/tab5-source-check --fetch --io-probe
```

The dated product page may change; a later product-page hash mismatch must be recorded as a new source snapshot, rather than silently replacing the receipt. Fixed software source hashes remain independently checkable. [checks.json](evidence/m5stack-tab5-runtime-blockers-20261126/checks.json) records the completed checks and excluded claims.

The minimum next implementation and acceptance work is:

1. Select and name one supported batch, with one pinned official guest build. Start with its exact P4 revision, IO-expander reset sequence, panel initialization and touch protocol. Other batches remain explicitly unsupported until separately checked.
2. Implement the DSI host/bridge and DW-GDMA register/descriptor paths used by that guest, bounded guest memory access, reset/clock/PHY readiness, command FIFO behavior, interrupts and the framebuffer sink. Exercise status and error transitions, not just successful register readback. For UserDemo's DMA2D-enabled drawing, implement and verify that copy path as well.
3. Run the guest's actual LCD initialization and copy a 720×1280 RGB565 frame (1,843,200 bytes) from guest memory through its descriptor/register stream. Compare exact color-bar/corner pixel values and a full-frame hash, then update the guest framebuffer and verify a changed frame/version. Include disabled backlight, display reset, sleep/off, invalid/short descriptors and out-of-range memory cases so a static host fixture cannot pass.
4. Inject press, move and release through the selected touch device. Capture the guest's I2C reads, ready/ack behavior or ST712x report format and any selected interrupt routing, then verify a resulting guest-drawn pixel change. Changing a host-side touch flag alone is insufficient.
5. Verify the resulting display through the runtime API and browser renderer; retain the guest firmware hash, exact register/descriptor evidence, pixel checks and remaining limitations. Only then may that batch receive a supported runtime declaration.

Camera/CSI (SC2356), ES8388/ES7210 audio, BMI270, RX8130CE, INA226, USB host, SDMMC socket wiring, analog charging/power and ESP-Hosted/C6 remain outside this audit's verified subset. Existing peripheral support on other boards does not establish Tab5 wiring or factory-firmware acceptance. C145 therefore keeps its current unavailable state and visible limitation.

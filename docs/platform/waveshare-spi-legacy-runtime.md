# Waveshare S3 legacy SPI displays: restored 1.28-inch profiles

This document describes the new implementation and new measurements made after the workspace reset on 2026-11-23. Earlier screenshots and execution logs were lost. They are not used as acceptance evidence here. The board source was recovered byte-for-byte as a starting point, then migrated to the current main SPI phase API and corrected against newly retrieved original schematics and component data sheets.

## Exact electrical profiles

| Property | ESP32-S3-LCD-1.28 | ESP32-S3-Touch-LCD-1.28 |
| --- | --- | --- |
| Board selector | `waveshare-s3-lcd128` | `waveshare-s3-touch-lcd128` |
| SoC / capacity | ESP32-S3R2 / 16 MiB flash / 2 MiB Quad PSRAM | ESP32-S3R2 / 16 MiB flash / 2 MiB Quad PSRAM |
| Display | GC9A01A, 240 × 240, circular glass | GC9A01A, 240 × 240, circular glass |
| LCD CLK / MOSI / CS / DC | GPIO 10 / 11 / 9 / 8 | GPIO 10 / 11 / 9 / 8 |
| LCD reset / backlight | GPIO 12 / 40 | GPIO 14 / 2 |
| LCD SDO | Not wired | GPIO 12; readback not implemented |
| Sensor SDA / SCL | GPIO 6 / 7 | GPIO 6 / 7 |
| Motion sensor | Schematic U4, QMI8658C | Rev. 3 schematic U3, QMI8658A |
| QMI address | 0x6B, SA0 grounded | 0x6B, SA0 grounded |
| QMI INT1 / INT2 | GPIO 47 / 48; not modeled | GPIO 4 / 3; not modeled |
| Touch | Absent on the schematic | CST816S, 0x15, same SDA / SCL |
| Touch reset / IRQ | Not fitted | GPIO 13 / 5 |
| BOOT | GPIO 0, active low | GPIO 0, active low |
| Physical TF slot | Absent | Absent |

The Touch drawing is explicitly marked Rev. 3. The firmware is published for this product, but its README does not identify a PCB revision. Its execution against this separately sourced wiring is tested; that does not establish compatibility with every earlier PCB revision. GPIO 4/5 motor driver footprints in the Touch drawing are marked NC. No motor is exposed as an available peripheral.

### Source conflicts and corrections

QST's QMI8658A and QMI8658C Rev. A data sheets, section 16.3.1, both specify SA0 low as **0x6B**, and SA0 high as **0x6A**. The old recovered board source had these reversed. Newly rendered schematic pages show SA0 grounded on both boards, so the new implementation attaches only 0x6B. There is no dual-address compatibility alias. The earlier suggestion that a factory access to 0x6B conflicted with these schematics was an emulator error and is withdrawn.

The pinned non-touch repository README describes a CST816S and OPI PSRAM. Its actual schematic shows no touch IC and an ESP32-S3R2. The profile follows the schematic and the exact device's memory configuration, rather than those copied README claims. The Touch README, drawing and S3R2 capacity agree. Firmware filename dates, repository creation times and drawing metadata are not product release dates.

The older public Touch Arduino QMI driver writes CTRL1 = 0x60 (including big endian) but interprets sensor words as little endian. The new QMI device follows QST's documented endianness. That Arduino source is a wiring/protocol reference; it is not asserted to be the source build of the separately published factory image.

## Digital display and input behavior

The board opts into `SpiQspiPins` so a single-wire display cannot accidentally accept a transfer using multiple data lanes. The accepted transfer must use the actual CLK, MOSI and active CS pads, an output DC pad, released LCD reset, byte-aligned command/data and no address or dummy phase. Unsupported phase modes are rejected. The physical route and phase snapshot includes `keep_cs`; display DC and RAM state persist naturally across split transactions, while a new command starts a new parameter stream. No alternate board name or shared dummy pins are used.

The panel uses a 240 × 240 generic DCS RAM. It does not acquire ST7789 RAMCTRL byte order from the shared `DcsPanel::st7789()` constructor. Supported screen operations are sleep, display enable, inversion, address windows, MADCTL, COLMOD and RGB565 RAM writes. GC9A01A B6h's GS and SS bits are handled locally from the data sheet: they alter output scan independently of MADCTL's RAM addressing. SS also alters RGB source assignment, as described on the B6h page. The display reference is the board's public initialization sequence B6h = 00h, 20h and MADCTL = 08h. This is a source-derived digital convention, not a measurement of the physical module's optical orientation or color calibration.

Backlight output follows the actual GPIO or LEDC duty. A circular pixel-center aperture clips the rendered framebuffer; the exact optical edge and brightness response have not been measured. Panel voltage, gamma, TE, scrolling, 18-bit pixels, SDO readback and reset pulse timing are not modeled. Unknown panel initialization commands are ignored without claiming analog completion.

The private CST816S model covers the documented 01h–06h single contact report, EDh interrupt pulse width, supported FAh touch/change masks, sleep and reset. RSTn has the internal pull-up documented in section 4.3. A touch event produces a timed physical GPIO 5 pulse; reading a report does not truncate that pulse. The test checks that the edge reaches the actual GPIO interrupt peripheral. Report, IRQ, hardware reset and sleep state are shared across both I2C controller views.

No project-specific CST silicon, project or firmware ID has been established. The model returns explicitly unknown zero values for those diagnostic identity bytes; this is not a claim that physical boards report zero. The reset FAh interrupt mask is also unknown. Before firmware writes FAh, changed host contact state generates a 1 ms pulse as an explicit simulation policy. Gestures, sensor scan cadence and automatic sleep timing are not modeled. A successful register-level test is not presented as proof that the factory application consumes touch events.

The shared [QMI8658 A/C model](qmi8658-register-model.md) supplies guest-cycle sampling, range, endian and documented command/reset behavior. Static motion is +1 g on Z and 25 °C; mode 1 is a deterministic test script. Physical PCB-to-screen axes, QMI interrupt waveforms, self-test, calibration and FIFO are unsupported. Reattaching buses after an MCU reboot preserves the externally powered QMI register state rather than inventing a sensor power cycle. Battery ADC, charging and RF are outside these profiles.

## Fixed original source and firmware

All 13 original source documents are stored as deterministic gzip under `docs/research/waveshare-legacy128-sources/`. The [manifest](../research/waveshare-legacy128-sources/manifest.json) records the new retrieval time, original URL, exact decompressed SHA-256 and archive SHA-256 for each file. GitHub connector bytes are identified as decoded repository blobs, not HTTP response bytes. The two firmware images remain remote links; their bytes are downloaded only into the temporary test cache and are not committed.

- LCD repository: `waveshareteam/ESP32-S3-LCD-1.28`, commit `193724abdac7ec6571519269af909c38fd042716`.
- Touch repository: `waveshareteam/ESP32-S3-Touch-LCD-1.28`, commit `e0325e262a0c081ef554bd05d052c901ded58933`.
- LCD factory: `firmware/ESP32-S3-LCD-1.28_FactoryOnly_20250909.bin`, 560,064 bytes, SHA-256 `bb390d0d68bcc129d12b6d6692500bf6001e482c2827c48e53168ac7007a5075`.
- Touch factory: `firmware/ESP32-S3-Touch-LCD-1.28_FactoryOnly_20251120.bin`, 1,393,680 bytes, SHA-256 `11f684711366e2968d301c64a887d5483d8f2ebff16d672d57b3b610e68d8608`.

Both images load at flash offset 0. Their official firmware READMEs identify them as tested factory recovery images. The new download hashes match the hashes retained in the earlier conversation, independently of the missing old local files.

## New execution verification

Fresh release execution and matched input-control results are recorded in the evidence manifest accompanying this board commit. The native register/route tests verify real electrical selection, GC9A01A scan behavior, missing-touch separation, CST report/reset/IRQ behavior, correct QMI address, shared bus state and sensor state across MCU bus reattachment. Full production-WASM and whole-workspace/CPU regression are separate root integration gates.

### Measured results

| Profile | Run | Display | Input compared with identical no-input timing | Status |
| --- | --- | --- | --- | --- |
| LCD-1.28 | 8,000 guest ms, 0 reboots | 1,521 RAMWR commands, 7,268,145 written pixels, 0 rejected SPI transfers | BOOT 0 pixels; scripted motion 0 pixels | Partial: animated display works; distinct input response not demonstrated |
| Touch-LCD-1.28 | 8,400 guest ms, 0 reboots | 40 RAMWR commands, 230,400 written pixels, 0 rejected SPI transfers | Early swipe 0 pixels; late click 0 pixels; 0 CST reports read | Trial: settings screen appears, but interactive factory operation is blocked |

The LCD demo changes 38,743 pixels during its one-second quiet interval. Raw later changes of 23,420 pixels after BOOT and 24,320 after motion therefore do not prove input success: the matched no-input executions produce exactly the same frames.

For Touch, a 22-event upward swipe at 2.2–3.1 seconds and a click at 8 seconds both produce no additional pixel changes. The reset pin is released and GPIO 5 is configured for interrupt input; protocol-level IRQ behavior is separately tested. The factory's last UART marker is `BLE Test`, and its display does not advance after the 4-second checkpoint. This identifies the observed stage, not a proven cause in a particular RF instruction. The QMI revision read now returns 7Ch at the corrected 6Bh address. Two I2C write NACKs remain visible; no unsupported register is made to ACK merely to suppress those messages.

The [new evidence manifest](../evidence/waveshare-legacy128-2026-11-23/manifest.json) includes the actual release invocation, test-source hash, guest-state snapshots, unmodified UART bytes, screenshots, compressed RGB565 buffers, input-control comparisons and successful Clippy receipt. There are six passing release tests: four protocol/route tests and the two actual factory runs. Passing these tests does not upgrade the explicitly recorded negative input findings.

To reproduce the external runs, supply `ESP32SIM_ROM_DIR` pointing to the S3 mask-ROM ELF and `ESP32SIM_WAVESHARE_LEGACY_SOURCES` containing each unchanged factory image at `<SKU>/firmware/<filename>`. Set `ESP32SIM_WAVESHARE_LEGACY_EVIDENCE` to a fresh output directory, then run:

```sh
cargo test --release -p esp32s3 board::waveshare_spi_legacy::tests --lib -- --include-ignored --skip external_touch128_initial --nocapture --test-threads=1
cargo clippy -p esp32s3 --all-targets -- -D warnings
```

`external_touch128_initial` remains an optional diagnostic for the 2, 4, 5, 6 and 8-second checkpoints; it is not needed to repeat the final input comparison.

## Subsequent host IMU input extension

Both 1.28 products now retain one shared QMI object for I2C transport and
`set_imu_sample`. Finite acceleration in g and angular rate in degrees per
second are held in sensor axes. Guest ODR, range, byte order, enabled sensors
and SyncSample locks still determine the register output. Selecting stationary
or scripted motion clears the host hold. The S3 browser ABI first delivers
deferred device time and publishes the current cycle.

Fresh guest I2C FIFO/Xtensa input tests and the browser ABI sampling-boundary
test are recorded in `docs/evidence/waveshare-qmi-host-input-2026-11-23/manifest.json`.
The default LCD1.28 factory run was repeated and its raw framebuffer, UART and
state outputs matched the earlier source version byte for byte. This adds a
digital sensor-input path; factory motion UI behavior and physical PCB/3D pose
calibration remain unverified.

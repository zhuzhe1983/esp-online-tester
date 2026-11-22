# Seeed XIAO ePaper EE adapters and fixed panel kits

The runtime registers **four bare adapters and 25 distinct adapter/panel
combinations**. All execute ESP32-S3 firmware with the XIAO ESP32-S3 Plus
configuration, 16 MiB flash and 8 MiB PSRAM. They are partial digital board
models. A bare EE02, EE03, EE04 or EE05 has no panel allocation, display frame,
BUSY output or simulated SPI respondent. Connecting a particular glass is an
explicit kit selection, never an implicit 7.5-inch default.

The same `esp32s3::board::make_board` registry serves native CLI and WASM.
Unsupported combinations, such as `seeed-xiao-epaper-ee03-p075-mono`, return
`None`. Product and execution-board IDs are identical.

## Hardware wiring and controls

The fixed source is [Seeed_GFX2 board configurations at
556df7cb](https://github.com/Seeed-Studio/Seeed_GFX2/blob/556df7cb0b4a097d9b81026a26fd5511f9c934d6/src/board/configs/XIAO_ePaper_Board_Configs.h).
XIAO aliases D8/D9/D10 resolve to GPIO7/GPIO8/GPIO9 in the official
[XIAO ESP32-S3 Plus variant](https://github.com/espressif/arduino-esp32/blob/master/variants/XIAO_ESP32S3_Plus/pins_arduino.h).
The numeric control nets in the EE configurations avoid the differing D11/D16
aliases in older XIAO board packages.

The factory app descriptor identifies ESP-IDF v4.4.7-dirty. MMIO addresses,
SPI matrix signals, GPIO256 software routing, CS_KEEP_ACTIVE bit30 and UART0
TX signal12/native function0 were checked against Espressif's **v4.4.7**
`soc.h`, `gpio_reg.h`, `spi_reg.h`, `gpio_sig_map.h` and `io_mux_reg.h`. These fields
match the existing v5.5.2 SPI fixtures; fetched header hashes are preserved in
`evidence/seeed-ee-20261122/sources.json`.

| Signal | EE02 | EE03 | EE04 | EE05 |
|---|---|---|---|---|
| SPI SCK / MOSI | GPIO7 / GPIO9 | same | same | same |
| Main CS / RESET / BUSY / power enable | GPIO44 / GPIO38 / GPIO4 / GPIO43 | same | same | same |
| Second panel CS | GPIO41 | absent | absent | absent |
| D/C | GPIO10 | absent, 16-bit TCON preambles | GPIO10 | GPIO10 |
| MISO | absent | GPIO8 | absent | absent |
| `key1` / `key2` / `key3`, active low | GPIO2 / GPIO3 / GPIO5 | same | same | GPIO2 / GPIO3 / **GPIO8** |
| `boot`, active low | GPIO0 | same | same | same |
| BAT_ADC | GPIO1 | same | same | same |
| ADC divider enable | GPIO6 | GPIO6 | GPIO6 | **GPIO39** |
| Green status LED, active low | GPIO21 | same | same | same |

The button and LED definitions are backed by the official
[SenseCraft header](https://github.com/Seeed-Projects/OSHW-reTerminal-Series-E-D/blob/8e03627d41f63b4133e606cb25c5c81b9d251494/examples/official/SenseCraft_HMI/src/boards/xiao_diy/config.h)
and [initialization source](https://github.com/Seeed-Projects/OSHW-reTerminal-Series-E-D/blob/8e03627d41f63b4133e606cb25c5c81b9d251494/examples/official/SenseCraft_HMI/src/boards/xiao_diy/xiao_diy.cpp).
GPIO inputs are delivered through the ordinary host GPIO path and are readable
from the guest's GPIO_IN register. The green LED is a digital on/off preview;
brightness, current and optical output are not calibrated.

Panel writes require real IO_MUX/GPIO-matrix SCK and MOSI routes, an asserted
physical CS, a driven D/C pin where present, supply enable and released reset.
Both SPI2 and SPI3 can reach the pins when routed correctly. This does not add
an unconditional per-instruction poll. GPIO43 is also native UART0 TX: the
existing steady UART idle-high convention is honored for that shared power
net. Serial bit-level supply ripple is not modeled. An undriven RESET is
treated as released in this digital subset; analog startup/reset settling is
not a hardware calibration claim.

EE03 uses actual IT8951 0x6000 command, 0x0000 data and 0x1000 read preambles.
A held software CS or `SPI_MISC.CS_KEEP_ACTIVE` preserves split transactions.
Releasing CS discards an unfinished transaction. Reads reach the guest only
through MISO GPIO8. Other kits have no modeled MISO conductor and return 0xff.
EE02's two selects independently address the two T133A01 halves.

## Battery model and the EE05 source conflict

The nominal battery supplies 2.0 V at GPIO1 only while the actual ADC enable
net is driven high, corresponding to a 4.0 V battery through equal 10 kΩ
divider resistors and TPS22916 gating. Host voltage/raw ADC overrides remain
authoritative. There is no charger, PMIC, discharge curve, battery-presence
noise, or power-domain analog simulation.

The physical sources are the [EE02 schematic](https://files.seeedstudio.com/wiki/Epaper/EE02/202000224_XIAO_ePaper_Display_Board_EE02_V1.pdf),
[EE03 schematic](https://files.seeedstudio.com/wiki/Epaper/EE03/XIAO_ePaper_Display_Board_EE03_V1.0_SCH_251217.pdf),
[EE04/TRMNL schematic](https://files.seeedstudio.com/wiki/XIAO_Gadget/TRMNL_Kit_Pic/XIAO_ePaper_driver_board_sch.pdf),
and [EE05 V1.11 SCH/PCB archive](https://files.seeedstudio.com/wiki/Epaper/EE05/XIAO_ePaper_Display_Board_EE05_V1.11_SCH&PCB_260420.zip).
EE05's `03 XIAO.kicad_sch` identifies U3 pad16 as D12; the PCB assigns pad16 to
ADC_EN, hence GPIO39. D5/GPIO6 is instead on the external I2C connection.
The V1.11 archive is 741,011 bytes, SHA-256
`4bae8bb3bba1abd0a38919b91f1f9b3fe1ad833a84efa29d1939db3ac70c6be9`,
fetched 2026-11-22T08:53:17.923670Z; the V1.0 schematic agrees.

The pinned SenseCraft source still defines common `ADC_EN=6` without an EE05
override. The simulator deliberately follows the physical GPIO39 net. A
firmware using GPIO6 can therefore report a missing/low battery on EE05.
This is a source/hardware compatibility issue, not a reason to fabricate a
voltage on an electrically unrelated pin. Factory observations must preserve
that outcome.

## Fixed glass profiles

Prefix every suffix below with `seeed-xiao-epaper-`. An EE04/EE05 row denotes
two separate board IDs with their own button and ADC-enable wiring.
Controller names follow the manufacturer compatibility drivers. For example,
the GDEY0213F51's Seeed driver is called JD79676; this is not a silicon-ID probe.

| Adapter + suffix | Controller | Native visible pixels | RAM / protocol distinction |
|---|---|---|---|
| EE02 `p133-sp6` | T133A01 | 1200 × 1600 | dual CS44/CS41, two 600-pixel halves, six colors |
| EE03 `p103-mono` | ED103TC2 glass + IT8951 TCON | 1872 × 1404 | full-duplex 16-bit preambles; native landscape |
| EE04 `p073-sp6` | ED2208 | 800 × 480 | packed six-color data |
| EE04/EE05 `p0154-mono` | SSD1681 | 200 × 200 | byte-addressed X, active-high BUSY |
| EE04/EE05 `p0213-mono` | SSD1680 | 122 × 250 | **128 storage pixels**, six hidden columns |
| EE04/EE05 `p0213-quad` | JD79676 | 122 × 250 | **128 storage pixels**, 2-bit B/W/Y/R wire colors |
| EE04/EE05 `p0213-flex` | UC8151D | 104 × 212 | three-byte TRES, seven-byte partial window |
| EE04/EE05 `p029-mono` | SSD1680 | 128 × 296 | byte-addressed X, active-high BUSY |
| EE04/EE05 `p029-quad` | JD79667 | 128 × 296 | four colors, active-low BUSY |
| EE04/EE05 `p029-flex` | UC8151D | 128 × 296 | three-byte TRES, seven-byte partial window |
| EE04/EE05 `p042-mono` | SSD1683 | 400 × 300 | byte-addressed X, active-high BUSY |
| EE04/EE05 `p0426-mono` | SSD1677 | 800 × 480 | 16-bit pixel X windows; glass X reflection |
| EE04/EE05 `p0583-mono` | UC8179 | 648 × 480 | 81 bytes per monochrome row |
| EE04/EE05 `p075-mono` | UC8179 | 800 × 480 | 100 bytes per monochrome row |

Panel dimensions and drivers come from the pinned
[panel configurations](https://github.com/Seeed-Studio/Seeed_GFX2/blob/556df7cb0b4a097d9b81026a26fd5511f9c934d6/src/panel/configs/Seeed_Panel_Configs.h).
The 2.13-inch flexible panel is GDEW0213I5FD, independently backed by the
[manufacturer-hosted UC8151D datasheet](https://files.seeedstudio.com/wiki/Other_Display/213-flexible/IC%20Driver%20UC8151D.pdf),
SHA-256 `d9fa26cc9c0b6959bffbe1105af5b897fd91fd40abb084be825e70c9fe79895d`.
The current GFX2 UC8151D nine-byte partial-window sequence is not silently
translated into UC8179-compatible behavior; the seven-byte hardware contract
is enforced. The public 2.13-inch flexible product is an EOL part, retained
because the official EE kit selector still lists it.

The small SSD panel configurations' *Breakout* mirror flags do not apply to
EE boards: only `Board_XIAO_ePaper_Breakout` declares
`usesPanelConfigBreakoutMirror`. The 4.26-inch config independently declares
`mirror=true`, so its board preview reflects native X. **Its complete physical
Y orientation is not yet calibrated against the factory image/hardware**:
the existing SSD1677 endpoint has a native gate-to-frame Y reversal. Passing a
geometry or pixel-count test does not establish readable text orientation.
The IT8951 preview accounts for the official 1bpp driver's X-window/row-word
reflection but does not inherit the reTerminal E1003's portrait enclosure
rotation. The 2.9-inch kits likewise expose native 128 × 296 pixels even where
the shop lists the same panel in landscape orientation.

Frames change only after valid guest RAM writes, activation and completion of
a bounded digital BUSY interval. Optical image retention is preserved across
reset and power loss; volatile RAM is separately reset on power loss. These
models do not claim measured refresh duration, ghosting, analog waveforms,
OTP contents, temperature response, or every scan/LUT mode. JD796xx currently
accepts the documented native PSR direction; unsupported directions do not
fabricate frames. No SD transport is attached solely because common firmware
defines generic SD pins. I2C PMIC, charger and sleep/wake circuits remain
unmodeled. An attached panel's BUSY pin keeps the model's digital idle level
when supply is removed; its unpowered high-impedance/leakage/pull network is
not simulated. Power gating here constrains command acceptance and volatile
RAM, not every electrical state. Bare adapters never drive BUSY at all.

## Verification and factory execution

```sh
cargo test -p esp32s3 --test seeed_ee
cargo clippy -p esp32s3 --all-targets -- -D warnings
```

Both passed on 2026-11-22. The 12 integration tests drive actual S3 MMIO,
not direct panel APIs. They cover all 25 registrations and frame bounds,
bare-board absence of frame/BUSY/SPI devices, all four key maps, GPIO39 versus
GPIO6 battery gating and host override precedence, LED routing, wrong
clock/MOSI/CS, power/reset, both BUSY polarities, split TCON transactions and
unwired MISO, the T133A01's independent halves, small-panel address formats,
122-visible/128-storage row integrity, four distinct wire colors, flexible
panel partial-window length, and 648-pixel UC8179 row stride.

Official SenseCraft 1.1.5 publishes 17 EE targets with four segments at
0x0, 0x8000, 0xe000 and **0x90000**. The partition table defines 5 MiB app0 at
0x90000, app1 at 0x590000 and 5 MiB SPIFFS at 0xa90000. The manifest supplies
no SPIFFS data image; the simulator must preserve an erased filesystem rather
than insert missing assets. Eight other official combinations do not yet have
a retrieved runnable default image. Board/protocol tests are not factory-UI
verification.

`tools/probe-seeed-ee.py` can reproduce native ROM runs from an explicit
download/layout receipt containing full segment SHA-256 values. It validates
every byte count and hash before loading, never overwrites an earlier output
directory, and records timeouts, no-frame results, instruction counts and
unredacted-output hashes. It does not infer compatibility from process exit.

```sh
python3 tools/probe-seeed-ee.py \
  --cli target/release/esp32sim \
  --rom "$ESP32SIM_ROM_DIR/esp32s3_rev0_rom.elf" \
  --layouts /path/to/official-ee-layouts.json \
  --output target/seeed-ee-factory-run --seconds 6
```

The [production-worker checkpoint](evidence/seeed-ee-20261122/README.md) retains
all 17 factory ROM starts in six-second windows and two representative
15-second runs from fresh storage. Every image reached application and e-paper-driver
initialization. The initial windows ended during the firmware's SD retries
with no frame. At 15 seconds EE04-P075 and EE05-P075 reached application-view
and Wi-Fi initialization, reporting 63% and 0% battery respectively, still
without a completed display frame. This exercises the GPIO39/GPIO6
distinction in the real firmware; it does **not** establish a working default
factory UI or a low-battery warning image. The 17 default-image compatibility
entries remain partial. No blind longer-window sweep replaced these results.

Raw console/receipts contain synthetic provisioning and storage identifiers
and are excluded from public evidence. Original hashes, byte counts, exact
guest time/instructions and identifier-free markers are retained. The 4.26
Y-orientation question remains unresolved because the factory runs produced
no frame to inspect; the SPI integration tests do not answer that question.

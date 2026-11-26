# Exact classic M5Stack controller runtime additions

Five independent product identities now have board models: StickC-Plus SE K016-P-SE,
StickC-Plus2 K016-P2, black M5Stick OLED K016-B, original Paper K049 and Paper COMM
K049-C. Their factory entries retain their original identities and unverified status.

| SKU | Runtime | Physical digital subset | Capacity |
| --- | --- | --- | --- |
| K016-P-SE | `stickc-plus-se` | ST7789V2 SPI13/15/5, DC23/RST18; A37/B39; AXP192 PEK and LCD rails; BM8563; no IMU response | 4 MiB Flash; no simulated PSRAM; PICO-D4 |
| K016-P2 | `stickc-plus2` | ST7789V2 SPI13/15/5, DC14/RST12/BL27; A37/B39/C35; MPU6886/BM8563 on21/22; no AXP192 | 8 MiB Flash;2 MiB PSRAM; PICO-V3-02 |
| K016-B | `m5stick-oled` | SH1107 page RAM through SPI18/23/14, DC27/RST33; button35; blue LED19; no gray-version IMU | 4 MiB Flash; no simulated PSRAM; WROOM32 |
| K049 | `m5paper-original` | IT8951 960×540 RAM,540×960 glass; SPI14/12/13/15, BUSY27, EPD power23; keys37/38/39; GT9110x14 on21/22/IRQ36; BM8563; SD SPI CS4 | 16 MiB Flash;8 MiB PSRAM |
| K049-C | `m5paper-comm` | Independently sourced COMM pin table uses the same IT8951/key/GT911/RTC/SD electrical routes | 16 MiB Flash;8 MiB PSRAM |

SE reuses the implemented Plus controller protocols while explicitly removing the
MPU6886 endpoint and rejecting IMU environment changes. Plus2 is a separate board:
its LCD rails and key C do not go through a fictional PMIC. HOLD4 is observed only;
MCU shutdown, wake and analog battery ADC38 are not implemented. Its eFuse package
code is6 (PICO-V3-02); SE remains package5 (PICO-D4).

OLED has a separate SH1107 command parser,128×128-bit page RAM,64×128 presentation,
column/page address, display on/off, inverse/all-on and scan controls. Arguments to
commands such as0xD3 remain on the command lane (DC low). No ILI934x/DCS command
substitution is used. The source U8x8 configuration supplies the nominal segment,
COM and offset baseline; no physical scan-direction/optical calibration is claimed.
The exact black SKU does not inherit the gray MPU9250. IP5306, IR and buzzer remain
outside this subset.

Paper variants retain separate board names, product storage namespaces and manifest
matching. The existing IT8951 engine is reused only after comparing exact product
pin tables. Current documentation calls G37 Right/G39 Left; older M5EPD headers
call G39 Right/G37 Left. This runtime retains current-document naming and exposes
the literal GPIOs. Paper GT911 is a single-contact register-scanning subset with
0x814E ready/count,0x8150 coordinates and explicit clear/rescan. It does not emulate
two contacts, gestures or calibrated interrupt timing. The SD endpoint requires
actual shared-bus routes, CS4 selection and idle clocks; SD traffic cannot draw.
SHT30, EEPROM, battery/hold shutdown, RTC alarm/wake and optical LUTs remain absent.
These common touch/SD improvements also apply to the existing Paper v1.1 board.

`esp32/tests/m5_classic_paper.rs` executes actual LX6 s32i/l32i through IO_MUX,
GPIO-matrix, SPI/I2C MMIO: pixels, input levels, absent devices, wrong pin routes,
package IDs, GT911 clear/rescan and SD CMD0 were checked. All67 board unit tests
and all6 integration tests passed. The separate public runtime/production
Worker/WASM harness is `tools/platform-m5-classic-completion-worker-test.mjs`; it
loads original SRAM diagnostics as bootloaders through the real mask ROM, with
normal partition/app manifest validation. It never patches a framebuffer from the
host. The first five-product central Worker result is retained in `evidence/m5-classic-completion-runtime/worker.json`. The three additional products and both thermal modes pass on central batch020; see `followup-worker.json`.

No manufacturer factory-image compatibility, browser WebGL/material acceptance or
physical hardware acceptance is inferred from these tests. Existing model geometry,
source-photo limitations and factory-first selection remain separately documented.

## Original StickT, T-Lite and BALA old FIRE

| SKU | Runtime | Source-backed digital subset | Capacity |
| --- | --- | --- | --- |
| K016-T | `stickt-original` | Original ST7789 SPI13/15/5/DC23/RST18; A37/B39; AXP192 and MPU6886; Lepton CCI0x2A on21/22, VSYNC38, reset26 and VoSPI2/25/0 | 4 MiB Flash; no simulated PSRAM; original documentation links PICO-D4 datasheet |
| K126 | `tlite` | StickC-Plus host plus MLX90640-BAA0x33 on HAT SDA0/SCL26; 832-word EEPROM/frame and status/control through real I2C | 4 MiB Flash; no simulated PSRAM; PICO-D4 |
| K014-B | `bala-fire-original` | Old ILI9341 with240×320 native RAM and fixed320×240 glass; SPI18/23/14/DC27/RST33/BL32; A39/B38/C37; SD SPI CS4/MISO19 and10 GRB LEDs RMT15 | 16 MiB Flash;4 MiB PSRAM |

The historical manufacturer `m5-docs/docs/en/core/m5stickt.md` still identifies
**K016-T**,4MB Flash,ST7789,MPU6886,AXP192 and48×30×29mm. This avoids the current
M5StickT URL's redirect to StickT2. Its SH200Q alternative batch is not emulated.
Original `M5-StickT` source independently confirms display, buttons and Lepton
routes; no T2 factory-image identity or execution verdict is copied. The encoder
MCU0x30, RTC, microphone, long power hold, complete optical/FFC calibration and
physical radio are outside this subset.

The K016-T photo-derived model has been rebuilt to a30×48×29mm envelope. The old
25.2mm depth was an estimate. Main-shell27.87mm depth is still an estimate derived
from the official overall envelope after subtracting estimated button/lens
protrusions. No manufacturer assembly was stretched: the existing independent
photo approximation is regenerated. Its source photos, open mounting-ear holes,
UNKNOWN board and independent framebuffer are retained. Node/Three checks measure
30×48×29.00000023mm and offline reconstruction reproduces both geometry and model
metadata byte-for-byte. This is not a1:1 photograph or real-device fit claim.

T-Lite's manufacturer `command_processor.cpp` fixes SDA0/SCL26. Its `mlx90640.cpp`
fixes the16-bit big-endian word protocol,832-word EEPROM at0x2400,832-word frame at
0x0400, status0x8000 bit3 ready/bit0 subpage, and rate control0x800D. Sensor scenes
sample a32×24 array from a160×120 normalized hotspot space. A ready frame remains
coherent until cleared; the next virtual-cycle scan obeys the selected0.5–64Hz
nominal rate. EEPROM is read-only and explicitly virtual; its identifier is SIM,
not a captured sensor serial or a claim of physical calibration.

The published nominal EEPROM avoids missing/zero calibration coefficients. An
independent host check compiles the unmodified pinned manufacturer extraction and
conversion struct against that vector: nominal Vdd3.3V, virtual ambient25.111328°C,
and pixel8000→9000 changes manufacturer Q7 output11453→12714. These numbers test
that the synthetic data is consumable by real driver math; they do not calibrate
the sensor. Reproduce with `tools/check-tlite-virtual-calibration.py` and the two
retained source gzip files. The original LX6 thermal guest separately reads actual
I2C/VoSPI bytes and sends its own SPI pixel commands; host input never replaces LCD
pixels or a temperature result.

BALA retains the exact old16/4/ILI9341 identity. Its native240×320 controller RAM is
transposed to the physically horizontal glass; it is not the newer320×240
ILI9342C identity. The known controller host is implemented. The robot chassis,
motor0x56, encoder, MPU6050 balance loop and mechanical dynamics remain unmodeled;
they are not represented by a frontend animation. IP5306, DAC audio and microphone
are also outside this controller subset.


The follow-up native run passes69 board tests and7 LX6 MMIO tests. The zero-delay
GT911 touch guest now passes the public runtime and unmodified production Worker
for both original Paper and COMM: white frame1 → four black pixels frame2 → white
frame3. Every other pixel is compared too. The guest reads ready bit7 before count,
ignores STATUS=0 after ACK, and consumes a ready zero-contact report as UP.

The retained earlier negative was a harness deadline, not a missing IT8951 update.
Read-only observation found the four black bytes in controller RAM and eight black
bytes in its optical RGB565 frame while the host still held frame1. Paper publishes
at8Hz of **guest** time; shared-host speed measured0.008–0.017×, so the old5-second
wall deadline could expire before the next125ms guest publication. Outbound-frame
tracing then showed white/black/white at approximately30M/60M/90M reference cycles,
with no pending backpressure and zero outstanding credits. The exact same7040-byte
guest image (`34314ebafa53ec6b6ac06294334eb16f02c933345cf2fa7c8a62fd84940b2972`)
passes when the harness waits for a fresh public frame number and the pixel predicate
under a30-second failure deadline. No guest delay, production Worker modification,
frame injection or runtime-engine change was adopted. Final acceptance uses the
original production Worker, without memory scanning or debug logging.

The original negative source remains in `paper-touch-negative.json` and its retained
gzip inputs. `paper-touch-resolution.json` records the diagnosis and outbound trace;
`paper-touch-worker.json` records both products through mask-ROM boot, real I2C/SPI
MMIO and public RGBA capture on batch018. These checks do not establish manufacturer
factory-image compatibility, physical optical timing or browser WebGL acceptance.

The same K049 touch guest also passes with the production Worker explicitly setting
`jit:false` (31.3seconds), retained in `paper-touch-interpreter-worker.json`. Use
`M5_CLASSIC_INTERPRETER=1` for this narrow execution-mode check; default runs still
use the normal JIT. Both modes produce the same complete-frame hashes.

Central batch020 completes all five follow-up Worker modes in27.1seconds: original
StickT, T-Lite and old BALA button-to-LCD; original StickT CCI/VoSPI-to-LCD; and
T-Lite I2C-to-LCD. The public runtime loads each original guest through mask-ROM
boot, then observes actual SPI-drawn red→green→red pixels and complete-frame hashes.
Both thermal guests emit exactly BASELINE→CHANGED→BASELINE, driven by host raw-scene
changes, rather than unconditional drawing or a frontend animation.

The first thermal diagnostic attempt is retained in
`followup-probe-corrections.json`. Its T-Lite guest had left the nominal2Hz sensor
control unchanged, so a30-second wall deadline arrived at only0.42guest seconds,
before the correct0.5second sensor deadline. The guest now configures32Hz through
real0x800D I2C writes, matching the pinned manufacturer's initial rate. The original
StickT guest also now accepts VoSPI packet-ID0 only and reads its left-edge pixel;
sampling odd packets at x80 had crossed the hotspot and produced extra serial state
changes. A strict three-report assertion rejects that earlier broad-sampling case.
These are diagnostic guest corrections; no runtime model or host frame path changed.
The final positive receipt is `followup-worker.json`.

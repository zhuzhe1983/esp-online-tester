# C5 display transport and Waveshare LCD 1.47

This increment is a new C5 implementation. It does not route C5 MMIO through a
C6 SoC. The original SRAM diagnostic now produces a 172×320 framebuffer,
responds to BOOT, and writes/reads an SD block through guest SPI commands. The
Waveshare factory application has not reached its entry point under the actual
rev100 ROM; the separate ROM limitation remains.

## Physical board and source boundary

`waveshare-c5-lcd147` means the hardware documented in
`waveshareteam/esp32-c5-lcd-1.47@acf81d83f8cc580fb8bc4f3f13e7332ab6b1332d`
and registry BSP `waveshare/esp32_c5_lcd_1_47` 1.0.0. Retained originals and
actual download receipts are in
`docs/research/waveshare-c5-lcd147-sources-2026-11-23/manifest.json`.

| Item | Source-backed board contract |
| --- | --- |
| Physical memory | 4 MiB embedded flash, no onboard PSRAM shown/listed; independent of the diagnostic's allocation |
| Crystal | Schematic U4 crystal network explicitly marked 48 MHZ |
| LCD | ST7789; 172×320 visible pixels within 240×320 GRAM; columns 34…205, rows 0…319 |
| LCD SPI2 | Clock GPIO7, data GPIO6, chip select GPIO23, D/C GPIO24, reset GPIO26 |
| Backlight | GPIO10, active high; BSP 5 kHz LEDC with 10-bit timer1 |
| SD | Shared clock7/data6; MISO5, chip select4 |
| BOOT | Schematic GPIO28 → BOOT net and switch, active low |
| RGB | GPIO8, BSP explicitly selects RGB byte order; digital RMT bit callback is wired, exact C5 RMT clock timing is not validated here |
| Absent | Touch, audio codec, RTC and I/O expander; none are invented |

The official product/reference calls the package `ESP32-C5FH4`, while the
schematic says `ESP32-C5HF4`. That spelling conflict is retained. Arduino uses
landscape rotation3; the BSP portrait contract has no swap or mirror. The
emulator's native framebuffer follows that portrait BSP contract. RGB/BGR and
inversion are interpreted relative to the BSP's RGB565/MADCTL0/INVON setup;
this is a software reference, not a physical color calibration. Native panel
analog gamma and porch behavior are outside the model.

## C5 transport

Fixed primary evidence: ESP-IDF v5.5.4 commit
`735507283d5b2f9fb363a1901172dbd9e847945d`; complete original register/HAL
responses are retained in `docs/research/c5-transport-sources-2026-11-23`.
`generate_transport_registers.py` deterministically generates reset/mask
metadata. Metadata alone never enables an unimplemented engine.

SPI2 is at `0x60081000`; its matrix signals are CLK56, D58, Q57, WP60, HD59,
CS0=61 and CS1…5=101…105. Native mux2 is CLK6/D7/Q2/WP5/HD4/CS0=10.
The independent C5 physical IO_MUX addressing from the memory increment is
retained. GPIO inversion and output-enable rules pass through physical routes.

Accepted USR captures phase widths, lengths, four lane routes, inputs,
hardware/software selects and `keep_cs`. A second USR while busy does not
replace it. The C5 QPI bit3 is rejected explicitly. CPU FIFO overflow, partial
bytes, unsupported DDR/octal/configuration/slave modes and an unknown source
clock do not silently produce a successful payload. The current exact clock
subset is the explicit 48 MHz crystal through PCR and SPI dividers. Selecting
PLL160, PLL120 or RC_FAST without a modeled source is rejected. The diagnostic
uses 24 MHz SPI; the manufacturer's 40 MHz PLL-based setup is not verified.

An accepted transfer remains busy for its programmed serial clock count at
the selected source frequency. This is a time floor only. FIFO prefetch, DMA
setup, bus arbitration, CS setup/hold and changes to D/C during a transaction
are not simulated at individual edge resolution. Clock disable/reset cancels
pending byte-level delivery. Functional DMA progress occurs when the payload
is gathered, before the SPI time floor expires; its EOF interrupts are not a
claim about precise FIFO pop timing.

AHB-GDMA is at `0x60080000`, with **three** channel pairs and peripheral
trigger **1** for SPI2. LINK control contains IN bits1/2/3 and OUT bits0/1/2;
full 32-bit descriptor addresses live separately at `+0x3ac…0x3c0`. No old
20-bit C6 LINK-address alias is accepted. Owner checks, partial-descriptor
progress, optional TX owner writeback, RX actual byte count, EOF/error flags,
and held reset are modeled. Cyclic/no-progress walks have a finite shared
descriptor budget. Unknown FIFO CPU push/pop, ETM, memory-copy engines and
extra channels are unavailable.

DMA accesses memory through the C5 memory map, including initialized Quad
PSRAM and valid MMU entries. This is not an address mask into SRAM. Bounds,
MMU invalidation, write protection, absent memory and unsupported external
bursts fail. A faulty TX request does not deliver a partial fake board frame;
already consumed descriptor progress is retained and descriptor error is
observable. DMA failure does not create a CPU load/store exception. RX writes
the actual board response through its own full-address descriptor chain.

LEDC is at `0x60007000`, with six channels and PCR clock/reset/power controls.
Static duty latches at the real configured timer wrap in the functional
clock model. Output changes have one scheduled deadline; no PWM query runs
on every instruction. Only static-duty-end IRQs currently have an autonomous
deadline. Timer-overflow/ETM/capture IRQ requests, hardware fades and unknown
clock rates are unsupported. No existing S3/C3/C6/P4 adapter was changed to
call the new C5 path.

## Actual diagnostic scope

`firmware/c5-lcd147-probe/` contains the original C and startup/linker source,
the rebuilt ELF and the ESP image. It runs through `Machine::load_input(App)`
and explicit SRAM `boot_app`, with no ROM hook or guest-function override.
The image uses RV32IMAC plus standard CSR/fence instructions, chip ID0x17 and
MP revision100…199. Source flash allocation is 4 MiB; the guest executes from
SRAM and does not validate flash transactions or a flash bootloader.

The host loads an 8 MiB disposable SD image through the public
`Machine::load_input(SdCard)` path. The C5 SoC delegates to the actual board
socket, while a bare board rejects attachment. The guest sends CMD0, CMD8,
CMD55/ACMD41, CMD24 and CMD17, writes LBA32, and compares all512 returned
bytes. This is SPI SD protocol/readback validation, **not a mounted filesystem**.

The guest initializes the LCD, paints a nonuniform RGB565 pattern through
real C5 SPI2/GDMA, and polls the physical BOOT input. A named `gpio boot`
script changes the contact. Observed results in the accompanying receipt:

- 172×320, 3,362 colors; three RAMWR full frames.
- Quiet control:0 changed pixels.
- BOOT down:35,840 changed pixels (140×256 rectangle).
- BOOT up: exact framebuffer restoration.
- SD:512 bytes written and read, one block each; CMD0=01.
- 31,000,000 retired/dispatched instructions;0 exceptions,0 resets.

The eight transport tests additionally exercise an independent 8 MiB PSRAM
fixture for DMA TX/RX and MMU invalidation. That fixture is **not** the LCD147
board and does not change its physical capacity. Exact byte timings, USB,
Wi-Fi/BLE, RGB wave timing, physical SD reliability, factory UI and power
sequencing have not been validated by this diagnostic.

Reproduce with the toolchain receipt in `c5-memory-runtime.md`, then:

```sh
sh firmware/c5-lcd147-probe/build.sh
CARGO_BUILD_JOBS=1 CARGO_PROFILE_RELEASE_LTO=false cargo test -p esp32c5 --release --test transport
CARGO_BUILD_JOBS=1 CARGO_PROFILE_RELEASE_LTO=false cargo run -p esp32c5 --release --example lcd147_probe -- firmware/c5-lcd147-probe/probe.bin /tmp/c5-lcd147-result
```

This board is an experimental source-wired runtime with an original project
diagnostic. Its official factory manifest must remain a separate unverified
trial until the actual ROM/bootloader/application chain succeeds.

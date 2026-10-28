# ESP32-P4 (RISC-V) — draft

The first chip here with two RISC-V cores: silicon **rev < 3** (v0.x, v1.x), as on the Waveshare
ESP32-P4-WIFI6 (v1.0).
Unmodified ESP-IDF firmware built for rev < 3 boots from the real mask ROM, through the real
2nd-stage bootloader, into FreeRTOS on one or both cores, reaches an SD card in the board's socket
through the SDMMC host ("SD card" below), presents it as a USB mass-storage device to
a host written in Rust on the high-speed USB-OTG port ("USB" below), and
reaches WiFi through an emulated ESP32-C6 on SDIO running unmodified ESP-Hosted slave firmware,
the two chips in lock-step in one process ("The co-processor" below). Silicon **rev ≥ 3**
(v3.x), what a stock `idf.py set-target esp32p4` builds for, has its own model, so far up to
hello_world on one and both cores ("Revisions" below). This page describes the chip and its
peripherals.

```sh
cargo build --release
FW=web/wasm/fw/public
./target/release/esp32sim-p4 --flash-mb 2 --bootloader $FW/p4-hello-bootloader.bin \
    --ptable $FW/p4-hello-ptable.bin --app $FW/p4-hello_world.bin --max-seconds 27
```

prints the ROM banner, the bootloader's partition table, the IDF startup log, `Hello world!`, the
countdown, and reboots through the ROM on `esp_restart()` (goldens `hello-p4`, `hello-p4-reboot`):

```
ESP-ROM:esp32p4-20230811
Build:Aug 11 2023
rst:0x1 (POWERON),boot:0x30f (SPI_FAST_FLASH_BOOT)
I (6) boot: ESP-IDF v5.5.3 2nd stage bootloader
I (7) boot: chip revision: v1.0
I (7) boot: efuse block revision: v0.3
I (35) cpu_start: cpu freq: 360000000 Hz
I (36) main_task: Calling app_main()
Hello world!
This is esp32p4 chip with 2 CPU core(s), , silicon revision v1.0, 2MB external flash
Restarting now.
rst:0xc (SW_CPU_RESET),boot:0x30f (SPI_FAST_FLASH_BOOT)
Core0 Saved PC:0x4ff02328
```

The images are IDF v5.5.3's hello_world for rev < 3 (`examples/hello_world-p4`): `p4-hello-*`
single-core, which the browser build runs too (`web/wasm/fw/p4-hello.json`), and `p4-hello-smp*`
both cores (golden `hello-p4-smp`, where both cores take their own systimer tick and cross-core
interrupts), which `tools/fetch-p4-firmware.sh` pins by SHA-256. `esp32sim --chip p4` is the same
binary; the boot mode defaults to `rom` and the console to UART0. The mask ROM ELF is
`esp32p4_rev0_rom.elf` (ECO0, from ESP-IDF's esp-rom-elfs or `tools/fetch-rom-elfs.sh`). No ROM is
published for the ECO2 silicon on v1.x boards. ECO0's jump table matches every entry of IDF
v5.5.3's rev < 3 `esp32p4.rom.ld`, so images built for v1.x link against it; the first two
console lines are ECO0's, not the board's, and so is the length of the heap region below the
ROM's own data, which IDF takes from the ROM's `ets_rom_layout` (`_dram0_rtos_reserved_start` is
0x4FF3FBA4 in the ECO0 ELF).

## The core

`Cpu::new_esp32p4()` is RV32IMAFC with the P4's EXT_ILL CSR, the Zicntr counters and CLIC mode;
the P4 builds core *i* with `hartid` *i*. Nothing of it is enabled on the C3 and C6 cores.

- **Counters.** `cycle`, `instret` and their high halves (0xC00/0xC02/0xC80/0xC82) read the
  `mcycle`/`minstret` counters. IDF on the P4 reads the cycle count with `rdcycle`
  (riscv/include/riscv/rv_utils.h); the bootloader's random-number loop waits on it.
- **CLIC mode** (RISC-V CLIC specification, "CLIC Interrupt Operation"; IDF v5.5.3
  riscv/vectors_clic.S, vectors.S). With `mtvec` in mode 3 the core takes the candidate the SoC
  presents (CPU line, 8-bit level, SHV) when its level is strictly above `mintstatus.mil` and
  `mstatus.MIE` is set. The SoC has applied the rev < 3 threshold, which is the memory-mapped
  `CLIC_INT_THRESH[31:24]`. On a take, `mcause` is 0x8000_0000 | MPIL (the previous level) << 16 |
  ID, with MPP and MPIE read through from `mstatus`; the level becomes the interrupt's. A
  hardware-vectored interrupt jumps to the word at `mtvt + 4*id` (CSR 0x307), a bus read made
  with MINHV set. Non-vectored interrupts and every exception enter at the 64-byte-aligned
  `mtvec` base, an exception recording the current level in MPIL. `mret` restores the level
  from MPIL. `mintstatus` is CSR 0x346 on rev < 3. Any presented candidate ends a WFI, takeable
  or not (a WFI may resume for any reason).
- **CLINT mode.** In `mtvec` modes 0 and 1 the core takes no CLIC candidate and does not count one
  as pending, so it idles: interrupts there would be `mie`/`mip`'s, nothing raises `mip` on this
  chip, and the ROM runs in mode 1 with `mie` = 0. `mcause` reads as written, without the
  aliases, and the ROM takes its exceptions at its base.
- **Not modelled:** a fault on the vector-table read is an instruction access fault at the
  table entry with MINHV left set, but `mret` does not resume the fetch as the specification has
  it, and MPIE/MPIL then describe the half-entered handler. IDF's table is in mapped RAM.
- **Harts.** `mhartid` reads the core's index and survives a reset. An LR reservation is checked
  with the bus at the SC (`Bus::reserve`, `reservation_held`): the P4 bus keeps one per hart, and
  any store by the other hart to the reserved word (word, half, byte, AMO, SC) breaks it. A trap
  breaks the core's own reservation, so an SC after a handler cannot overwrite the handler's
  store (IDF's vectors issue no SC of their own). The machine tells the bus which core it is
  about to run (`Soc::select_core`), which is how each core sees its own CLIC block at
  0x20800000 and the other's at 0x20810000.

The CLIC and the interrupt matrices are registers (`periph::Clic`, `periph::IntMatrix`): sources
reach each core's CLIC through its matrix; IE, the attributes and the levels read back, the control
words take byte writes as the four byte registers they are, and an edge-triggered ID clears when it
is taken hardware-vectored (`Soc::interrupt_taken`) or when software writes 1 to IP and nothing
else: IDF acknowledges with a 32-bit `REG_SET_BIT` of IP, and its SHV/TRIG updates are
read-modify-writes of the same word that write a pending IP back as 1, which must not consume the
edge.

The core's P4 paths cost the other chips nothing: each is behind a flag only
`Cpu::new_esp32p4` sets (`misa.F`, `has_ext_ill`, `user_counters`, `clic`), the reservation hooks
are `Bus` defaults that a single-hart bus inlines away, and the machine's `select_core` and
`interrupt_taken` are empty `Soc` defaults. The hello demos keep their instruction counts.

## Revisions

ESP-IDF v5.5.x builds for rev ≥ 3 unless `CONFIG_ESP32P4_SELECTS_REV_LESS_V3=y` (it defaults to
n, IDF v5.5.4 `components/esp_hw_support/port/esp32p4/Kconfig.hw_support`, whose help calls the
two families incompatible), and they differ in ISA, interrupt controller, memory layout and ROM
(the table at the end of this section). Each is an `esp32p4::Revision`: `RevLt3`, the efuses
reporting v1.0, on the ECO0 ROM `esp32p4_rev0_rom.elf`, and `RevGe3`, the efuses reporting v3.1
(the lowest IDF v5.5.4 builds for by default, `ESP32P4_REV_MIN_301`), on the ECO5 ROM
`esp32p4_rev300_rom.elf`. `tools/fetch-rom-elfs.sh` fetches both from esp-rom-elfs 20260528.

Bootloaders and apps record the revisions they run on in their image header
(`min_chip_rev_full`, `max_chip_rev_full`; `esp_image_header_t`, IDF v5.5.3
`components/bootloader_support/include/esp_app_format.h`): 100 to 199 with the examples'
`CONFIG_ESP32P4_SELECTS_REV_LESS_V3=y` and `CONFIG_ESP32P4_REV_MIN_100=y`, 301 to 399 by default.
esp32sim-p4 (and `--chip p4`) runs the rev ≥ 3 model when the bootloader asks for 300 or more,
the app when there is no bootloader, or the bootloader inside `--flash-image`
(`esp32p4::built_for_rev3`), and the rev < 3 model otherwise. A stock build runs as it is
(`examples/hello_world-p4`, `sdkconfig.defaults.rev3`; goldens `hello-p4-rev3` and, both cores
from `sdkconfig.defaults.rev3-smp`, `hello-p4-rev3-smp`; `tools/fetch-p4-firmware.sh` fetches
the images):

```sh
./target/release/esp32sim-p4 --flash-mb 2 --bootloader $FW/p4-hello-rev3-bootloader.bin \
    --ptable $FW/p4-hello-ptable.bin --app $FW/p4-hello-rev3_world.bin --max-seconds 12
```

```
ESP-ROM:esp32p4-eco5-20250430
Build:Apr 30 2025
rst:0x1 (POWERON),boot:0x30f (SPI_FAST_FLASH_BOOT)
load:0x4ffac2c0,len:0xe88
I (7) boot: chip revision: v3.1
I (34) cpu_start: cpu freq: 400000000 Hz
I (34) heap_init: At 4FF30340 len 0008AC80 (555 KiB): RETENT_RAM
Hello world!
This is esp32p4 chip with 2 CPU core(s), , silicon revision v3.1, 2MB external flash
```

`--coproc` picks the revision the same way ("The co-processor"). The browser build takes it from
the board name: `esp32p4` is rev < 3, `esp32p4-rev3` rev ≥ 3. An image for the other revision
stops the run before the first instruction: a rev ≥ 3 app behind a rev < 3 bootloader, a rev < 3
app behind a rev ≥ 3 bootloader, or, in the browser, an image for the other board name:

```
[emu] build/hello_world.bin was built for ESP32-P4 rev ≥ v3.0 (min_chip_rev_full=301); this model is rev < 3 (v1.0). Rebuild with CONFIG_ESP32P4_SELECTS_REV_LESS_V3=y, or run a bootloader and app both built for rev ≥ 3.
```

The check (`Soc::check_image`) runs on every bootloader and app the CLI loads (`--bootloader`,
`--app`, `--flash-at`, and those inside `--flash-image`) and on the browser build's loads, and
only on headers whose chip ID is the P4's.

What the rev ≥ 3 model changes is the core's CLIC, which is the specification's
(`Cpu::new_esp32p4_rev3`, `Cpu::clic_std`): the threshold is the `mintthresh` CSR, an interrupt
being taken only above both it and the current level, and `mintstatus` reads at 0xFB1 (IDF v5.5.4
`riscv/include/esp_private/interrupt_clic.h:50-66`). The model keeps the memory-mapped threshold
as well; IDF's rev ≥ 3 builds set `mintthresh` instead (`INTTHRESH_STANDARD`). Without the
change, the threshold left to the SoC as on rev < 3, the one-core hello_world still runs, but the
two-core boot gets no further than `cpu_start: Multicore app` (1008 exceptions in 12 emulated
seconds, both cores ending in `ets_delay_us`). The core's EXT_ILL flags every FP instruction taken
with the FPU off (`Cpu::ext_ill_all_fp`): rev ≥ 3 fixed the EXT_ILL bug of rev < 3, which left
FLW, FSW, their compressed forms and the FP CSRs unflagged, and IDF's rev ≥ 3 builds no longer
decode `mtval` for them (v5.5.4 `components/riscv/vectors.S:279-280`). Without that, the sdmmc
example panics with an illegal instruction at the first `c.flw` in `sdmmc_card_init`. The ROM
leaves the bootloader its stack at
0x4FFBCFC0 (`SOC_ROM_STACK_START_REV2`, IDF v5.5.4 `include/soc/soc.h`). The rest is the ROM's,
the bootloader's and the app's own doing: on rev ≥ 3 the bootloader loads at the top of L2MEM and
the app above the L2 cache at its bottom (its first RAM segment at 0x4FF20000), and L2MEM is RAM
throughout here.

So far it is the rev < 3 model with those changes. Within the headers of the blocks the model
implements, no register moves between IDF v5.5.4's `soc/esp32p4/register/hw_ver1` and `hw_ver3`;
the changes are added bits and registers, a few removed bits and the eFuse system words, none in
a field the model gives a meaning to ([the comparison](evidence/p4-rev3-registers/README.md)).
One register moves between headers, which that comparison cannot see: `WDEV_RND_REG`, the random
number, is LP_SYSTEM's RNG_DATA on rev < 3 and LP_TRNG's (0x50126004, `trng_reg.h`, a header only
`hw_ver3` has) on rev ≥ 3; the model has both. Of the blocks it does
not implement, many differ (DMA2D, PPA, MIPI-CSI's bridge, ISP and PMU among them), and `hw_ver3`
adds 23 headers. hello_world does not reach the differences, and neither do ESP-Hosted with the C6
(golden `hosted-p4-rev3`) and the sdmmc example (golden `sdcard-p4-rev3`); USB has not been run on
rev ≥ 3. The
model runs at 360 MHz on both revisions, where IDF sets rev ≥ 3 to 400 MHz: the systimer and the
timer groups count from the XTAL and keep time, but code that counts CPU cycles against the
400 MHz IDF assumes, a busy-wait on the cycle counter such as `esp_rom_delay_us`, should take
about 11% longer in emulated time than on silicon (not measured).

| | rev < 3 (v0.x, v1.x) | rev ≥ 3 |
| --- | --- | --- |
| `-march` | `rv32imafc_zicsr_zifencei_xesppie` | `rv32imafc_zicsr_zifencei_xesploop_xespv` |
| CLIC threshold | memory-mapped, `0x20800008` bits 31:24 | `mintthresh` CSR (0x347) |
| `mintstatus` | CSR 0x346 | CSR 0xFB1 |
| EXT_ILL for FLW/FSW and the FP CSRs | not flagged (`SOC_CPU_HAS_FPU_EXT_ILL_BUG`) | flagged |
| L2 cache | top of L2MEM | bottom of L2MEM |
| ROM published in esp-rom-elfs | `esp32p4_rev0_rom.elf` (ECO0, `esp32p4-20230811`) | `esp32p4_rev300_rom.elf` (ECO5, `esp32p4-eco5-20250430`) |
| ROM stack for the bootloader | 0x4FF3CFC0 | 0x4FFBCFC0 |
| CPU clock IDF sets | 360 MHz | 400 MHz (modelled at 360 MHz) |
| efuses here | v1.0 | v3.1 |

## SD card

`--sd IMAGE` puts a card holding the image in the reference board's SD socket; `--sd-rw IMAGE`
does the same and writes through to the file, where `--sd` keeps what the firmware writes in
memory and never modifies the file. `--sd-prg-us N` gives the card a programming time, and
`--sd-serial HEX` its CID serial number. A card put in the socket again replaces the one there,
which is how the browser build's load kind 8 swaps images. ESP-IDF v5.5.3's own sdmmc example
(`examples/sd_card-p4`, golden `sdcard-p4`) mounts a FAT32 card through it, writes `hello.txt`,
renames it, reads it back and unmounts:

```sh
./target/release/esp32sim-p4 --flash-mb 2 --bootloader $FW/p4-sdcard-bootloader.bin \
    --ptable $FW/p4-hello-ptable.bin --app $FW/p4-sdcard.bin --sd-rw card.img --max-seconds 1
```

```
I (87) example: Filesystem mounted
Name: SIMSD
Type: SDHC
Speed: 20.00 MHz (limit: 20.00 MHz)
Size: 64MB
CSD: ver=2, sector_size=512, capacity=131072 read_bl_len=9
SSR: bus_width=4
I (87) example: Read from file: 'Hello SIMSD!'
I (87) example: Card unmounted
```

The golden's card is a 64 MiB image the test lays out by hand (`tests/sd_image.rs`: an MBR,
FAT32 from LBA 2048), so its bytes are the same on every host; afterwards the test reads
`FOO.TXT` and `NIHAO.TXT` back from it. A blank image ends in `Failed to mount filesystem`
(the example does not format) and an empty socket in `ESP_ERR_TIMEOUT`, the card's commands
timing out (`sd_card_p4_unformatted_and_absent`).

The second golden, `sdcard-sectors-p4` (`examples/sd_card-p4-sectors`), adds raw transfers of 64
and 128 sectors from DMA-capable buffers: CMD25 and CMD18 with the auto-stop, longer than the
16 KiB the driver's four descriptors hold, so its interrupt handler refills them; a misaligned
buffer the driver splits into CMD17s; a read past the end the driver refuses (0x104); and a card
that programs for 2 ms, which the driver waits out polling CMD13 (`write 0x0 in 2039 us`).

### The host

The SDMMC block (0x50083000) is Synopsys' DesignWare Mobile Storage Host with Espressif's
wrapper from 0x800. It is modelled once, in `esp-periph/src/sdmmc.rs`, for the chips that carry
the core: the ESP32 and S3 have the same registers below 0x200 and a CLOCK register at 0x800
(`SdmmcLayout`); only the P4 is wired. Sources are IDF v5.5.3
`soc/esp32p4/register/hw_ver1/soc/sdmmc_struct.h` and `sdmmc_reg.h`, and what the driver does
with them, `esp_driver_sdmmc/src/sdmmc_host.c` and `sdmmc_transaction.c`,
`hal/esp32p4/include/hal/sdmmc_ll.h`.

A command runs to completion in the register write that starts it (the bus calls
`Sdmmc::run` after any write that left one pending): the response, CMD_DONE, the whole data
phase through the IDMAC, DATA_OVER and the auto-stop. Nothing on the bus takes emulated time;
the devices see it (`Sdmmc::now_ns`, set before every SDMMC access from the device clock) for
their own busy periods. A write's DATA_OVER comes when its last block is on the card, before the
card has programmed it, as on the DesignWare core.

With `--sd-timing` (`SdCardConfig::timed_bus`) the bus takes its time. The data still moves
through the IDMAC in the write that starts the command, but CMD_DONE comes when the command and
its response are through (48 bits, NCR 2, 48 or 136 bits, NCC 8 card clocks; an unanswered one
after the 64-clock response timeout), DATA_OVER when every block is (a start bit, the block's
bits over the slot's width, CRC16 and an end bit on each line, a write's CRC status token, two
clocks between blocks: SD Physical Layer Simplified Specification v9.10 §3.6.1, §4.12), ACD after
the auto-stop's own command, and a write's card programs from the end of its data on, BCI
following when it is done. The card clock is the one IDF computes in `sdmmc_host_get_real_freq`:
HP_SYS_CLKRST PERI_CLK_CTRL01.REG_SDIO_LS_CLK_SRC_SEL (PLL_F160M or the 200 MHz SDIO PLL), divided
by PERI_CLK_CTRL02's SDIO_LS EDGE_L + 1, then by the slot's CLKDIV divider (CLKSRC picks it; n
divides by 2n). The deferred interrupts are raised from the device ticks at their time
(`Sdmmc::next_event_ns`, `advance`), which bound the bus's deadline. The card's read access time
and the IDMAC's pace through the descriptors are not timed: a long transfer's descriptors come
back at once, its DATA_OVER at the end.

| register | modelled |
| --- | --- |
| CTRL 0x00 | controller, FIFO and DMA reset (bits 0-2) clear at once; the controller and DMA resets abort a transfer; INT_ENABLE (4) gates the controller's interrupt; DMA_ENABLE, USE_INTERNAL_DMA (25) read back |
| CLKDIV, CLKSRC, CLKENA, TMOUT, FIFOTH, DEBNCE, USRID, UHS, RST_N, EMMCDDR, ENSHIFT | read back from their reset values; the card clock is not checked |
| CTYPE 0x18 | 1, 4 or 8 lines per slot; a data phase on other lines than the card drives fails every block's CRC (DCRC) |
| BLKSIZ, BYTCNT | block and transfer size, 512 after reset |
| INTMASK, MINTSTS, RINTSTS | MINTSTS is RINTSTS & INTMASK; RINTSTS is write-1-to-clear |
| CMDARG, CMD 0x2C | START (31) stays set until the command is taken; a clock-update command (UPDATE_CLK_REG, 21) is taken at once without CMD_DONE; CHECK_RESPONSE_CRC (8) asks for RCRC; STOP_ABORT ends a transfer, a suspended one too; SEND_AUTO_STOP sends CMD12 after the data |
| RESP0-3 | a short response in RESP0, an R2 in RESP0-3 (bit 0 first), the auto-stop's in RESP1 |
| STATUS 0x48 | FIFO empty, below the transmit watermark, command FSM idle, DAT3 high with a device on the slot, DATA_BUSY from the card at the current time, the index of the last response; reading it changes neither the card nor its time |
| CDETECT, WRTPRT 0x50, 0x54 | the GPIO matrix's card-detect and write-protect inputs of each slot (below) |
| TCBCNT, TBBCNT | bytes moved to or from the card, and through the FIFO |
| VERID, HCON | 0x5432270A and HCON's field reset values |
| BMOD, PLDMND, DBADDR, IDSTS, IDINTEN, DSCADDR, BUFADDR | the IDMAC (below) |
| CARDTHRCTL 0x100 | BUSY_CLR_INT_EN (1): BCI (RINTSTS 13) at the first host access after the card that took a write releases DAT0 (the host keeps no clock to raise it at that moment); a STATUS read that raises it re-derives the interrupt line |
| CLK_EDGE_SEL, RAW_INTS, DLL_CLK_CONF, DLL_CONF 0x800-0x80C | CLK_EDGE_SEL from its reset value; RAW_INTS reads RINTSTS (a guess: the header says only "raw ints"); DLL_CONF reports DLL_CAL_END |

A device answers a command with a response or an `SdCmdError`: no response (RTO with CMD_DONE,
also for a slot with nothing on it), a response with a bad CRC (RCRC when the command asks for
the check) or a malformed one (RESP_ERR); after either error the data phase does not start. A
data error is the device's `SdDataError`: a read that gets no data is DRTO, a block with a bad
CRC either way DCRC (IDF: ESP_ERR_INVALID_CRC), a write that gets no CRC status EBE; DATA_OVER
follows it, as the core reports data-over after a data error, with the IDMAC's CES. The
auto-stop goes out after a data error as well. That is an assumption that only a board can
settle, and IDF depends on it to get the card out of its data state. The interrupt is source
23 (`ETS_SDIO_HOST_INTR_SOURCE`, include/soc/interrupts.h): `CTRL.INT_ENABLE && MINTSTS`, or
TI/RI in IDSTS & IDINTEN with IDINTEN.NI, or FBE/DU/CES with IDINTEN.AI. IDF enables NI, RI and
TI only, so a DU interrupts it through the RI before it.

The IDMAC walks descriptors in memory (`sdmmc_desc_t`, sdmmc_struct.h): chained (CH, DES3) or in
a ring (the next 16 bytes plus BMOD's skip length, back to DBADDR after ER), one or two buffers
each. A finished descriptor goes back to the CPU with OWN cleared and its other bits kept, and
raises RI (reception) or TI (transmission) with NIS unless DIC is set. One the CPU still owns
suspends the transfer with DU and AIS; a PLDMND write fetches it again. An address it cannot
reach is a fatal bus error (FBE, AIS, FBE_CODE). It reaches L2MEM and PSRAM through its MMU, at
cached or uncached addresses (`SOC_SDMMC_PSRAM_DMA_CAPABLE`); caches are not modelled. IDF's
driver keeps four descriptors of up to 4 KiB in a ring and refills them from its interrupt
handler, so a transfer over 16 KiB suspends on DU once per refill, which silicon only does when
the CPU is slower than the card.

**Slots and the socket.** A device hangs on pins, and the routing decides which slot reaches it,
looked up when a command starts (`Peripherals::sd_routes`). Slot 0 is wired to its IO MUX pins,
CLK 43, CMD 44, D0-D3 39-42 (`soc/esp32p4/include/soc/sdmmc_pins.h`): it reaches the socket
while the clock pad selects function 0 (`FUNC_GPIO43_SD1_CCLK_PAD`, io_mux_reg.h). Slot 1 goes
through the GPIO matrix: it reaches a device whose clock pad is a GPIO (function 1) driving
`SD_CARD_CCLK_2_PAD_OUT_IDX` (gpio_sig_map.h). The `--sd` card sits on slot 0's pins, as on the
reference board; IDF's example reaches it through slot 1 and the matrix (`SDMMC_HOST_DEFAULT()`
is slot 1 on the P4), an application configured for slot 0 through the IO MUX. ESP-Hosted's C6
is an `SdDevice` attached at CLK 18, CMD 19, D0-D3 14-17 (`Peripherals::attach_sd`, which
replaces a device already on those pins), which slot 1 reaches the same way ("The co-processor").
CDETECT and WRTPRT read `SD_CARD_DETECT_N_1/2` and `SD_CARD_WRITE_PRT_1/2` (signals 126, 127,
130, 131) through FUNCn_IN_SEL: a pin's level or a constant, inverted if asked. The driver
routes constant low to card detect and inverted constant high to write protect when the board
has neither line, so a card counts as present on both slots.

**Clocks, reset, pins, power.** HP_SYS_CLKRST's SOC_CLK_CTRL1.REG_SDMMC_SYS_CLK_EN (bit 14) and
PERI_CLK_CTRL01/02 (the SDIO_LS clock source, enable and divider edges) read back, which is what
`sdmmc_host_get_real_freq` computes the bus frequency from (160 MHz PLL / 8 = 20 MHz for the
example, / 4 = 40 MHz for ESP-Hosted's link); the clock gate is not checked. LP_CLKRST
HP_SDMMC_EMAC_RST_CTRL.RST_EN_SDMMC (0x5011104C bit 28) resets the host's registers and leaves
what is attached (lp_clkrst_reg.h). IO_MUX is register RAM; the driver only writes it. The card's
on-chip LDO (channel 4, `sd_pwr_ctrl_by_on_chip_ldo.c` through `hal/esp32p4/include/hal/ldo_ll.h`)
is PMU configuration the driver writes and never polls, register RAM here; its `ldo: The
voltage value 0 is out of the recommended range` warning is IDF's own.

**GPIO layout.** The P4 has 57 PIN registers from 0x74, which puts FUNCn_IN_SEL_CFG at 0x158 and
FUNCn_OUT_SEL_CFG at 0x558, four bytes above the C6's (register/hw_ver1/soc/gpio_reg.h).
`periph::GpioP4` moves both arrays down four bytes into the shared model, keeps PIN49-56 and
OUT_SEL49-56 as register RAM, and resets FUNCn_IN_SEL to the header's values (0x3F, constant
high, or 0x3E, constant low). Pin levels stay the shared model's 49.

### The card

`esp-periph/src/sdmmc/card.rs`, after the SD Physical Layer Simplified Specification v9.10:

- **States** (§4.2, §4.3, the transitions of §4.8): idle, ready, ident, stby, tran, data, rcv,
  prg, dis and ina. An R1 shows CURRENT_STATE as the command found the card, READY_FOR_DATA
  cleared while programming (§4.10.1). A command that is not legal in the state, including the
  SDIO commands 5, 52 and 53, is not answered and sets ILLEGAL_COMMAND for the next response;
  the error bits clear with the response that reports them. A card deselected while programming
  goes to dis and from there to stby when programming ends; selected again it returns to prg
  only while it still programs.
- **Identification** (§4.2): CMD0; CMD8 echoes its check pattern for 2.7-3.6 V and stays silent
  otherwise; CMD55 and ACMD41, busy for one poll, then ready with CCS. Without HCS, or without a
  CMD8 since the reset, the card stays busy, as a high capacity card must (§4.2.3). CMD2 and CMD10
  give the CID, CMD3 an RCA (R6), CMD9 the CSD, CMD7 selects and deselects (R1b), CMD4 and CMD15.
- **Registers** (chapter 5): CID from `SdCardConfig`, by default MID 0, OID "ES", product
  "SIMSD", revision 1.0, a serial number derived from the image size and 2024-01; CSD version 2.0
  with C_SIZE from the image,
  TAAC 1 ms, CCC 0x535 (classes 0, 2, 4, 5, 8, 10), TRAN_SPEED 25 MHz or 50 MHz once high speed is
  switched on; both with their CRC7 (§4.5). SCR: SD 3.0x, security 2.00 (3.00 above 32 GiB),
  1 and 4 lines, CMD23. SD status (ACMD13, §4.10.2): bus width, speed class 10, AU 4 MiB. OCR
  2.7-3.6 V, no 1.8 V.
- **Transfer** (§4.3): CMD6 check and switch (§4.3.10; group 1 offers default and high speed),
  ACMD6 bus width, CMD13, CMD16 (blocks stay 512 bytes), CMD17 and CMD18 reads, CMD24 and CMD25
  writes, CMD12, CMD23 block count (for the next CMD18 or CMD25; any other read or write clears
  it), ACMD22 written blocks, ACMD23 and ACMD42 accepted, CMD32, CMD33 and CMD38 erase (erased
  blocks read 0). A start address past the end answers OUT_OF_RANGE and moves no data; a
  multi-block transfer that runs past it stops there.
- **Time.** The card answers at once. ACMD41 reports busy `SdCardConfig::init_polls` times (1).
  After the last block of a write, after the CMD12 that ends a multi-block write, and after an
  erase the card programs for `SdCardConfig::prg_ns` of emulated time (`--sd-prg-us`; 0 by
  default): DAT0 busy, prg, not ready for data, and a read or write command meanwhile is illegal.
  The driver's own writes wait for it with CMD13, but its write returns only after that, so a
  read from another task can reach the card while it programs: no response (IDF:
  `sdmmc_send_cmd returned 0x107`), then a CMD13 status of 0x400e00, ILLEGAL_COMMAND in prg.
- **Image.** A card of the image's size rounded down to 512 KiB (C_SIZE's unit), from 512 KiB to
  2 TiB, held by an `SdStore`: `FileStore` (copy-on-write or write-through) or `MemStore`. It is
  SDHC up to 32 GiB and SDXC above (SCR security 3.00); a card under 2 GB would be SDSC on
  silicon, and this one is SDHC at every size (IDF prints `SDHC` for both). A copy-on-write
  erase is kept as a range, not a block per address. The card stays in the socket through a chip
  reset, with its state; IDF's init resets it with CMD0.
- **Not modelled**: SDSC (byte addresses, CSD 1.0), UHS-I and CMD11, DDR, eMMC, lock and unlock,
  write protection, the PIO data FIFO at 0x200, the card-detect interrupt, and (without
  `--sd-timing`) the time a command or a transfer takes on the bus.

`--debug sdmmc` prints every command, its response and the card's state. The run's report adds
the commands, the unanswered ones and the bytes moved.

The SD card costs the other chips nothing: it is new code the P4 alone calls, plus a default
method on `SocBus` and a load kind (8) for the browser build. The P4 pays one flag test per
peripheral write, an address compare per peripheral read, and a few compares per GPIO and SDMMC
access.

## USB

`--usb-host FILE` plugs a host into the high-speed USB-OTG port and runs FILE's commands
([cli.md](cli.md), "USB host"). ESP-IDF v5.5.3's own tusb_msc example (`examples/usb_msc-p4`,
golden `usbmsc-p4`), built for SD card storage, presents the card in the socket over USB mass
storage through TinyUSB 0.21.0~2 and esp_tinyusb 2.4.0; the host in `host.txt` enumerates it,
writes a file on its FAT32 volume and reads it back, ejects it and polls it while the example's
application owns the card:

```sh
./target/release/esp32sim-p4 --flash-mb 4 --bootloader $FW/p4-usbmsc-bootloader.bin \
    --ptable $FW/p4-usbmsc-ptable.bin --app $FW/p4-usbmsc.bin --sd-rw card.img \
    --script examples/usb_msc-p4/console.txt --usb-host examples/usb_msc-p4/host.txt --max-seconds 2
```

```
[usb-host] t=0.190625 port reset: High speed
[usb-host] t=0.203250 device 303a:4002, USB 2.00, class ef/02/01, EP0 64 bytes, 1 configuration(s)
[usb-host] t=0.203750 interface 0: class 08/06/50, endpoints 01 (512 bytes) 81 (512 bytes)
[usb-host] t=0.206375 READ CAPACITY: 131072 blocks of 512 bytes (64 MiB)
[usb-host] t=0.459375 fat-write FILE0001.BIN: 1048576 bytes in 16 chunks, 0.248 s, 4.22 MB/s, second half 4.35 MB/s; paced at 4.10 MB/s, worst 1.9 ms late (chunk 0), kept up
[usb-host] t=0.502625 fat-read FILE0001.BIN: 1048576 bytes, hash 39fabf41c211861c, 24.24 MB/s, matches what was written
[usb-host] t=0.504250 ejected
[usb-host] t=0.504875 TEST UNIT READY: sense 2/3a/00 (MEDIUM NOT PRESENT)
[usb-host] t=1.510750 TEST UNIT READY passed after 5 failed polls
[usb-host] t=1.511125 READ CAPACITY: 131072 blocks of 512 bytes (64 MiB)
[usb-host] t=1.513625 root: FILE0001.BIN (1048576), README.MD (379)
```

When the host configures the device, esp_tinyusb takes the card from the application and
exposes it; the eject (START STOP UNIT with LoEj) gives it back, the example's console writes
README.MD on it and `expose` hands it to USB again. The golden pins the console, the host's log
with its emulated times and the instruction count, then reads the file, README.MD and the raw
blocks the host wrote back from the image. A second golden, `usbmsc-p4-faults`
(`examples/usb_msc-p4/faults.txt`), has the same example meet the faults a USB host meets
("Faults").

### The core

The DWC2 core (Synopsys DesignWare Hi-Speed USB 2.0 OTG) is modelled once, in
`esp-periph/src/usb_otg.rs`, as a device with buffer DMA; what differs between instances is
`Dwc2Config` (the S3's full-speed core would be a second configuration, not wired). Sources: IDF
v5.5.3 `soc/esp32p4/include/soc/usb_dwc_struct.h` (the v4.30a HS instance),
`soc/esp32p4/register/hw_ver1/soc/usb_dwc_cfg.h`, `hal/esp32p4/include/hal/usb_dwc_ll.h`; the bit
names and what the driver expects of them: TinyUSB 0.21.0~2 `src/portable/synopsys/dwc2/`
`dwc2_type.h`, `dwc2_common.c`, `dcd_dwc2.c`. It is the P4's block 0x50000 as an optional
device-table entry, so it is ticked and asked for deadlines only while a host is plugged in. The
interrupt is source 93 (`ETS_USB_OTG_INTR_SOURCE`, include/soc/interrupts.h): GAHBCFG.GINT and
an unmasked GINTSTS bit. LP_CLKRST HP_USB_CLKRST_CTRL1.RST_EN_USB_OTG20 (0x50111048 bit 2,
lp_clkrst_reg.h) holds the core in reset while it is set: the registers go to their reset values
when it is asserted and ignore writes until it is released. `usb_new_phy` pulses it through
`_usb_utmi_ll_reset_register`; a chip reset keeps the cable. Its DMA reaches L2MEM and PSRAM like
the SDMMC IDMAC (`SOC_PSRAM_DMA_CAPABLE`; IDF's own host driver puts DWC DMA memory in PSRAM on
the P4, components/usb/hcd_dwc.c:50).

| register | modelled |
| --- | --- |
| GOTGCTL 0x000 | the VBUS, A- and B-valid overrides and the HNP bits read back; CIDSTS 1 (a B-device); BSESVLD from VBUS or the B-valid override, ASVLD from the A-valid override |
| GOTGINT 0x004 | write 1 to clear; SEDET when VBUS goes |
| GAHBCFG, GUSBCFG 0x008, 0x00C | read back; GINT gates the interrupt; GUSBCFG.FHMOD keeps D+ released |
| GRSTCTL 0x010 | AHBIDL reads 1; CSRST is a core soft reset: from 4.20a CSRST stays set until software clears it and CSRSTDONE is write 1 to clear (`usb_dwc_ll_grstctl_core_soft_reset`), before it CSRST clears itself; RXFFLSH, TXFFLSH, FCRST and HSRST clear at once, TXFNUM reads back |
| GINTSTS, GINTMSK 0x014, 0x018 | the latched bits are write 1 to clear; OTGINT follows GOTGINT, GINAKEFF and BOUTNAKEFF the global NAKs, IEPINT and OEPINT DAINT & DAINTMSK, SOF the (micro)frames since the last clear; CURMOD 0 |
| GRXSTSR, GRXSTSP 0x01C, 0x020 | 0: slave mode is not modelled |
| GRXFSIZ, GNPTXFSIZ, DIEPTXF1-7, GDFIFOCFG | read back, from the configuration's FIFO depths (DIEPTXF8-15 read 0); an IN endpoint enabled with a TX FIFO smaller than its packets NAKs and counts a FIFO size error |
| GNPTXSTS 0x02C | four free queue entries, the FIFO empty |
| GSNPSID, GHWCFG1-4 0x040-0x050 | 0x4F54430A and 0, 0x215FFFD0, 0x03805EB5, 0xDFF1A030 (below) |
| DCFG 0x800 | read back; DSPD 0 makes the next reset end at high speed |
| DCTL 0x804 | SDIS releases D+ (the host sees a disconnect); SGINAK/CGINAK and SGONAK/CGONAK set and clear GINSTS/GONSTS at once; RWUSIG signals remote wakeup |
| DSTS 0x808 | SUSPSTS, ENUMSPD (0 high, 1 full speed), SOFFN counting microframes at high speed |
| DIEPMSK, DOEPMSK, DAINT, DAINTMSK, DIEPEMPMSK | a DAINT bit is set while the endpoint's interrupt register has a bit its mask lets through; DAINTMSK does not gate it |
| DIEPCTLn, DOEPCTLn | IN endpoints 0-7 and OUT endpoints 0-15 (OTG20_NUM_IN_EPS 8, OTG20_NUM_EPS 15); the IN registers past 7 read 0. MPS, USBAEP, DPID, NAKSTS, EPTYP, STALL, TXFNUM; EPENA is set by software and cleared by the core at the end of a transfer and at SETUP done; EPDIS disables an enabled endpoint once it NAKs, an IN endpoint with NAK set (SNAK, INEPNE), an OUT endpoint under the global OUT NAK (SGONAK, BOUTNAKEFF), the order dcd_dwc2's `edpt_disable` follows: until then EPDIS reads 1 and the endpoint stays enabled, then EPENA and EPDIS clear with EPDISD; SNAK (and INEPNE), CNAK, SD0PID, SD1PID; control endpoint 0 takes DIEPCTL0's packet size both ways, a SETUP clears its STALL and sets NAK on both, control OUT 0 cannot be disabled |
| DIEPINTn, DOEPINTn | write 1 to clear; TXFE reads 1 |
| DIEPTSIZn, DOEPTSIZn | XferSize and PktCnt count down by packet (17 and 8 bits, OTG20_TRANS_COUNT_WIDTH and OTG20_PACKET_COUNT_WIDTH; 7 and 2 or 1 on endpoint 0), STUPCNT on control OUT 0 |
| DIEPDMAn, DOEPDMAn | the DMA address, advancing with the data |
| DTXFSTSn | the TX FIFO's depth: it is empty between transactions |
| PCGCCTL 0xE00 | read back |
| the rest | register RAM, as are the UTMI PHY (0x5009C000), USB_WRAP and the clock gates the driver sets (HP_SYS_CLKRST SOC_CLK_CTRL1.REG_USB_OTG20_SYS_CLK_EN, LP_CLKRST HP_USB_CLKRST_CTRL1's PHY reference clock, HP_SYSTEM USBOTG20_CTRL); the clock gate is not checked |

A transaction is what the host asks of the core and is answered at once from the endpoint
registers:

- **SETUP** (always acknowledged, USB 2.0 §8.5.3): written by DMA at DOEPDMA0, which advances 8
  bytes, STUPCNT counting down from the 3 TinyUSB arms, Setup Packet Received set; past three
  back-to-back SETUPs the last slot is written again with B2BSTUP. The token after them ends the
  setup phase: SETUP (DOEPINT0 bit 3) and control OUT 0 handed back (EPENA cleared), so the driver
  finds the last SETUP at DOEPDMA0 - 8 (`handle_epout_dma`). A SETUP that comes while control OUT
  0 is not enabled waits, as in the RxFIFO, until it is.
- **IN**: NAK unless the endpoint is enabled, not NAKing and no global IN NAK holds; STALL if set;
  otherwise min(XferSize, MPS) bytes from DIEPDMA with the endpoint's data PID, and the transfer
  ends (XFRC, EPENA cleared) on its last packet. A zero XferSize sends a zero-length packet.
- **OUT**: NAK and STALL the same way; a packet with the wrong data PID is acknowledged and dropped
  (§8.6.4); otherwise written by DMA at DOEPDMA, XferSize counts down by its length, and a short
  packet or the last of PktCnt ends the transfer (XFRC, EPENA cleared, NAK set). More than XferSize
  is cut at XferSize with BBLEERR.
- A DMA access the memory map refuses is AHBERR and a NAK.

Bus events come from the link: VBUS on sets SRQINT, VBUS off SEDET (TinyUSB reports the unplug);
a bus reset sets USBRST and clears USBActEP but on endpoint 0, its end ENUMDNE at the speed the
chirp allowed; SOFs count from enumeration; early suspend (ESUSP) and suspend (USBSUSP, SUSPSTS),
then resume (WKUINT). `--debug usb_otg` prints every transaction and bus event.

**Identity.** GSNPSID is 4.30a: `usb_dwc_struct.h` describes "the High Speed v4.30a instance"
and `hal/usb_dwc_hal.c` accepts 4.00a and 4.30a. GHWCFG1-4 are the values TinyUSB 0.21.0~2
recorded from silicon (`dwc2_info.py`, `ESP32-P4`): internal DMA, a UTMI+/ULPI HS PHY, 15 device
endpoints besides endpoint 0, 16 host channels, dynamic FIFOs, per-endpoint interrupts, FIFO
queue depths of 4, 8 IN endpoints, one control endpoint, dedicated TX FIFOs, descriptor DMA, all
as `usb_dwc_cfg.h` configures the instance; TinyUSB reads the PHY type, the endpoint count, the
DMA architecture and the PHY width from them. The open points are listed below.

### The host

The host is two layers, the host controller on the cable and the software that drives it.

`esp_periph::usb_otg::UsbLink` (`usb_otg/link.rs`) is the host controller, owned by the core it
is plugged into and running in device time: one request at a time from host software (power,
reset, suspend, resume, a control or a bulk transfer), carried out as the transactions and bus
states a controller puts on the wire, each at its own time, against the device side
(`UsbFunction`, which the core implements). Its timing model, in CPU cycles:

- A transaction takes its payload plus a fixed protocol overhead at the line rate: 55 bytes at
  480 Mbit/s at high speed, 13 at 12 Mbit/s at full speed (USB 2.0 §5.8.4, the bulk protocol
  overhead). The transactions of a transfer follow back to back, and none crosses a (micro)frame
  boundary, so a high-speed bulk transfer moves at most 13 packets of 512 bytes in a 125 µs
  microframe (53 MB/s). `UsbTiming::frame_budget` (`rate` in a script) caps the payload per
  microframe below that.
- A NAKed transaction is tried again at the start of the next microframe (the next frame at full
  speed): what an EHCI controller does once a queue head's NAK counter runs out, and how a
  high-speed bulk OUT is paced by PING (§8.5.1). A NAK costs a retry a microframe, not a stream of
  them, so a waiting endpoint is cheap to emulate.
- SOFs every 125 µs from the end of the bus reset while the port is enabled and not suspended
  (§8.4.3); the core counts them from when they start rather than receiving each one, and asks
  for a deadline only while its SOF interrupt is unmasked.
- A bus reset holds SE0 for 50 ms (TDRSTR from a root port, §7.1.7.5) and ends in the speed the
  device's chirp allows (high speed while DCFG.DSPD is 0); resume drives K for 20 ms (TDRSMDN,
  §7.1.7.7). With SOFs stopped the core sees early suspend after 3 ms and suspend 125 µs later
  (§7.1.7.6).
- One device hangs on the cable: tokens reach it whatever its address.
- `UsbLink::inject(at_ns, LinkFault::VbusOff)` drops VBUS at a set time whatever the host is
  doing then: a transfer in flight ends `Disconnected` and the core sees a session end (SEDET),
  which TinyUSB reports as an unplug.

Its next event is the core's device deadline, so idle cores sleep through a transfer to the
packet that wakes them, and the bus runs what is due after the device ticks of a round.

`esp_soc::usb_host` is the host software, written against a `UsbPort` (the link, and a way to let
emulated time run to a given nanosecond, which `Machine` implements by running to that exact
cycle). `UsbHost` learns that a request finished at the next microframe boundary after it did
(the one-microframe interrupt threshold Linux's EHCI driver programs) and submits the next one
there, so the firmware runs between the transfers of a command:

- `enumerate`: VBUS, waiting for the pull-up, the 100 ms connect debounce (TATTDB, §7.1.7.3), the
  port reset, 10 ms of reset recovery (§9.2.6.2), the device descriptor's first 64 bytes at address
  0, SET_ADDRESS and its 2 ms recovery (§9.2.6.3), the device and configuration descriptors, the
  strings, SET_CONFIGURATION (§9.1.2, §9.4); control transfers time out after 5 s.
- `reset_device`: Linux's `usb_reset_device` (v6.12 drivers/usb/core/hub.c,
  `usb_reset_and_verify_device` at 6090 and `hub_port_init`'s new scheme at 4985-5034): reset,
  GET_DESCRIPTOR(64) at address 0, reset again, SET_ADDRESS to the old address, the device and
  configuration descriptors read and compared with the old ones, SET_CONFIGURATION with the old
  configuration; 50 ms after each reset (TRSTRCY "plus some extra", hub.c:3112-3113), 10 ms
  after SET_ADDRESS.
- `msc::Msc`: SCSI over bulk-only transport (BOT 1.0): CBW and CSW with tags, the CSW checked
  for validity and meaning (§6.3), a STALLed data stage cleared and the CSW read, a STALLed CSW
  cleared and read once more, REQUEST SENSE after CHECK CONDITION; INQUIRY, TEST UNIT READY, READ
  CAPACITY(10), MODE SENSE(6), PREVENT ALLOW MEDIUM REMOVAL, START STOP UNIT, READ(10) and
  WRITE(10) of any length in commands of 240 blocks at most (usb-storage's `max_sectors`,
  scsiglue.c:670), SYNCHRONIZE CACHE(10); `wait_ready` polls TEST UNIT READY as a host polls its drive.
  A command may take 30 s (sd's SD_TIMEOUT, drivers/scsi/sd.h:15, set at sd.c:3970-3972) before it
  is aborted. The LUN is a `fat32::BlockDevice`. Recovery (`Msc::recovery`) is Linux's by default,
  from v6.12 drivers/usb/storage/transport.c:
  - a transport error (a CBW that fails, a phase error, a CSW with the wrong tag or a status past
    2, a CSW still bad after the second read) or a command aborted at its timeout goes to
    `Handle_Errors` (transport.c:903-926): a port reset first (`usb_stor_port_reset`,
    `reset_device` above), the class reset only if that fails (`usb_stor_Bulk_reset`,
    `usb_stor_reset_common` at 1347: the request with a 5 s timeout, 6 s for the device to
    recover, then the halt cleared on bulk IN and bulk OUT);
  - a data stage that runs long (babble) with a good status gives a faked INVALID FIELD IN CDB
    (`fake_sense`, 1186 and 1305-1310), and the CSW is still read;
  - a zero-length packet where the CSW should be is read past once (1229-1233);
  - a residue past the transfer length is clamped to it (1298).

  `Recovery::Bot` (`recovery bot`) is BOT 1.0's reset recovery for all of these (Bulk-Only Mass
  Storage Reset and CLEAR_FEATURE(ENDPOINT_HALT) on both endpoints, §5.3.4). What Linux's SCSI
  layer adds on top is not modelled: the retries of a failed command (sd's `max_retries`,
  SD_MAX_RETRIES 5) and the error handler's escalation past the abort. The review that asked for
  this described Linux's timeout handling as the class reset first, then a port reset; the
  v6.12 source does the port reset first, and the class reset only when the port reset fails.
- `fat32::Fat32` mounts a volume (an MBR's FAT32 partition, or from LBA 0), lists, reads and
  writes 8.3 files in the root directory in a vfat-like order: the directory entry at once, the
  data a chunk at a time as it comes (one WRITE(10) per contiguous run of clusters), the FAT
  sectors of each chunk's clusters in both FATs, then the entry's size and first cluster and
  FSInfo. It is not Linux's writeback order, in which the page cache batches the data, the buffer
  cache holds the FAT and directory sectors, inode writeback updates the entry periodically and
  FSInfo is written at sync or unmount: a file cut off mid-write here keeps size 0 and no cluster
  with its clusters orphaned in the FAT, where Linux would usually have left a shorter file.
  Allocation starts at FSInfo's hint and writes the last cluster allocated back as the hint; the
  free count written is FSInfo's as it stands on the volume less what this host allocated since.
  `FileWriter::fat_every` writes the FAT less often.
- `fat32::write_paced` writes a file at a fixed rate: chunk i is due at t0 + i x gap (t0
  taken before the file is created), the host waits only for a chunk that is not due yet, and the
  result (`PacedStats`) has the rate overall and over the second half, the worst lateness against
  the schedule and whether the host kept up (never a whole chunk behind).
- `script::Runner` runs a command file (`--usb-host`), each command logging what it did. A
  NOT READY seen by `ready` or `poll` drops the mounted volume, and a failure that leaves no
  device on the port drops the interface and the volume, so the next `mount` reads the volume
  again. The CLI runs it under the run's `--max-insns` and reboot policy
  (`usb_host::run_machine_until`).

A test drives the same API between runs of the machine:

```rust
esp_soc::SocBus::attach_usb_host(&mut m.bus, Default::default())?;
let mut host = UsbHost::new(&mut m);              // the machine is the host's port
host.enumerate(10_000_000_000)?;                  // connect timeout, ns
let mut msc = Msc::open(host)?;
msc.wait_ready(100_000_000, 10_000_000_000)?;     // TEST UNIT READY every 100 ms
msc.read_capacity()?;
let mut fs = Fat32::mount(&mut msc)?;
let mut f = fs.create(&mut msc, "FILE0001.BIN")?;
f.write(&mut fs, &mut msc, &chunk)?;
f.close(&mut fs, &mut msc)?;
```

`esp-soc/src/usb_host/tests.rs` runs the host against a mass-storage device written in Rust at
the transaction level, with knobs for what BOT's host side must survive (MEDIUM NOT PRESENT, short
data stages, STALLs, phase errors, wrong tags, NAKs, a zero-length packet before the CSW, an
endpoint that stops answering) under both recovery policies, and for a paced write;
`esp-periph/src/usb_otg/tests.rs` drives the core with TinyUSB's register sequences against the
link, including control writes of one and two packets (the status stage is DATA1, §8.5.3), an
endpoint disable waiting for its NAK, the reset line held, and an injected VBUS loss.

**Throughput.** A host writing in 64 KiB chunks as fast as the drive takes them, then reading the
file back (FAT32, the card's programming time 0). The second half is the rate from the middle
chunk to the close. The 32 MiB file is thousands of times the firmware's buffer, and its second
half runs at the rate of the whole and of the 4 MiB file, so these are steady-state rates:

| firmware | SD bus | file | write | second half | read |
| --- | --- | --- | --- | --- | --- |
| tusb_msc example (8 KiB MSC buffer) | at once | 4 MiB | 9.1 MB/s | | 24.8 MB/s |
| tusb_msc example | at once | 32 MiB | 9.2 MB/s | 9.2 MB/s | |
| tusb_msc example | `--sd-timing`, 4 bits at 20 MHz | 4 MiB | 4.4 MB/s | | 6.8 MB/s |
| tusb_msc example | `--sd-timing` | 32 MiB | 4.4 MB/s | 4.4 MB/s | |

Paced at 64 KiB every 16 ms (4.1 MB/s, `fat32::write_paced`), the example keeps up
with the SD bus timed or not, over 1 MiB (the golden) and 16 MiB files. The latest chunk starts
1.9 ms after its due time, and it is the first, behind the directory entry. With the bus timed it
keeps up with a 7% margin (4.4 against 4.1 MB/s). esp_tinyusb writes the card after each buffer
arrives, in the TinyUSB task (tinyusb_msc.c:351-388), so only the next buffer's DMA proceeds
alongside a card write. Two things the model leaves out would each slow it: the card's
programming time is 0 (`--sd-prg-us`, `SdCardConfig::prg_ns`), and cache misses and PSRAM stalls
are not modelled. Before the rate is compared with a board, `--sd-prg-us` wants a value measured
on a card: the busy time after a multi-block write.

The USB port and the timed SD bus cost the other chips nothing and the P4 nothing while unused:
the core is an optional device-table entry, the bus tests one flag per round for the link and
one for a timed SD bus, whose work and deadline sit in cold functions (inlined, they cost the P4
hello demos 0.9-1.0%).

**Faults.** `examples/usb_msc-p4/faults.txt` (golden `usbmsc-p4-faults`) has the tusb_msc
example meet three faults; the example logs each hand-over of the card ("Storage mounted to
application: No" when the host has it, "Yes" when the application has it), and the golden pins the
sequence:

| fault | what the host does | what the example does |
| --- | --- | --- |
| a WRITE(10) of 240 blocks with a 5 ms command timeout | aborts it mid data stage, resets the port (Linux's recovery), writes the blocks again and verifies them | nothing visible but one line: `proc_write10_cmd 910: ASSERT FAILED` (below); the next commands work |
| a port reset after an eject | `port-reset`: two resets, the descriptors compared, SET_CONFIGURATION | takes the card back from the application: "No" |
| VBUS lost 100 ms into a 1 MiB file | the write fails `Disconnected`; the interface and the volume are dropped; `attach` again | gives the card to the application on the unplug ("Yes"), takes it again on the new SET_CONFIGURATION ("No") |

The file cut by the VBUS loss is on the card as an entry of size 0 and no cluster, its clusters
allocated in the FAT (the golden checks both): the vfat-like writer's failure mode, not Linux's.

What the sources say about the hand-overs (TinyUSB 0.21.0~2, esp_tinyusb 2.4.0):

- SET_CONFIGURATION with a non-zero value calls `tud_mount_cb` (usbd.c:1006-1012). A bus reset
  alone does not unmount (usbd.c:717-721); an unplug calls `tud_umount_cb` (723-727).
- esp_tinyusb's `tud_mount_cb` runs `msc_storage_mount_to_usb` (tinyusb.c:62-65), which unmounts
  the card from the application unless `auto_mount_off` is set (tinyusb_msc.c:776-790);
  `tud_umount_cb` runs `msc_storage_mount_to_app` (tinyusb.c:86-89, tinyusb_msc.c:760-774).
- esp_tinyusb's default READ(10) and WRITE(10) callbacks do not check who has the card
  (tinyusb_msc.c:1379-1388 and 1393-1411; `msc_storage_write_sector_deferred` writes whatever the
  mount point): a write that arrives after the application took the card goes to the card under
  the application's FatFs. Only TEST UNIT READY looks at the mount point (1321-1336), and it
  reports NOT READY, MEDIUM NOT PRESENT; it never reports UNIT ATTENTION.

So a port reset, which Linux's usb-storage does to recover from any transport error or timeout,
takes the card from an application that is using it, unless the firmware sets `auto_mount_off`.

The assertion after the aborted write reads, from the sources, as a race in the stack rather than
in the model: dcd_dwc2's bus reset handler closes the endpoints in the interrupt (it clears
`xfer_status`, dcd_dwc2.c:775) while the TinyUSB task still holds the write's last transfer
completion; asking for the next chunk then finds bulk OUT closed (`dcd_edpt_xfer`,
dcd_dwc2.c:687-688) and msc_device.c:910 asserts. The bus reset event that follows resets the
class (`mscd_reset`). A 5 ms timeout cuts a live transfer, which is what opens the window; Linux's
30 s timeout aborts a device that has stopped. Seen only in emulation; not checked on silicon.

**Open points**, each a guess where the sources disagree or say nothing:

- GSNPSID: TinyUSB's `dwc2_info.py` lists 0x4F54400A (4.00a) for the P4, against IDF's 4.30a.
  TinyUSB's own reset takes either path, and 4.30a's is the one modelled.
- GHWCFG2 and GHWCFG3 differ from `usb_dwc_cfg.h` in single point (0 against
  OTG20_SINGLE_POINT 1), the FS PHY type (3 against 2), the counter widths (16 and 7 bits against 17
  and 8), synchronous reset and battery charger support; the registers keep TinyUSB's values, the
  counters `usb_dwc_cfg.h`'s widths.
- Reset values not in any header: DCTL.SDIS 1 (no pull-up until a driver connects), GUSBCFG's
  USBTrdTim 5, the FIFO sizes from the configured depths; which registers a core soft reset keeps
  (the PHY selection in GUSBCFG, DCFG.DSPD, DCTL.SDIS, PCGCCTL).
- No XFRC with a SETUP, the fourth back-to-back SETUP rewriting the last slot, OUT babble cut at
  XferSize, no NYET handshake, USB reset leaving DCFG.DevAddr to the driver.
- The NAK retry at the next microframe is one controller's policy, not the specification's; a
  controller that retries within the microframe would see a NAKing device sooner.

## The co-processor

The P4 has no radio. On the reference board an ESP32-C6 sits on SDIO slot 1 and runs ESP-Hosted's
slave firmware; the P4's application calls `esp_wifi_*` as usual, and esp_wifi_remote and
ESP-Hosted's host driver carry the calls and the packets over SDIO. `--coproc` puts an emulated C6
there, running the unmodified slave, and runs both chips in one process ([cli.md](cli.md),
"Co-processor (P4)"). IDF v5.5.3's WiFi station example with esp_hosted 2.12.13 and ESP-Hosted
2.12.13's slave (`examples/esp-hosted-p4`, golden `hosted-p4`) join the virtual access point
through the C6's radio:

```sh
./target/release/esp32sim-p4 --flash-mb 2 --bootloader $FW/p4-hosted-bootloader.bin \
    --ptable $FW/p4-hosted-ptable.bin --app $FW/p4-hosted-station.bin \
    --coproc c6:bootloader=$FW/c6-hosted-bootloader.bin,ptable=$FW/c6-hosted-ptable.bin,app=$FW/c6-hosted-slave.bin,otadata=$FW/c6-hosted-otadata.bin,stub=0x420bac5c=0 \
    --wifi ssid=esp32sim,psk=esp32sim-pass --net none --max-seconds 4
```

```
W (81) H_SDIO_DRV: Reset slave using GPIO[54]
[c6] ESP-ROM:esp32c6-20220919
[c6] I (115) co-pro-main:                 ESP-Hosted-MCU Slave FW version :: 2.12.13
[c6] I (115) SDIO_SLAVE: sdio_init: sending mode: SDIO_SLAVE_SEND_STREAM
I (1601) sdio_wrapper: SDIO master: Slot 1, Data-Lines: 4-bit Freq(KHz)[40000 KHz]
I (1621) sdio_wrapper: Function 1 Blocksize: 512
I (1721) H_SDIO_DRV: Card init success, TRANSPORT_RX_ACTIVE
I (1771) transport: Identified slave [esp32c6]
I (1771) transport: capabilities: 0xd
[c6] W (2728) slave_wifi_std: Sta mode connected
I (2771) RPC_WRAP: ESP Event: Station mode: Connected
I (3771) wifi station: got ip:10.0.2.15
```

The P4 holds the C6's EN low for 10 ms on GPIO 54 and waits 1.5 s
(CONFIG_ESP_HOSTED_SDIO_RESET_DELAY_MS) while the C6 boots again; the card comes up (CMD52 I/O
reset, CMD0, CMD8, CMD5 twice, CMD3, CMD7, then CMD52 for the function, its interrupt enables,
four lines, high speed and both block sizes), the host opens the data path with a slave
interrupt, and the slave answers with its INIT event: the chip, the capabilities and its firmware
version. The host compares that with its own and prints `Version mismatch` only when the
major.minor differ (transport_drv.c, `compare_fw_version`); it prints nothing here, 2.12 against
2.12. The station's RPCs follow, the C6 joins the access point, and the P4's lwIP takes a lease
through the link 3.77 s after power-up. The same station built for rev ≥ 3 (`station/sdkconfig.defaults.rev3`,
IDF v5.5.4, images `p4-hosted-rev3-*`, golden `hosted-p4-rev3`) runs on the rev ≥ 3 model with the
same C6 and takes its lease at 3.77 s too. On a Waveshare ESP32-P4-WIFI6 with a v3.2 chip and the
board's own C6 the image goes through the same steps (SDIO up, slave identified, transport
active, WiFi started); that C6's factory firmware reports version 0.0.0, so the host also prints
`Version mismatch`. Built for a real 2.4 GHz network instead of `esp32sim`, it connects at 5.5 s
and takes its lease at 6.8 s.

**In the browser.** The demo `p4-c6-hosted` (`web/wasm/fw/p4-c6-hosted.json`, board
`esp32p4-rev3+c6`) runs the rev ≥ 3 pair in the page: both chips from their ROMs, the P4's console
and the C6's side by side (the "both chips" tab). A board name ending in `+c6` makes the module
create the pair (`esp32p4::coproc::CoSim`); a manifest's `coproc` section lists the C6's images,
which load through `esp32sim_load` with bit 8 of the kind set and `esp32sim_load_at` with bit 31 of
the offset set; a stub `c6:NAME` and the WiFi configuration go to the C6. The golden pins both consoles in the order the slices
produced them, the P4's instruction count, the C6's run and its SDIO, WiFi and network counts.
19 frames go by before the C6's driver has given its MAC a receive ring (beacons before the
radio is on; the report counts them apart from frames that find no free descriptor, of which
there are none). Co-simulating costs about a second of CPU per emulated second while the
station idles: the golden's 4 s take 3.6 s, 8 s take 7.7 s (both chips, one host thread).

### The slave

`esp32c6/src/sdio.rs` models the three blocks IDF v5.5.3's sdio_slave driver programs
(`esp_driver_sdio/src/sdio_slave.c`, `hal/sdio_slave_hal.c`, `hal/esp32c6/include/hal/sdio_slave_ll.h`;
registers: `soc/esp32c6/register/soc/sdio_hinf_reg.h`, `sdio_slc_reg.h`, `sdio_slc_host_reg.h`),
HINF at 0x60016000, SLC at 0x60017000 and SLCHOST at 0x60018000 (`reg_base.h:35-37`), as one
optional device: the SLC entry raises source 64, `ETS_SLC0_INTR_SOURCE` (`interrupts.h:86`), and
HINF and SLCHOST are aliases of it. SLC's "rx" link carries slave-to-host data, its "tx" link
host-to-slave data, the driver's sending and receiving. A C6 run without a host costs the table
entries and one address compare after a peripheral write.

| block, register | modelled |
| --- | --- |
| HINF CFG_DATA0 0x00 | DEVICE_ID_FN1 0x6666, USER_ID_FN1 0x0092 at reset; the CIS's MANFID |
| HINF CFG_DATA1 0x04 | SDIO_ENABLE (the card answers nothing without it), IOREADY1/2 (CCCR I/O Ready), HIGHSPEED_ENABLE (CCCR SHS), SDIO_INT_MASK, SDIO_VER 0x232 at reset; the read-only HIGHSPEED_MODE, IOENABLE1/2 and CD_DISABLE show the host's CCCR |
| SLC SLCCONF0 0x00 | TX_RST and RX_RST reset a link; RX_AUTO_WRBACK writes a sent descriptor back with the owner cleared |
| SLC SLC0INT_RAW/ST/ENA/CLR 0x04-0x10 | FRHOST_BIT0-7 from the host, TX_OVF, TX_DONE, TX_SUC_EOF, RX_DONE, RX_EOF; ST is RAW & ENA, the SLC0 line |
| SLC SLC0RX_LINK, SLC0RX_LINK_ADDR 0x3C, 0x40 | START reads the descriptor at once and raises RX_DONE when the hardware owns it (the driver polls RX_DONE right after); STOP |
| SLC SLC0TX_LINK, SLC0TX_LINK_ADDR 0x44, 0x48 | START, STOP; RESTART follows the next pointer of the last buffer closed |
| SLC SLCINTVEC_TOHOST 0x5C | bits 7:0 raise SLCHOST's TOHOST_BIT0-7 |
| SLC SLC0TOKEN0/1 0x60, 0x64 | WR, INC, INC_MORE; the count at 27:16 and at SLCHOST TOKEN_RDATA |
| SLC SLC0_LEN_CONF, SLC0_LENGTH 0xF4, 0xF8 | LEN_WR, LEN_INC, LEN_INC_MORE set the cumulative length SLCHOST PKT_LEN shows (20 bits, wrapping); the first length write after a START on a packet raises the host's RX_NEW_PACKET |
| SLCHOST TOKEN_RDATA 0x44 | TOKEN1 at 27:16, TOKEN0 at 11:0 (TOKEN1 can be held for a fault test) |
| SLCHOST SLC0HOST_INT_RAW/ST 0x50, 0x58 | TOHOST_BIT0-7, RX_UDF, TX_OVF, RX_NEW_PACKET; ST is RAW & FUNC1_INT_ENA |
| SLCHOST PKT_LEN 0x60 | the length in 19:0; LEN_CHECK (31:20) reads 0 |
| SLCHOST CONF_W0-W15 0x6C-0xB8 | shared scratch, both sides; CONF_W7's low byte is the host's interrupt to the slave (FRHOST_BIT0-7) and reads 0 |
| SLCHOST SLC0HOST_INT_CLR, FUNC1_INT_ENA 0xD4, 0xDC | write 1 to clear; the interrupt enable |
| the rest | register RAM (timing, check sums, the windows, SLC1, function 2) |

The card the host sees follows the SDIO Simplified Specification v3.00 (SD Specifications Part
E1, 2011):

| command | the card |
| --- | --- |
| CMD5 | R4 (§3.3): C set, two I/O functions, no memory, I/O OCR 2.7-3.6 V; answered only before an RCA is assigned, and with argument 0 it does not start initialization (§3.1.2); nothing else is answered before a CMD5 since power-up or the I/O reset |
| CMD3 | R6 (§4.3): RCA 0x0001, the SDIO-only status bits zero |
| CMD7 | with the RCA: selected, R1b; with another: deselected, no answer (§6.14) |
| CMD52 | R5 (§5.2.1): IO_CURRENT_STATE CMD, the data; read after write (RAW); FUNCTION_NUMBER for a function past two |
| CMD53 | R5 with IO_CURRENT_STATE TRN (§5.3): bytes (count 0 is 512) or blocks of the function's block size, incrementing or fixed address; infinite block counts are not modelled |
| CMD0, CMD8, ACMD41, CMD1 and the rest | no answer: an I/O-only card ignores memory commands (§4.4: CMD0 does not reset the I/O part) |

Function 0 is the CCCR (§6.9 Table 6-1: revisions from SDIO_VER, I/O Enable, I/O Ready, Int
Enable, Int Pending, I/O Abort with RES and the abort select, Bus Interface Control, Card
Capability 0x17, the common CIS pointer, FN0 block size, Bus Speed Select with SHS and EHS), the
FBR's block sizes (§6.10) and the common CIS at 0x1000: DEVICE, MANFID 0092/6666, FUNCID, FUNCE,
CONFIG, CFTABLE_ENTRY and END, the tuples IDF's `sdmmc_io_print_cis_info` printed for an ESP32-C6
slave (esp_hosted 2.12.13 `docs/esp32_p4_function_ev_board.md`). The I/O reset (§4.4) clears the
function enables, interrupt enables, bus width, high speed and block sizes and takes the card
back to needing CMD5; the SLC side is the C6's and stays as its driver left it. Function 1 is
Espressif's protocol (IDF v5.5.3 `docs/en/api-reference/protocols/esp_sdio_slave_protocol.rst`):
addresses below 0x400 reach SLCHOST's registers a byte at a time (ESP-Hosted and
esp_serial_slave_link mask register addresses with 0x3FF), and above them is the FIFO, where a
transfer to address A carries a packet of 0x1F800 - A bytes:

- **Slave to host.** The driver's send link is a chain of descriptors with EOF on the last
  (stream mode collects every queued packet). A host read takes the packet's bytes from the
  descriptors in SRAM, zero-fills past the packet (ESP-Hosted reads whole 512-byte blocks), hands
  each descriptor back with RX_DONE and the last with RX_EOF, which is when the driver returns
  the buffers and starts the next chain. Reading more than the link holds is an underflow
  (RX_UDF, zeros, and the CMD53 succeeds). Starting the link on a descriptor the hardware owns
  raises the slave's RX_DONE at once: the driver starts a one-byte descriptor and waits for that
  RX_DONE (`send_isr_invoker_enable`). The host's RX_NEW_PACKET waits for the length the driver
  writes right after the START (`send_new_packet`): raised at the START, ESP-Hosted's combined
  register read (INT_RAW to PKT_LEN in one CMD53) could land between the two, see a packet with
  the old length, clear the interrupt and wait for ever. `SdioStats::stale_new_packets` counts host
  reads of INT_RAW that show RX_NEW_PACKET with PKT_LEN where the host last read it (an upper
  bound: a host PKT_LEN poll in between also counts); the report names it when it is not zero.
  A CMD52 into the FIFO is a one-byte transfer to address A: a read takes the packet's next
  byte, a write is a byte of a packet of 0x1F800 - A bytes and closes it at the last address.
- **Host to slave.** The driver loads receive buffers (START for the first, the tail's next pointer
  and RESTART for the rest, TOKEN1 counting them; the host subtracts the buffers it has used). A
  host write fills the current buffer, spills into the next, and closes the last with its length
  and EOF when the packet's 0x1F800 - A bytes are in; the block padding is dropped. A packet may
  span two CMD53s (a block, then the remainder at the incremented address). With no buffer the
  bytes are lost and TX_OVF goes to both sides.
- **When the C6 sees it.** The bytes cross the bus in the host's register write that runs the
  transfer, but what the C6 sees of it, each descriptor handed back (owner, length, EOF) and its
  RX_DONE, RX_EOF, TX_DONE and TX_SUC_EOF, waits for the data phase's end on the bus: with
  `--sd-timing` its wire time after the command, and when the P4's IDMAC resumes a suspended
  transfer (a read longer than IDF's four descriptors) the moment of the resume, which the C6,
  behind the P4 then, has not reached. The slave queues the hand-back and the co-simulation stops
  the C6 at that cycle to apply it. Until then the buffers are the hardware's, so moving the bytes
  early is invisible to the driver. Untimed, at a command the C6 already stands at the command's
  time, so nothing waits.
- **Interrupts.** The slave raises the host's TOHOST bits through SLCINTVEC_TOHOST; DAT1 goes low
  while SLC0HOST_INT_ST is not zero, IENM and IEN1 are set in the CCCR (§6.3) and HINF does not
  mask it. The host's byte in CONF_W7 raises FRHOST_BIT0-7 on the slave, as ESP-Hosted's "open
  data path" interrupt does.

`--debug sdio` prints the slave's commands, the descriptors handed over and the I/O reset. The
I/O reset (§4.4) leaves SLCHOST's counters (TOKEN1, PKT_LEN) to the C6's driver: Espressif's
driver resets them itself and says the host has to ask it to (`sdio_slave.c`, header comment),
which ESP-Hosted's slave does on the host's reset interrupt (`sdio_reset`, `sdio_slave_reset`).
Guesses, each from the sources' silence:

- the CCCR revision bytes as SDIO_VER's split (0x32, 0x02), the card capability (0x17), two
  functions in R4, CMD5's C bit set at once, and the FN block sizes at 512 after a reset (the
  protocol document's default; the specification says 0);
- PKT_LEN's LEN_CHECK as 0, INT_ST under FUNC1_INT_ENA rather than SLC0HOST_INT_ENA, the 0x400
  split between the register window and the FIFO;
- RX_DONE at a link START on an owned descriptor, and the host's RX_NEW_PACKET at the length write
  that follows it;
- a read past the send link: zeros, RX_UDF, and a CMD53 that succeeds; silicon may stall the bus
  and retry instead (SLCHOST's SLC0_HOST_RD_RETRY_INT, SLC's SLC0_RD_RETRY_THRESHOLD);
- a write with no receive buffer: the bytes dropped, TX_OVF to both sides, and a CMD53 that
  succeeds (a host is expected to wait for TOKEN1 and never get there);
- the I/O reset: what it does to SLCHOST's INT_RAW and CONF_W registers is not documented; they are
  left as they are, like the counters;
- the card answers from power-up whatever the C6's IO MUX, its PCR clock gate or its ROM is doing:
  a host that sends CMD5 while the C6's ROM boots is answered here, which silicon may not do.

Not modelled: the card's busy and read wait, its
CRC (it never answers with a CRC error or a malformed response), suspend and resume, function 2,
the second SLC (SLC1), the host's token registers (TOKEN_WDATA, TOKEN_CON), the PCR reset of the
block (`PCR_SDIO_SLAVE_CONF.RST_EN`, which the driver pulses before it configures a block already
at its reset values) and the pins (the card answers however the C6's IO MUX is set).

### The host side

An `SdDevice` says whether it is an SDIO function (`io`). After each command the P4 samples the
function's DAT1: the slot's D1 pin (15) reads low while the function interrupts, which is what
IDF's `sdmmc_host_io_int_wait` looks at first, and RINTSTS's SDIO bit for the slot (16 + slot)
is set when DAT1 goes low, edge triggered as that function's comment describes it
(`esp_driver_sdmmc/src/sdmmc_host.c`). The ISR masks it and wakes the waiting task. The `card_int`
and card-detect inputs IDF ties high are not consulted. Nothing of this runs while only a memory
card is on the bus. The C6's EN follows GPIO 54 (`GpioP4::drive`; ESP-Hosted's
`ESP_HOSTED_SDIO_GPIO_RESET_SLAVE`, 54 on the P4, active high: low resets), read from OUT1 and
ENABLE1, which the W1TS and W1TC registers reach for every pin to 56; the IO MUX function of pins
49-56 is not checked.

### Lock-step

`esp32p4::coproc` (`esp32p4/src/coproc.rs`) runs a P4 machine and a C6 machine on one clock, nanoseconds from power-up
(the P4 at 9 cycles per 25 ns, the C6 at 4). Slices of 10 µs (`slice-us`) end on a fixed grid:

1. the P4 runs to the slice's end (`max_cycles`, both cores);
2. the C6 runs to the same instant (`run_until_cycle`);
3. the lines between the chips are exchanged: the slave's DAT1 into the P4 (D1 and RINTSTS), the
   P4's EN line into the C6.

The SDIO bus is host-driven, so the transactions are where the chips meet exactly: when the P4's
host starts a command, the C6 first runs to that nanosecond, then the command and its data are
carried out against the slave as it stands, the descriptors read and written in the C6's SRAM;
the C6 goes on from there in its next run. The C6 is never ahead of the P4. What the P4 senses
without a command, the slave's interrupt, is sampled after every command and at every slice end,
so it reaches the P4 at most one slice late, as a frame reaches csim's medium at the end of the
slice it started in (`--cooja`). EN is read at slice ends only, so it takes effect up to a slice
late: driven low, the C6 runs to that instant and stops; released after at least the C6's t_RST
(50 µs, ESP32-C6 Series Datasheet, "Chip Power-up and Reset"), the C6 powers up from its ROM (a
chip reset with cause POWERON; its LP domain and SRAM are kept, which a real power-up would not
keep), and its clock goes on from that instant. A shorter pulse is ridden out: the C6 runs on
from where it stopped. The restarted P4 makes GPIO 54 an output a moment before it writes the
level, and a slice end can catch that as a one-slice pulse; the report counts such pulses. At
power-up EN is the pull-up's: the C6 boots with the P4, as on the board, until ESP-Hosted resets
it. A chip that resets itself comes back through its ROM; a P4 reset keeps the SD bus's time
(the device clock starts again at a chip reset, `Peripherals::cycle_base` carries the cycles
before it), so the C6 still meets every command at its nanosecond, and the report counts
commands that ever find the C6 ahead (none should). The co-simulation reads no host clock and no
host randomness and the order of every step is fixed, so a run is the same to the instruction
every time, and the same however `run_until` is called
(`the_pair_runs_the_same_whichever_way_it_is_driven` runs the pair for 2 s whole and in uneven
pieces). The virtual network's SNTP server answers its own clock plus emulated time; `--net
none,epoch=S` fixes that clock, and without it the host's clock at the first answer sets it
([networking-howto.md](networking-howto.md)), so a firmware that asks for the time is
reproducible only with an epoch. The goldens and tests pass one. Both consoles are
collected a line at a time and printed in the order the slices produced them. The browser build
runs the same pair ("In the browser" above).

**Cost.** Without `--coproc` nothing of this runs: the C6's slave is an optional device-table
entry and one address compare after a peripheral write, the P4's SDIO path one flag tested where
the SDMMC host runs (`has_sdio`), the fault hooks and the deferred hand-back live in the
co-simulation, and the rest is in cold functions.

### Throughput

ESP-Hosted's own transport test (RawTP) moves 1460-byte packets as fast as the link takes them
and reports the rate every second on both consoles. Its images are fetched with the pair's
(`p4-hosted-rawtp-rx.bin`, `p4-hosted-rawtp-tx.bin`, `c6-hosted-rawtp-slave.bin`; the Kconfig
recipe is in `examples/esp-hosted-p4/README.md`), and the goldens `hosted_rawtp_rx` and
`hosted_rawtp_tx` pin 10 s of each with the bus timed. Both sides wait 5 s after the data path
opens, so traffic starts near 7.7 s. The steady per-second reports, 12 s runs:

| direction | SDIO bus | emulator | Espressif, P4 + C6, 4 bits at 40 MHz |
| --- | --- | --- | --- |
| C6 to P4 (host receive) | at once | 427-474 Mbit/s | 80 Mbit/s |
| C6 to P4 | `--sd-timing`, 4 bits at 40 MHz | 139.5 Mbit/s | |
| P4 to C6 (host transmit) | at once | 428-430 Mbit/s | 72 Mbit/s |
| P4 to C6 | `--sd-timing` | 106.3 Mbit/s | |

Deferring the slave's hand-back to the end of the data phase on the bus ("When the C6 sees it") does
not move these rates: the C6 is busy elsewhere for longer than a block's wire time. The board
numbers are Espressif's (esp_hosted 2.12.13 `docs/performance_optimization.md`, "ESP32-C6 as
Co-Processor", over the air; iperf there gives 30-32 Mbit/s TCP and 49-50 UDP). Without timing a
transfer takes no time on the bus, so the rate is what the two CPUs' drivers sustain at one
instruction a cycle: the P4's ESP-Hosted tasks and memcpy, the C6's ISR and the SLC hand-overs.
`--sd-timing` gives the link the card's wire-time model ("The host"): a 512-byte block in 4 bits at
40 MHz is 1044 card clocks, 157 Mbit/s on the data lines, and each command adds 106 clocks and each
packet its register reads (the interrupt status, the length), which leaves the 139 Mbit/s measured;
transmit pays the TOKEN1 reads and the padding of each packet to whole blocks. What is still missing
against Espressif's 72-80: the slave's busy and FIFO latency (it answers at once), the CPUs' caches
and PSRAM stalls, and the time the data takes through the C6's DMA.

The WiFi side is not part of these numbers, and through the air it is the first ceiling. The
virtual access point's receive pacing (`esp-soc/src/wifi.rs`, `StationLink::next_rx`) delivers
one frame per 400 µs of airtime (`RX_GAP_US`) and none while the last descriptor it filled is
still unrecycled, for up to 50 ms (`RX_STALL_US`), as an 802.11b/g link without A-MPDU would:
about 30 Mbit/s of 1500-byte frames at best.

### Faults

`--coproc c6:...,fault=KIND@WHEN` (repeatable; [cli.md](cli.md)) injects faults at points the run
itself fixes, the Nth command or transfer of a kind or a shared time, so a faulted run is as
reproducible as a clean one, and nothing of it runs without `--coproc`. `cli/tests/cosim.rs` runs
each against the hosted pair and pins ESP-Hosted 2.12.13's reaction:

| fault | what happens on the link | ESP-Hosted's reaction |
| --- | --- | --- |
| `read-crc@N[:K]`, `read-timeout@N` | the Nth FIFO read fails (0x109, 0x107) after the slave gave its data, or at the block holding byte K | `Failed to read data`; the packet is lost; the lost one was an RPC response, which times out after 5 s, and the station example aborts and restarts the P4, which resets the C6 and starts again |
| `reg-read-crc@N` | the Nth register-window read fails | `failed to read registers`, a transport failure and an immediate host restart (`H_TRANSPORT_RESTART_ON_FAILURE`); the pair takes its lease again |
| `write-crc@N`, `write-nostatus@N` | the Nth FIFO write: the card drops the block (CRC status error) or takes it without a status | `Failed to send data`, retried (`MAX_SDIO_WRITE_RETRY`); the lease on time |
| `cmd-timeout@N`, `resp-crc@N` | the Nth command never arrives, or is carried out with a bad response CRC | one error; the link goes on |
| `wedge@T` | the C6's CPU stops, its SDIO hardware answers on, TOKEN and PKT_LEN frozen | silence: no error, no restart, no lease |
| `dead-bus@T` | nothing answers | silence |
| `drop-irq@T` | one DAT1 edge goes unlatched (`SdDevice::edge_lost`), D1 stays low | silence: the receive task already waits on the edge, and checks D1's level only when it starts to wait |
| `spurious-irq@T+D` | D1 low with nothing pending | the host reads the registers, finds nothing; harmless |
| `token-stall@T+D` | TOKEN1 shows no free receive buffer | the host polls TOKEN1 at growing intervals (`sdio_is_write_buffer_available`) and its traffic waits: the lease 0.5 s late |

A C6 reset in the middle of the traffic (`a_c6_reset_mid_traffic_ends_in_a_host_restart`) loses
the RPC in flight and ends in a host restart too. The three silent stalls are the finding: in
this configuration ESP-Hosted's receive task waits for the slave's interrupt with no timeout and
the station example waits for its connect event the same way, so a wedged co-processor, a dead
bus or a single lost DAT1 edge stop the WiFi for good without a log line.

## Compared with silicon

Two Waveshare ESP32-P4-WIFI6 boards, console on UART0 through the board's USB-UART bridge
(2026-10-24/25). Timestamps aside:

- **v1.3 chip** (ROM `esp32p4-eco2-20240710`, 32 MB flash): `p4-hello` and `p4-hello-smp`, both
  boots, print what the goldens hold except the ROM's two banner lines, the heap region below the
  ROM's data (`len 00004BF0`, 12 bytes more than ECO0 leaves), the chip revision, the flash
  size warning for the larger part and, after the two-core image's restart, the value of
  `Core1 Saved PC` (0x12 bytes later here: "Not there yet", core stall). `Core0 Saved PC` is the
  store that requested the reset, on silicon and here (ASSIST_DEBUG 0x48 and 0xC8,
  `AssistDebug`). The two-core image's `Minimum free heap size` is 16 bytes higher on silicon
  beyond those 12. `p4-sdcard` with the socket empty prints the same lines, the timeout 20 ms
  after `Mounting filesystem` on both; with a 64 GB SDHC card the same lines apart from the
  card's name and size; the model's card answers at once, so mounting takes 30 ms of emulated
  time against 40 ms on the board, and the file operations none against 50 ms. The ESP-Hosted
  station (`examples/esp-hosted-p4`) against the board's own C6 firmware (it reports version
  0.0.0) brings SDIO up, identifies the slave and starts WiFi as here; rebuilt for a real 2.4 GHz
  network it connects and gets an address.
- **v3.2 chip** (ROM `esp32p4-eco7-20260109`, 16 MB flash, a 3.5-inch LCD): the rev ≥ 3
  hello_world on one core and on both prints what `hello-p4-rev3` and `hello-p4-rev3-smp` hold
  except the ROM's banner lines, `boot:0x31f` for `boot:0x30f`, the chip revision (v3.2) and
  efuse block revision (v1.2), the flash chip (`generic`) and its size warning, and the same
  `Core1 Saved PC` difference as on v1.3; the two-core `Minimum free heap size` is again 16 bytes
  higher on silicon. The heap region below the ROM's data has the same length on ECO5 and ECO7.
  The sdmmc example built for rev ≥ 3 prints what `sdcard-p4-rev3` holds apart from the card's
  name and size, on the same 64 GB card.

Beyond that, the ROMs and IDF's own sources are what this page is checked against.

## What the bring-up found

- **The ROM waits for the flash controller's AXI FIFOs to report empty**, in SPI_MEM_C's
  SMEM_AXI_ADDR_CTRL (0x178), not at 0x170 as on the C6; IDF waits on the same bits in the PSRAM
  controller.
- **The ROM checks the bootloader's appended SHA-256** with the SHA accelerator before it jumps;
  without one it prints `SHA-256 comparison failed` and boots anyway.
- **The cache controller's size registers must hold their defaults**: IDF derives cache sizes
  from them, so the block is seeded from `cache_reg.h` and every operation reports done at once.
- **`rdcycle` is the P4's cycle count.** The C3 and C6 read it from MPCCR; without `cycle` the
  bootloader's random-number loop never ends.
- **A CLIC control word is four registers.** The ROM and IDF write IE, the attributes and the
  level one byte at a time; a read-modify-write of the whole word would write IP back.
- **The SDIO slave's send link must answer at the register write that starts it.** The driver
  starts a one-byte descriptor and spins on RX_DONE before it does anything else, so the DMA reads
  the descriptor in the write's own bus call (`sdio_slc_write`), not later.
- **IDF reads the D1 pin, not only the host's interrupt bit.** `sdmmc_host_io_int_wait` returns at
  once if D1 is low, so DAT1 is a GPIO input level as well as RINTSTS's edge.
- **Slices must end on a fixed grid.** Cutting a slice where a caller happened to stop moved where
  EN was seen and where the P4's scheduling rounds fell, and the C6's `Saved PC` after its reset
  differed between two drivings of the same run; the lock-step test caught it.
- **A P4 reset restarted the SD bus's clock.** The device clock starts again with the new
  peripheral set, and the SDMMC host took its time from it, so after a P4 software reset every
  command reached the C6 with a time the C6 had passed and it answered up to a slice stale; a
  slot-0 card's busy period was measured on the wrong clock too. Found in review; the SD bus now
  counts from power-on.
- **The host's packet announcement must wait for the length.** Raising RX_NEW_PACKET at the rx
  link's START left a few instructions in which ESP-Hosted's combined register read could see a
  packet with its old length and wait for ever. Found in review, not seen in a run.
- **A restarted P4 glitches EN.** ESP-Hosted makes GPIO 54 an output (its output latch still 0
  after the reset) before it writes the level (`transport_gpio_reset`), so EN is low for a few
  instructions; a slice end can see that, so EN needs the C6's t_RST before it resets the chip.

## What is modelled

| | |
| --- | --- |
| CPU | two RV32IMAFC cores (`Cpu::new_esp32p4`, `misa` 0x40001125): F with the lazy FPU traps, EXT_ILL, Zicntr, CLIC mode, `mhartid`; on rev ≥ 3 the specification's CLIC CSRs (`Cpu::new_esp32p4_rev3`); core 1 held in reset until HP_SYS_CLKRST releases it. Xesppie and the hardware loops decode as illegal |
| Memory | 128 KB mask ROM at 0x4FC00000 (reset vector), 768 KB L2MEM at 0x4FF00000 and its uncached alias at 0x8FF00000, 8 KB TCM at 0x30100000, LP ROM (empty) and 32 KB LP RAM; flash (0x40000000) and PSRAM (0x48000000) windows behind two 1024-entry MMUs of 64 KB pages, reached through SPI_MEM_C / SPI_MEM_S item registers, with their uncached aliases |
| Interrupts | both interrupt matrices (128 sources, rev < 3) and both CLIC register blocks, delivered through the cores' CLIC mode |
| Peripherals | UART0/1, USB-Serial/JTAG, systimer (16 MHz from XTAL), TIMG0/1, SHA, efuse (v1.0 or v3.1, block v0.3, security efuses zero), the flash controller (SPI0 cache side, SPI1 commands, a GigaDevice part answering RDID), the PSRAM controller with an AP Memory x16 PSRAM, the cache controller, HP_SYSTEM (cross-core interrupts, stall status), HP_SYS_CLKRST (divider update, PLL calibration done, core 1 clock and reset), LP_SYSTEM / LP_CLKRST / LP_TIMER (reset cause, software reset, core 1 boot address, RTC counter, the SDMMC reset), the analog I2C master (regi2c read-back), ASSIST_DEBUG's saved PC, the RNG (LP_SYSTEM's RNG_DATA, and LP_TRNG's that rev ≥ 3 builds read), GPIO with the P4's register layout, the SDMMC host with its IDMAC and an SD card in the board's socket (`--sd`), the high-speed USB-OTG core as a device with buffer DMA and a host on its port (`--usb-host`), an SDIO function on slot 1 with its interrupt on DAT1 (`--coproc`: an ESP32-C6 running ESP-Hosted's slave, "The co-processor"), and for `--board waveshare-p4-lcd35` ([boards.md](boards.md)): GPSPI2 (the shared model) with transmit DMA through the AXI-GDMA, I2C1 (its clock from HP_SYS_CLKRST), I2S1 transmitting through the AHB-GDMA at 44.1 kHz, LEDC, and GPIO's pins 49–56 and its interrupt status words; everything else is register RAM, listed by `--log-periph` |
| Time base | 360 MHz, the rev < 3 application clock, on both revisions; one instruction is one cycle |

The revision is a type parameter (`esp32p4::Revision`): `RevLt3` and `RevGe3`, with its own ROM
and the `mintthresh` CSR ("Revisions"); the CLI picks it from the images. The browser build
runs both: `web/wasm/fw/p4-hello.json` boots the rev < 3 hello_world (board `esp32p4`), and
`web/wasm/fw/p4-c6-hosted.json` the rev ≥ 3 ESP-Hosted station with its C6 (board
`esp32p4-rev3+c6`), both run by `tools/wasm-test.mjs`; the firmware dialog offers `esp32p4` and
`esp32p4-rev3`.

## Not there yet

- **Rev ≥ 3 beyond hello_world and ESP-Hosted**: its register-layout differences and 400 MHz
  ("Revisions").
- **Core stall**: PMU's CPU_SW_STALL (stall code 0x86, IDF v5.5.4
  `hal/esp32p4/include/hal/cpu_utility_ll.h`) does not stop the other core, and HP_SYSTEM's
  CORESTALLED_ST reads 0. Nor does LP_CLKRST's HPCORE1_SW_RESET reset core 1: the model clears the
  bit and ignores it. `esp_restart_noos` does both to core 1 before the chip reset, resets it and
  then stalls it (IDF v5.5.4 `esp_system/port/soc/esp32p4/system_internal.c:174-175`), so core 1
  runs on here and its `Core1 Saved PC` is 0x12 bytes later than on silicon, on both revisions
  ("Compared with silicon"); which of the two would fix that is not tried. Anything else that
  relies on stalling the other core does not get it either.
- **The rest of the AXI-GDMA and AHB-GDMA**: the two front ends carry GPSPI2's and I2S1's
  transmit chains only. mbedTLS's SHA and AES in DMA mode run through the AXI GDMA on the P4
  (IDF v5.5.3 `mbedtls/port/crypto_shared_gdma`, `SOC_AXI_GDMA_SUPPORTED`), so SHA_BUSY
  (0x50091018) never clears and the caller spins on it. I2S1's clock dividers (HP_SYS_CLKRST)
  are not read: it streams at 44.1 kHz.
- **The LP core, MIPI-DSI/CSI, DMA2D/PPA**, and Xesppie's save and restore, which FreeRTOS runs
  only for a task that uses PIE.
- **The co-processor**: the SDIO link without timing by default and the slave
  without busy or read wait, CRC or suspend ("The co-processor"); faults are injected on the link
  rather than arising from a model of the bus. `--usb-host` does not run with `--coproc` yet.
- **The temperature sensor** (LP_TSENS) is register RAM: IDF's driver reads OUT as 0, which it
  turns into −48 °C and ESP_FAIL. A model would give a fixed die temperature from OUT and the DAC
  range the driver writes over regi2c; it is a `device_set!` entry and needs a CPU-time receipt.
- **USB**: the full-speed OTG core (0x50040000) and USB_WRAP, host mode, the core's slave mode
  (the data FIFOs), descriptor DMA, isochronous and interrupt endpoints, LPM, the
  per-endpoint interrupt source (94), and the time a transaction takes inside the core: it
  answers at once. The UTMI PHY is register RAM. The host checks the drive between commands: it
  writes only between polls that passed, mounts the volume again after a NOT READY instead of
  keeping its FAT and directories cached as Linux's page and buffer caches do, and writes FAT32 in
  a vfat-like order rather than Linux's writeback order ("The host").
- **SD timing** is off by default: a command and its data are done in the register write that
  starts them. `--sd-timing` times the bus but not the card's read access time.
- **No cache model, so DMA coherency bugs pass.** The data cache is not modelled: the CPU and
  every DMA engine see the same memory at once. Firmware that omits `esp_cache_msync` or whose
  call fails, a DMA buffer that is not cache-line aligned or shares a line with other data, or a
  buffer changed after the DMA was started all work here and can corrupt data on silicon. The
  USB core reads IN data at the IN token (`Dwc2::token_in`), where buffer DMA fetches it once the
  endpoint is enabled, so a buffer the driver changes after setting EPENA goes out changed here
  (dcd_dwc2 cleans the buffer and then enables the endpoint, `edpt_schedule_packets`). A cheap
  checker would keep two bits per 64-byte line of DMA-capable memory: written by the CPU since
  the line was last cleaned, and written by DMA since it was last invalidated. CPU stores set the
  first, DMA writes the second, and the cache controller's sync operations (CACHE_SYNC_CTRL with
  CACHE_SYNC_MAP, CACHE_SYNC_ADDR and CACHE_SYNC_SIZE, cache_reg.h, which `esp_cache_msync` ends
  in) clear them. A DMA read of a line with the first bit set, or a CPU read of a line with the
  second, is a coherency bug to report, with the address and the PC. It sits on every store, so it
  would be behind a flag, off by default.
- **GPIO** has the P4's register layout (`periph::GpioP4`), but pin levels, interrupts and edges
  are the shared model's 49 pins; the P4 has 55.
- **`--boot app`** maps the image and jumps to it without the clocks and caches the bootloader
  would have set.
- **`Machine::run_until_cycle`** still schedules core 0 alone; nothing that drives the P4 needs it.
- **Unmodelled blocks** hello_world touches: PMU, PVT, APB_SARADC, LP_WDT, LP_ANA_PERI, LP_ADC,
  KEYMNG, UART2-4, TRACE0/1, LP_UART, LP_PERI, LP_GPIO and DSI, all configuration the firmware
  writes and reads back (`--log-periph` lists them). The watchdogs never fire.

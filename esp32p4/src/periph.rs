//! ESP32-P4 peripherals.
//!
//! One `device_set!` table with BASE 0, so a block number is the top 20 bits of its address:
//! `0x500ca` is UART0 at 0x500CA000, `0x20800` the CLIC at 0x20800000. Base addresses are IDF
//! v5.5.3 components/soc/esp32p4/register/hw_ver1/soc/reg_base.h and include/soc/clic_reg.h.
//! The shared models (UART, USB-Serial/JTAG, systimer, timer groups, SPI flash controller,
//! efuse) come from `esp-periph`; the chip-only blocks are written here.

use emu_core::{ClockDomain, ClockTree};
use esp_periph::{device_set, mmio, Device, DeviceSet, Dispatch, Misc, RegRam, WriteEffect, NO_SOURCE};
use esp_periph::{Efuse, Gdma, GpSpi, Gpio, Ledc, LedcLayout, Rng, Sha, SpiMem, Systimer, TimerGroup, Uart, UartLayout, UsbSerialJtag};
use esp_periph::sdmmc::{Sdmmc, SdmmcLayout, SLOTS};
use esp_periph::usb_otg::{Dwc2, Dwc2Config};
use esp_periph::{RST_POWERON, RST_SW_CPU, RST_SW_SYS};

/// The emulator's time base: the rev < 3 application clock, CPLL 360 MHz
/// (IDF v5.5.3 sdkconfig CONFIG_ESP_DEFAULT_CPU_FREQ_MHZ_360). One instruction is one cycle.
pub const CPU_HZ: u64 = 360_000_000;

/// Interrupt sources, numbered as IDF v5.5.3 components/soc/esp32p4/include/soc/interrupts.h
/// numbers them; the interrupt matrix has one map register per source at 4 * n
/// (register/hw_ver1/soc/interrupt_core0_reg.h). rev < 3 has 128 sources.
pub mod src {
    pub const USB_SERIAL_JTAG: usize = 22;
    /// ETS_SPI2_INTR_SOURCE: GPSPI2
    pub const GPSPI2: usize = 25;
    pub const I2C1: usize = 45;
    pub const LEDC: usize = 52;
    pub const I2S1: usize = 28;
    /// ETS_AHB_PDMA_IN_CH0..2 (56-58) and OUT_CH0..2 (59-61)
    pub const AHB_DMA_IN_CH0: usize = 56;
    pub const AHB_DMA_OUT_CH0: usize = 59;
    /// ETS_AXI_PDMA_IN_CH0..2 (62-64) and OUT_CH0..2 (65-67)
    pub const AXI_DMA_IN_CH0: usize = 62;
    pub const AXI_DMA_OUT_CH0: usize = 65;
    /// ETS_SDIO_HOST_INTR_SOURCE: the SDMMC host
    pub const SDIO_HOST: usize = 23;
    pub const UART0: usize = 31; pub const UART1: usize = 32;
    pub const TG0_T0: usize = 46; pub const TG0_T1: usize = 47;
    pub const TG1_T0: usize = 49; pub const TG1_T1: usize = 50;
    pub const SYSTIMER_TARGET0: usize = 53;
    pub const GPIO_INTR0: usize = 74;
    /// HP_SYSTEM_CPU_INT_FROM_CPU_0..3: the FreeRTOS yield and the IPC interrupts
    pub const FROM_CPU0: usize = 79;
    /// ETS_USB_OTG_INTR_SOURCE: the high-speed USB-OTG core
    pub const USB_OTG: usize = 93;
    pub const COUNT: usize = 128;
    /// The name IDF gives a source, for diagnostics.
    pub fn name(s: usize) -> &'static str {
        match s {
            USB_SERIAL_JTAG => "USB_SERIAL_JTAG", SDIO_HOST => "SDIO_HOST", GPSPI2 => "SPI2", I2C1 => "I2C1", LEDC => "LEDC",
            I2S1 => "I2S1", 56..=58 => "AHB_PDMA_IN", 59..=61 => "AHB_PDMA_OUT", 62..=64 => "AXI_PDMA_IN", 65..=67 => "AXI_PDMA_OUT", UART0 => "UART0", UART1 => "UART1", TG0_T0 => "TG0_T0", TG0_T1 => "TG0_T1",
            TG1_T0 => "TG1_T0", TG1_T1 => "TG1_T1", 53 => "SYSTIMER_TARGET0", 54 => "SYSTIMER_TARGET1", 55 => "SYSTIMER_TARGET2",
            GPIO_INTR0 => "GPIO_INTR0", 79 => "FROM_CPU_INTR0", 80 => "FROM_CPU_INTR1", 81 => "FROM_CPU_INTR2", 82 => "FROM_CPU_INTR3",
            USB_OTG => "USB_OTG", _ => "?",
        }
    }
}

/// The USB-OTG HS core's register block, DR_REG_USB2_BASE (register/hw_ver1/soc/reg_base.h); its
/// data FIFOs from 0x50001000 are not modelled (buffer DMA does not use them).
pub const USB_OTG_BLOCK: u32 = 0x50000;

/// CLIC interrupt IDs: 0-15 are the core's own, IDF's CPU interrupt lines 0-31 are IDs 16-47
/// (IDF v5.5.3 include/soc/clic_reg.h CLIC_EXT_INTR_NUM_OFFSET, hal/include/hal/interrupt_clic_ll.h).
pub const CLIC_IDS: usize = 48;
pub const CLIC_EXT_OFFSET: u32 = 16;

/// The two interrupt matrices (0x500D6000 core 0, +0x800 core 1): a map register per source
/// holding the CLIC ID it raises, then INTR_STATUS_0..3 at 0x200 (read-only, filled from the
/// source state in `pre_access`), CLOCK_GATE at 0x210 and DATE at 0x3FC.
/// IDF v5.5.3 register/hw_ver1/soc/interrupt_core0_reg.h:1550-1616.
pub struct IntMatrix { pub map: [[u32; src::COUNT]; 2], pub status: [u32; 4], ram: RegRam }
impl Default for IntMatrix { fn default() -> Self { Self::new() } }
impl IntMatrix {
    pub fn new() -> Self {
        let mut ram = RegRam::new();
        for core in 0..2 { ram.write(core * 0x800 + 0x210, 1); ram.write(core * 0x800 + 0x3fc, 0x0200_3020); }   // REG_CLK_EN, DATE
        IntMatrix { map: [[0; src::COUNT]; 2], status: [0; 4], ram }
    }
}
impl Device for IntMatrix {
    fn read(&mut self, off: u32) -> u32 {
        let (core, o) = ((off >> 11) as usize & 1, off & 0x7ff);
        match o {
            0x000..=0x1fc => self.map[core][(o / 4) as usize],
            0x200..=0x20c => self.status[((o - 0x200) / 4) as usize],
            _ => self.ram.read(off),
        }
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        let (core, o) = ((off >> 11) as usize & 1, off & 0x7ff);
        match o {
            0x000..=0x1fc => self.map[core][(o / 4) as usize] = v & 0x3f,
            0x200..=0x20c => {}
            _ => self.ram.write(off, v),
        }
        WriteEffect::INTMAP
    }
}

/// One core's CLIC register block. `CLIC_INT_CTRL(i)` packs four byte registers: IP (byte 0,
/// bit 0), IE (byte 1, bit 8), the attributes (byte 2: SHV bit 16, TRIG bits 18:17, MODE bits
/// 23:22, read-only machine mode) and CTL (byte 3; reset 0x1F); with NLBITS = 3 the level is
/// CTL[7:5], the unimplemented low bits reading as ones. IDF v5.5.3 include/soc/clic_reg.h.
/// A level-triggered ID's IP follows its matrix input. An edge-triggered ID's IP latches the
/// rising edge, clears when the core takes it hardware-vectored (CLIC specification) and when
/// software writes 1 to it without changing the rest of the word, which is how IDF acknowledges
/// one (riscv/include/esp_private/interrupt_clic.h:321-324, `rv_utils_intr_edge_ack`, "clearing
/// its pending flag" in esp_hw_support/include/hal/interrupt_controller_hal.h:226).
#[derive(Clone)]
pub struct ClicCore {
    /// CLIC_INT_CONFIG.MNLBITS (bits 4:1); NVBITS reads 1
    pub mnlbits: u32,
    /// CLIC_INT_THRESH: on rev < 3 the machine-mode threshold is this byte, bits 31:24
    /// (IDF v5.5.3 riscv/include/esp_private/interrupt_clic.h:50-58)
    pub thresh: u32,
    /// IE, the attributes and CTL of each ID; IP is kept apart
    pub ctrl: [u32; CLIC_IDS],
    /// edge-triggered IDs: IP latched by an edge
    pub ip_latch: u64,
    /// IDs whose interrupt-matrix input is asserted now
    pub input: u64,
}
impl Default for ClicCore { fn default() -> Self { ClicCore { mnlbits: 0, thresh: 0, ctrl: [0x1f << 24 | 3 << 22; CLIC_IDS], ip_latch: 0, input: 0 } } }
impl ClicCore {
    fn edge(&self, id: usize) -> bool { self.ctrl[id] & (1 << 17) != 0 }
    pub fn pending(&self, id: usize) -> bool { if self.edge(id) { self.ip_latch & (1 << id) != 0 } else { self.input & (1 << id) != 0 } }
    pub fn enabled(&self, id: usize) -> bool { self.ctrl[id] & (1 << 8) != 0 }
    /// The 8-bit level of an ID: CTL with the bits below NLBITS read as ones.
    pub fn level(&self, id: usize) -> u32 { (self.ctrl[id] >> 24) | 0x1f }
    pub fn vectored(&self, id: usize) -> bool { self.ctrl[id] & (1 << 16) != 0 }
    /// The highest-level enabled, pending ID above the threshold, the higher ID on a tie (the
    /// RISC-V CLIC specification's rule): what the core takes if MIE is set and its own level
    /// is lower.
    pub fn candidate(&self) -> Option<(usize, u32)> {
        let thresh = self.thresh >> 24;
        let mut best: Option<(usize, u32)> = None;
        for id in 0..CLIC_IDS {
            if !self.enabled(id) || !self.pending(id) { continue; }
            let l = self.level(id);
            if l > thresh && best.is_none_or(|(_, b)| l >= b) { best = Some((id, l)); }
        }
        best
    }
    /// A new matrix input: edge-triggered IDs latch the rising edges (TRIG bit 18 selects the
    /// falling edge, not used by IDF).
    pub fn set_input(&mut self, input: u64) {
        let rising = input & !self.input;
        for id in 0..CLIC_IDS { if rising & (1 << id) != 0 && self.edge(id) { self.ip_latch |= 1 << id; } }
        self.input = input;
    }
    /// The core took `id`: a hardware-vectored edge interrupt is no longer pending.
    pub fn taken(&mut self, id: usize) { if id < CLIC_IDS && self.edge(id) && self.vectored(id) { self.ip_latch &= !(1 << id); } }
    fn read(&self, off: u32) -> u32 {
        match off {
            0x0 => 1 | (self.mnlbits & 0x1e),                                                     // INT_CONFIG: NVBITS = 1
            0x4 => CLIC_IDS as u32 | 3 << 21,                                                     // INT_INFO: NUM_INT 48, CTLBITS 3
            0x8 => self.thresh,
            0x1000..=0x10bc => { let id = ((off - 0x1000) / 4) as usize; self.ctrl[id] | self.pending(id) as u32 }
            _ => 0,
        }
    }
    /// A write of the byte lanes in `lanes` (bit n = byte n): the ROM and IDF write the four byte
    /// registers of a control word one at a time.
    fn write(&mut self, off: u32, v: u32, lanes: u32) {
        let mask = (0..4).filter(|b| lanes & (1 << b) != 0).fold(0u32, |m, b| m | 0xff << (8 * b));
        match off {
            0x0 => self.mnlbits = (self.mnlbits & !mask) | (v & mask & 0x1e),
            0x8 => self.thresh = (self.thresh & !mask) | (v & mask & 0xff00_0000),
            0x1000..=0x10bc => {
                let id = ((off - 0x1000) / 4) as usize;
                let before = self.ctrl[id];
                self.ctrl[id] = (self.ctrl[id] & !mask) | (v & mask & 0xff07_0100) | 3 << 22;
                // IDF acknowledges an edge with a 32-bit REG_SET_BIT of IP (interrupt_clic.h:321-324)
                // and reconfigures SHV and TRIG with the same read-modify-write (interrupt_clic_ll.h:
                // 99-102, esp_rom/patches/esp_rom_clic.c), which writes a pending IP back as 1: only a
                // write that changes nothing else is the acknowledge.
                if lanes & 1 != 0 && v & 1 != 0 && self.edge(id) && self.ctrl[id] == before { self.ip_latch &= !(1 << id); }
            }
            _ => {}
        }
    }
}

/// The CLIC of both cores: core 0's at 0x20800000 (configuration) and 0x20801000 (one control
/// word per ID), core 1's at +0x10000 (IDF v5.5.3 include/soc/clic_reg.h DUALCORE_CLIC_CTRL_OFF).
/// Here offsets 0x00000-0x01FFF are core 0's block and 0x10000-0x11FFF core 1's; silicon decodes
/// the window per core, each core seeing its own block first, and the bus swaps the halves when
/// core 1 is the one running (`SocBus::cur_core`).
#[derive(Default)]
pub struct Clic { pub core: [ClicCore; 2], log: bool }
impl Clic {
    pub fn write_lanes(&mut self, off: u32, v: u32, lanes: u32) {
        if self.log { eprintln!("[clic] core{} {:#06x} <- {:#010x} (lanes {:#x})", (off >> 16) & 1, off & 0xffff, v, lanes); }
        self.core[(off >> 16) as usize & 1].write(off & 0xffff, v, lanes);
    }
}
impl Device for Clic {
    fn read(&mut self, off: u32) -> u32 { self.core[(off >> 16) as usize & 1].read(off & 0xffff) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { self.write_lanes(off, v, 0xf); WriteEffect::INTMAP }
    fn debug(&mut self, on: bool) { self.log = on; }
}

/// The L1/L2 cache controller (0x3FF10000). Configuration reads back, from the register
/// defaults; every operation finishes at once, so its DONE bit reads 1 and its start bit
/// clears; a freeze is done as soon as it is asked for.
/// IDF v5.5.3 register/hw_ver1/soc/cache_reg.h.
pub struct Cache { ram: RegRam }
impl Default for Cache { fn default() -> Self { Self::new() } }
impl Cache {
    /// Non-zero register defaults (cache_reg.h, the `default:` of each field).
    const DEFAULTS: [(u32, u32); 28] = [
        (0x00c, 0x1), (0x010, 0x40), (0x014, 0x8), (0x018, 0x100), (0x01c, 0x8), (0x024, 0x55555), (0x028, 0x55555),
        (0x030, 0x33333), (0x034, 0x33333), (0x044, 0x3fff3fff), (0x054, 0x3fff3fff), (0x064, 0x3fff3fff), (0x074, 0x3fff3fff),
        (0x084, 0x3fff3fff), (0x09c, 0x1f), (0x264, 0x4000_0000), (0x270, 0x10), (0x278, 0x400), (0x27c, 0x8),
        (0x284, 0x50_0000), (0x288, 0x50_0000), (0x290, 0x30_0000), (0x294, 0x30_0000), (0x2a4, 0xffff_ffff),
        (0x3bc, 0xf), (0x3c8, 0x4000_0000), (0x3d4, 0x1), (0x3fc, 0x0230_4130),
    ];
    const PRELOAD: [u32; 6] = [0x0a8, 0x0b4, 0x0c0, 0x0cc, 0x0d8, 0x2a8];
    const AUTOLOAD: [u32; 6] = [0x0e4, 0x0f8, 0x10c, 0x120, 0x134, 0x2b4];
    pub fn new() -> Self { let mut ram = RegRam::new(); for (o, v) in Self::DEFAULTS { ram.write(o, v); } Cache { ram } }
}
impl Device for Cache {
    fn read(&mut self, off: u32) -> u32 {
        let v = self.ram.read(off);
        match off {
            0x02c => (v & 0x1_1111) << 2 | v & !(0x1_1111 << 2),     // L1 FREEZE_CTRL: each FREEZE_DONE follows its FREEZE_EN
            0x28c => (v & 1 << 20) << 2 | v & !(1 << 22),            // L2 FREEZE_CTRL: FREEZE_DONE follows FREEZE_EN
            0x088 => v | 1 << 2,                                      // LOCK_CTRL: LOCK_DONE
            0x098 => v | 1 << 4,                                      // SYNC_CTRL: SYNC_DONE
            o if Self::PRELOAD.contains(&o) || Self::AUTOLOAD.contains(&o) => v | 1 << 1,   // PRELOAD_DONE, AUTOLOAD_DONE
            _ => v,
        }
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        let v = match off {
            0x088 => v & !3,                                          // LOCK_ENA, UNLOCK_ENA self-clear
            0x098 => v & !0xf,                                        // INVALIDATE/CLEAN/WRITEBACK(_INVALIDATE)_ENA self-clear
            o if Self::PRELOAD.contains(&o) => v & !1,                // PRELOAD_ENA self-clears
            _ => v,
        };
        self.ram.write(off, v);
        WriteEffect::NONE
    }
}

/// HP_SYSTEM (0x500E5000): the four software interrupts the cores raise on each other
/// (CPU_INT_FROM_CPU_0..3 at 0x10, bit 0) and the core-stall status.
/// IDF v5.5.3 register/hw_ver1/soc/hp_system_reg.h:40-88, 362-380.
#[derive(Default)]
pub struct HpSys { pub from_cpu: u32, ram: RegRam }
impl Device for HpSys {
    fn read(&mut self, off: u32) -> u32 {
        match off {
            0x10..=0x1c => (self.from_cpu >> ((off - 0x10) / 4)) & 1,
            0x64 => 0,                                                // CPU_CORESTALLED_ST: no core stalled
            _ => self.ram.read(off),
        }
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        match off {
            0x10..=0x1c => { let b = (off - 0x10) / 4; if v & 1 != 0 { self.from_cpu |= 1 << b } else { self.from_cpu &= !(1 << b) } }
            _ => self.ram.write(off, v),
        }
        WriteEffect::NONE
    }
    fn irq_sources(&self) -> u64 { (self.from_cpu & 0xf) as u64 }
}

/// HP_SYS_CLKRST (0x500E6000): clock dividers, gates and resets. Configuration reads back, from
/// the register defaults for the fields the boot reads before writing them; the divider-update
/// strobe clears at once and every PLL calibration reports done.
/// IDF v5.5.3 register/hw_ver1/soc/hp_sys_clkrst_reg.h:29-50 (ROOT_CLK_CTRL0), the
/// SOC_CLK_CTRL0 and HP_RST_EN0 defaults, 2989-3040 (ANA_PLL_CTRL0).
pub struct HpClkRst { ram: RegRam }
impl Default for HpClkRst { fn default() -> Self { Self::new() } }
impl HpClkRst {
    pub const SOC_CLK_CTRL0: u32 = 0x14;
    pub const HP_RST_EN0: u32 = 0xc0;
    pub fn new() -> Self {
        let mut ram = RegRam::new();
        ram.write(Self::SOC_CLK_CTRL0, 0xe6df_97af);                  // CORE1_CPU_CLK_EN (bit 4) off
        ram.write(Self::HP_RST_EN0, 0x100);                            // RST_EN_CORE1_GLOBAL (bit 8) on
        HpClkRst { ram }
    }
    /// The SDMMC host's clock into its card-clock dividers: PERI_CLK_CTRL01.REG_SDIO_LS_CLK_SRC_SEL
    /// (bit 23) picks PLL_F160M (0) or the 200 MHz SDIO PLL (1), PERI_CLK_CTRL02's SDIO_LS edges
    /// divide it by EDGE_L + 1, by 1 when all three are 0 (hp_sys_clkrst_reg.h:1150-1270;
    /// sdmmc_ll_select_clk_source, sdmmc_ll_get_clock_div in hal/esp32p4/include/hal/sdmmc_ll.h).
    pub fn sdio_ls_hz(&self) -> u64 {
        let src = if self.ram.read(0x34) >> 23 & 1 != 0 { 200_000_000 } else { 160_000_000 };
        let edges = self.ram.read(0x38);
        let (l, h, n) = (edges >> 9 & 0xf, edges >> 13 & 0xf, edges >> 17 & 0xf);
        src / if l == 0 && h == 0 && n == 0 { 1 } else { l as u64 + 1 }
    }
    /// Core 1 runs once its clock is on and its global reset released (hal/esp32p4/include/hal/cpu_utility_ll.h).
    pub fn core1_released(&self) -> bool { self.ram.read(Self::SOC_CLK_CTRL0) & (1 << 4) != 0 && self.ram.read(Self::HP_RST_EN0) & (1 << 8) == 0 }
}
impl Device for HpClkRst {
    fn read(&mut self, off: u32) -> u32 {
        let v = self.ram.read(off);
        match off {
            0x04 => v & !(1 << 4),                                    // ROOT_CLK_CTRL0.SOC_CLK_DIV_UPDATE
            0xbc => v | 0x155,                                        // ANA_PLL_CTRL0: PLLA, CPU, SDIO, SYS PLL and MSPI CAL_END
            _ => v,
        }
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { self.ram.write(off, v); WriteEffect::NONE }
}

/// The always-on LP blocks as one device: LP_SYSTEM at 0x000 (software reset, core 1's boot
/// address, the store registers), LP_CLKRST at 0x1000 (reset cause, per-core software reset),
/// LP_TIMER at 0x2000 (the RTC counter). All of it survives a chip reset.
/// IDF v5.5.3 register/hw_ver1/soc/lp_system_reg.h:48-152, 838; lp_clkrst_reg.h:231-473;
/// lp_timer_reg.h.
pub struct LpAon {
    pub ram: [RegRam; 3],
    pub reset_cause: u32,
    pub sw_reset: bool,
    /// RTC slow-clock ticks since power-on and the value latched by an UPDATE
    pub slow_ticks: u64,
    pub time_latch: u64,
}
impl Default for LpAon { fn default() -> Self { Self::new() } }
impl LpAon {
    pub const BOOT_ADDR_HP_CORE1: u32 = 0x164;
    pub fn new() -> Self {
        let mut ram = [RegRam::new(), RegRam::new(), RegRam::new()];
        ram[0].write(0x08, 0x1fff_c7f8);                              // LP_SYSTEM_REG_SYS_CTRL default
        LpAon { ram, reset_cause: RST_POWERON, sw_reset: false, slow_ticks: 0, time_latch: 0 }
    }
    fn request_reset(&mut self, cause: u32) { self.reset_cause = cause; self.sw_reset = true; }
    pub fn core1_boot_addr(&self) -> u32 { self.ram[0].read(Self::BOOT_ADDR_HP_CORE1) }
}
impl Device for LpAon {
    fn read(&mut self, off: u32) -> u32 {
        let (blk, o) = ((off >> 12) as usize, off & 0xfff);
        match (blk, o) {
            (0, 0x08) => self.ram[0].read(o) & !(1 << 1),             // SYS_SW_RST reads 0
            (1, 0x10) => (self.ram[1].read(o) & !(0x3f << 7)) | (self.reset_cause & 0x3f) << 7,   // RESET_CAUSE.HPCORE0_RESET_CAUSE
            (1, 0x14) => self.ram[1].read(o) & !(1 << 13 | 1 << 29),  // HPCPU_RESET_CTRL0: HPCORE0/1_SW_RESET read 0
            (2, 0x10) => self.ram[2].read(o) & !(1 << 28),            // LP_TIMER_UPDATE self-clears
            (2, 0x14) => self.time_latch as u32,                      // LP_TIMER_MAIN_BUF0_LOW
            (2, 0x18) => (self.time_latch >> 32) as u32,              // LP_TIMER_MAIN_BUF0_HIGH
            (b, o) if b < 3 => self.ram[b].read(o),
            _ => 0,
        }
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        let (blk, o) = ((off >> 12) as usize, off & 0xfff);
        match (blk, o) {
            (0, 0x08) => { if v & (1 << 1) != 0 { self.request_reset(RST_SW_SYS); } self.ram[0].write(o, v & !(1 << 1)); }
            (1, 0x14) => { if v & (1 << 13) != 0 { self.request_reset(RST_SW_CPU); } self.ram[1].write(o, v & !(1 << 13 | 1 << 29)); }
            // HP_SDMMC_EMAC_RST_CTRL.RST_EN_SDMMC (bit 28) resets the SDMMC host
            // (lp_clkrst_reg.h:989-1005; sdmmc_ll_reset_register pulses it)
            (1, 0x4c) => { self.ram[1].write(o, v); if v & (1 << 28) != 0 { return WriteEffect::PERIPH_RESET; } }
            // HP_USB_CLKRST_CTRL1.RST_EN_USB_OTG20 (bit 2) holds the USB-OTG HS core in reset
            // (lp_clkrst_reg.h:928-952; _usb_utmi_ll_reset_register pulses it)
            (1, 0x48) => { let old = self.ram[1].read(o); self.ram[1].write(o, v); if (old ^ v) & (1 << 2) != 0 { return WriteEffect::PERIPH_RESET; } }
            (2, 0x10) => { if v & (1 << 28) != 0 { self.time_latch = self.slow_ticks; } self.ram[2].write(o, v); }
            (b, o) if b < 3 => self.ram[b].write(o, v),
            _ => {}
        }
        WriteEffect::NONE
    }
    fn clock(&self) -> Option<ClockDomain> { Some(ClockDomain::RtcSlow) }
    fn tick(&mut self, ticks: u64) { self.slow_ticks += ticks; }
}

/// The analog I2C master (I2C_ANA_MST, 0x50124000): the regi2c path to the PLL, bias and SAR
/// trim registers, a CTRL word per host — [7:0] block, [15:8] register, [23:16] data, [24]
/// write, [25] busy. A transfer completes at once; written values read back per (block,
/// register), which is how the clock code reads back the CPU PLL divider it programmed.
/// IDF v5.5.3 components/esp_rom/patches/esp_rom_regi2c_esp32p4.c, register/hw_ver1/soc/i2c_ana_mst_reg.h.
#[derive(Default)]
pub struct AnaMst { ram: RegRam, pub ana: esp_periph::Regi2c }
impl Device for AnaMst {
    fn read(&mut self, off: u32) -> u32 {
        match off {
            0x00 | 0x04 => self.ana.read(0, self.ram.read(off), |_| &0),
            _ => self.ram.read(off),
        }
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        if off == 0x00 || off == 0x04 { self.ana.write(0, v); }
        self.ram.write(off, v);
        WriteEffect::NONE
    }
}

/// ASSIST_DEBUG (0x3FF06000): the ROM prints each core's recorded PC as `CoreN Saved PC` after a
/// reset, and nothing for a core whose register reads 0. IDF v5.5.4 register/hw_ver1/soc/
/// assist_debug_reg.h: CORE_0_RCD_PDEBUGPC at 0x48, CORE_1_RCD_PDEBUGPC at 0xC8; the ECO0 ROM's
/// `main` reads both (0x4fc028e4-0x4fc02918).
#[derive(Default)]
pub struct AssistDebug { pub saved_pc: [u32; 2], ram: RegRam }
impl Device for AssistDebug {
    fn read(&mut self, off: u32) -> u32 { match off { 0x48 => self.saved_pc[0], 0xc8 => self.saved_pc[1], _ => self.ram.read(off) } }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { self.ram.write(off, v); WriteEffect::NONE }
}

/// The systimer counts XTAL / 2.5 = 16 MHz (IDF v5.5.3 include/soc/soc_caps.h
/// SOC_SYSTIMER_FIXED_DIVIDER), which is 22.5 CPU cycles at 360 MHz: it is fed XTAL ticks (9
/// cycles) and steps two counts per five of them, so it keeps exact time.
#[derive(Default)]
pub struct SystimerP4 { pub st: Systimer, frac: u64 }
impl Device for SystimerP4 {
    fn read(&mut self, off: u32) -> u32 { self.st.read(off) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { self.st.write(off, v); WriteEffect::NONE }
    fn irq_sources(&self) -> u64 { Device::irq_sources(&self.st) }
    fn clock(&self) -> Option<ClockDomain> { Some(ClockDomain::Xtal) }
    fn tick(&mut self, xtal: u64) {
        self.frac += xtal * 2;
        let n = self.frac / 5;
        self.frac %= 5;
        if n > 0 { self.st.tick(n); }
    }
    fn has_deadline(&self) -> bool { true }
    fn next_deadline(&self) -> Option<u64> { self.st.next_deadline().map(|t| (t * 5).saturating_sub(self.frac).div_ceil(2)) }
}

/// The cache side of a memory controller (SPI_MEM_C for flash, SPI_MEM_S for PSRAM):
/// SMEM_AXI_ADDR_CTRL (0x178) reports its AXI FIFOs empty, which the ROM's `SPI_init` and IDF's
/// `psram_ctrlr_ll_wait_all_transaction_done` wait for; the rest is the shared model or register RAM.
/// IDF v5.5.3 register/hw_ver1/soc/spi_mem_c_reg.h:1675-1720, spi_mem_s_reg.h:2168-2200.
pub struct MspiCache<D: Device>(pub D);
impl<D: Device> Device for MspiCache<D> {
    fn read(&mut self, off: u32) -> u32 { match off { 0x178 => self.0.read(off) | 0xfc00_0000, _ => self.0.read(off) } }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { self.0.write(off, v) }
    fn debug(&mut self, on: bool) { self.0.debug(on) }
}

/// The efuse block as this P4 reads it: wafer and block revision in RD_MAC_SYS_2 (0x4C), the MAC
/// in RD_MAC_SYS_0/1 (0x44/0x48), every security efuse zero.
/// IDF v5.5.3 register/hw_ver1/soc/efuse_struct.h:553-624, hal/esp32p4/include/hal/efuse_ll.h:80-110.
pub fn efuse_p4(mac: [u8; 6], wafer: (u32, u32), blk: (u32, u32)) -> Efuse {
    let mut e = Efuse::new(mac);
    e.write(0x48, (mac[0] as u32) << 8 | mac[1] as u32);
    e.write(0x4c, (wafer.1 & 0xf) | (wafer.0 & 3) << 4 | (blk.1 & 7) << 8 | (blk.0 & 3) << 11 | (wafer.0 >> 2 & 1) << 23);
    e.write(0x6c, 0);
    e
}

/// A flash controller with the reference board's GigaDevice part answering RDID: manufacturer
/// 0xC8, memory type 0x40 (its console prints `detected chip: gd`); the capacity byte follows
/// the flash size. PSRAM has a controller of its own on this chip.
fn flash_controller(spi1: bool) -> SpiMem {
    let mut s = SpiMem::new(spi1);
    s.has_psram = false;
    s.jedec = [0xc8, 0x40, 0x18];
    s
}

/// The PSRAM controller's command side with an AP Memory hex (x16) PSRAM on CS1, which the
/// shared controller model already speaks: register read 0x4040 and write 0xC0C0, sync read
/// 0x0000 and write 0x8080 (IDF v5.5.3 components/esp_psram/device/esp_psram_impl_ap_hex.c:20-30).
/// The mode registers read what the reference board's 32 MB part reports after IDF has set it
/// up: MR0 0x30 (fixed latency, read latency 4, drive 0), MR1 vendor 0x0D, MR2 0xDF (256 Mbit,
/// generation 4, good die), MR3 0x20 (slow refresh), MR8 0x4B (2048-byte wrapped bursts, x16);
/// the bits the board's console does not show are zero.
fn psram_controller() -> SpiMem {
    let mut s = SpiMem::new(true);
    s.psram_mr = [0x30, 0x0d, 0xdf, 0x20, 0, 0, 0, 0, 0x4b];
    s
}

/// The P4's GPIO matrix on the shared model, which has the C6's layout and 49 pins. The P4's 57
/// PIN registers from 0x74 put FUNCn_IN_SEL_CFG at 0x158 and FUNCn_OUT_SEL_CFG at 0x558, four
/// bytes above the C6's, so both arrays move down four bytes into the model; PIN49-56 and
/// OUT_SEL49-56 are register RAM. FUNCn_IN_SEL resets to 0x3F (constant high), or to 0x3E
/// (constant low) for the signals in `IN_SEL_LOW`.
/// IDF v5.5.3 register/hw_ver1/soc/gpio_reg.h, include/soc/gpio_pins.h.
pub struct GpioP4 { pub g: Gpio }
const IN_SEL_LOW: [(usize, usize); 10] = [(10, 45), (47, 66), (74, 78), (89, 114), (117, 118), (126, 127), (130, 133), (138, 156), (158, 203), (214, 255)];
impl Default for GpioP4 { fn default() -> Self { Self::new() } }
impl GpioP4 {
    pub fn new() -> Self {
        let mut g = Gpio::with_pins(57);
        g.strap = 0x30f;
        g.func_in_sel = [0x3f; 256];
        for (lo, hi) in IN_SEL_LOW { for s in lo..=hi { g.func_in_sel[s] = 0x3e; } }
        GpioP4 { g }
    }
    /// The level an input signal sees: FUNCn_IN_SEL bits 5:0 pick a pin or a constant (0x3F
    /// high, 0x3E low), bit 6 inverts.
    pub fn in_signal(&self, sig: usize) -> bool {
        let v = self.g.func_in_sel[sig];
        let level = match v & 0x3f { 0x3f => true, 0x3e => false, p => self.g.level(p as u8) };
        level ^ (v & 0x40 != 0)
    }
    /// FUNCn_OUT_SEL bits 8:0 of a pin: the output signal on it, 256 for GPIO_OUT.
    pub fn out_signal(&self, pin: u8) -> u32 { self.g.func_out_sel.get(pin as usize).map_or(0x100, |v| v & 0x1ff) }
    /// The level a pin's GPIO_OUT drives while its output is enabled, through pin 63. The
    /// W1TS/W1TC registers reach every bit of OUT1 and ENABLE1 (pins 32-56, gpio_reg.h:65-161);
    /// the IO MUX function of pins 49-56 is not checked.
    pub fn drive(&self, pin: u8) -> Option<bool> {
        let bit = 1u64.checked_shl(pin as u32)?;
        (self.g.enable & bit != 0).then_some(self.g.out & bit != 0)
    }
}
impl Device for GpioP4 {
    /// The P4's 57 pins: PIN49-56 (0x138-0x154) past the shared S3 map, which stops at PIN48, and
    /// the matrix tables one word later than the S3's (FUNC0_IN_SEL 0x158, FUNC0_OUT_SEL 0x558),
    /// FUNC49-56_OUT_SEL at 0x61C-0x638 (register/hw_ver3/soc/gpio_reg.h). The interrupt status
    /// is INTR_0 / INTR1_0 (0x5C, 0x60: pins 0-31, 32-56) for interrupt 0 and INTR_1 / INTR1_1
    /// (0x64, 0x68) for interrupt 1, which `gpio_ll_get_intr_status(_high)` reads (hal/gpio_ll.h:157-174);
    /// both show the latched status, as the shared model's PCPU_INT does.
    fn read(&mut self, off: u32) -> u32 {
        match off {
            0x5c | 0x64 => self.g.read(0x44),
            0x60 | 0x68 => self.g.read(0x50),
            0x138..=0x154 => self.g.pin[((off - 0x74) / 4) as usize],
            0x61c..=0x638 => self.g.func_out_sel[((off - 0x558) / 4) as usize],
            0x158..=0x618 => self.g.read(off - 4),
            _ => self.g.read(off),
        }
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        match off {
            0x138..=0x154 => self.g.set_pin_config(((off - 0x74) / 4) as usize, v),
            0x61c..=0x638 => self.g.func_out_sel[((off - 0x558) / 4) as usize] = v,
            0x158..=0x618 => self.g.write(off - 4, v),
            _ => self.g.write(off, v),
        }
        WriteEffect::NONE
    }
    fn irq_sources(&self) -> u64 { self.g.irq() as u64 }
}

/// The AXI-GDMA (0x5008A000): three channel pairs, the IN channels' blocks at 0x68 * n, the OUT
/// channels' at 0x138 + 0x68 * n, each register in the order conf/interrupts first
/// (INT_RAW, INT_ST, INT_ENA, INT_CLR, CONF0, CONF1, FIFO status, POP/PUSH, LINK1, LINK2,
/// STATE, EOF addresses, DSCR, DSCR_BF0/1, PRI, PERI_SEL), the CRC registers after them not
/// modelled (IDF v5.5.4 register/hw_ver3/soc/axi_dma_reg.h). The transfers are the shared
/// GDMA's: this maps the layout onto it. Two differences: a descriptor address is a whole word
/// (LINK2), not 20 bits, and the start/stop bits are in LINK1 (IN: AUTO_RET 0, STOP 1, START 2,
/// RESTART 3, PARK 4; OUT: STOP 0, START 1, RESTART 2, PARK 3). The interrupt bits are the
/// shared model's (OUT_DONE 0, OUT_EOF 1, OUT_DSCR_ERR 2, OUT_TOTAL_EOF 3). GPSPI2, LCD0 and
/// PARLIO are its peripherals (include/soc/gdma_channel.h: SPI2 is trigger 1).
pub struct AxiDma { pub gdma: Gdma, link2: [[u32; 3]; 2], ram: RegRam }
impl Default for AxiDma { fn default() -> Self { Self::new() } }
impl AxiDma {
    pub const CHANNELS: usize = 3;
    pub fn new() -> Self { AxiDma { gdma: Gdma::new(), link2: [[0; 3]; 2], ram: RegRam::new() } }
    /// (out, channel, register within its block) of an offset in the channel area
    fn locate(off: u32) -> Option<(bool, usize, u32)> {
        let (out, rel) = if off < 0x138 { (false, off) } else if off < 0x270 { (true, off - 0x138) } else { return None };
        Some((out, (rel / 0x68) as usize, rel % 0x68))
    }
    /// The shared model's offset for an AXI register other than LINK1/LINK2.
    fn shared(out: bool, ch: usize, k: u32) -> Option<u32> {
        let t = match k {
            0x00 => 0x08, 0x04 => 0x0c, 0x08 => 0x10, 0x0c => 0x14, 0x10 => 0x00, 0x14 => 0x04, 0x18 => 0x18, 0x1c => 0x1c,
            0x28 => 0x24, 0x2c => 0x28, 0x30 => 0x2c, 0x34 => 0x30, 0x38 => 0x34, 0x3c => 0x38, 0x40 => 0x44, 0x44 => 0x48,
            _ => return None,
        };
        Some(ch as u32 * 0xc0 + if out { 0x60 } else { 0 } + t)
    }
}
impl Device for AxiDma {
    fn read(&mut self, off: u32) -> u32 {
        if let Some((out, ch, k)) = Self::locate(off) {
            let running = if out { self.gdma.out[ch].running } else { self.gdma.inp[ch].running };
            return match k {
                0x20 => if running { 0 } else if out { 1 << 3 } else { 1 << 4 },     // PARK: the link is idle
                0x24 => self.link2[out as usize][ch],
                _ => match Self::shared(out, ch, k) { Some(o) => self.gdma.read(o), None => self.ram.read(off) },
            };
        }
        match off { 0x28c..=0x2a0 => 1, _ => self.ram.read(off) }                   // IN/OUT_RESET_AVAIL: a reset is allowed
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        let Some((out, ch, k)) = Self::locate(off) else { self.ram.write(off, v); return WriteEffect::NONE };
        match k {
            0x24 => self.link2[out as usize][ch] = v,
            0x20 => {
                let (stop, start, restart) = if out { (v & 1, v >> 1 & 1, v >> 2 & 1) } else { (v >> 1 & 1, v >> 2 & 1, v >> 3 & 1) };
                let addr = self.link2[out as usize][ch];
                self.gdma.addr_base = addr & 0xfff0_0000;
                // the shared LINK: address 19:0, then OUT STOP 20 / START 21 / RESTART 22, IN STOP 21 / START 22 / RESTART 23
                let s = if out { 20 } else { 21 };
                let word = (addr & 0xf_ffff) | stop << s | start << (s + 1) | restart << (s + 2);
                self.gdma.write(ch as u32 * 0xc0 + if out { 0x80 } else { 0x20 }, word);
            }
            _ => match Self::shared(out, ch, k) { Some(o) => self.gdma.write(o, v), None => self.ram.write(off, v) },
        }
        WriteEffect::NONE
    }
    /// bits 0..3 the OUT channels, 5..8 the IN channels, as the shared model numbers them
    fn irq_sources(&self) -> u64 { Device::irq_sources(&self.gdma) }
    fn debug(&mut self, on: bool) { self.gdma.dbg = on; }
}

/// The AHB-GDMA (0x50085000): the C6's GDMA layout (each channel's IN and OUT interrupt blocks
/// at 0x10 * n and 0x30 + 0x10 * n, MISC_CONF 0x64, the channel blocks from 0x70 every 0xC0, OUT
/// at +0x60), with the LINK registers' start/stop bits placed as on the AXI-GDMA and the
/// descriptor address in its own word: IN_LINK_ADDR 0x3AC + 4n, OUT_LINK_ADDR 0x3B8 + 4n (IDF
/// v5.5.4 register/hw_ver3/soc/ahb_dma_reg.h). I2S0-2 are its peripherals (triggers 3-5).
pub struct AhbDma { pub gdma: Gdma, link_addr: [[u32; 3]; 2], ram: RegRam }
impl Default for AhbDma { fn default() -> Self { Self::new() } }
impl AhbDma {
    pub fn new() -> Self { AhbDma { gdma: Gdma::new(), link_addr: [[0; 3]; 2], ram: RegRam::new() } }
    /// The shared model's offset in the C6 layout; None for the LINK registers, which `write` translates.
    fn map(off: u32) -> Option<u32> { esp_periph::gdma::GdmaAhb::map_c6(off) }
    /// (out, channel) of a LINK register
    fn link(off: u32) -> Option<(bool, usize)> {
        if !(0x70..0x70 + 3 * 0xc0).contains(&off) { return None; }
        let rel = off - 0x70; let (n, k) = ((rel / 0xc0) as usize, rel % 0xc0);
        match k { 0x10 => Some((false, n)), 0x70 => Some((true, n)), _ => None }
    }
}
impl Device for AhbDma {
    fn read(&mut self, off: u32) -> u32 {
        if let Some((out, ch)) = Self::link(off) {
            let running = if out { self.gdma.out[ch].running } else { self.gdma.inp[ch].running };
            return if running { 0 } else if out { 1 << 3 } else { 1 << 4 };                  // PARK: the link is idle
        }
        match off {
            0x3ac..=0x3b4 => self.link_addr[0][((off - 0x3ac) / 4) as usize],
            0x3b8..=0x3c0 => self.link_addr[1][((off - 0x3b8) / 4) as usize],
            _ => match Self::map(off) { Some(o) => self.gdma.read(o), None => self.ram.read(off) },
        }
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        if let Some((out, ch)) = Self::link(off) {
            let (stop, start, restart) = if out { (v & 1, v >> 1 & 1, v >> 2 & 1) } else { (v >> 1 & 1, v >> 2 & 1, v >> 3 & 1) };
            let addr = self.link_addr[out as usize][ch];
            self.gdma.addr_base = addr & 0xfff0_0000;
            let s = if out { 20 } else { 21 };
            self.gdma.write(ch as u32 * 0xc0 + if out { 0x80 } else { 0x20 }, (addr & 0xf_ffff) | stop << s | start << (s + 1) | restart << (s + 2));
            return WriteEffect::NONE;
        }
        match off {
            0x3ac..=0x3b4 => self.link_addr[0][((off - 0x3ac) / 4) as usize] = v,
            0x3b8..=0x3c0 => self.link_addr[1][((off - 0x3b8) / 4) as usize] = v,
            _ => match Self::map(off) { Some(o) => self.gdma.write(o, v), None => self.ram.write(off, v) },
        }
        WriteEffect::NONE
    }
    fn irq_sources(&self) -> u64 { Device::irq_sources(&self.gdma) }
    fn debug(&mut self, on: bool) { self.gdma.dbg = on; }
}

/// The pins an SD bus device hangs on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SdPins { pub clk: u8, pub cmd: u8, pub data: [u8; 4] }
impl SdPins {
    /// The SD socket of the reference board (Waveshare ESP32-P4-WIFI6), on slot 0's IO MUX
    /// pins: SDMMC_SLOT0_IOMUX_PIN_NUM_CLK 43, CMD 44, D0-D3 39-42
    /// (IDF v5.5.3 components/soc/esp32p4/include/soc/sdmmc_pins.h).
    pub const SOCKET: SdPins = SdPins { clk: 43, cmd: 44, data: [39, 40, 41, 42] };
    /// ESP-Hosted's co-processor link on slot 1 through the GPIO matrix: CLK 18, CMD 19, D0-D3
    /// 14-17, the defaults esp_hosted 2.12.13 gives the ESP32-P4 (Kconfig,
    /// ESP_HOSTED_PRIV_SDIO_PIN_*_SLOT_1) and the reference board's wiring.
    pub const HOSTED: SdPins = SdPins { clk: 18, cmd: 19, data: [14, 15, 16, 17] };
}

/// GPIO matrix signals of the SDMMC host (IDF v5.5.3 include/soc/gpio_sig_map.h): slot 1's
/// clock output, and per slot the card-detect and write-protect inputs.
pub const SIG_SD_CCLK_2: u32 = 0;
pub const SIG_SD_CARD_DETECT_N: [usize; SLOTS] = [126, 127];
pub const SIG_SD_WRITE_PRT: [usize; SLOTS] = [130, 131];
/// IO_MUX pad register of a pin (0x500E1000 + 4 + 4n), MCU_SEL in bits 14:12; function 0 of
/// slot 0's clock pad is SD1_CCLK_PAD, function 1 is the GPIO (register/hw_ver1/soc/io_mux_reg.h).
fn pad_function(io_mux: &RegRam, pin: u8) -> u32 { io_mux.read(4 + 4 * pin as u32) >> 12 & 7 }

pub struct Peripherals {
    pub uart: [Uart; 2],
    pub usb: UsbSerialJtag,
    pub systimer: SystimerP4,
    pub timg: [TimerGroup; 2],
    pub gpio: GpioP4,
    pub io_mux: RegRam,
    /// the SD/MMC host (0x50083000) and the pins of each device on its bus
    pub sdmmc: Sdmmc,
    /// the high-speed USB-OTG core (0x50000000) and the cable to a host, if one is attached
    pub usb_otg: Dwc2,
    pub sd_pins: Vec<SdPins>,
    /// an SDIO function is on the bus: its DAT1 is sampled after each command (`sync_sdio`)
    pub has_sdio: bool,
    pub efuse: Efuse,
    /// SPI_MEM_C: the flash controller's cache side (the MMU registers are the bus's)
    pub spi0: MspiCache<SpiMem>,
    /// SPI_MEM_S: the PSRAM controller's cache side (its MMU registers are the bus's)
    pub psram0: MspiCache<RegRam>,
    /// SPI1_MEM_S: the PSRAM controller's command side, the hex PSRAM on its CS1
    pub psram1: SpiMem,
    /// SPI1_MEM_C: the flash controller's command side
    pub spi1: SpiMem,
    /// SHA accelerator: the ROM checks the bootloader's appended SHA-256 with it
    pub sha: Sha,
    pub intmtx: IntMatrix,
    pub clic: Clic,
    pub cache: Cache,
    pub hp_sys: HpSys,
    pub hp_clkrst: HpClkRst,
    pub lp: LpAon,
    pub ana_mst: AnaMst,
    /// GPSPI2 (0x500D0000): the shared model, the C6's register map
    pub spi2: GpSpi,
    /// I2C1 (0x500C5000), the bus Waveshare's BSP puts its touch controller and codec on: the
    /// shared model, the C6's register map less CLK_CONF, which is HP_SYS_CLKRST's here
    pub i2c1: esp_periph::i2c::I2c,
    /// AXI-GDMA: GPSPI2's DMA
    pub axi_dma: AxiDma,
    /// AHB-GDMA: the I2S controllers' DMA
    pub ahb_dma: AhbDma,
    /// I2S1 (0x500C7000), the BSP's codec port (CONFIG_BSP_I2S_NUM 1): the shared model, the
    /// C6's register map. Its clock dividers are HP_SYS_CLKRST's and not read: 44.1 kHz.
    pub i2s1: esp_periph::I2s,
    /// LEDC (0x500D3000); its clock is HP_SYS_CLKRST PERI_CLK_CTRL22's (`ledc_clock`)
    pub ledc: Ledc,
    pub assist_debug: AssistDebug,
    /// WDEV_RND_REG: LP_SYSTEM_REG_RNG_DATA (0x501101A4) on rev < 3, LP_TRNG's RNG_DATA
    /// (0x50126004) on rev >= 3 (IDF v5.5.4 register/hw_ver1/soc/wdev_reg.h:16,
    /// lp_system_reg.h:1251; hw_ver3/soc/wdev_reg.h:16, trng_reg.h:52). Both read this one generator.
    pub rng: Rng,
    /// register RAM behind unmodelled blocks, first-touch logging, pc attribution
    pub misc: Misc,
    pub spi_exec: bool,
    pub psram_exec: bool,
    clock: ClockTree<4>,
    /// CPU cycles from power-on to the start of this peripheral set: the device clock restarts at a
    /// chip reset, and the SD bus's time (its card's busy period, a co-processor's clock) does not
    pub cycle_base: u64,
    last_status: [u32; 4],
}

// Every peripheral, its 4 KB block (address >> 12) and its interrupt sources.
// Clocks: XTAL 40 MHz for the systimer and the timer groups (TIMG clock source XTAL,
// include/soc/clk_tree_defs.h), RC_SLOW 136 kHz (SOC_CLK_RC_SLOW_FREQ_APPROX) for the LP timer.
device_set! { Peripherals; clock: (clock) CPU_HZ, [(ClockDomain::Xtal, 9), (ClockDomain::Apb, 9), (ClockDomain::RtcSlow, 2647), (ClockDomain::Cpu, 1)];
    0x20800 "CLIC" (clic) => [];
    0x50000 "USB_OTG" optional (usb_otg) => [src::USB_OTG];
    0x20801 "CLIC_CTRL" alias (clic) delta 0x1000 => [];
    0x20810 "CLIC_OTHER" alias (clic) delta 0x10000 => [];
    0x20811 "CLIC_OTHER_CTRL" alias (clic) delta 0x11000 => [];
    0x3ff06 "ASSIST_DEBUG" (assist_debug) => [];
    0x3ff10 "CACHE" (cache) => [];
    0x50083 "SDMMC" (sdmmc) => [src::SDIO_HOST];
    0x50085 "AHB_DMA" (ahb_dma) => [src::AHB_DMA_OUT_CH0, src::AHB_DMA_OUT_CH0 + 1, src::AHB_DMA_OUT_CH0 + 2, NO_SOURCE, NO_SOURCE,
                                    src::AHB_DMA_IN_CH0, src::AHB_DMA_IN_CH0 + 1, src::AHB_DMA_IN_CH0 + 2, NO_SOURCE, NO_SOURCE];
    0x5008a "AXI_DMA" (axi_dma) => [src::AXI_DMA_OUT_CH0, src::AXI_DMA_OUT_CH0 + 1, src::AXI_DMA_OUT_CH0 + 2, NO_SOURCE, NO_SOURCE,
                                    src::AXI_DMA_IN_CH0, src::AXI_DMA_IN_CH0 + 1, src::AXI_DMA_IN_CH0 + 2, NO_SOURCE, NO_SOURCE];
    0x5008c "FLASH_SPI0" (spi0) => [];
    0x5008d "FLASH_SPI1" (spi1) => [];
    0x5008e "PSRAM_MSPI0" (psram0) => [];
    0x5008f "PSRAM_MSPI1" (psram1) => [];
    0x50091 "SHA" (sha) => [];
    0x500c2 "TIMG0" (timg[0]) => [src::TG0_T0, src::TG0_T1];
    0x500c5 "I2C1" (i2c1) => [src::I2C1];
    0x500c7 "I2S1" (i2s1) => [src::I2S1];
    0x500c3 "TIMG1" (timg[1]) => [src::TG1_T0, src::TG1_T1];
    0x500ca "UART0" (uart[0]) => [src::UART0];
    0x500cb "UART1" (uart[1]) => [src::UART1];
    0x500d0 "SPI2" (spi2) => [src::GPSPI2];
    0x500d2 "USB_SERIAL_JTAG" (usb) => [src::USB_SERIAL_JTAG];
    0x500d3 "LEDC" (ledc) => [src::LEDC];
    0x500d6 "INTMTX" (intmtx) => [];
    0x500e0 "GPIO" (gpio) => [src::GPIO_INTR0];
    0x500e1 "IO_MUX" (io_mux) => [];
    0x500e2 "SYSTIMER" (systimer) => [src::SYSTIMER_TARGET0, src::SYSTIMER_TARGET0 + 1, src::SYSTIMER_TARGET0 + 2];
    0x500e5 "HP_SYS" (hp_sys) => [src::FROM_CPU0, src::FROM_CPU0 + 1, src::FROM_CPU0 + 2, src::FROM_CPU0 + 3];
    0x500e6 "HP_SYS_CLKRST" (hp_clkrst) => [];
    0x50110 "LP_RNG" (rng) @ 0x1a4..=0x1a7 => [];
    // LP_TRNG, rev >= 3's random source. hw_ver1 has no block here and its IDF never reads it, so
    // it is mounted on both revisions; CFG, RSTN and DATE stay register RAM.
    0x50126 "LP_TRNG" (rng) @ 0x4..=0x7 => [];
    0x50110 "LP_SYS" (lp) => [];
    0x50111 "LP_CLKRST" alias (lp) delta 0x1000 => [];
    0x50112 "LP_TIMER" alias (lp) delta 0x2000 => [];
    0x50124 "I2C_ANA_MST" (ana_mst) => [];
    0x5012d "EFUSE" (efuse) => [];
}

impl DeviceSet for Peripherals {
    const BASE: u32 = 0;
    fn block_name(block: u32) -> &'static str { Peripherals::block_name(block) }
    fn misc(&self) -> &Misc { &self.misc }
    fn misc_mut(&mut self) -> &mut Misc { &mut self.misc }
    fn pre_access(&mut self, block: u32, off: u32, write: bool) {
        // INTR_STATUS_0..3 read the source state.
        if block == 0x500d6 && !write && (0x200..=0x20c).contains(&(off & 0x7ff)) { self.intmtx.status = self.source_status(); }
        if (block == 0x50110 && off == 0x1a4 || block == 0x50126 && off == 0x4) && !write { self.rng.now = self.clock.cycles() as u32; }
        // I2C1's clock: HP_SYS_CLKRST PERI_CLK_CTRL10 I2C1_CLK_SRC_SEL bit 26 (1 = RC_FAST),
        // PERI_CLK_CTRL11 I2C1_CLK_DIV_NUM 7:0, NUMERATOR 15:8, DENOMINATOR 23:16
        // (register/hw_ver3/soc/hp_sys_clkrst_reg.h:1443-1470; hal/i2c_ll.h:186-226 writes div_a to
        // the numerator), repacked into the shared S3 CLK_CONF layout.
        if block == 0x500c5 {
            let (c10, c11) = (self.hp_clkrst.read(0x40), self.hp_clkrst.read(0x44));
            self.i2c1.set_external_clock_config(Some((c11 & 255) | (c11 >> 8 & 63) << 8 | (c11 >> 16 & 63) << 14 | (c10 >> 26 & 1) << 20));
        }
        if block == 0x50083 { self.sdmmc.now_ns = self.now_ns(); if self.sdmmc.timed { self.sdmmc.ciu_hz = self.hp_clkrst.sdio_ls_hz(); } }
        // CDETECT and WRTPRT read the card-detect and write-protect signals of the GPIO matrix.
        if block == 0x50083 && (off == 0x50 || off == 0x54) && !write {
            let (cd, wp) = (0..SLOTS).fold((0, 0), |(cd, wp), s| (cd | (self.gpio.in_signal(SIG_SD_CARD_DETECT_N[s]) as u32) << s, wp | (self.gpio.in_signal(SIG_SD_WRITE_PRT[s]) as u32) << s));
            self.sdmmc.card_detect_n = cd;
            self.sdmmc.write_protect = wp;
        }
    }
}

impl Peripherals {
    /// The UART has the C6's register map (IDF v5.5.3 register/hw_ver1/soc/uart_reg.h: RXFIFO_CNT
    /// 7:0, CONF0_SYNC.RXFIFO_RST bit 22, CONF1 thresholds 7:0), as do the systimer and TIMG.
    /// GPIO_STRAP_REG reads 0x30F, SPI fast flash boot (include/soc/boot_mode.h).
    pub fn new(mac: [u8; 6], wafer: (u32, u32)) -> Self {
        Peripherals {
            uart: [Uart::new(UartLayout::C6), Uart::new(UartLayout::C6)], usb: UsbSerialJtag::new(CPU_HZ), systimer: SystimerP4::default(),
            timg: [TimerGroup::new(), TimerGroup::new()], gpio: GpioP4::new(), io_mux: RegRam::new(),
            sdmmc: Sdmmc::new(SdmmcLayout::Esp32p4), sd_pins: Vec::new(), has_sdio: false, usb_otg: Dwc2::new(Dwc2Config::esp32p4_hs(CPU_HZ)),
            efuse: efuse_p4(mac, wafer, (0, 3)),
            spi0: MspiCache(flash_controller(false)), psram0: MspiCache(RegRam::new()), spi1: flash_controller(true), psram1: psram_controller(),
            sha: Sha::new(), intmtx: IntMatrix::new(), clic: Clic::default(), cache: Cache::new(), hp_sys: HpSys::default(), hp_clkrst: HpClkRst::new(),
            lp: LpAon::new(), ana_mst: AnaMst::default(), assist_debug: AssistDebug::default(), rng: Rng::new(),
            spi2: GpSpi::new(), i2c1: esp_periph::i2c::I2c::new(), axi_dma: AxiDma::new(), ahb_dma: AhbDma::new(), i2s1: esp_periph::I2s::new(CPU_HZ), ledc: Ledc::new(LedcLayout::P4),
            misc: Misc::new(), spi_exec: false, psram_exec: false, clock: Self::new_clock(), cycle_base: 0, last_status: [0; 4],
        }
    }

    /// Block names for `--log-periph`, from IDF v5.5.3 register/hw_ver1/soc/reg_base.h.
    pub fn block_name(block: u32) -> &'static str {
        match block {
            0x20800 | 0x20801 => "CLIC", 0x20810 | 0x20811 => "CLIC_OTHER", 0x20000..=0x2ffff => "CPU_SUBSYSTEM",
            0x3ff04 => "TRACE0", 0x3ff05 => "TRACE1", 0x3ff06 => "ASSIST_DEBUG", 0x3ff0e => "L2MEM_MON", 0x3ff0f => "TCM_MON", 0x3ff10 => "CACHE",
            0x50000..=0x5003f => "USB_OTG_HS", 0x50040..=0x5007f => "USB_OTG_FS", 0x50080 => "USB_WRAP", 0x50081 => "DW_GDMA", 0x50082 => "REGDMA",
            0x50083 => "SDMMC", 0x50084 => "H264", 0x50085 => "AHB_DMA", 0x50086 => "JPEG", 0x50087 => "PPA", 0x50088 => "DMA2D",
            0x50089 => "KEYMNG", 0x5008a => "AXI_DMA", 0x5008c => "FLASH_SPI0", 0x5008d => "FLASH_SPI1", 0x5008e => "PSRAM_MSPI0",
            0x5008f => "PSRAM_MSPI1", 0x50090 => "AES", 0x50091 => "SHA", 0x50092 => "RSA", 0x50093 => "ECC", 0x50094 => "DS",
            0x50095 => "HMAC", 0x50096 => "ECDSA", 0x50098..=0x5009b => "EMAC", 0x5009c => "USBPHY", 0x5009d => "DDRPHY", 0x5009e => "PVT",
            0x5009f => "CSI", 0x500a0 => "DSI", 0x500a1 => "ISP", 0x500a2 => "RMT", 0x500a3 => "BITSCRAMBLER", 0x500a4 => "AXI_ICM",
            0x500a5 => "HP_PERI_PMS", 0x500a6 => "DMA_PMS", 0x500a7 => "H264_DMA2D",
            0x500c0 => "MCPWM0", 0x500c1 => "MCPWM1", 0x500c2 => "TIMG0", 0x500c3 => "TIMG1", 0x500c4 => "I2C0", 0x500c5 => "I2C1",
            0x500c6 => "I2S0", 0x500c7 => "I2S1", 0x500c8 => "I2S2", 0x500c9 => "PCNT", 0x500ca => "UART0", 0x500cb => "UART1",
            0x500cc => "UART2", 0x500cd => "UART3", 0x500ce => "UART4", 0x500cf => "PARL_IO", 0x500d0 => "SPI2", 0x500d1 => "SPI3",
            0x500d2 => "USB_SERIAL_JTAG", 0x500d3 => "LEDC", 0x500d5 => "SOC_ETM", 0x500d6 => "INTMTX", 0x500d7 => "TWAI0",
            0x500d8 => "TWAI1", 0x500d9 => "TWAI2", 0x500da => "I3C_MST", 0x500db => "I3C_SLV", 0x500dc => "LCD_CAM", 0x500de => "ADC",
            0x500df => "UHCI", 0x500e0 => "GPIO", 0x500e1 => "IO_MUX", 0x500e2 => "SYSTIMER", 0x500e3 => "MEM_MON",
            0x500e4 => "AUDIO_ADDC", 0x500e5 => "HP_SYS", 0x500e6 => "HP_SYS_CLKRST",
            0x50110 => "LP_SYS", 0x50111 => "LP_CLKRST", 0x50112 => "LP_TIMER", 0x50113 => "LP_ANA_PERI", 0x50114 => "LP_HUK",
            0x50115 => "PMU", 0x50116 => "LP_WDT", 0x50118 => "LP_MAILBOX", 0x50119 => "RTC", 0x50120 => "LP_PERI",
            0x50121 => "LP_UART", 0x50122 => "LP_I2C", 0x50123 => "LP_SPI", 0x50124 => "I2C_ANA_MST", 0x50125 => "LP_I2S", 0x50126 => "LP_TRNG",
            0x50127 => "LP_ADC", 0x50128 => "LP_TOUCH", 0x5012a => "LP_GPIO", 0x5012b => "LP_IOMUX", 0x5012c => "LP_INTR",
            0x5012d => "EFUSE", 0x5012e => "LP_PERI_PMS", 0x5012f => "LP_TSENSOR",
            _ => "?",
        }
    }

    pub fn read32(&mut self, addr: u32) -> u32 { mmio::read32(self, addr) }

    pub fn write32(&mut self, addr: u32, v: u32) {
        let fx = mmio::write32(self, addr, v);
        if addr == 0x500e_609c { self.ledc_clock(v); }
        if fx.contains(WriteEffect::SPI_EXEC | WriteEffect::PERIPH_RESET) {
            if fx.contains(WriteEffect::PERIPH_RESET) {
                match addr {
                    0x5011_1048 => { self.usb_otg.set_reset(v & (1 << 2) != 0); Dispatch::refresh_optional(self, USB_OTG_BLOCK); }
                    0x5011_104c => self.sdmmc.reset_registers(),
                    _ => {}
                }
            }
            if fx.contains(WriteEffect::SPI_EXEC) { if addr >> 12 == 0x5008f { self.psram_exec = true; } else { self.spi_exec = true; } }
        }
    }

    /// HP_SYS_CLKRST PERI_CLK_CTRL22: LEDC_CLK_SRC_SEL 1:0 (0 XTAL, 1 RC_FAST, 2 PLL_F80M),
    /// LEDC_CLK_EN bit 2 (register/hw_ver3/soc/hp_sys_clkrst_reg.h:2510-2524, hal/ledc_ll.h:98-115;
    /// RC_FAST is SOC_CLK_RC_FAST_FREQ_APPROX, 20 MHz).
    fn ledc_clock(&mut self, v: u32) {
        self.ledc.external_clock_hz = if v & (1 << 2) == 0 { 0 } else { match v & 3 { 0 => 40_000_000, 1 => 20_000_000, 2 => 80_000_000, _ => 0 } };
    }

    /// Which device each SDMMC slot reaches. Slot 0 is wired to its IO MUX pins: a device on
    /// them is on slot 0 while the clock pad selects function 0. Slot 1 goes through the GPIO
    /// matrix: a device is on it when its clock pad is a GPIO (function 1) driving slot 1's
    /// SD_CARD_CCLK_2 signal.
    pub fn sd_routes(&self) -> [Option<usize>; SLOTS] {
        let mut r = [None; SLOTS];
        for (i, p) in self.sd_pins.iter().enumerate() {
            match pad_function(&self.io_mux, p.clk) {
                0 if *p == SdPins::SOCKET => r[0] = Some(i),
                1 if self.gpio.out_signal(p.clk) == SIG_SD_CCLK_2 => r[1] = Some(i),
                _ => {}
            }
        }
        r
    }

    /// Put a device on the SD bus at `pins`, in place of one already there.
    pub fn attach_sd(&mut self, pins: SdPins, mut dev: Box<dyn esp_periph::sdmmc::SdDevice>) {
        dev.set_log(self.sdmmc.logging());
        self.has_sdio |= dev.io();
        match self.sd_pins.iter().position(|&p| p == pins) {
            Some(i) => self.sdmmc.devices[i] = dev,
            None => { self.sdmmc.attach(dev); self.sd_pins.push(pins); }
        }
    }

    /// DAT1 of each SDIO function on the bus: its D1 pin's input level (low while it interrupts,
    /// which `sdmmc_host_io_int_wait` reads first) and the slot's RINTSTS SDIO bit. True when an
    /// interrupt line may have moved.
    pub fn sync_sdio(&mut self) -> bool {
        self.sdmmc.route = self.sd_routes();
        let mut changed = self.sdmmc.poll_sdio();
        for (i, pins) in self.sd_pins.iter().enumerate() {
            if let Some(d) = self.sdmmc.devices.get(i).filter(|d| d.io()) { changed |= self.gpio.g.set_input(pins.data[1], !d.interrupt()); }
        }
        changed
    }

    /// Emulated time in nanoseconds since power-on, from the device clock.
    pub fn now_ns(&self) -> u64 { ((self.cycle_base + self.clock.cycles()) as u128 * 1_000_000_000 / CPU_HZ as u128) as u64 }

    /// Advance every clocked device by `cycles` CPU cycles.
    pub fn tick(&mut self, cycles: u64) { Dispatch::tick(self, cycles); }
    /// CPU cycles until the earliest device deadline, `u32::MAX` when nothing is armed.
    pub fn cycles_until_timer(&self) -> u32 { Dispatch::cycles_until_deadline(self) }
    pub fn source_status(&self) -> [u32; 4] { Dispatch::source_status(self) }

    /// Present the asserted sources to each core's CLIC through its matrix. Returns true if any
    /// source changed.
    pub fn refresh_lines(&mut self) -> bool {
        let st = self.source_status();
        let changed = st != self.last_status;
        self.last_status = st;
        for core in 0..2 {
            let mut input = 0u64;
            for (w, &word) in st.iter().enumerate() {
                let mut bits = word;
                while bits != 0 {
                    let s = w * 32 + bits.trailing_zeros() as usize;
                    bits &= bits - 1;
                    let id = self.intmtx.map[core][s] as usize;
                    if id < CLIC_IDS { input |= 1 << id; }
                }
            }
            self.clic.core[core].set_input(input);
        }
        changed
    }

    /// Sources routed to a core's CLIC ID.
    pub fn sources_of(&self, core: usize, id: u32) -> Vec<usize> { (0..src::COUNT).filter(|&s| self.intmtx.map[core & 1][s] == id).collect() }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AXI-GDMA OUT channel 2: a whole-word descriptor address in LINK2, START in LINK1 bit 1, the
    /// peripheral select and interrupt words of its block at 0x138 + 2 * 0x68 (axi_dma_reg.h).
    #[test]
    fn axi_dma_starts_an_out_link_from_link2() {
        let mut d = AxiDma::new();
        let base = 0x138 + 2 * 0x68;
        d.write(base + 0x44, 1);                                      // PERI_SEL: GPSPI2
        d.write(base + 0x24, 0x4ff1_2340);                            // LINK2
        assert_eq!(d.read(base + 0x20), 1 << 3);                      // parked
        d.write(base + 0x20, 1 << 1);                                 // START
        assert_eq!(d.gdma.out_channel_for(1), Some(2));
        assert_eq!(d.gdma.out[2].desc, 0x4ff1_2340);
        assert_eq!((d.read(base + 0x20), d.read(base + 0x24)), (0, 0x4ff1_2340));
        d.write(base + 0x08, 1 << 3);                                 // INT_ENA: TOTAL_EOF
        d.gdma.out[2].int_raw |= 1 << 3;
        assert_eq!((d.read(base + 0x04), Device::irq_sources(&d)), (1 << 3, 1 << 2));
        d.write(base + 0x0c, 1 << 3);
        assert_eq!(d.read(base), 0);
        d.write(base + 0x20, 1);                                      // STOP
        assert_eq!(d.gdma.out_channel_for(1), None);
    }

    /// AHB-GDMA: the C6's layout, the address in OUT_LINK_ADDR (0x3B8 + 4n) and START in
    /// OUT_LINK bit 1 (0x70 + 0xC0 n + 0x70), I2S1 its trigger 4 (ahb_dma_reg.h, gdma_channel.h).
    #[test]
    fn ahb_dma_starts_an_out_link_from_its_address_word() {
        let mut d = AhbDma::new();
        let ch = 0x70 + 0xc0;
        d.write(ch + 0x90, 4);                                        // OUT_PERI_SEL_CH1
        d.write(0x3bc, 0x4ffa_0000);                                  // OUT_LINK_ADDR_CH1
        d.write(ch + 0x70, 1 << 1);                                   // OUT_LINK_CH1: START
        assert_eq!((d.gdma.out_channel_for(4), d.gdma.out[1].desc), (Some(1), 0x4ffa_0000));
        assert_eq!((d.read(0x3bc), d.read(ch + 0x70)), (0x4ffa_0000, 0));
        d.gdma.out[1].int_raw |= 1;
        assert_eq!(d.read(0x40), 1);                                  // OUT_INT_RAW_CH1
    }

    #[test]
    fn systimer_keeps_16_mhz_on_xtal_ticks() {
        let mut t = SystimerP4::default();
        t.st.write(0x0, 1 << 30);                                     // unit 0 counting
        for _ in 0..1000 { Device::tick(&mut t, 7); }                 // 7000 XTAL ticks at 40 MHz = 175 us
        assert_eq!(t.st.unit[0], 2800);                               // 175 us at 16 MHz
        t.st.write(0x34, 0); t.st.write(0x20, 2810); t.st.write(0x50, 1); t.st.write(0x0, 1 << 30 | 1 << 24);
        assert_eq!(t.next_deadline(), Some(25));                      // 10 counts = 25 XTAL ticks
        Device::tick(&mut t, 24);
        assert_eq!(t.st.unit[0], 2809);
        Device::tick(&mut t, 1);
        assert_eq!(t.st.unit[0], 2810);
        t.st.write(0x64, 1);
        assert_eq!(Device::irq_sources(&t), 1);
    }

    #[test]
    fn clic_takes_strictly_above_threshold_and_latches_edges() {
        let mut c = ClicCore::default();
        let id = 16 + 6;
        c.write(0x1000 + 4 * id as u32, 1 << 8 | 1 << 29, 0xf);            // IE, level 1, level-triggered
        c.set_input(1 << id);
        assert_eq!(c.read(0x1000 + 4 * id as u32) & 1, 1);
        assert_eq!(c.candidate(), Some((id, 0x3f)));
        c.write(0x8, 0x3f << 24, 0xf);                                     // threshold level 1 masks level 1
        assert_eq!(c.candidate(), None);
        c.write(0x8, 0x1f << 24, 0xf);
        assert_eq!(c.candidate(), Some((id, 0x3f)));
        c.set_input(0);
        assert_eq!(c.candidate(), None);
        let e = 16 + 7;
        c.write(0x1000 + 4 * e as u32, 1 << 8 | 1 << 17 | 3 << 29, 0xf);   // edge-triggered, level 3
        c.set_input(1 << e);
        c.set_input(0);
        assert_eq!(c.candidate(), Some((e, 0x7f)));
        c.write(0x1000 + 4 * e as u32, 1 << 8, 0x2);                         // IE byte alone keeps IP
        assert_eq!(c.candidate(), Some((e, 0x7f)));
        let word = c.read(0x1000 + 4 * e as u32);
        c.write(0x1000 + 4 * e as u32, word | 1 << 16, 0xf);                 // set SHV by read-modify-write
        assert_eq!(c.candidate(), Some((e, 0x7f)), "a reconfiguring RMW keeps the pending edge");
        let word = c.read(0x1000 + 4 * e as u32);
        assert_eq!(word & 1, 1);
        c.write(0x1000 + 4 * e as u32, word, 0xf);                           // IDF's acknowledge: REG_SET_BIT(IP)
        assert_eq!(c.candidate(), None);
        c.set_input(1 << e); c.set_input(0);
        c.write(0x1000 + 4 * e as u32, 1, 0x1);                              // a byte write of IP = 1
        assert_eq!(c.candidate(), None);
        c.set_input(1 << e);
        c.write(0x1000 + 4 * e as u32, 1 << 16 | 1 << 17, 0x4);              // hardware-vectored
        c.taken(e);
        assert_eq!(c.candidate(), None, "a vectored edge interrupt clears when it is taken");
        assert_eq!(c.read(0x0), 1);
        assert_eq!(c.read(0x4), 48 | 3 << 21);
        assert_eq!(c.read(0x1000) >> 22, 0x1f << 2 | 3, "CTL resets to 0x1F, MODE reads machine");
    }

    #[test]
    fn matrix_routes_sources_to_clic_ids_per_core() {
        let mut p = Peripherals::new([0; 6], (1, 0));
        p.write32(0x500d_6000 + 4 * src::FROM_CPU0 as u32, 16 + 9);
        p.write32(0x2080_1000 + 4 * 25, 1 << 8 | 1 << 29);
        p.write32(0x500e_5010, 1);
        assert!(p.refresh_lines());
        assert_eq!(p.clic.core[0].candidate(), Some((25, 0x3f)));
        assert_eq!(p.clic.core[1].candidate(), None);
        assert_eq!(p.read32(0x500d_6200 + 4 * (src::FROM_CPU0 as u32 / 32)) >> (src::FROM_CPU0 % 32) & 1, 1);
        assert_eq!(p.sources_of(0, 25), [src::FROM_CPU0]);
        p.write32(0x500e_5010, 0);
        p.refresh_lines();
        assert_eq!(p.clic.core[0].candidate(), None);
    }

    #[test]
    fn both_revisions_random_sources_read_the_generator() {
        // WDEV_RND_REG: LP_SYSTEM RNG_DATA on rev < 3, LP_TRNG RNG_DATA on rev >= 3.
        let mut p = Peripherals::new([0; 6], (3, 1));
        for addr in [0x5011_01a4, 0x5012_6004] {
            let (a, b) = (p.read32(addr), p.read32(addr));
            assert!(a != 0 && b != 0 && a != b, "{addr:#x}: {a:#x} {b:#x}");
        }
        p.write32(0x5012_6000, 1);                                    // CFG stays register RAM
        assert_eq!(p.read32(0x5012_6000), 1);
    }

    #[test]
    fn cache_operations_finish_at_once() {
        let mut p = Peripherals::new([0; 6], (1, 0));
        p.write32(0x3ff1_0098, 1);                                    // invalidate
        assert_eq!(p.read32(0x3ff1_0098), 1 << 4);
        p.write32(0x3ff1_028c, 1 << 20);
        assert_eq!(p.read32(0x3ff1_028c), 1 << 20 | 1 << 22);
        p.write32(0x3ff1_002c, 1 << 16);
        assert_eq!(p.read32(0x3ff1_002c), 1 << 16 | 1 << 18);
        assert_eq!(p.read32(0x3ff1_0278), 0x400);
    }

    #[test]
    fn efuse_reports_v1_0_and_block_v0_3() {
        let e = efuse_p4([0x30, 0xed, 0xa0, 0x00, 0x11, 0x22], (1, 0), (0, 3));
        assert_eq!(e.read(0x4c), 0x310);
        assert_eq!(e.read(0x44), 0xa000_1122);
        assert_eq!(e.read(0x48), 0x30ed);
    }

    #[test]
    fn core1_starts_held_in_reset() {
        let mut p = Peripherals::new([0; 6], (1, 0));
        assert!(!p.hp_clkrst.core1_released());
        let clk = p.read32(0x500e_6014);
        p.write32(0x500e_6014, clk | 1 << 4);
        assert!(!p.hp_clkrst.core1_released());
        p.write32(0x500e_60c0, 0);
        assert!(p.hp_clkrst.core1_released());
    }

    fn card() -> Box<esp_periph::sdmmc::SdCard> { Box::new(esp_periph::sdmmc::SdCard::new(Box::new(esp_periph::sdmmc::MemStore(vec![0; 1 << 20]))).unwrap()) }

    #[test]
    fn gpio_selects_sit_four_bytes_above_the_c6s() {
        let mut p = Peripherals::new([0; 6], (1, 0));
        assert_eq!(p.read32(0x500e_0158 + 4 * 126), 0x3e, "SD_CARD_DETECT_N_1 resets to constant low");
        assert_eq!(p.read32(0x500e_0158 + 4 * 2), 0x3f, "most inputs to constant high");
        p.write32(0x500e_0158 + 4 * 2, 0x80 | 40);
        assert_eq!(p.gpio.g.func_in_sel[2], 0x80 | 40, "FUNC2_IN_SEL is the model's entry 2");
        p.write32(0x500e_0558 + 4 * 43, SIG_SD_CCLK_2);
        assert_eq!((p.gpio.out_signal(43), p.read32(0x500e_0558 + 4 * 43)), (0, 0));
        assert_eq!(p.gpio.out_signal(42), 0x100, "the others still drive GPIO_OUT");
        p.write32(0x500e_0154, 0x1234);
        assert_eq!(p.read32(0x500e_0154), 0x1234, "PIN56 is register RAM");
        assert_eq!(p.gpio.g.func_in_sel[0], 0x3f, "and not FUNC0_IN_SEL");
    }

    /// Slot 0 takes the socket on its IO MUX pins, slot 1 through the GPIO matrix; card detect
    /// and write protect come from the matrix inputs.
    #[test]
    fn sd_slots_follow_the_pad_and_matrix_routing() {
        let mut p = Peripherals::new([0; 6], (1, 0));
        p.attach_sd(SdPins::SOCKET, card());
        assert_eq!(p.sd_routes(), [Some(0), None], "at reset the clock pad is SD1_CCLK");
        p.write32(0x500e_1000 + 4 + 4 * 43, 1 << 12);
        assert_eq!(p.sd_routes(), [None, None], "a GPIO driving GPIO_OUT");
        p.write32(0x500e_0558 + 4 * 43, SIG_SD_CCLK_2);
        assert_eq!(p.sd_routes(), [None, Some(0)], "a GPIO driving slot 1's clock");
        assert_eq!(p.read32(0x5008_3050), 0, "card detect resets to constant low on both slots: present");
        p.write32(0x500e_0158 + 4 * 127, 0x80 | 0x3f);
        assert_eq!(p.read32(0x5008_3050), 2);
        p.write32(0x500e_0158 + 4 * 131, 0x80 | 0x40 | 0x3f);
        assert_eq!(p.read32(0x5008_3054), 0, "constant high inverted: not write protected");
        p.gpio.g.set_input(5, true);
        p.write32(0x500e_0158 + 4 * 130, 0x80 | 5);
        assert_eq!(p.read32(0x5008_3054), 1, "a pin's level");
    }

    /// A card put in an occupied socket replaces the one there; one on other pins is another device.
    #[test]
    fn attaching_twice_replaces_the_card() {
        let mut p = Peripherals::new([0; 6], (1, 0));
        p.attach_sd(SdPins::SOCKET, card());
        p.attach_sd(SdPins::SOCKET, card());
        assert_eq!((p.sdmmc.devices.len(), p.sd_pins.len()), (1, 1));
        p.attach_sd(SdPins { clk: 18, cmd: 19, data: [14, 15, 16, 17] }, card());
        assert_eq!(p.sdmmc.devices.len(), 2);
        p.write32(0x500e_1000 + 4 + 4 * 18, 1 << 12);
        p.write32(0x500e_0558 + 4 * 18, SIG_SD_CCLK_2);
        assert_eq!(p.sd_routes(), [Some(0), Some(1)], "the socket on slot 0, the other device on slot 1");
    }

    #[test]
    fn lp_clkrst_resets_the_sdmmc_host() {
        let mut p = Peripherals::new([0; 6], (1, 0));
        p.attach_sd(SdPins::SOCKET, card());
        p.write32(0x5008_3024, 0xffff);
        assert_eq!(p.read32(0x5008_3024), 0xffff);
        p.write32(0x5011_104c, 1 << 28);
        p.write32(0x5011_104c, 0);
        assert_eq!(p.read32(0x5008_3024), 0, "INTMASK back to its reset value");
        assert_eq!(p.sdmmc.devices.len(), 1, "the card stays in its socket");
        assert_eq!(p.read32(0x5011_104c), 0);
    }

    /// HP_USB_CLKRST_CTRL1.RST_EN_USB_OTG20 holds the USB-OTG core in reset while set; other bits
    /// of the register leave it alone.
    #[test]
    fn lp_clkrst_holds_the_usb_otg_core_in_reset() {
        let mut p = Peripherals::new([0; 6], (1, 0));
        p.write32(0x5000_0008, 1);
        assert_eq!(p.read32(0x5000_0008), 1);
        p.write32(0x5011_1048, 1 << 2);
        assert_eq!(p.read32(0x5000_0008), 0, "GAHBCFG back to its reset value");
        p.write32(0x5000_0008, 1);
        assert_eq!(p.read32(0x5000_0008), 0, "writes are ignored while the reset is held");
        p.write32(0x5011_1048, 0);
        p.write32(0x5000_0008, 1);
        p.write32(0x5011_1048, 1 << 30);
        assert_eq!(p.read32(0x5000_0008), 1, "the PHY clock bit is not a reset");
    }

    #[test]
    fn lp_reset_cause_and_software_reset() {
        let mut p = Peripherals::new([0; 6], (1, 0));
        assert_eq!(p.read32(0x5011_1010) >> 7 & 0x3f, RST_POWERON);
        p.write32(0x5011_1014, 1 << 13);
        assert!(p.lp.sw_reset);
        assert_eq!(p.lp.reset_cause, RST_SW_CPU);
        assert_eq!(p.read32(0x5011_1014) & 1 << 13, 0);
    }
}

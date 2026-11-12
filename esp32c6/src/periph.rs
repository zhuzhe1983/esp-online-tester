//! ESP32-C6 peripherals.
//!
//! The data-path IP is the C3's — UART, USB-Serial/JTAG, systimer, timer groups, GPIO, the SPI
//! flash controller, SHA/AES/RSA have the same register layouts — so those models come from
//! `esp-periph`. Written here: the address map, the interrupt matrix with its PLIC/INTPRI
//! front-ends, the L1 cache controller, the always-on LP blocks (reset cause, software reset,
//! RTC timer, store registers, watchdog), PCR, PMU and the RNG.

use crate::radio::Ieee802154;
use crate::wifi::{ModemBb, WifiMac};
use emu_core::{ClockDomain, ClockTree};
use esp_periph::{device_set, mmio, Device, DeviceSet, Dispatch, Misc, RegRam, WriteEffect, NO_SOURCE};
use esp_periph::{Aes, Efuse, Gpio, GpSpi, Ledc, LedcLayout, Mcpwm, Rsa, Sha, SpiMem, Systimer, TimerGroup, Uart, UartLayout, UsbSerialJtag};
use esp_periph::{RST_POWERON, RST_SW_CPU, RST_SW_SYS};

pub const CPU_HZ: u64 = 160_000_000;
pub const PERIPH_BASE: u32 = 0x6000_0000;
pub const PERIPH_END: u32 = 0x6010_0000;
/// The CPU-subsystem window: PLIC (machine 0x000, user 0x400) and CLINT (0x800, 0xc00).
pub const CPU_SUB_BASE: u32 = 0x2000_1000;
pub const CPU_SUB_END: u32 = 0x2000_2000;

/// Interrupt sources, numbered by the order of the `INTMTX_CORE0_*_MAP_REG` registers (which is
/// also `soc/interrupts.h`'s order on this chip). Only the sources we can assert are listed.
pub mod src {
    pub const WIFI_MAC: usize = 0; pub const WIFI_PWR: usize = 2;
    pub const LP_TIMER: usize = 7; pub const ZB_MAC: usize = 12; pub const PMU: usize = 13; pub const EFUSE: usize = 14;
    pub const LP_RTC_TIMER: usize = 15; pub const LP_WDT: usize = 18;
    /// software interrupts, raised by writing `INTPRI_CPU_INTR_FROM_CPU_n`: the FreeRTOS yield
    pub const FROM_CPU0: usize = 22;
    pub const GPIO: usize = 30; pub const MSPI: usize = 40; pub const I2S: usize = 41;
    pub const UART0: usize = 43; pub const UART1: usize = 44; pub const LEDC: usize = 45;
    pub const USB_SERIAL_JTAG: usize = 48; pub const RMT: usize = 49; pub const I2C_EXT0: usize = 50;
    pub const TG0_T0: usize = 51; pub const TG0_T1: usize = 52; pub const TG0_WDT: usize = 53;
    pub const TG1_T0: usize = 54; pub const TG1_T1: usize = 55; pub const TG1_WDT: usize = 56;
    pub const SYSTIMER_T0: usize = 57; pub const SYSTIMER_T1: usize = 58; pub const SYSTIMER_T2: usize = 59;
    pub const MCPWM0: usize = 61;
    // IDF v5.5.4 components/soc/esp32c6/include/soc/interrupts.h:83-85.
    pub const PARLIO: usize = 63;
    // IDF v5.5.3 components/soc/esp32c6/include/soc/interrupts.h:86, ETS_SLC0_INTR_SOURCE.
    pub const SLC0: usize = 64;
    pub const DMA_IN_CH0: usize = 66; pub const DMA_OUT_CH0: usize = 69; pub const GPSPI2: usize = 72;
    pub const AES: usize = 73; pub const SHA: usize = 74; pub const RSA: usize = 75; pub const ECC: usize = 76;
    pub const COUNT: usize = 77;
}

/// The interrupt matrix (`INTMTX`, 0x60010000): 77 peripheral sources, each mapped to one of the
/// 31 CPU interrupt lines, then the three read-only status registers.
pub struct IntMatrix {
    pub map: [u32; src::COUNT],
    /// `INTMTX_CORE0_INT_STATUS_REG_0..2`: the asserted sources, published by
    /// `Peripherals::refresh_lines`, which also uses it as its change-detection baseline.
    pub status: [u32; 4],
    ram: RegRam,
}
impl Default for IntMatrix { fn default() -> Self { Self::new() } }
impl IntMatrix {
    pub fn new() -> Self { IntMatrix { map: [0; src::COUNT], status: [0; 4], ram: RegRam::new() } }
    pub fn read(&self, off: u32) -> u32 {
        match off {
            off if off < src::COUNT as u32 * 4 => self.map[(off / 4) as usize],
            // INTMTX_CORE0_INT_STATUS_REG_0..2. The last map register is ECC_INTR_MAP at
            // 0x130, so the source map ends exactly where these begin (src::COUNT * 4 == 0x134)
            // and without this arm they fall through to register RAM and read zero -- leaving
            // esp-hal's vectored handler unable to tell which source fired, so it never clears
            // the latch and a level-triggered line re-fires forever. ESP-IDF v5.5.4,
            // components/soc/esp32c6/register/soc/interrupt_matrix_reg.h.
            0x134..=0x13c => self.status[((off - 0x134) / 4) as usize],
            _ => self.ram.read(off),
        }
    }
    /// The status registers are read-only: a write to them lands in register RAM that
    /// `read` shadows.
    pub fn write(&mut self, off: u32, v: u32) {
        match off { off if off < src::COUNT as u32 * 4 => self.map[(off / 4) as usize] = v & 0x1f, _ => self.ram.write(off, v) }
    }
}
impl Device for IntMatrix {
    fn read(&mut self, off: u32) -> u32 { IntMatrix::read(self, off) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { IntMatrix::write(self, off, v); WriteEffect::NONE }
}

/// The CPU interrupt controller: enable, type, priority and threshold per line, reachable as
/// the PLIC (0x20001000, what ESP-IDF drives) and as INTPRI (0x600C5000, the same state with the
/// C3's register order plus the four software-interrupt latches). A line is taken when it is
/// enabled, its priority is at or above the threshold, and `mstatus.MIE` is set; the CPU then
/// vectors to `mtvec + 4*line`. Level lines follow the source; edge lines latch until cleared.
pub struct Intc {
    pub lines: esp_periph::intmtx::Lines,
    /// INTPRI_CPU_INTR_FROM_CPU_0..3
    pub sw_int: u32,
    ram: RegRam,
}
impl Default for Intc { fn default() -> Self { Self::new() } }
impl Intc {
    pub fn new() -> Self { Intc { lines: Default::default(), sw_int: 0, ram: RegRam::new() } }

    /// INTPRI register order.
    pub fn intpri_read(&self, off: u32) -> u32 {
        match off {
            0x00 => self.lines.enable, 0x04 => self.lines.int_type, 0x08 => self.lines.level | self.lines.edge_pending,
            0x0c..=0x88 => self.lines.pri[((off - 0x0c) / 4) as usize], 0x8c => self.lines.thresh,
            0x90..=0x9c => (self.sw_int >> ((off - 0x90) / 4)) & 1,
            0xa8 => 0,
            _ => self.ram.read(off),
        }
    }
    pub fn intpri_write(&mut self, off: u32, v: u32) {
        match off {
            0x00 => self.lines.enable = v, 0x04 => self.lines.int_type = v,
            0x0c..=0x88 => self.lines.pri[((off - 0x0c) / 4) as usize] = v & 0xf, 0x8c => self.lines.thresh = v & 0xf,
            0x90..=0x9c => { let b = (off - 0x90) / 4; if v & 1 != 0 { self.sw_int |= 1 << b } else { self.sw_int &= !(1 << b) } }
            0xa8 => self.lines.edge_pending &= !v,
            _ => self.ram.write(off, v),
        }
    }
    /// PLIC register order: enable, type, clear, EIP status, 32 priorities, then the threshold at
    /// 0x90 (the interrupt handler raises it to the taken line's priority + 1 before enabling
    /// nesting) and the claim register. The user-level copy at 0x400 is accepted and ignored.
    pub fn plic_read(&self, off: u32) -> u32 {
        match off {
            0x00 => self.lines.enable, 0x04 => self.lines.int_type, 0x08 => 0, 0x0c => self.lines.level | self.lines.edge_pending,
            0x10..=0x8c => self.lines.pri[((off - 0x10) / 4) as usize], 0x90 => self.lines.thresh,
            0x94 => self.lines.pending().unwrap_or(0),          // CLAIM: the line being taken
            _ => self.ram.read(off),
        }
    }
    pub fn plic_write(&mut self, off: u32, v: u32) {
        match off {
            0x00 => self.lines.enable = v, 0x04 => self.lines.int_type = v, 0x08 => self.lines.edge_pending &= !v,
            0x10..=0x8c => self.lines.pri[((off - 0x10) / 4) as usize] = v & 0xf, 0x90 => self.lines.thresh = v & 0xff,
            _ => self.ram.write(off, v),
        }
    }
}
impl Device for Intc {
    fn read(&mut self, off: u32) -> u32 { Intc::intpri_read(self, off) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { Intc::intpri_write(self, off, v); WriteEffect::NONE }
    fn irq_sources(&self) -> u64 { (self.sw_int & 0xf) as u64 }
}

/// The L1 cache controller. As on the C3 only the "operation finished" bits matter: the ROM and
/// the bootloader kick a sync/lock/preload and poll for done.
pub struct Cache { ram: RegRam }
impl Default for Cache { fn default() -> Self { Self::new() } }
impl Cache {
    pub fn new() -> Self { Cache { ram: RegRam::new() } }
    pub fn read(&self, off: u32) -> u32 {
        let v = self.ram.read(off);
        match off {
            0x02c => if v & (1 << 16) != 0 { v | (1 << 18) } else { v & !(1 << 18) },   // FREEZE_CTRL: DONE follows ENA
            0x088 => v | (1 << 2),                 // LOCK_CTRL: LOCK_DONE
            0x098 => v | (1 << 4),                 // SYNC_CTRL: SYNC_DONE
            0x0d8 => v | (1 << 1),                 // PRELOAD_CTRL: PRELOAD_DONE
            0x134 => v | (1 << 1),                 // AUTOLOAD_CTRL: AUTOLOAD_DONE
            0x3fc => 0x2207_0400,                  // DATE
            _ => v,
        }
    }
    pub fn write(&mut self, off: u32, v: u32) { self.ram.write(off, v); }
}
impl Device for Cache {
    fn read(&mut self, off: u32) -> u32 { Cache::read(self, off) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { Cache::write(self, off, v); WriteEffect::NONE }
}

/// The always-on LP system blocks, one device: LP_CLKRST at 0x000 (reset cause, CPU reset),
/// LP_AON at 0x400 (the ten STORE registers, the software-reset bits), LP_WDT at 0x800, LP_TIMER
/// at 0xc00 (the RTC counter the app reads for wall time). All of it survives a CPU reset.
pub struct LpSys {
    pub ram: RegRam,
    pub reset_cause: u32,
    pub sw_reset: bool,
    /// LP_TIMER: RTC slow-clock ticks since power-on and the value latched by an UPDATE
    pub slow_ticks: u64,
    pub time_latch: u64,
}
impl Default for LpSys { fn default() -> Self { Self::new() } }
impl LpSys {
    pub const CLKRST: u32 = 0x000; pub const AON: u32 = 0x400; pub const WDT: u32 = 0x800; pub const TIMER: u32 = 0xc00;
    pub fn new() -> Self { LpSys { ram: RegRam::new(), reset_cause: RST_POWERON, sw_reset: false, slow_ticks: 0, time_latch: 0 } }
    fn request_reset(&mut self, cause: u32) { self.reset_cause = cause; self.sw_reset = true; }
    pub fn read(&self, off: u32) -> u32 {
        match off {
            0x010 => (self.ram.read(off) & !0x1f) | (self.reset_cause & 0x1f),   // LP_CLKRST_RESET_CAUSE
            0xc14 => self.time_latch as u32, 0xc18 => (self.time_latch >> 32) as u32,   // LP_TIMER_MAIN_BUF0
            0xc10 => self.ram.read(off) & !(1 << 28),                                   // MAIN_TIMER_UPDATE self-clears
            0x3fc | 0x7fc | 0xbfc | 0xffc => 0x2207_0400,
            _ => self.ram.read(off),
        }
    }
    pub fn write(&mut self, off: u32, v: u32) {
        match off {
            0x434 => { if v & (1 << 31) != 0 { self.request_reset(RST_SW_SYS); } self.ram.write(off, v & !(1 << 31)); }   // LP_AON_SYS_CFG.HPSYS_SW_RESET
            0x438 => { if v & (1 << 28) != 0 { self.request_reset(RST_SW_CPU); } self.ram.write(off, v & !(1 << 28)); }   // LP_AON_CPUCORE0_CFG.CPU_CORE0_SW_RESET
            0xc10 => { if v & (1 << 28) != 0 { self.time_latch = self.slow_ticks; } self.ram.write(off, v); }
            _ => self.ram.write(off, v),
        }
    }
}
impl Device for LpSys {
    fn read(&mut self, off: u32) -> u32 { LpSys::read(self, off) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { LpSys::write(self, off, v); WriteEffect::NONE }
    fn clock(&self) -> Option<ClockDomain> { Some(ClockDomain::RtcSlow) }
    fn tick(&mut self, ticks: u64) { self.slow_ticks += ticks; }
}

/// The C6's SPI flash controller is the shared `SpiMem` plus a few status registers of its own:
/// the ROM's `SPI_init` waits for the AXI FIFOs to report empty (AXI_ERR_ADDR, 0x170) before
/// touching the flash.
pub struct SpiMemC6(pub SpiMem);
impl Device for SpiMemC6 {
    fn read(&mut self, off: u32) -> u32 {
        match off {
            0x170 => 0xfc00_0000,                  // ALL_AXI_TRANS_AFIFO_EMPTY and every AFIFO empty/idle bit
            _ => Device::read(&mut self.0, off),
        }
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { Device::write(&mut self.0, off, v) }
    fn debug(&mut self, on: bool) { Device::debug(&mut self.0, on) }
}

/// The analog I2C master (regi2c): two hosts, each a CTRL word — [7:0] slave block, [15:8]
/// register, [23:16] data, [24] write, [25] busy — through which the ROM, IDF and the PHY blob
/// reach the PLL, bias, ADC and RF trim registers. Written values read back per (block,
/// register), like the S3 model. Two things the blobs poll for must come back set:
/// `ANA_CONF0.BBPLL_CAL_DONE`, and the RF block's (0x63) status bits — its registers read as
/// 0xff until written, which is what the PHY's calibration loops wait for.
pub struct AnaMst { ram: RegRam, pub ana: esp_periph::Regi2c }
impl Default for AnaMst { fn default() -> Self { Self::new() } }
impl AnaMst {
    pub fn new() -> Self { AnaMst { ram: RegRam::new(), ana: Default::default() } }
    fn ctrl_read(&self, off: u32) -> u32 {
        let c = self.ram.read(off);
        // the RF block (0x63): register 0 is the sigma-delta modulator status the PHY's
        // `wait_i2c_sdm_stable` polls for 0x5b; the other status registers read all-ones
        self.ana.read(0, c, |key| match key { 0x0063 => &0x5b, k if k & 0xff == 0x63 => &0xff, _ => &0 })
    }
}
impl Device for AnaMst {
    fn read(&mut self, off: u32) -> u32 {
        match off {
            0x00 | 0x04 => self.ctrl_read(off),
            0x18 => self.ram.read(off) | (1 << 24),
            _ => self.ram.read(off),
        }
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        if off == 0x00 || off == 0x04 { self.ana.write(0, v); }
        self.ram.write(off, v);
        WriteEffect::NONE
    }
}

/// ASSIST_DEBUG: the ROM enables PC recording at boot and, after a reset, prints the last
/// recorded PC as `Saved PC` — the instruction after the store that requested the reset.
pub struct AssistDebug { pub saved_pc: u32, ram: RegRam }
impl Default for AssistDebug { fn default() -> Self { Self::new() } }
impl AssistDebug {
    pub fn new() -> Self { AssistDebug { saved_pc: 0, ram: RegRam::new() } }
}
impl Device for AssistDebug {
    fn read(&mut self, off: u32) -> u32 { match off { 0x48 => self.saved_pc, _ => self.ram.read(off) } }   // CORE_0_RCD_PDEBUGPC
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { self.ram.write(off, v); WriteEffect::NONE }
}

pub use esp_periph::rmt_compact::RmtCompact as RmtC6;

/// The C6's GDMA: three channels in the AHB layout (`esp_periph::gdma::AhbLayout::C6`),
/// descriptors in SRAM.
pub use esp_periph::gdma::GdmaAhb as GdmaC6;

/// PCR: peripheral clock and reset control. Configuration reads back; the hardware-fixed clock
/// tree fields read their silicon values, because the clock code derives the current CPU
/// frequency from them (SOC_ROOT → HP_ROOT is a fixed ÷3 on the PLL path, XTAL is 40 MHz, the
/// PLL is 480 MHz) and asserts on a divider it cannot explain.
pub struct Pcr { ram: RegRam }
impl Default for Pcr { fn default() -> Self { Self::new() } }
impl Pcr {
    pub fn new() -> Self {
        let mut p = Pcr { ram: RegRam::new() };
        p.ram.write(0x11c, 0x300);          // AHB_FREQ_CONF: AHB_HS_DIV_NUM = 3
        p.ram.write(0x128, 0x1f);           // PLL_DIV_CLK_EN: every PLL-derived clock on
        p
    }
    pub fn read(&self, off: u32) -> u32 {
        let v = self.ram.read(off);
        match off {
            0x110 => (v & 0x00ff_0000) | 40 << 24 | 2 << 8,   // SYSCLK_CONF: CLK_XTAL_FREQ 40, HS_DIV_NUM 2, LS_DIV_NUM 0 (all HRO)
            0x114 => v | 0x5,                                  // CPU_WAITI_CONF: CPUPERIOD_SEL 1, PLL_FREQ_SEL 1 (HRO)
            0x124 => 480 << 8 | 20,                            // SYSCLK_FREQ_QUERY_0: PLL 480 MHz, FOSC 20 MHz
            0xffc => 0x2207_0400,
            _ => v,
        }
    }
}
impl Device for Pcr {
    fn read(&mut self, off: u32) -> u32 { Pcr::read(self, off) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        self.ram.write(off, v);
        // IDF v5.5.5 soc/esp32c6/register/soc/pcr_reg.h:1322-1336: ECC_CONF 0xd8, ECC_RST_EN bit 1.
        if off == 0xd8 && v & 2 != 0 { WriteEffect::PERIPH_RESET } else { WriteEffect::NONE }
    }
}

/// LPPERI + 0x8 is the hardware RNG (`WDEV_RND_REG` on this chip). The model is the shared
/// xorshift32 in `esp_periph::rng`; re-exported here so `esp32c6::periph::Rng` keeps working.
pub use esp_periph::Rng;

/// Seed the efuse block the way this C6 reads back (a Waveshare ESP32-C6-LCD-1.47, MAC
/// dc:1e:d5:6e:8c:dc, `hw/c6-efuse.txt`): wafer v0.1, package 1, block revision v0.3, embedded
/// flash present. BLK1 word 3 (bits 96..127) holds WAFER_VERSION_MINOR (bit 114, 4 bits),
/// WAFER_VERSION_MAJOR (118), PKG_VERSION (120), BLK_VERSION_MINOR (123), BLK_VERSION_MAJOR (126);
/// word 4 holds FLASH_CAP (128) and FLASH_VENDOR (133). The bootloader prints the wafer and block
/// revisions on every boot and refuses an app whose `min_chip_rev` is above the wafer version.
pub fn efuse_c6(mac: [u8; 6], rev_major: u32, rev_minor: u32, pkg: u32, blk_major: u32, blk_minor: u32) -> Efuse {
    let mut e = Efuse::new(mac);
    e.write(0x48, (mac[0] as u32) << 8 | mac[1] as u32 | 0xfffe << 16);       // BLK1 word 1: MAC high, MAC_EXT ff:fe
    e.write(0x50, (rev_minor & 0xf) << 18 | (rev_major & 3) << 22 | (pkg & 7) << 24 | (blk_minor & 7) << 27 | (blk_major & 3) << 30);
    e.write(0x54, 1 | 1 << 5);                                                 // FLASH_CAP = 1 (4 MB), FLASH_VENDOR = 1
    e.write(0x6c, 0);                                                          // (the S3 layout's BLK_VERSION_MAJOR lives elsewhere here)
    e
}

/// Cold devices share one allocation so unused ECC/PARLIO do not grow the hot bus.
/// I2S stays at offset zero: its existing data path keeps the same indirection.
#[repr(C)]
pub struct OptionalPeripherals {
    pub i2s0: esp_periph::I2s,
    pub ecc: crate::ecc::Ecc,
    pub parlio: crate::parlio::Parlio,
    pub sdio: crate::sdio::SdioSlave,
}

pub struct Peripherals {
    pub adc: esp_periph::sar_adc::SarAdc,
    pub uart: [Uart; 2],
    pub usb: UsbSerialJtag,
    pub systimer: Systimer,
    pub timg: [TimerGroup; 2],
    pub gpio: Gpio,
    pub io_mux: RegRam,
    pub i2c: esp_periph::i2c::I2c,
    pub efuse: Efuse,
    pub spi0: SpiMemC6,
    pub spi1: SpiMemC6,
    pub sha: Sha,
    pub aes: Aes,
    pub rsa: Rsa,
    pub rmt: RmtC6,
    pub gdma: GdmaC6,
    pub spi2: GpSpi,
    pub radio: Ieee802154,
    /// the modem baseband block: the PHY library's channel-switch handshake (`wifi.rs`)
    pub modem_bb: ModemBb,
    /// the 802.11 MAC, as far as the WiFi library has asked for it (`wifi.rs`)
    pub wifi_mac: WifiMac,
    pub intmtx: IntMatrix,
    pub intc: Intc,
    pub cache: Cache,
    pub lpsys: LpSys,
    pub pcr: Pcr,
    pub ana_mst: AnaMst,
    pub assist_debug: AssistDebug,
    pub rng: Rng,
    /// the CPU-subsystem window behind the PLIC: user-level PLIC and the CLINT, unmodelled
    pub cpu_sub: RegRam,
    /// register RAM behind unmodelled blocks, first-touch logging, pc attribution
    pub misc: Misc,
    pub spi_exec: bool,
    clock: ClockTree<4>,
    pub ledc: Ledc,
    pub mcpwm: Mcpwm,
    /// Boxed: these devices are cold unless firmware configures them.
    pub optional: Box<OptionalPeripherals>,
    /// SPI command or I2S RX pending, refreshed only when either changes.
    pub work_pending: bool,
}

// Every peripheral, where it sits (4 KB block number from 0x60000000), and its interrupt sources.
device_set! { Peripherals; inline always; clock: (clock) CPU_HZ, [(ClockDomain::Systimer, 10), (ClockDomain::Apb, 2), (ClockDomain::RtcSlow, 1067), (ClockDomain::Cpu, 1)];
    0x04 "I2C0" optional (i2c) => [src::I2C_EXT0];
    0x0e "APB_SARADC" (adc) => [];
    0x00 "UART0" (uart[0]) => [src::UART0];
    0x01 "UART1" (uart[1]) => [src::UART1];
    0x02 "SPI0" (spi0) => [];
    0x03 "SPI1" (spi1) => [];
    0x06 "RMT" (rmt) => [src::RMT];
    0x15 "PARL_IO" optional (optional.parlio) => [src::PARLIO];
    // The SDIO slave: HINF, SLC and SLCHOST (IDF v5.5.3 soc/esp32c6/register/soc/reg_base.h:35-37), one device.
    0x17 "SDIO_SLC" optional (optional.sdio) delta 0x1000 => [src::SLC0];
    0x16 "SDIO_HINF" alias (optional.sdio) => [];
    0x18 "SDIO_SLCHOST" alias (optional.sdio) delta 0x2000 => [];
    0x07 "LEDC" optional (ledc) => [src::LEDC];
    0x08 "TIMG0" (timg[0]) => [src::TG0_T0];
    0x09 "TIMG1" (timg[1]) => [src::TG1_T0];
    0x0a "SYSTIMER" (systimer) => [src::SYSTIMER_T0, src::SYSTIMER_T1, src::SYSTIMER_T2];
    0x0c "I2S" optional (optional.i2s0) => [src::I2S];
    0x0f "USB_SERIAL_JTAG" (usb) => [src::USB_SERIAL_JTAG];
    0x10 "INTMTX" (intmtx) => [];
    0x14 "MCPWM" optional (mcpwm) => [src::MCPWM0];
    // three channels; the model numbers its sources out 0..4 then in 0..4
    0x80 "GDMA" (gdma) => [src::DMA_OUT_CH0, src::DMA_OUT_CH0 + 1, src::DMA_OUT_CH0 + 2, NO_SOURCE, NO_SOURCE, src::DMA_IN_CH0, src::DMA_IN_CH0 + 1, src::DMA_IN_CH0 + 2, NO_SOURCE, NO_SOURCE];
    0x81 "SPI2" (spi2) => [src::GPSPI2];
    0x88 "AES" (aes) => [src::AES];
    0x89 "SHA" (sha) => [];
    0x8a "RSA" (rsa) => [src::RSA];
    // IDF v5.5.5 soc/esp32c6/register/soc/reg_base.h:44; include/soc/interrupts.h:98.
    0x8b "ECC_MULT" optional (optional.ecc) => [src::ECC];
    0x90 "IO_MUX" (io_mux) => [];
    0x91 "GPIO" (gpio) => [src::GPIO];
    0x96 "PCR" (pcr) => [];
    0xa0 "MODEM_BB" (modem_bb) => [];
    0xa3 "IEEE802154" (radio) => [src::ZB_MAC];
    0xa4 "WIFI_MAC" (wifi_mac) => [src::WIFI_MAC];
    0xaf "I2C_ANA_MST" (ana_mst) delta -0x800 @ 0x800..=0xfff => [];
    // the LP address space: PMU at 0xb0000 is generic; the four LP blocks below are one device
    0xb0 "LP_CLKRST" (lpsys) delta -0x400 @ 0x400..=0x7ff => [];
    0xb0 "EFUSE" (efuse) delta -0x800 @ 0x800..=0xbff => [];
    0xb0 "LP_TIMER" alias (lpsys) @ 0xc00..=0xfff => [];
    0xb1 "LP_AON" alias (lpsys) delta 0x400 @ 0x000..=0x3ff => [];
    0xb1 "LP_WDT" alias (lpsys) delta -0x400 @ 0xc00..=0xfff => [];
    0xb2 "LPPERI_RNG" (rng) @ 0x808..=0x80b => [];
    0xc2 "ASSIST_DEBUG" (assist_debug) => [];
    0xc5 "INTPRI" (intc) => [src::FROM_CPU0, src::FROM_CPU0 + 1, src::FROM_CPU0 + 2, src::FROM_CPU0 + 3];
    0xc8 "CACHE" (cache) => [];
}

impl DeviceSet for Peripherals {
    const BASE: u32 = PERIPH_BASE;
    fn block_name(block: u32) -> &'static str { Peripherals::block_name(block) }
    fn misc(&self) -> &Misc { &self.misc }
    fn misc_mut(&mut self) -> &mut Misc { &mut self.misc }
    fn pre_access(&mut self, block: u32, _off: u32, _write: bool) {
        // IDF v5.5.4 components/soc/esp32c6/register/soc/pcr_reg.h:240-278.
        // Repack PCR's A/B/NUM/SEL fields into the shared S3/C3 layout.
        if block == 0x04 {
            let clock = self.pcr.read(0x24);
            self.i2c.set_external_clock_config(Some(((clock >> 12) & 255) | ((clock & 63) << 8) | (((clock >> 6) & 63) << 14) | (clock & (1 << 20))));
        }
        if block == 0x0e { self.adc.now_cycles = self.clock.cycles(); }
        if block == 0xb2 { self.rng.now = self.clock.cycles() as u32; }
        if block == 0xa3 { self.radio.log_unknown = self.misc.log_unknown; }
    }
}

impl Peripherals {
    pub fn new(mac: [u8; 6]) -> Self {
        Peripherals {
            adc: esp_periph::sar_adc::SarAdc::new(true, CPU_HZ),
            uart: [Uart::new(UartLayout::C6), Uart::new(UartLayout::C6)], usb: UsbSerialJtag::new(CPU_HZ), systimer: Systimer::new(),
            timg: [TimerGroup::new(), TimerGroup::new()], gpio: { let mut g = Gpio::new(); g.func_out_sel.fill(128); g }, io_mux: RegRam::new(), i2c: esp_periph::i2c::I2c::new(), ledc: Ledc::new(LedcLayout::C6), mcpwm: Mcpwm::new(87, 8),
            efuse: efuse_c6(mac, 0, 1, 1, 0, 3),
            spi0: SpiMemC6({ let mut s = SpiMem::new(false); s.has_psram = false; s }),
            spi1: SpiMemC6({ let mut s = SpiMem::new(true); s.has_psram = false; s }),   // no PSRAM on the C6
            sha: Sha::new(), aes: Aes::new(), rsa: Rsa::new(),
            rmt: RmtC6::new(CPU_HZ), gdma: GdmaC6::new(esp_periph::gdma::AhbLayout::C6, 0x4080_0000), spi2: GpSpi::new(), radio: Ieee802154::new(), modem_bb: ModemBb::new(), wifi_mac: WifiMac::new(),
            intmtx: IntMatrix::new(), intc: Intc::new(), cache: Cache::new(), lpsys: LpSys::new(), pcr: Pcr::new(), ana_mst: AnaMst::new(), assist_debug: AssistDebug::new(),
            rng: Rng::new(), cpu_sub: RegRam::new(),
            misc: Misc::new(), spi_exec: false, clock: Self::new_clock(),
            optional: Box::new(OptionalPeripherals { i2s0: esp_periph::I2s::new(CPU_HZ), ecc: Default::default(), parlio: crate::parlio::Parlio::new(), sdio: Default::default() }), work_pending: false,
        }
    }

    pub fn block_name(block: u32) -> &'static str {
        match block {
            0x00 => "UART0", 0x01 => "UART1", 0x02 => "SPI0", 0x03 => "SPI1", 0x04 => "I2C0", 0x05 => "UHCI0",
            0x06 => "RMT", 0x07 => "LEDC", 0x08 => "TIMG0", 0x09 => "TIMG1", 0x0a => "SYSTIMER", 0x0b => "TWAI0",
            0x0c => "I2S", 0x0d => "TWAI1", 0x0e => "APB_SARADC", 0x0f => "USB_SERIAL_JTAG", 0x10 => "INTMTX",
            0x11 => "ATOMIC", 0x12 => "PCNT", 0x13 => "SOC_ETM", 0x14 => "MCPWM", 0x15 => "PARL_IO", 0x16 => "HINF",
            0x17 => "SLC", 0x18 => "SLCHOST", 0x19 => "PVT_MONITOR", 0x80 => "GDMA", 0x81 => "SPI2", 0x88 => "AES",
            0x89 => "SHA", 0x8a => "RSA", 0x8b => "ECC_MULT", 0x8c => "DS", 0x8d => "HMAC", 0x90 => "IO_MUX",
            0x91 => "GPIO", 0x92 => "MEM_MONITOR", 0x93 => "PAU", 0x95 => "HP_SYSTEM", 0x96 => "PCR", 0x98 => "TEE",
            0x99 => "HP_APM", 0x9f => "MISC", 0xa0 => "MODEM_BB", 0xa3 => "IEEE802154", 0xa4 => "WIFI_MAC", 0xa9 => "MODEM_SYSCON", 0xaf => "I2C_ANA_MST", 0xb0 => "PMU/LP_CLKRST/EFUSE/LP_TIMER",
            0xb1 => "LP_AON/LP_UART/LP_I2C/LP_WDT", 0xb2 => "LP_IO/LP_I2C_ANA/LPPERI/LP_ANA_PERI",
            0xb3 => "LP_TEE/LP_APM/OTP_DEBUG", 0xc0 => "TRACE", 0xc2 => "ASSIST_DEBUG", 0xc5 => "INTPRI", 0xc8 => "CACHE",
            _ => "?",
        }
    }

    pub fn uart_route(&self, port: usize) -> esp_soc::uart::UartRoute {
        let clock = self.pcr.read(4 + port as u32 * 12);
        esp_soc::uart::UartPins::C6.route(port, &self.gpio, &self.io_mux,
            self.uart[port].baud(clock, 20_000_000))
    }

    pub fn uart_pin_input(&mut self, input: &esp_soc::uart::UartInput) {
        let routes: [_; 2] = std::array::from_fn(|port| self.uart_route(port));
        esp_soc::uart::uart_pin_input(&mut self.uart, input, &routes);
    }

    pub fn read32(&mut self, addr: u32) -> u32 { mmio::read32(self, addr) }

    fn i2c_pin(&self, signal: usize) -> Option<u8> {
        esp_soc::pins::ChipPins::C6.routes(&self.gpio, &self.io_mux).i2c_pin(signal)
    }

    pub fn write32(&mut self, addr: u32, v: u32) {
        // IDF v5.5.4 components/soc/esp32c6/register/soc/io_mux_reg.h:42-50 (FUN_PD/PU).
        if (0x60090004..=0x6009007c).contains(&addr) {
            self.gpio.set_pad(((addr - 0x60090004) / 4) as u8, v);
        }
        if addr == 0x6000_4004 && v & (1 << 5) != 0 && self.i2c.has_pinned_devices() {
            self.i2c.set_pins(self.i2c_pin(46).zip(self.i2c_pin(45)));
        }
        if addr == PERIPH_BASE + 0x96034 && v & 2 != 0 { self.ledc = Ledc::new(LedcLayout::C6); }
        if addr == PERIPH_BASE + 0x9609c && v & 2 != 0 { self.mcpwm = Mcpwm::new(87, 8); }
        let fx = mmio::write32(self, addr, v);
        // One test covers every effect, as the SPI test alone did before RX existed.
        if fx.contains(WriteEffect::SPI_EXEC | WriteEffect::RX_CONF | WriteEffect::PERIPH_RESET) {
            if fx.contains(WriteEffect::PERIPH_RESET) { self.pcr_reset(); }
            self.spi_exec |= fx.contains(WriteEffect::SPI_EXEC);
            self.refresh_work();
        }
        if addr == PERIPH_BASE + 0x96034 || addr == PERIPH_BASE + 0x96038 {
            let conf = self.pcr.read(0x34); let clock = self.pcr.read(0x38);
            self.ledc.external_clock_hz = if conf & 3 != 1 || clock & (1 << 22) == 0 { 0 } else { match (clock >> 20) & 3 { 1 => 80_000_000, 2 => 17_500_000, 3 => 40_000_000, _ => 0 } };
            self.refresh_optional(0x07);
        }
        if addr == PERIPH_BASE + 0x9609c || addr == PERIPH_BASE + 0x960a0 {
            let conf = self.pcr.read(0x9c); let clock = self.pcr.read(0xa0);
            let source = match (clock >> 20) & 3 { 1 => 160_000_000, 2 => 40_000_000, 3 => 17_500_000, _ => 0 };
            self.mcpwm.source_hz = source / (((clock >> 12) & 0xff) + 1) as u64;
            self.mcpwm.clock_enabled = conf & 3 == 1 && clock & (1 << 22) != 0 && source != 0;
            self.refresh_optional(0x14);
        }
    }

    pub fn refresh_work(&mut self) { self.work_pending = self.spi_exec || self.optional.i2s0.rx_running(); }

    /// PCR reset pulse for a peripheral modeled here; only ECC uses it so far.
    #[cold]
    #[inline(never)]
    fn pcr_reset(&mut self) {
        self.optional.ecc.reset();
        self.refresh_optional(0x8b);
    }

    /// The CPU-subsystem window (0x20001000): the machine-level PLIC is the interrupt controller;
    /// the user-level PLIC and the CLINT read back what was written.
    pub fn cpu_sub_read(&mut self, off: u32) -> u32 {
        if off < 0x400 { self.intc.plic_read(off) } else { self.cpu_sub.read(off) }
    }
    pub fn cpu_sub_write(&mut self, off: u32, v: u32) {
        if off < 0x400 { self.intc.plic_write(off, v) } else { self.cpu_sub.write(off, v) }
    }

    /// Advance every clocked device by `cycles` CPU cycles (16 MHz systimer, 80 MHz APB-domain
    /// timers, the RTC slow clock), with delivered-tick accounting so a slow clock never drifts.
    pub fn tick(&mut self, cycles: u64) { Dispatch::tick(self, cycles); }

    /// CPU cycles until the earliest device deadline (systimer, TIMG, the radio's air-time and
    /// timer countdowns, a running RMT channel), conservative by one device tick;
    /// `u32::MAX` when nothing is armed. What a host that skips idle time may skip.
    pub fn cycles_until_timer(&self) -> u32 { Dispatch::cycles_until_deadline(self) }
    /// CPU cycles since this peripheral set was built.
    pub(crate) fn clock_cycles(&self) -> u64 { self.clock.cycles() }

    /// Which interrupt sources are asserted right now.
    pub fn source_status(&self) -> [u32; 4] { Dispatch::source_status(self) }

    /// Refresh the interrupt matrix; returns true if any source changed.
    pub fn refresh_lines(&mut self) -> bool {
        let st = self.source_status();
        let changed = st != self.intmtx.status;
        self.intmtx.status = st;
        self.intc.lines.update(&self.intmtx.map, &st);
        changed
    }
}

#[cfg(test)]
mod parlio_layout_tests {
    use super::*;
    #[test]
    fn parlio_storage_shares_the_cold_i2s_allocation() {
        assert_eq!(std::mem::offset_of!(OptionalPeripherals, i2s0), 0);
        assert!(std::mem::offset_of!(OptionalPeripherals, parlio) >= size_of::<esp_periph::I2s>());
    }
}

#[cfg(test)]
mod intmtx_status_tests {
    use super::*;

    /// The source map ends exactly where the status registers begin, so either one being off by
    /// a register turns INTR_STATUS into scratch register RAM.
    #[test]
    fn status_registers_sit_directly_above_the_source_map() {
        let mut m = IntMatrix::new();
        assert_eq!(src::COUNT as u32 * 4, 0x134, "ECC_INTR_MAP at 0x130 is the last map register");
        m.map[src::ECC] = 7;
        assert_eq!(m.read(0x130), 7);

        m.status = [1 << src::FROM_CPU0, 1 << (src::UART0 - 32), 1 << (src::ECC - 64), 0];
        assert_eq!(m.read(0x134), 1 << 22);
        assert_eq!(m.read(0x138), 1 << 11);
        assert_eq!(m.read(0x13c), 1 << 12);

        // Read-only, and nothing above them changes.
        m.write(0x134, u32::MAX);
        assert_eq!(m.read(0x134), 1 << 22);
        assert_eq!(m.read(0x140), 0);
    }

    /// esp-hal's vectored handler reads these to find the source behind a CPU line; a software
    /// interrupt is the FreeRTOS/embassy yield that exercises it first.
    #[test]
    fn refresh_lines_publishes_the_asserted_source_to_the_status_registers() {
        let mut p = Peripherals::new([0; 6]);
        p.intmtx.map[src::FROM_CPU0] = 1;
        assert_eq!(p.read32(0x6001_0134), 0);

        p.write32(0x600c_5090, 1); // INTPRI_CPU_INTR_FROM_CPU_0
        assert!(p.refresh_lines());
        assert_eq!(p.read32(0x6001_0134), 1 << src::FROM_CPU0);
        assert_eq!(p.intc.lines.level, 1 << 1);
        assert!(!p.refresh_lines());

        p.write32(0x600c_5090, 0);
        assert!(p.refresh_lines());
        assert_eq!(p.read32(0x6001_0134), 0);
        assert_eq!(p.intc.lines.level, 0);
    }
}

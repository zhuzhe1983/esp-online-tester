//! ESP32-S3 peripherals. The IP shared with other chips lives in `esp-periph`; here are the
//! S3-only blocks (interrupt matrix, WiFi MAC, PCNT, GP-SPI, LCD_CAM, EXTMEM, WDEV, regi2c) and
//! the `Peripherals` set whose one table (`ENTRIES`) drives dispatch, sources, ticks and deadlines.
use emu_core::{ClockDomain, ClockTree};
use esp_periph::{GpSpi, Ledc, LedcLayout, Mcpwm};
use esp_periph::{device_set, mmio, Device, DeviceSet, Dispatch, Misc, WriteEffect};
pub use esp_periph::rmt::RmtDma as Rmt;
pub use esp_periph::{read_desc, reset_cause_name, Aes, DirtyMem, DmaDesc, Efuse, Gdma, GdmaInCh, GdmaOutCh, Gpio, I2s, RegRam, RmtTxCh, Rsa, RtcCntl, Sha, SpiMem, SystemRegs, Systimer, Timer, TimerGroup, Uart, UartLayout, UsbSerialJtag,
                    APB_HZ, DMA_ADDR_BASE, GDMA_CHANNELS, GDMA_CH_STRIDE, RMT_MEM_WORDS, RST_POWERON, RST_RTCWDT_CPU, RST_RTCWDT_RTC, RST_RTCWDT_SYS, RST_SW_CPU, RST_SW_SYS, RTC_SLOW_HZ, SYSTIMER_HZ, XTAL_HZ};
use std::collections::HashMap;

pub const PERIPH_BASE: u32 = 0x6000_0000;
pub const PERIPH_END: u32 = 0x600D_0000;

// interrupt sources (soc/interrupts.h, with the enum's explicit gaps)
pub const SRC_GPIO: usize = 16;
pub const SRC_UART0: usize = 27;
pub const SRC_UART1: usize = 28;
pub const SRC_UART2: usize = 29;
pub const SRC_SPI2: usize = 21;
/// ETS_SPI3_INTR_SOURCE (IDF v5.5.5 components/soc/esp32s3/include/soc/interrupts.h:41; v4.4.8
/// components/soc/esp32s3/include/soc/periph_defs.h:81).
pub const SRC_SPI3: usize = 22;
/// IDF v5.5.1 soc/esp32s3/include/soc/interrupts.h:48.
pub const SRC_SDIO_HOST: usize = 30;
pub const SRC_PWM0: usize = 31;
pub const SRC_PWM1: usize = 32;
pub const SRC_LEDC: usize = 35;
pub const SRC_PCNT: usize = 41;
pub const SRC_AES: usize = 77;
pub const SRC_LCD_CAM: usize = 24;
pub const SRC_I2S0: usize = 25;
pub const SRC_I2S1: usize = 26;
pub const SRC_RMT: usize = 40;
pub const SRC_I2C0: usize = 42;
pub const SRC_I2C1: usize = 43;
pub const SRC_TG0_T0: usize = 50;
pub const SRC_TG0_T1: usize = 51;
pub const SRC_TG0_WDT: usize = 52;
pub const SRC_TG1_T0: usize = 53;
pub const SRC_TG1_T1: usize = 54;
pub const SRC_TG1_WDT: usize = 55;
pub const SRC_SYSTIMER_T0: usize = 57;
pub const SRC_SYSTIMER_T1: usize = 58;
pub const SRC_SYSTIMER_T2: usize = 59;
pub const SRC_DMA_IN_CH0: usize = 66;
pub const SRC_DMA_OUT_CH0: usize = 71;
pub const SRC_FROM_CPU0: usize = 79;
pub const SRC_USB_SERIAL_JTAG: usize = 96;
pub const NUM_SOURCES: usize = 99;

pub const CPU_HZ: u64 = 240_000_000;

// ------------------------------------------------------------------ Interrupt matrix
/// `status` is the live source state, refreshed by `Peripherals::pre_access` before a read.
pub struct IntMatrix { pub map: [[u32; NUM_SOURCES]; 2], ram: RegRam, pub status: [u32; 4] }
impl Default for IntMatrix { fn default() -> Self { Self::new() } }

impl IntMatrix {
    pub fn new() -> Self { IntMatrix { map: [[6; NUM_SOURCES]; 2], ram: RegRam::new(), status: [0; 4] } }
    pub fn read(&self, off: u32) -> u32 { let status = &self.status;
        let (core, o) = if off >= 0x800 { (1usize, off - 0x800) } else { (0usize, off) };
        let idx = (o >> 2) as usize;
        if idx < NUM_SOURCES { return self.map[core][idx]; }
        match o { 0x18c => status[0], 0x190 => status[1], 0x194 => status[2], 0x198 => status[3], 0x7fc => 0x2007210, _ => self.ram.read(off) }
    }
    pub fn write(&mut self, off: u32, v: u32) {
        let (core, o) = if off >= 0x800 { (1usize, off - 0x800) } else { (0usize, off) };
        let idx = (o >> 2) as usize;
        if idx < NUM_SOURCES { self.map[core][idx] = v & 0x1f; } else { self.ram.write(off, v); }
    }
}

pub const SRC_RSA: usize = 76;
// ------------------------------------------------------------------ WiFi MAC (blocks 0x33/0x34)
pub const SRC_WIFI_MAC: usize = 0;
/// The 802.11 MAC the closed `libpp`/`libnet80211` drive. Undocumented by Espressif; the register
/// layout matches the classic ESP32's as reverse-engineered by esp32-open-mac (0x3ff73000 there,
/// 0x60033000 here). Modelled from the blob's own accesses — see docs/wifi-plan.md.
///   TX: 5 slots; slot n has TX_CONFIG at 0xd1c-8n and PLCP0 at 0xd20-8n; PLCP0 = (desc & 0xfffff) | 0x600000,
///       bits 31:30 start the transmission. Completion: TXQ_STATE_COMPLETE (0xcc8) bit n, cleared via 0xcc4;
///       DMA_INT_STATUS (0xc48) bit 7, cleared via 0xc4c.
///   RX: descriptor ring base at 0x088 (dma_list_item: size:12 length:12 _:6 has_data:1 owner:1, packet, next).
pub struct WifiMac { pub ram: RegRam, pub ram2: RegRam, pub log: bool,
                     /// TSF: 1 MHz counter (offset applied to the CPU cycle clock), latched into WDEV 0x18/0x1c
                     pub tsf_offset: i64, pub tsf_latched: u64, pub now_cycles: u64,
                     /// interrupt events (0xc3c; cleared by writing 0xc40): bit 7 = TX complete, bits 14/24 = RX data (libpp wDev_ProcessFiq)
                     pub events: u32, pub pwr_events: u32,
                     /// per-queue completion bitmap (0xca8 bits 10:0, cleared via 0xca4)
                     pub txq_complete: u32, pub txq_error: u32,
                     pub tx_pending: Vec<(u8, u32)>,
                     /// RX descriptor ring: base written by the driver (0x088), the descriptor the hardware fills next, the last one filled
                     pub rx_base: u32, pub rx_next: u32, pub rx_last: u32,
                     /// the access point, the network and the counts, shared with the other chips
                     pub link: crate::wifi::StationLink }
impl Default for WifiMac { fn default() -> Self { Self::new() } }

impl WifiMac {
    pub fn new() -> Self { WifiMac { ram: RegRam::new(), ram2: RegRam::new(), log: false, tsf_offset: 0, tsf_latched: 0, now_cycles: 0, rx_base: 0, rx_next: 0, rx_last: 0, events: 0, pwr_events: 0, txq_complete: 0, txq_error: 0, tx_pending: Vec::new(), link: Default::default() } }
    pub fn irq(&self) -> bool { self.events != 0 || self.pwr_events != 0 }
    /// TX queue n has its PLCP0 register at 0xd08 - 8n (hal_mac_txq_enable: (0x0c0067a1 - n) << 3).
    fn txq_of(off: u32) -> Option<u8> { if off <= 0xd08 && (0xd08 - off).is_multiple_of(8) && (0xd08 - off) / 8 < 16 { Some(((0xd08 - off) / 8) as u8) } else { None } }
    pub fn read(&mut self, block: u32, off: u32) -> u32 {
        let v = match (block, off) {
            (0x33, 0xd14) => self.ram.read(off) | 1,                 // hal_init: writes bit 1, waits for bit 0
            (0x33, 0xc3c) => self.events,
            (0x33, 0x088) => self.rx_base & 0xf_ffff, (0x33, 0x08c) => self.rx_next & 0xf_ffff, (0x33, 0x090) => self.rx_last,
            (0x33, 0xca8) => self.txq_error & 0x7ff,                 // txq state types 0/1 (errors/collisions)
            (0x33, 0xcb0) => self.txq_complete & 0xf,                    // txq state type 2: completed queues
            (0x35, 0x118) => self.pwr_events,
            (0x35, 0x18) => self.tsf_latched as u32,
            (0x35, 0x1c) => (self.tsf_latched >> 32) as u32,
            (0x35, 0x128) => self.ram2.read(off),
            (0x33, _) => self.ram.read(off),
            (_, _) => self.ram2.read(off),
        };
        if self.log { eprintln!("[wifi] rd {:#x}+{:#05x} -> {:#010x}", block, off, v); }
        v
    }
    pub fn write(&mut self, block: u32, off: u32, v: u32) {
        if self.log { eprintln!("[wifi] wr {:#x}+{:#05x} <- {:#010x}", block, off, v); }
        match (block, off) {
            (0x33, 0xc40) => { self.events &= !v; }
            (0x33, 0x088) => {
                // BASE_RX_DSCR: where the hardware restarts when the ring runs dry. Software rewrites it
                // every time it recycles descriptors, but that must NOT rewind the hardware's current
                // pointer — doing so re-delivers into descriptors the stack has already moved past.
                self.rx_base = DMA_ADDR_BASE | (v & 0xf_ffff);
                if self.rx_next == 0 { self.rx_next = self.rx_base; }
                self.ram.write(off, v);
            }
            (0x33, 0x084) => {
                // DSCR_RELOAD: software has appended recycled descriptors and asks the hardware to
                // re-read the chain.
                // Measured against the blob: rewinding here makes every second frame land in a
                // descriptor the stack has moved past, and it is recycled instead of indicated. The
                // hardware keeps its own pointer; base only matters once the ring has run dry.
                if v & 1 != 0 && self.rx_next == 0 { self.rx_next = self.rx_base; }
                self.ram.write(off, v & !1);
            }
            (0x35, 0x11c) => { self.pwr_events &= !v; }
            (0x35, 0x0c) => {
                let now = (self.now_cycles / (CPU_HZ / 1_000_000)) as i64;
                if v & 3 != 0 { self.tsf_latched = (now + self.tsf_offset) as u64; }                              // latch
                if v & (1 << 4) != 0 { let set = (self.ram2.read(0x10) as u64) | ((self.ram2.read(0x14) as u64) << 32); self.tsf_offset = set as i64 - now; }   // load
                self.ram2.write(off, v);
            }
            (0x33, 0xca4) => { self.txq_error &= !(v & 0x7ff); }
            (0x33, 0xcac) => { self.txq_complete &= !(v & 0xf); }
            (0x33, o) if Self::txq_of(o).is_some() => {                                   // MAC_TX_PLCP0[queue]
                self.ram.write(off, v);
                if v & (1 << 31) != 0 { let q = Self::txq_of(o).unwrap(); self.tx_pending.push((q, DMA_ADDR_BASE | (v & 0xf_ffff))); }
            }
            (0x33, _) => self.ram.write(off, v),
            (_, _) => self.ram2.write(off, v),
        }
    }
    /// Hardware finished sending the frame in `queue`.
    pub fn tx_done(&mut self, queue: u8) {
        self.txq_complete |= 1 << queue; self.events |= 1 << 7;
        let o = 0xd08 - 8 * queue as u32; let v = self.ram.read(o); self.ram.write(o, v & !(3 << 30));
        // result word (hal_mac_get_txq_pmd): bits 15:12 = status code, 0 = success (3 would trap the blob)
        let r = 0x320 - 76 * queue as u32; let w = self.ram2.read(r); self.ram2.write(r, w & !(0xf << 12));
    }
}

// ------------------------------------------------------------------ PCNT (pulse counter)
/// Four units, two channels each: a signal input counts on rising/falling edges (mode 0 ignore,
/// 1 increment, 2 decrement) and a control input modifies that (hctrl/lctrl mode 0 keep, 1 invert,
/// 2 disable). Inputs arrive through the GPIO matrix (signals 33 + 4*unit .. 36 + 4*unit).
/// Counters are 16-bit signed; high/low limits, thresholds and zero crossings raise the unit's interrupt.
pub struct Pcnt { pub conf: [[u32; 3]; 4], pub cnt: [i16; 4], pub status: [u32; 4], pub int_raw: u32, pub int_ena: u32, pub ctrl: u32, ram: RegRam, pub events: u64 }
impl Default for Pcnt { fn default() -> Self { Self::new() } }

impl Pcnt {
    pub fn new() -> Self { Pcnt { conf: [[0; 3]; 4], cnt: [0; 4], status: [0; 4], int_raw: 0, int_ena: 0, ctrl: 0, ram: RegRam::new(), events: 0 } }
    pub fn irq(&self) -> bool { self.int_raw & self.int_ena != 0 }
    pub fn read(&self, off: u32) -> u32 {
        match off {
            0x00..=0x2c => { let u = (off / 12) as usize; self.conf[u][((off % 12) / 4) as usize] }
            0x30..=0x3c => self.cnt[((off - 0x30) / 4) as usize] as u16 as u32,
            0x40 => self.int_raw, 0x44 => self.int_raw & self.int_ena, 0x48 => self.int_ena,
            0x50..=0x5c => self.status[((off - 0x50) / 4) as usize],
            0x60 => self.ctrl, 0xfc => 0x1912_0400,
            _ => self.ram.read(off),
        }
    }
    pub fn write(&mut self, off: u32, v: u32) {
        match off {
            0x00..=0x2c => { let u = (off / 12) as usize; self.conf[u][((off % 12) / 4) as usize] = v; }
            0x48 => self.int_ena = v, 0x4c => self.int_raw &= !v,
            0x60 => { self.ctrl = v; for u in 0..4 { if v & (1 << (2 * u)) != 0 { self.cnt[u] = 0; } } }
            _ => self.ram.write(off, v),
        }
    }
    /// A GPIO input changed: `sig(idx)` gives the level of matrix input signal `idx` (None = not routed).
    pub fn gpio_edge(&mut self, pin: u8, level: bool, sig: &dyn Fn(u32) -> Option<(u8, bool)>) {
        for u in 0..4 {
            if self.ctrl & (1 << (2 * u)) != 0 || self.ctrl & (1 << (2 * u + 1)) != 0 { continue; }   // reset held / paused
            let conf0 = self.conf[u][0];
            for ch in 0..2u32 {
                let Some((sp, _)) = sig(33 + 4 * u as u32 + ch) else { continue };
                if sp != pin { continue; }
                let sh = 16 + 8 * ch;
                let mode = if level { (conf0 >> (sh + 2)) & 3 } else { (conf0 >> sh) & 3 };          // pos_mode / neg_mode
                let mut delta: i32 = match mode { 1 => 1, 2 => -1, _ => 0 };
                if delta != 0 {
                    let ctrl_level = sig(35 + 4 * u as u32 + ch).is_none_or(|(_, l)| l);
                    let cm = if ctrl_level { (conf0 >> (sh + 4)) & 3 } else { (conf0 >> (sh + 6)) & 3 };   // hctrl / lctrl
                    match cm { 1 => delta = -delta, 2 => delta = 0, _ => {} }
                }
                if delta != 0 { self.count(u, delta); }
            }
        }
    }
    fn count(&mut self, u: usize, delta: i32) {
        let conf0 = self.conf[u][0]; let conf1 = self.conf[u][1]; let conf2 = self.conf[u][2];
        let old = self.cnt[u] as i32; let mut new = old + delta;
        let mut ev = 0u32;
        if conf0 & (1 << 12) != 0 && new >= (conf2 & 0xffff) as i16 as i32 { ev |= 1 << 5; new = 0; }         // h_lim
        if conf0 & (1 << 13) != 0 && new <= (conf2 >> 16) as i16 as i32 { ev |= 1 << 4; new = 0; }             // l_lim
        if conf0 & (1 << 14) != 0 && new == (conf1 & 0xffff) as i16 as i32 { ev |= 1 << 3; }                   // thres0
        if conf0 & (1 << 15) != 0 && new == (conf1 >> 16) as i16 as i32 { ev |= 1 << 2; }                      // thres1
        if conf0 & (1 << 11) != 0 && new == 0 && old != 0 { ev |= if delta > 0 { 1 } else { 2 }; }             // zero (mode: 1 from negative, 2 from positive)
        self.cnt[u] = new as i16; self.events += 1;
        if ev != 0 { self.status[u] = ev; self.int_raw |= 1 << u; }
    }
}

// ------------------------------------------------------------------ LCD_CAM (camera side)
/// LCD_CAM captures sensor frames through GDMA trigger 5 while CAM_START is set.
/// The sensor frame clock and VSYNC continue while capture is stopped.
pub struct LcdCam { pub ram: RegRam, cam_ctrl: u32, pub cam_ctrl1: u32, pub int_raw: u32, int_ena: u32, running: bool,
                    pub frame_cycles: u64, pub acc: u64, pub frames: u64, pub dropped: u64,
                    // LCD side (RGB / DPI mode): the panel is refreshed from a GDMA out-channel on trigger 5
                    pub lcd_clock: u32, pub lcd_user: u32, pub lcd_ctrl: u32, pub lcd_ctrl1: u32, pub lcd_acc: u64, pub lcd_frames: u64, pub lcd_line: Vec<u8>, pub lcd_fifo: std::collections::VecDeque<u8>, pub lcd_log: bool,
                    cam_frame: Option<std::sync::Arc<Vec<u8>>>, cam_active: bool, cam_clock_active: bool,
                    clock_enabled: bool, pub frame_dropped: bool, pub frame_start: u64 }
impl Default for LcdCam { fn default() -> Self { Self::new() } }

impl LcdCam {
    pub fn new() -> Self { LcdCam { ram: RegRam::new(), cam_ctrl: 0, cam_ctrl1: 0, int_raw: 0, int_ena: 0, running: false, cam_active: false, cam_clock_active: false, cam_frame: None, clock_enabled: false, frame_dropped: false, frame_start: 0, frame_cycles: CPU_HZ / 10, acc: 0, frames: 0, dropped: 0,
                                    lcd_clock: 0, lcd_user: 0, lcd_ctrl: 0, lcd_ctrl1: 0, lcd_acc: 0, lcd_frames: 0, lcd_line: Vec::new(), lcd_fifo: std::collections::VecDeque::new(), lcd_log: false } }
    pub fn irq(&self) -> bool { self.int_raw & self.int_ena != 0 }
    /// LCD RGB mode running: LCD_START (USER bit 27) with LCD_RGB_MODE_EN (CTRL bit 31).
    pub fn lcd_running(&self) -> bool { self.lcd_user & (1 << 27) != 0 && self.lcd_ctrl & (1 << 31) != 0 }
    /// (active width, active height, bytes per pixel, CPU cycles per frame) from the timing registers.
    pub fn lcd_geometry(&self) -> (u32, u32, u32, u64) {
        // the registers hold (value - 1): lcd_ll_set_horizontal/vertical_timing
        let ha = ((self.lcd_ctrl1 >> 8) & 0xfff) + 1; let ht = ((self.lcd_ctrl1 >> 20) & 0xfff) + 1;
        let va = ((self.lcd_ctrl >> 11) & 0x3ff) + 1; let vt = ((self.lcd_ctrl >> 21) & 0x3ff) + 1;
        let bpp = if self.lcd_user & (1 << 23) != 0 { 2 } else { 1 };
        // lcd_clk = src / (div_num + div_b/div_a); pclk = lcd_clk / (clkcnt_n + 1) unless CLK_EQU_SYSCLK
        let src = match (self.lcd_clock >> 29) & 3 { 1 => 40_000_000f64, 2 => 240_000_000.0, _ => 160_000_000.0 };
        let div_num = ((self.lcd_clock >> 9) & 0xff).max(1) as f64; let div_b = ((self.lcd_clock >> 17) & 0x3f) as f64; let div_a = ((self.lcd_clock >> 23) & 0x3f) as f64;
        let lcd_clk = src / (div_num + if div_a > 0.0 { div_b / div_a } else { 0.0 });
        let n = if self.lcd_clock & (1 << 6) != 0 { 1.0 } else { (self.lcd_clock & 0x3f) as f64 + 1.0 };
        let pclk = (lcd_clk / n).max(1_000_000.0) as u64;
        let frame_px = (ht as u64) * (vt as u64);
        (ha, va, bpp, frame_px * CPU_HZ / pclk)
    }
    pub fn read(&self, off: u32) -> u32 {
        match off { 0x00 => self.lcd_clock, 0x04 => self.cam_ctrl, 0x08 => self.cam_ctrl1, 0x14 => self.lcd_user & !((1 << 20) | (1 << 28)), 0x1c => self.lcd_ctrl, 0x20 => self.lcd_ctrl1,
                    0x64 => self.int_ena, 0x68 => self.int_raw, 0x6c => self.int_raw & self.int_ena, _ => self.ram.read(off) }
    }
    pub fn write(&mut self, off: u32, v: u32) {
        match off {
            0x00 => self.lcd_clock = v,
            0x14 => { let was = self.lcd_running(); self.lcd_user = v;
                      if v & (1 << 28) != 0 { self.lcd_line.clear(); self.lcd_fifo.clear(); self.lcd_acc = 0; }                       // LCD_RESET
                      if !was && self.lcd_running() { self.lcd_line.clear(); self.lcd_acc = 0; }
                      if self.lcd_log { eprintln!("[lcd] USER <- {:#010x} (start {} reset {} update {})", v, v >> 27 & 1, v >> 28 & 1, v >> 20 & 1); } }
            0x18 => { if v & (1 << 27) != 0 { self.lcd_fifo.clear(); if self.lcd_log { eprintln!("[lcd] AFIFO reset"); } } self.ram.write(off, v); }   // LCD_MISC.AFIFO_RESET
            0x1c => self.lcd_ctrl = v, 0x20 => self.lcd_ctrl1 = v,
            0x04 => { self.cam_ctrl = v & !(1 << 4); self.refresh_cam_active(); }                                                                          // CAM_UPDATE (self-clearing)
            0x08 => { self.cam_ctrl1 = v & !(3 << 30); self.running = v & (1 << 29) != 0; if v & (1 << 30) != 0 { self.running = false; self.cam_ctrl1 &= !(1 << 29); } self.refresh_cam_active(); }   // CAM_START / CAM_RESET
            0x64 => { self.int_ena = v; self.refresh_cam_active(); }, 0x70 => self.int_raw &= !v,
            _ => self.ram.write(off, v),
        }
    }
    /// Cache activity at register/frame changes so idle ticks only read a flag.
    fn refresh_cam_active(&mut self) {
        // IDF v5.5.4 soc/lcd_cam_reg.h: CAM_CLK_SEL bits 29..30, zero disables XCLK.
        self.cam_active = self.clock_enabled && (self.cam_ctrl >> 29) & 3 != 0 && (self.running || self.int_ena & (1 << 2) != 0);
        self.cam_clock_active = self.clock_enabled && (self.cam_ctrl >> 29) & 3 != 0 && (self.cam_active || self.cam_frame.is_some());
    }
    pub fn cam_ctrl(&self) -> u32 { self.cam_ctrl }
    pub fn cam_active(&self) -> bool { self.cam_active }
    pub fn cam_clock_active(&self) -> bool { self.cam_clock_active }
    pub fn running(&self) -> bool { self.running }
    pub fn cam_frame(&self) -> Option<&std::sync::Arc<Vec<u8>>> { self.cam_frame.as_ref() }
    pub fn set_cam_frame(&mut self, frame: Option<std::sync::Arc<Vec<u8>>>) { self.cam_frame = frame; self.frame_start = self.acc; self.refresh_cam_active(); }
    pub fn set_clock_enabled(&mut self, enabled: bool) { self.clock_enabled = enabled; self.refresh_cam_active(); }
    /// Advance the sensor clock independently of CAM_START and CAM_RESET.
    pub fn frame_due(&mut self, cycles: u64) -> bool { self.acc = self.acc.saturating_add(cycles); if self.acc >= self.frame_cycles.max(1) { self.acc %= self.frame_cycles.max(1); true } else { false } }
}

// ------------------------------------------------------------------ EXTMEM (cache controller; MMU table lives in the bus)
pub struct Extmem { pub ram: RegRam }
impl Default for Extmem { fn default() -> Self { Self::new() } }

impl Extmem {
    pub fn new() -> Self { Extmem { ram: RegRam::new() } }
    pub fn read(&self, off: u32) -> u32 {
        let v = self.ram.read(off);
        match off {
            0x28 => v | (1 << 3),                 // DCACHE_SYNC_CTRL: SYNC_DONE
            0x88 => v | (1 << 1),                 // ICACHE_SYNC_CTRL: SYNC_DONE
            0x40 | 0x94 => v | (1 << 1),          // *CACHE_PRELOAD_CTRL: PRELOAD_DONE
            0x4c | 0xa0 => v | (1 << 3),          // *CACHE_AUTOLOAD_CTRL: AUTOLOAD_DONE
            0x34 => v | (1 << 1),                 // DCACHE_OCCUPY_CTRL: OCCUPY_DONE
            0x150 | 0x154 => if v & 1 != 0 { v | (1 << 2) } else { v & !(1 << 2) },   // *CACHE_FREEZE: FREEZE_DONE follows FREEZE_ENA
            0x1c | 0x7c => v | (1 << 2),      // *CACHE_LOCK_CTRL: 0x1cONE
            0x130 => 0x1001,                      // CACHE_STATE: icache/dcache idle
            0x3fc => 0x2101070,
            _ => v,
        }
    }
    pub fn write(&mut self, off: u32, v: u32) { self.ram.write(off, v); }
}

// ------------------------------------------------------------------ WDEV (radio) block: only the hardware RNG register matters to us
pub struct Wdev { state: u64, ram: RegRam }
impl Default for Wdev { fn default() -> Self { Self::new() } }

impl Wdev {
    pub fn new() -> Self { Wdev { state: 0x9E37_79B9_7F4A_7C15, ram: RegRam::new() } }
    pub fn read(&mut self, off: u32) -> u32 {
        match off {
            0x7c => { let mut x = self.state; x ^= x << 13; x ^= x >> 7; x ^= x << 17; self.state = x; (x >> 16) as u32 }   // WDEV_RND_REG (xorshift64)
            _ => self.ram.read(off),
        }
    }
    pub fn write(&mut self, off: u32, v: u32) { self.ram.write(off, v); }
}

// ------------------------------------------------------------------ I2C_MST: analog "regi2c" master (PLL / SAR ADC trim registers)
pub struct I2cMst { pub ram: RegRam, pub ana: esp_periph::Regi2c }
impl Default for I2cMst { fn default() -> Self { Self::new() } }

impl I2cMst {
    pub fn new() -> Self { I2cMst { ram: RegRam::new(), ana: Default::default() } }
    pub fn read(&mut self, off: u32) -> u32 {
        match off {
            0x0 => self.ana.read(0, self.ram.read(off), |_| &0),
            // analog-block handshakes (BBPLL cal, pkdet, txdc/rxdc comparators...): the blob writes a start bit and
            // polls a done bit in 26:24; comparator sign bits 31:30 read as 0 — enough for its search loops to run
            0x40..=0x5c => (self.ram.read(off) & 0x3fff_ffff) | (7 << 24),
            _ => self.ram.read(off),
        }
    }
    pub fn write(&mut self, off: u32, v: u32) {
        if off == 0 { self.ana.write(0, v); }
        self.ram.write(off, v);
    }
}


// ------------------------------------------------------------------ S3-only devices as `Device`
impl Device for IntMatrix {
    fn read(&mut self, off: u32) -> u32 { IntMatrix::read(self, off) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { IntMatrix::write(self, off, v); WriteEffect::INTMAP }
}
/// The MAC spans blocks 0x33..0x35; the table mounts it three times with `delta` = block index << 12.
impl Device for WifiMac {
    fn read(&mut self, off: u32) -> u32 { WifiMac::read(self, 0x33 + (off >> 12), off & 0xfff) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { WifiMac::write(self, 0x33 + (off >> 12), off & 0xfff, v); WriteEffect::NONE }
    fn irq_sources(&self) -> u64 { self.irq() as u64 }
    fn debug(&mut self, on: bool) { self.log = on; }
}
impl Device for Pcnt {
    fn read(&mut self, off: u32) -> u32 { Pcnt::read(self, off) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { Pcnt::write(self, off, v); WriteEffect::NONE }
    fn irq_sources(&self) -> u64 { self.irq() as u64 }
}
impl Device for LcdCam {
    fn read(&mut self, off: u32) -> u32 { LcdCam::read(self, off) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { LcdCam::write(self, off, v); WriteEffect::NONE }
    fn irq_sources(&self) -> u64 { self.irq() as u64 }
    fn debug(&mut self, on: bool) { self.lcd_log = on; }
}
impl Device for Extmem {
    fn read(&mut self, off: u32) -> u32 { Extmem::read(self, off) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { Extmem::write(self, off, v); WriteEffect::NONE }
}
impl Device for Wdev {
    fn read(&mut self, off: u32) -> u32 { Wdev::read(self, off) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { Wdev::write(self, off, v); WriteEffect::NONE }
}
impl Device for I2cMst {
    fn read(&mut self, off: u32) -> u32 { I2cMst::read(self, off) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { I2cMst::write(self, off, v); WriteEffect::NONE }
}
/// The FE (RF front end) block is otherwise unmodelled; the one bit the WiFi blob polls is the
/// inferred "IQ estimation done" flag at +0x174. Ideal-radio PHY bring-up completes
/// even without an AP; this is not association or a silicon calibration timing model.
/// ESP-IDF v5.5.4 soc/fe_reg.h:23-31 names IQ_EST bits in FE_GEN_CTRL (+0x90),
/// not this undocumented result register; the IQ naming comes from those bits.
pub struct FeIq { word: u32 }
impl Device for FeIq {
    fn read(&mut self, _off: u32) -> u32 { self.word | (1 << 16) }
    fn write(&mut self, _off: u32, v: u32) -> WriteEffect { self.word = v; WriteEffect::NONE }
}

// ------------------------------------------------------------------ all together
pub struct Peripherals {
    pub usb: UsbSerialJtag,
    pub uart: [Uart; 3],
    pub systimer: Systimer,
    pub timg: [TimerGroup; 2],
    pub intmatrix: IntMatrix,
    pub gpio: Gpio,
    pub rtc: RtcCntl,
    pub efuse: Efuse,
    pub system: SystemRegs,
    pub extmem: Extmem,
    pub spi0: SpiMem,
    pub spi1: SpiMem,
    pub i2c: [crate::i2c::I2c; 2],
    pub lcd_cam: LcdCam,
    pub spi2: GpSpi,
    /// GP-SPI3 (Arduino `HSPI`), the same register block as SPI2, at DR_REG_SPI3_BASE 0x60025000
    /// (IDF v5.5.5 components/soc/esp32s3/register/soc/reg_base.h:39; v4.4.8 soc.h:74).
    pub spi3: GpSpi,
    pub pcnt: Pcnt,
    pub wifi: WifiMac,
    pub fe: FeIq,
    pub aes: Aes,
    pub rsa: Rsa,
    pub sha: Sha,
    pub wdev: Wdev,
    pub i2c_mst: I2cMst,
    pub gdma: Gdma,
    pub i2s0: I2s,
    pub i2s1: I2s,
    pub rmt: Rmt,
    pub io_mux: RegRam,
    /// register RAM behind unmodelled blocks, first-touch logging, pc attribution
    pub misc: Misc,
    /// experiment hook: ESP_EMU_FAKE_READ=addr:or[:and],... applied to register reads
    pub fake_reads: HashMap<u32, (u32, u32)>,
    clock: ClockTree<4>,
    pub spi_exec: bool,       // SPI1 command pending execution against the flash array
    last_status: [u32; 4],
    pub intmatrix_dirty: bool,
    pub ledc: Ledc,
    pub mcpwm: [Mcpwm; 2],
    pub sdmmc: esp_periph::sdmmc::Sdmmc,
}

// Every peripheral, where it sits, and its interrupt source numbers. Entries for one block are
// tried in order: a range-limited entry goes before the full-block one behind it.
device_set! { Peripherals; clock: (clock) CPU_HZ, [(ClockDomain::Systimer, 15), (ClockDomain::Apb, 3), (ClockDomain::RtcSlow, 1600), (ClockDomain::Cpu, 1)];
    0x00 "UART0" (uart[0]) => [SRC_UART0];
    0x10 "UART1" (uart[1]) => [SRC_UART1];
    0x2e "UART2" (uart[2]) => [SRC_UART2];
    0x38 "USB_SERIAL_JTAG" (usb) => [SRC_USB_SERIAL_JTAG];
    0x23 "SYSTIMER" (systimer) => [SRC_SYSTIMER_T0, SRC_SYSTIMER_T1, SRC_SYSTIMER_T2];
    0x1f "TIMG0" (timg[0]) => [SRC_TG0_T0, SRC_TG0_T1];
    0x20 "TIMG1" (timg[1]) => [SRC_TG1_T0, SRC_TG1_T1];
    0x19 "LEDC" optional (ledc) => [SRC_LEDC];
    0x1e "MCPWM0" optional (mcpwm[0]) => [SRC_PWM0];
    0x2c "MCPWM1" optional (mcpwm[1]) => [SRC_PWM1];
    0xc2 "INTERRUPT" (intmatrix) => [];
    0x04 "GPIO" (gpio) => [SRC_GPIO];
    0x08 "RTC" (rtc) => [];
    0x07 "EFUSE" (efuse) => [];
    0xc0 "SYSTEM" (system) => [SRC_FROM_CPU0, SRC_FROM_CPU0 + 1, SRC_FROM_CPU0 + 2, SRC_FROM_CPU0 + 3];
    0xc4 "EXTMEM" (extmem) => [];
    0x09 "IO_MUX" (io_mux) => [];
    0x02 "SPI1" (spi1) => [];
    0x03 "SPI0" (spi0) => [];
    0x3b "SHA" (sha) => [];
    0x0e "I2C_MST" (i2c_mst) => [];
    0x3f "GDMA" (gdma) => [SRC_DMA_OUT_CH0, SRC_DMA_OUT_CH0 + 1, SRC_DMA_OUT_CH0 + 2, SRC_DMA_OUT_CH0 + 3, SRC_DMA_OUT_CH0 + 4,
                           SRC_DMA_IN_CH0, SRC_DMA_IN_CH0 + 1, SRC_DMA_IN_CH0 + 2, SRC_DMA_IN_CH0 + 3, SRC_DMA_IN_CH0 + 4];
    0x0f "I2S0" (i2s0) => [SRC_I2S0];
    0x2d "I2S1" (i2s1) => [SRC_I2S1];
    0x16 "RMT" (rmt) => [SRC_RMT];
    0x13 "I2C0" optional (i2c[0]) => [SRC_I2C0];
    0x27 "I2C1" optional (i2c[1]) => [SRC_I2C1];
    0x41 "LCD_CAM" (lcd_cam) => [SRC_LCD_CAM];
    0x24 "SPI2" (spi2) => [SRC_SPI2];
    0x25 "SPI3" (spi3) => [SRC_SPI3];
    // IDF v5.5.1 ESP32-S3 sdmmc_struct.h: native DesignWare host at 0x60028000.
    0x28 "SDMMC" optional (sdmmc) => [SRC_SDIO_HOST];
    0x17 "PCNT" (pcnt) => [SRC_PCNT];
    0x33 "WIFI_MAC" (wifi) => [SRC_WIFI_MAC];
    0x34 "WIFI_MAC" alias (wifi) delta 0x1000 => [];
    // block 0x35 is the MAC's third block for a few registers (TSF, interrupts) and WDEV otherwise
    0x35 "WIFI_MAC" alias (wifi) delta 0x2000 @ 0x0c..=0x1f => [];
    0x35 "WIFI_MAC" alias (wifi) delta 0x2000 @ 0x118..=0x11f => [];
    0x35 "WIFI_MAC" alias (wifi) delta 0x2000 @ 0x128..=0x12b => [];
    0x35 "WDEV" (wdev) => [];
    0x3a "AES" (aes) => [SRC_AES];
    0x3c "RSA" (rsa) => [SRC_RSA];
    0x06 "FE" (fe) @ 0x174..=0x177 => [];
}

impl DeviceSet for Peripherals {
    const BASE: u32 = PERIPH_BASE;
    fn block_name(block: u32) -> &'static str { Peripherals::block_name(block) }
    fn misc(&self) -> &Misc { &self.misc }
    fn misc_mut(&mut self) -> &mut Misc { &mut self.misc }
    /// The three registers whose value depends on another device.
    fn pre_access(&mut self, block: u32, _off: u32, write: bool) {
        match block {
            0xc2 if !write => self.intmatrix.status = self.source_status(),   // INTERRUPT_*_STATUS reads the live sources
            0x35 => self.wifi.now_cycles = self.clock.cycles(),               // TSF timestamps
            0x08 => self.rtc.now_cycles = self.clock.cycles(),                // SENS ADC samples analog sources at this time
            _ => {}
        }
    }
}

impl Peripherals {
    /// CPU cycles since this peripheral set was built.
    pub(crate) fn clock_cycles(&self) -> u64 { self.clock.cycles() }
    pub fn new(mac: [u8; 6]) -> Self {
        Peripherals {
            usb: UsbSerialJtag::new(CPU_HZ), uart: [Uart::new(UartLayout::S3), Uart::new(UartLayout::S3), Uart::new(UartLayout::S3)], systimer: Systimer::new(),
            timg: [TimerGroup::new(), TimerGroup::new()], intmatrix: IntMatrix::new(), gpio: Gpio::new(), ledc: Ledc::new(LedcLayout::S3), mcpwm: [Mcpwm::new(160, 9), Mcpwm::new(166, 9)], rtc: RtcCntl::new(),
            efuse: Efuse::new(mac), system: SystemRegs::new(0x30), extmem: Extmem::new(), spi0: SpiMem::new(false), spi1: SpiMem::new(true),
            i2c: [crate::i2c::I2c::new(), crate::i2c::I2c::new()], lcd_cam: LcdCam::new(), spi2: GpSpi::new(), spi3: GpSpi::with_host(3), pcnt: Pcnt::new(), wifi: WifiMac::new(), fe: FeIq { word: 0 },
            aes: Aes::new(), rsa: Rsa::new(), sha: Sha::new(), wdev: Wdev::new(), i2c_mst: I2cMst::new(), gdma: Gdma::new(), i2s0: I2s::new(CPU_HZ), i2s1: I2s::new(CPU_HZ), rmt: Rmt::new(CPU_HZ),
            io_mux: RegRam::new(), misc: Misc::new(), fake_reads: std::env::var("ESP_EMU_FAKE_READ").ok().map(|v| v.split(',').filter_map(|e| { let mut p = e.split(':'); let a = u32::from_str_radix(p.next()?.trim_start_matches("0x"), 16).ok()?; let o = u32::from_str_radix(p.next().unwrap_or("0").trim_start_matches("0x"), 16).ok()?; let m = u32::from_str_radix(p.next().unwrap_or("ffffffff").trim_start_matches("0x"), 16).ok()?; Some((a, (o, m))) }).collect()).unwrap_or_default(),
            clock: Self::new_clock(),
            sdmmc: esp_periph::sdmmc::Sdmmc::new(esp_periph::sdmmc::SdmmcLayout::Esp32),
            spi_exec: false, last_status: [0; 4], intmatrix_dirty: false,
        }
    }

    pub fn block_name_pub(block: u32) -> String { Self::block_name(block).to_string() }
    fn block_name(block: u32) -> &'static str {
        match block {
            0x00 => "UART0", 0x02 => "SPI1", 0x03 => "SPI0", 0x04 => "GPIO", 0x05 => "FE2", 0x06 => "FE", 0x07 => "EFUSE", 0x08 => "RTC", 0x09 => "IO_MUX",
            0x0b => "HINF", 0x0c => "UHCI1", 0x0f => "I2S0", 0x10 => "UART1", 0x11 => "BT", 0x13 => "I2C0", 0x14 => "UHCI0", 0x15 => "SLCHOST", 0x16 => "RMT", 0x17 => "PCNT",
            0x18 => "SLC", 0x19 => "LEDC", 0x1c => "NRX", 0x1d => "BB", 0x1e => "PWM0", 0x1f => "TIMG0", 0x20 => "TIMG1", 0x21 => "RTC_SLOWMEM", 0x23 => "SYSTIMER",
            0x24 => "SPI2", 0x25 => "SPI3", 0x26 => "APB_CTRL", 0x27 => "I2C1", 0x28 => "SDMMC", 0x2a => "PERI_BACKUP", 0x2b => "TWAI", 0x2c => "PWM1", 0x2d => "I2S1", 0x2e => "UART2", 0x33 => "WIFI_MAC", 0x34 => "WIFI_MAC2", 0x35 => "WDEV", 0x0e => "I2C_MST",
            0x38 => "USB_SERIAL_JTAG", 0x39 => "USB_WRAP", 0x3a => "AES", 0x3b => "SHA", 0x3c => "RSA", 0x3d => "DS", 0x3e => "HMAC", 0x3f => "GDMA", 0x40 => "APB_SARADC", 0x41 => "LCD_CAM",
            0xc0 => "SYSTEM", 0xc1 => "SENSITIVE", 0xc2 => "INTERRUPT", 0xc4 => "EXTMEM", 0xc5 => "MMU", 0xce => "ASSIST_DEBUG", 0xcf => "ASSIST_DEBUG2", 0xd0 => "WCL",
            _ => "?",
        }
    }

    pub fn uart_route(&self, port: usize) -> esp_soc::uart::UartRoute {
        // ESP-IDF v5.5.4 components/soc/esp32s3/include/soc/clk_tree_defs.h:
        // UART_SCLK_RTC = SOC_MOD_CLK_RC_FAST, SOC_CLK_RC_FAST_FREQ_APPROX = 17.5 MHz.
        // IDF 4.4 used 20 MHz (RTC_CLK_FREQ).
        let clock = self.uart[port].clock_config();
        esp_soc::uart::UartPins::S3.route(port, &self.gpio, &self.io_mux,
            self.uart[port].baud(clock, 17_500_000))
    }

    pub fn uart_pin_input(&mut self, input: &esp_soc::uart::UartInput) {
        let routes: [_; 3] = std::array::from_fn(|port| self.uart_route(port));
        esp_soc::uart::uart_pin_input(&mut self.uart, input, &routes);
    }

    pub fn read32(&mut self, addr: u32) -> u32 {
        let mut v = mmio::read32(self, addr);
        if !self.fake_reads.is_empty() { if let Some(&(o, m)) = self.fake_reads.get(&addr) { v = (v & m) | o; } }
        v
    }

    pub fn write32(&mut self, addr: u32, v: u32) {
        // IDF v5.5.4 components/soc/esp32s3/register/soc/io_mux_reg.h:42-50 (FUN_PD/PU).
        if (0x60009004..=0x600090c4).contains(&addr) {
            self.gpio.set_pad(((addr - 0x60009004) / 4) as u8, v);
        }
        if matches!(addr, 0x6001_3004 | 0x6002_7004) && v & (1 << 5) != 0 {
            let bus = usize::from(addr == 0x6002_7004);
            if self.i2c[bus].has_pinned_devices() {
                let sda = if bus == 0 { 90 } else { 92 };
                let pins = self.i2c_pin(sda).zip(self.i2c_pin(sda - 1));
                self.i2c[bus].set_pins(pins);
            }
        }
        if addr == PERIPH_BASE + 0xc0020 && v & (1 << 11) != 0 { self.ledc = Ledc::new(LedcLayout::S3); }
        if addr == PERIPH_BASE + 0xc0020 {
            for (group, bit) in [17, 20].iter().enumerate() { if v & (1 << bit) != 0 { self.mcpwm[group] = Mcpwm::new(160 + group as u32 * 6, 9); } }
        }
        let fx = mmio::write32(self, addr, v);
        if fx.contains(WriteEffect::SPI_EXEC) { self.spi_exec = true; }
        if fx.contains(WriteEffect::INTMAP) { self.intmatrix_dirty = true; }
        // ESP-IDF v5.5.4 soc/system_reg.h: PERIP_CLK_EN1/RST_EN1 LCD_CAM bit 8.
        if addr == PERIPH_BASE + 0xc001c || addr == PERIPH_BASE + 0xc0024 {
            self.lcd_cam.set_clock_enabled(self.system.read(0x1c) & (1 << 8) != 0 && self.system.read(0x24) & (1 << 8) == 0);
        }
        if addr == PERIPH_BASE + 0xc0018 || addr == PERIPH_BASE + 0xc0020 {
            for (group, bit) in [17, 20].iter().enumerate() { self.mcpwm[group].clock_enabled = self.system.read(0x18) & (1 << bit) != 0 && self.system.read(0x20) & (1 << bit) == 0; }
            self.ledc.clock_enabled = self.system.read(0x18) & (1 << 11) != 0 && self.system.read(0x20) & (1 << 11) == 0;
            for block in [0x19, 0x1e, 0x2c] { self.refresh_optional(block); }
        }
    }

    /// Returns the number of words applied.
    pub fn init_regs(&mut self, addr: u32, v: u32) -> bool {
        if !(PERIPH_BASE..PERIPH_END).contains(&addr) { return false; }
        let block = (addr - PERIPH_BASE) >> 12; let off = addr & 0xfff;
        match block {
            0x08 => self.rtc.ram.write(off, v),
            0xc0 => { if (0x30..=0x3c).contains(&off) { return false; } self.system.ram.write(off, v) }
            0xc4 => self.extmem.ram.write(off, v),
            0x09 => self.io_mux.write(off, v),
            0x02 => { if off == 0 || (0x58..=0x94).contains(&off) { return false; } self.spi1.regs.write(off, v) }
            0x03 => { if off == 0 { return false; } self.spi0.regs.write(off, v) }
            0x0e => self.i2c_mst.ram.write(off, v),
            0x26 | 0xc1 => self.misc.generic.entry(block).or_default().write(off, v),
            _ => return false,
        }
        true
    }

    /// Advance device time by `cycles` CPU cycles (every clocked device gets its own clock's
    /// ticks), then route GPIO input edges to the pulse counters.
    pub fn tick(&mut self, cycles: u64) -> bool {
        let mut irq_changed = Dispatch::tick(self, cycles);
        if !self.gpio.input_changes.is_empty() {
            let before = self.pcnt.irq();
            let gpio = &mut self.gpio;
            let input = gpio.input;
            let selections = &gpio.func_in_sel;
            let sig = |idx: u32| -> Option<(u8, bool)> {
                let sel = *selections.get(idx as usize)?;
                if sel & 0x80 == 0 { return None; }                       // not routed through the matrix
                let pin = (sel & 0x3f) as u8; if pin >= 49 { return None; }
                let lvl = (input >> pin) & 1 != 0; Some((pin, lvl ^ (sel & 0x40 != 0)))
            };
            for (pin, level) in gpio.input_changes.drain(..) { self.pcnt.gpio_edge(pin, level, &sig); }
            irq_changed |= before != self.pcnt.irq();
        }
        irq_changed
    }

    /// Raw per-source interrupt status (4 × 32 bits).
    pub fn source_status(&self) -> [u32; 4] { Dispatch::source_status(self) }

    /// The I2S controller carrying the board's audio output (whichever has played samples).
    pub fn audio(&self) -> &I2s { if self.i2s1.frames_out > self.i2s0.frames_out { &self.i2s1 } else { &self.i2s0 } }

    /// Cycles until the next systimer or timer-group alarm can fire — the only device events
    /// firmware times precisely. Slightly conservative (one device tick early), never late.
    pub fn cycles_until_timer(&self) -> u32 { Dispatch::cycles_until_deadline(self) }
    pub fn lines_dirty(&mut self) -> bool { let st = self.source_status(); if st != self.last_status { self.last_status = st; true } else { false } }
    /// Interrupt lines for both cores in one pass over the sources that are active.
    pub fn cpu_lines_both(&self) -> (u32, u32) {
        let st = self.last_status;
        let (mut l0, mut l1) = (0u32, 0u32);
        for (w, &word) in st.iter().enumerate() {
            let mut bits = word;
            while bits != 0 {
                let b = bits.trailing_zeros(); bits &= bits - 1;
                let src = w * 32 + b as usize; if src >= NUM_SOURCES { break; }
                let n0 = self.intmatrix.map[0][src]; if n0 < 32 { l0 |= 1 << n0; }
                let n1 = self.intmatrix.map[1][src]; if n1 < 32 { l1 |= 1 << n1; }
            }
        }
        (l0, l1)
    }

    /// CPU interrupt lines for `core` (bit n = Xtensa interrupt n) after the interrupt matrix.
    pub fn cpu_lines(&self, core: usize) -> u32 {
        let st = self.source_status();
        let mut lines = 0u32;
        for src in 0..NUM_SOURCES {
            if st[src / 32] & (1 << (src % 32)) != 0 { let n = self.intmatrix.map[core][src]; if n < 32 { lines |= 1 << n; } }
        }
        lines
    }
    /// SYSTEM_CORE_1_CONTROL_0: (clkgate_en, reseting, runstall)
    pub fn core1_control(&self) -> (bool, bool, bool) { let v = self.system.ram.read(0); (v & 2 != 0, v & 4 != 0, v & 1 != 0) }
}

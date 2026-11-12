//! S31 UART0/RTC-WDT/eFuse interrupt routing and the documented power-on reset-cause registers.
//!
//! Prefer `intmtx::Lines` only when the map fits (C3/C6: single-core PLIC/INTPRI front end,
//! ≤32 CPU lines, `Lines::update` over a flat source→line map). S31 does not: 169 sources,
//! dual-core MAP windows (+0x800), and CLIC delivery (`ClicCore`, IDs 16..47) — the same local
//! matrix+CLIC shape as S3/`esp32p4::IntMatrix`, not `Lines`. Source 152 (EFUSE) is also
//! outside DeviceSet's 128-bit `source_status`; IRQs are gathered via `Irqs` into this matrix.
use esp_periph::clic::{ClicCore, CLIC_IDS, CLIC_EXT_OFFSET};

// ESP-IDF v6.1 soc/esp32s31/include/soc/{interrupts,clic_reg}.h and
// register/soc/{reg_base,interrupt_core0_reg,lp_clkrst_reg}.h. The other core's
// matrix is +0x800. CLIC MMIO is local to the executing hart.
pub const MATRIX_BASE: u32 = 0x2058_5000;
pub const CLIC_BASE: u32 = 0x1080_0000;
pub const RESET_CAUSE0: u32 = 0x2070_1030;
pub const RESET_CAUSE1: u32 = 0x2070_1034;
pub const UART0_SOURCE: usize = 9;
pub const CACHE_SOURCE: usize = 69; // Fixed S31 interrupt_core0_reg.h CACHE_INTR_MAP0x114 /4.
pub const USB_SERIAL_SOURCE: usize = 2; // v6.1 interrupts.h / interrupt_core0_reg.h offset8.
pub const MONITOR_SOURCE: usize = 119; // v6.1 interrupts.h: ETS_ASSIST_DEBUG_INTR_SOURCE.
pub const RTC_WDT_SOURCE: usize = 136; // interrupt_core0_reg.h: LP_WDT_INTR_MAP at 0x220.
pub const USB_OTG_SOURCE: usize = 99; // IDF master interrupts.h: ETS_USB_OTGHS_INTR_SOURCE.
pub const SPI2_SOURCE: usize = 4; // IDF master interrupts.h: ETS_SPI2_INTR_SOURCE.
/// ETS_AXI_PDMA_IN_CH0_INTR_SOURCE; IN ch1-2, then OUT ch0-2 follow.
pub const AXI_DMA_IN_SOURCE: usize = 48;
pub const PPA_SOURCE: usize = 102; // IDF master interrupts.h: ETS_PPA_INTR_SOURCE.
/// ETS_DMA2D_IN_CH0_INTR_SOURCE; IN ch1-2, then OUT ch0-3 follow.
pub const DMA2D_IN_SOURCE: usize = 105;
pub const I2C0_SOURCE: usize = 23;
/// IDF master interrupts.h: ETS_I2S0_INTR_SOURCE 6, ETS_I2S1_INTR_SOURCE 7.
pub const I2S0_SOURCE: usize = 6;
/// ETS_AHB_PDMA_IN_CH0_INTR_SOURCE; IN ch1-4, then OUT ch0-4 (41-45) follow.
pub const AHB_DMA_IN_SOURCE: usize = 36;
/// ETS_GPIO_INTR0_SOURCE; lines 1-3 follow.
pub const GPIO_SOURCE: usize = 61; // ETS_I2C0_INTR_SOURCE; ETS_I2C1_INTR_SOURCE follows.
pub const EFUSE_SOURCE: usize = 152; // interrupt_core0_reg.h: LP_EFUSE_INTR_MAP at 0x260.
pub(crate) const SOURCE_COUNT: usize = 169;
const SOURCES: usize = SOURCE_COUNT;
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Irqs { pub uart: bool, pub wdt: bool, pub efuse: bool, pub usb: bool, pub monitor: bool, pub cache: bool, pub mwdt: [bool; 2], pub systimer: [bool; 3], pub psram: bool, pub brownout: bool, pub from_cpu: [bool; 4], pub usb_otg: bool, pub spi2: bool, pub spi3: bool, pub i2c: [bool; 2], pub gpio: [bool; 4], pub axi_dma: u64,
    /// DMA2D: bit i = IN ch0-2, then OUT ch0-3; PPA.
    pub dma2d: u8, pub ppa: bool,
    /// ETS_MODEM_WIFI_MAC_INTR_SOURCE (`wifi.rs`)
    pub wifi_mac: bool,
    /// AHB-GDMA, as `esp_periph::gdma::Gdma` numbers it: bits 0-4 OUT ch0-4, 5-9 IN ch0-4
    pub ahb_dma: u64,
    pub i2s: [bool; 2] }
impl Irqs {
    // Fixed S31 interrupts.h: TIMERGRP0_WDT27,TIMERGRP1_WDT30,SYSTIMER_TARGET0..2 sources33..35,LP_ANAPERI143.
    pub(crate) fn sources(self) -> [(usize, bool); 53] { [(UART0_SOURCE, self.uart), (RTC_WDT_SOURCE, self.wdt), (EFUSE_SOURCE, self.efuse), (USB_SERIAL_SOURCE, self.usb), (MONITOR_SOURCE, self.monitor), (CACHE_SOURCE, self.cache), (27, self.mwdt[0]), (30, self.mwdt[1]), (33, self.systimer[0]), (34, self.systimer[1]), (35, self.systimer[2]), (112, self.psram), (143, self.brownout), (65, self.from_cpu[0]), (66, self.from_cpu[1]), (67, self.from_cpu[2]), (68, self.from_cpu[3]), (USB_OTG_SOURCE, self.usb_otg), (SPI2_SOURCE, self.spi2), (SPI2_SOURCE + 1, self.spi3), (I2C0_SOURCE, self.i2c[0]), (I2C0_SOURCE + 1, self.i2c[1]), (GPIO_SOURCE, self.gpio[0]), (GPIO_SOURCE + 1, self.gpio[1]), (GPIO_SOURCE + 2, self.gpio[2]), (GPIO_SOURCE + 3, self.gpio[3]),
        (AXI_DMA_IN_SOURCE, self.axi_dma >> 5 & 1 != 0), (AXI_DMA_IN_SOURCE + 1, self.axi_dma >> 6 & 1 != 0), (AXI_DMA_IN_SOURCE + 2, self.axi_dma >> 7 & 1 != 0),
        (AXI_DMA_IN_SOURCE + 3, self.axi_dma & 1 != 0), (AXI_DMA_IN_SOURCE + 4, self.axi_dma >> 1 & 1 != 0), (AXI_DMA_IN_SOURCE + 5, self.axi_dma >> 2 & 1 != 0),
        (PPA_SOURCE, self.ppa), (DMA2D_IN_SOURCE, self.dma2d & 1 != 0), (DMA2D_IN_SOURCE + 1, self.dma2d >> 1 & 1 != 0), (DMA2D_IN_SOURCE + 2, self.dma2d >> 2 & 1 != 0),
        (DMA2D_IN_SOURCE + 3, self.dma2d >> 3 & 1 != 0), (DMA2D_IN_SOURCE + 4, self.dma2d >> 4 & 1 != 0), (DMA2D_IN_SOURCE + 5, self.dma2d >> 5 & 1 != 0), (DMA2D_IN_SOURCE + 6, self.dma2d >> 6 & 1 != 0), (crate::wifi::MAC_SOURCE, self.wifi_mac),
        (I2S0_SOURCE, self.i2s[0]), (I2S0_SOURCE + 1, self.i2s[1]),
        (AHB_DMA_IN_SOURCE, self.ahb_dma >> 5 & 1 != 0), (AHB_DMA_IN_SOURCE + 1, self.ahb_dma >> 6 & 1 != 0), (AHB_DMA_IN_SOURCE + 2, self.ahb_dma >> 7 & 1 != 0), (AHB_DMA_IN_SOURCE + 3, self.ahb_dma >> 8 & 1 != 0), (AHB_DMA_IN_SOURCE + 4, self.ahb_dma >> 9 & 1 != 0),
        (AHB_DMA_IN_SOURCE + 5, self.ahb_dma & 1 != 0), (AHB_DMA_IN_SOURCE + 6, self.ahb_dma >> 1 & 1 != 0), (AHB_DMA_IN_SOURCE + 7, self.ahb_dma >> 2 & 1 != 0), (AHB_DMA_IN_SOURCE + 8, self.ahb_dma >> 3 & 1 != 0), (AHB_DMA_IN_SOURCE + 9, self.ahb_dma >> 4 & 1 != 0)] }
}

pub struct Interrupts {
    map: [[u32; SOURCES]; 2], clock: [u32; 2], date: [u32; 2],
    pub clic: [ClicCore; 2], pub(crate) reset_cause: [u32; 2],
    /// The CLIC inputs the last `refresh_inputs` derived and from which sources; a MAP write
    /// forgets them.
    inputs: Option<(Irqs, [u64; 2])>,
}
impl Default for Interrupts {
    fn default() -> Self {
        // HPCOREn_RESET_CAUSE[6:1] = 1 for POR. RESET_FLAG retains its documented reset 0;
        // no software-reset, watchdog or power-domain state machine is implied here.
        Self { map: [[0; SOURCES]; 2], clock: [0; 2], date: [38_834_448; 2],
            clic: [ClicCore::default(), ClicCore::default()], reset_cause: [1 << 1; 2], inputs: None }
    }
}
impl Interrupts {
    pub fn reset_read(&self, core: usize) -> u32 { self.reset_cause[core] }
    pub fn reset_write(&mut self, core: usize, value: u32) {
        // Cause and flag are RO; only the two write-triggered clear bits affect them.
        if value & (1 << 30) != 0 { self.reset_cause[core] &= !(0x3f << 1); }
        if value & (1 << 31) != 0 { self.reset_cause[core] &= !1; }
    }
    /// CLIC ID field of the interrupt-matrix route (PASS_LEVEL bits ignored).
    pub(crate) fn map_id(&self, core: usize, source: usize) -> u32 {
        self.map[core & 1][source] & 0x3f
    }
    pub fn matrix_read(&self, core: usize, off: u32, irq: Irqs) -> Option<u32> {
        match off {
            0x000..=0x2a0 => Some(self.map[core][off as usize / 4]),
            0x2a8..=0x2bc => Some(irq.sources().into_iter().filter(|&(source, active)| active && source / 32 == (off - 0x2a8) as usize / 4).fold(0, |word, (source, _)| word | 1 << (source % 32))),
            0x2fc => Some(self.clock[core]), 0x7fc => Some(self.date[core]),
            _ => None,
        }
    }
    pub fn matrix_write(&mut self, core: usize, off: u32, value: u32) -> bool {
        self.inputs = None;
        match off {
            // Unlike the old P4, S31 also has PASS_LEVEL bits 9:8. interrupt_core0_reg.h:
            // zero disables the privilege *remap* feature (plain routing, as ROM/IDF
            // intr_matrix_set writes only MAP), one remaps to S-level (no S-mode here,
            // so not delivered), two/three remap with M-level.
            0x000..=0x2a0 => self.map[core][off as usize / 4] = value & 0x33f,
            0x2a8..=0x2bc => {}, // source status is read-only
            0x2fc => self.clock[core] = value & 1, 0x7fc => self.date[core] = value & 0x0fff_ffff,
            _ => return false,
        }
        true
    }
    pub fn refresh(&mut self, irq: Irqs, threshold: u32) -> Option<u32> { self.refresh_core(0, irq, threshold) }
    /// `refresh_core(0, irq, 0)` and a zero threshold on hart 1, without the candidate: what the
    /// SoC does with both cores' CLIC inputs after every device change. The routing of the
    /// sources is remembered while the sources and the MAP stay the same.
    pub fn refresh_inputs(&mut self, irq: Irqs) {
        let inputs = match self.inputs {
            Some((last, inputs)) if last == irq => inputs,
            _ => { let inputs = std::array::from_fn(|core| self.input(core, irq)); self.inputs = Some((irq, inputs)); inputs }
        };
        for (clic, input) in self.clic.iter_mut().zip(inputs) { clic.set_input(input); clic.thresh = 0; }
    }
    /// The CLIC input word of `core` for these sources.
    fn input(&self, core: usize, irq: Irqs) -> u64 {
        let mut input = 0;
        for (source, active) in irq.sources() {
            let route = self.map[core][source]; let id = (route & 0x3f) as usize;
            let m_mode = (route >> 8) & 3 != 1;
            if active && m_mode && (CLIC_EXT_OFFSET as usize..CLIC_IDS).contains(&id) { input |= 1u64 << id; }
        }
        input
    }
    pub fn refresh_core(&mut self, hart: usize, irq: Irqs, threshold: u32) -> Option<u32> {
        if hart >= 2 { return None; }
        for core in 0..2 {
            let input = self.input(core, irq);
            self.clic[core].set_input(input);
        }
        // Standard S31 threshold is the executing hart's CSR, not P4's MMIO register.
        self.clic[hart].thresh = (threshold & 0xff) << 24;
        self.clic[hart].candidate().and_then(|(id, level)| {
            if !(CLIC_EXT_OFFSET as usize..CLIC_IDS).contains(&id) { return None; }
            let line = id as u32 - CLIC_EXT_OFFSET;
            Some(line | level << riscv_rv32::exec::CLIC_IRQ_LEVEL_SHIFT | if self.clic[hart].vectored(id) { riscv_rv32::exec::CLIC_IRQ_SHV } else { 0 })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn efuse_and_wdt_share_status_word_without_overwriting_each_other() {
        let mut i = Interrupts::default();
        assert_eq!(i.matrix_read(0, 0x2b8, Irqs { wdt: true, efuse: true, ..Irqs::default() }), Some((1 << 8) | (1 << 24)));
        i.clic[0].write(0x1000 + 4 * 19, 1 << 8 | 1 << 29, 0xf);
        i.matrix_write(0, 0x260, 0x213);
        assert!(i.refresh(Irqs { efuse: true, ..Irqs::default() }, 0x1f).is_some());
        assert_eq!(i.refresh(Irqs::default(), 0x1f), None);
    }
    #[test]
    fn s31_pass_level_remap_and_strictly_higher_priority() {
        // IDF v6.1 esp_cpu_intr/crosscore init writes MAP only (PASS_LEVEL 0) and FreeRTOS
        // relies on that route, so 0 is plain M routing; only S-level remap (1) is withheld.
        let mut i = Interrupts::default();
        i.clic[0].write(0x1000 + 4 * 17, 1 << 8 | 1 << 29, 0xf);
        for pass in 0..=3 {
            i.matrix_write(0, 4 * UART0_SOURCE as u32, pass << 8 | 17);
            assert_eq!(i.refresh(Irqs { uart: true, ..Irqs::default() }, 0x1f).is_some(), pass != 1);
        }
        assert_eq!(i.refresh(Irqs { uart: true, ..Irqs::default() }, 0x3f), None);
        assert!(i.refresh(Irqs { uart: true, ..Irqs::default() }, 0x1f).is_some()); assert_eq!(i.refresh(Irqs::default(), 0x1f), None);
        for id in [0, 15, 48, 63] { i.matrix_write(0, 4 * UART0_SOURCE as u32, 0x200 | id); assert_eq!(i.refresh(Irqs { uart: true, ..Irqs::default() }, 0x1f), None); }
    }
    #[test]
    fn reset_cause_is_read_only_except_for_documented_clear_triggers() {
        let mut i = Interrupts::default(); assert_eq!(i.reset_read(0), 2);
        i.reset_write(0, !(3 << 30)); assert_eq!(i.reset_read(0), 2);
        i.reset_write(0, 1 << 30); assert_eq!(i.reset_read(0), 0); assert_eq!(i.reset_read(1), 2);
    }
    #[test]
    fn executing_hart_uses_its_own_route_enable_and_threshold() {
        let mut i = Interrupts::default(); let irq = Irqs { uart: true, ..Default::default() };
        i.matrix_write(1, 4 * UART0_SOURCE as u32, 0x211);
        i.clic[1].write(0x1044, 1 << 8 | 1 << 29, 0xf);
        assert_eq!(i.refresh_core(0, irq, 0), None); assert!(i.refresh_core(1, irq, 0).is_some());
        assert_eq!(i.refresh_core(1, irq, 0x3f), None); assert!(i.refresh_core(1, irq, 0x1f).is_some());
        i.clic[1].write(0x1044, 0, 2); assert_eq!(i.refresh_core(1, irq, 0), None);
        assert_eq!(i.refresh_core(usize::MAX, irq, 0), None);
    }

}

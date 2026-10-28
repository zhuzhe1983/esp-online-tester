//! S31 XTAL/CPLL dividers, functional calibration, peripheral gates and limited RTC selectors.
//! ESP-IDF v6.1 fff9895c82d744c7237be8847347bdd1b07c6643:
//! soc/esp32s31/register/soc/{hp_sys_clkrst,hp_alive_sys}_reg.h, esp_hal_clock/esp32s31/include/hal/clk_tree_ll.h,
//! and esp_hw_support/port/esp32s31/rtc_clk.c (40/20/40÷3/40÷6 MHz reset configuration).
use emu_core::Fault;
pub const LP_BASE: u32 = 0x2070_1000;
pub const BASE: u32 = 0x2058_7000;
pub const UART0_CTRL: u32 = BASE + 0x88;
pub const UART0_MEM: u32 = 0x2058_628c;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rates { pub cpu_hz: u64, pub mem_hz: u64, pub sys_hz: u64, pub apb_hz: u64 }
pub struct Clocks { pub mpll: crate::mpll::Mpll, pending: [u32; 5], active: [u32; 5], uart0: u32, ref_240m: u32, uart_pad_sleep: u8, aux_uart: [u32; 3], uart_mem: u32, modem: u32, psram: u32, lp_root: u32, fosc: bool, cpll_div: u32, cpll_powered: bool, cal_stop: bool, cal_done: bool, cal_left: Option<u8> }
impl Default for Clocks {
    fn default() -> Self { Self { mpll: crate::mpll::Mpll::default(), pending: [0, 0, 1, 2, 1], active: [0, 0, 1, 2, 1], uart0: 0x83, ref_240m: 0x101, uart_pad_sleep: 15, aux_uart: [0x83; 3], uart_mem: 0, modem: 1, psram: 0x181, lp_root: 1 << 26, fosc: true, cpll_div: 0xf2, cpll_powered: false, cal_stop: false, cal_done: false, cal_left: None } }
}
impl Clocks {
    // lp_clkrst_reg.h: reset source selectors and RO XTAL40MHz field. Selecting
    // unsupported clocks is rejected until their power/rate model is connected.
    pub fn is_lp_mmio(address: u32) -> bool { matches!((address & !3).wrapping_sub(LP_BASE), 0 | 0x48 | 0x4c | 0x54) }
    pub fn lp_read(&self, address: u32) -> Result<u32, Fault> {
        match address.wrapping_sub(LP_BASE) { 0 => Ok(self.lp_root), 0x48 => Ok(40), 0x4c => Ok(self.cpll_div), 0x54 => Ok(self.mpll.divider()), _ => Err(Fault::Unmapped) }
    }
    pub fn lp_write(&mut self, address: u32, value: u32) -> Result<(), Fault> {
        match address.wrapping_sub(LP_BASE) {
            0 => {
                // Only RC_SLOW and RC_FAST/XTAL are present; no invented XTAL32K input.
                if value & 0xf380_0000 != 0 || (value >> 26) & 3 > 1 { return Err(Fault::Prohibited); }
                self.lp_root = value & 0x0c00_0000;
            }
            0x54 => self.mpll.write_divider(value)?,
            0x48 => {}, // documented RO, firmware cannot change the physical XTAL field
            0x4c => { if self.using_cpll() { return Err(Fault::Prohibited); } self.cpll_div = value & 0xfff; self.cal_done = false; self.cal_left = None; }
            _ => return Err(Fault::Unmapped),
        }
        Ok(())
    }
    pub fn is_mmio(address: u32) -> bool { (BASE..BASE + 0x18).contains(&address) || matches!((address & !3).wrapping_sub(BASE), 0x40 | 0x68 | 0x8c | 0x90 | 0x94 | 0x174 | 0x17c | 0x2010 | 0x2014 | 0x2018 | 0x201c) }
    pub fn uart_read(&self, address: u32) -> u32 { if address == UART0_CTRL { self.uart0 } else { self.uart_mem } }
    fn validate_uart(value: u32) -> Result<(), Fault> {
        if value & 0x70 != 0 { return Err(Fault::Prohibited); }
        let num = (value >> 16) & 255; let den = value >> 24;
        if num != 0 && (den == 0 || num >= den) { return Err(Fault::Prohibited); }
        Ok(())
    }
    pub fn aux_uart_clock(&self, index: usize) -> Result<(bool, bool, u64), Fault> {
        let v = *self.aux_uart.get(index).ok_or(Fault::Unmapped)?;
        let bus = v & 0xb == 3; let core = v & 0x85 == 0x81;
        let num = u64::from((v >> 16) & 255); let den = if num == 0 { 1 } else { u64::from(v >> 24) };
        let hz = 40_000_000 * den / (u64::from(((v >> 8) & 255) + 1) * den + num);
        Ok((bus, core, if core { hz } else { 0 }))
    }
    pub fn uart_write(&mut self, address: u32, value: u32) -> Result<(), Fault> {
        if address == UART0_CTRL {
            // v6.1 uart_ll.h: 0=XTAL, 1=RC_FAST, 2=PLL80M. Only the XTAL root
            // is implemented. FORCE_NORST/global reset interaction remains unsupported.
            Self::validate_uart(value)?; self.uart0 = value;
        } else {
            // Low-power retention/shutdown is not modeled. Preserve mode configuration
            // while awake, but do not pretend entry to a powered-down memory state works.
            if value & 4 != 0 { return Err(Fault::Prohibited); }
            self.uart_mem = value & 15;
        }
        Ok(())
    }
    pub fn psram_bus_on(&self) -> bool { self.psram & 5 == 1 }
    pub fn psram_on(&self) -> bool { self.psram_core_hz().is_some() }
    pub fn psram_core_hz(&self) -> Option<u64> {
        if self.psram & 0x107 != 0x101 { return None; }
        // psram_ctrlr_ll.h: 0=XTAL,1=MPLL,2=CPLL. PLL gate is independent.
        let pll = self.psram & 0x80 != 0;
        let root = match (self.psram >> 5) & 3 {
            0 => 40_000_000, 1 if pll => self.mpll.output_hz()?,
            2 if pll && self.cpll_powered && self.cal_done => self.cpll_hz()?, _ => return None,
        };
        Some(root / u64::from(((self.psram >> 9) & 255) + 1))
    }
    /// S31 REF_240M_CTRL0: independent gate and integer divider plus one.
    pub fn ref_240m_hz(&self, bbpll: Option<u64>) -> Option<u64> {
        if self.ref_240m & 0x100 == 0 { None } else { bbpll.map(|hz| hz / u64::from((self.ref_240m & 255) + 1)) }
    }
    pub fn modem_on(&self) -> bool { self.modem != 0 }
    pub fn uart_bus_on(&self) -> bool { self.uart0 & 0xb == 3 }
    pub fn uart_core_on(&self) -> bool { self.uart0 & 0x85 == 0x81 }
    pub fn uart_sclk_hz(&self) -> u64 {
        if !self.uart_core_on() { return 0; }
        let integer = u64::from((self.uart0 >> 8) & 255) + 1;
        let num = u64::from((self.uart0 >> 16) & 255); let den = u64::from(self.uart0 >> 24);
        let den = if num == 0 { 1 } else { den };
        40_000_000 * den / (integer * den + num)
    }
    pub fn set_fosc_power(&mut self, on: bool) { self.fosc = on; }
    // clk_tree_defs.h gives nominal17.5MHz; older LL comments call this20MHz.
    // These are functional rates, not oscillator calibration or a wall-clock model.
    pub fn rc_fast_hz(&self) -> u64 { if self.fosc { 17_500_000 } else { 0 } }
    pub fn rtc_fast_hz(&self) -> u64 { if self.lp_root & (1 << 26) != 0 { 40_000_000 } else if self.fosc { 17_500_000 } else { 0 } }
    pub fn rtc_slow_hz(&self) -> u64 { 136_000 }
    /// Fixed clk_tree_ll frequency-calculation mux. Returns absent clocks as zero;
    /// unconnected mux inputs remain unavailable at the register write boundary.
    pub fn calibration_hz(&self, source: u32) -> u64 {
        let rates = self.rates();
        match source { 0 => self.mpll.output_hz().unwrap_or(0),
            2 if self.cpll_powered && self.cal_done => self.cpll_hz().unwrap_or(0),
            4 => rates.sys_hz, 5 => rates.apb_hz, 6 => rates.mem_hz,
            8 => self.rtc_slow_hz(), 12 => 40_000_000, _ => 0 }
    }
    pub fn using_cpll(&self) -> bool { self.active[0] == 1 }
    pub fn set_cpll_power(&mut self, on: bool) {
        self.cpll_powered = on;
        if !on { self.cal_done = false; self.cal_left = None; }
    }
    fn cpll_hz(&self) -> Option<u64> {
        let reference = self.cpll_div & 15; let feedback = (self.cpll_div >> 4) & 255;
        if reference == 0 || feedback == 0 { None } else { Some(40_000_000 * u64::from(feedback) / u64::from(reference)) }
    }
    /// Functional scheduling quantum, not measured PLL settling time. A calibration
    /// advances only after the actual start command with power and the supported320MHz
    /// configuration. Reads/RO writes cannot complete it or fabricate oscillator input.
    pub fn advance_calibration(&mut self, cycles: u32) {
        if !self.cpll_powered || self.cal_stop || self.cpll_hz() != Some(320_000_000) { return; }
        if let Some(left) = self.cal_left {
            if cycles >= u32::from(left) { self.cal_left = None; self.cal_done = true; }
            else { self.cal_left = Some(left - cycles as u8); }
        }
    }
    fn rates_of(&self, regs: &[u32; 5]) -> Result<Rates, Fault> {
        // Mux1 is CPLL, mux2 RC_FAST, mux3 PLL_F240M per v6.1 LL. The generated
        // header's older “PLL_F300M” prose is not used to invent a fixed PLL rate.
        let source = match regs[0] {
            0 => 40_000_000,
            1 if self.cpll_powered && self.cal_done => self.cpll_hz().ok_or(Fault::Prohibited)?,
            _ => return Err(Fault::Prohibited),
        };
        let div = u64::from((regs[1] & 255) + 1);
        let num = u64::from((regs[1] >> 8) & 7); let den = u64::from((regs[1] >> 11) & 7);
        if num != 0 && (den == 0 || num >= den) { return Err(Fault::Prohibited); }
        // SYS/APB fractional divider behavior is not used by the fixed LL and is
        // deliberately unsupported pending a documented constraint model.
        if regs[3] & !255 != 0 || regs[4] & !255 != 0 { return Err(Fault::Prohibited); }
        let den = if num == 0 { 1 } else { den };
        let cpu_den = div * den + num;
        let root = source * den; let mem_div = u64::from(regs[2] + 1);
        let sys_div = u64::from(regs[3] + 1); let apb_div = u64::from(regs[4] + 1);
        let rates = Rates { cpu_hz: root / cpu_den, mem_hz: root / (cpu_den * mem_div),
            sys_hz: root / (cpu_den * sys_div), apb_hz: root / (cpu_den * sys_div * apb_div) };
        if rates.cpu_hz > 320_000_000 || rates.mem_hz > 160_000_000 || rates.sys_hz > 106_666_666 || rates.apb_hz > 53_333_333 { return Err(Fault::Prohibited); }
        Ok(rates)
    }
    /// Computed functional domain rates; not an instruction-to-clock timing model.
    pub fn rates(&self) -> Rates { self.rates_of(&self.active).unwrap_or(Rates { cpu_hz: 0, mem_hz: 0, sys_hz: 0, apb_hz: 0 }) }
    pub fn read(&self, off: u32) -> Result<u32, Fault> {
        if off & 3 != 0 { return Err(Fault::Misaligned); }
        match off { 0..=0x10 => Ok(self.pending[off as usize / 4]), 0x14 => Ok(0), 0x2010 | 0x2014 | 0x2018 | 0x201c => Ok(u32::from((self.uart_pad_sleep >> ((off - 0x2010) / 4)) & 1) << 31), 0x8c | 0x90 | 0x94 => Ok(self.aux_uart[((off - 0x8c) / 4) as usize]), 0x17c => Ok(self.ref_240m), 0x40 => Ok(self.modem), 0x68 => Ok(self.psram), 0x174 => Ok(u32::from(self.cal_done) << 2 | u32::from(self.cal_stop) << 3 | self.mpll.control()), _ => Err(Fault::Unmapped) }
    }
    pub fn write(&mut self, off: u32, value: u32) -> Result<(), Fault> {
        if off & 3 != 0 { return Err(Fault::Misaligned); }
        match off {
            0 => self.pending[0] = value & 3,
            4 | 0xc | 0x10 => self.pending[off as usize / 4] = value & 0x3fff,
            8 => self.pending[2] = value & 1,
            0x14 => if value & 1 != 0 {
                self.rates_of(&self.pending)?;
                // Functional atomic latch; no fabricated PLL lock/busy poll or physical delay.
                self.active = self.pending;
            },
            // MODEM_CTRL0 has only the documented RW clock gate, reset1. This does
            // not implement RF or wireless operation.
            0x174 => {
                // Other PLL controls still unsupported; RO END bits never become host data.
                if value & 0xa2 != 0 { return Err(Fault::Prohibited); }
                if self.using_cpll() && value & 8 == 0 { return Err(Fault::Prohibited); }
                self.mpll.write_control(value);
                self.cal_stop = value & 8 != 0;
                if self.cal_stop { self.cal_left = None; }
                else { self.cal_done = false; self.cal_left = Some(16); }
            }
            // Fixed UART1/2/3 clock words share documented0x83 defaults and
            // XTAL/divider/gate fields. Their data FIFOs/IRQ/GPIO are still absent;
            // accepting clock configuration does not supply fake UART traffic.
            0x8c | 0x90 | 0x94 => { Self::validate_uart(value)?; self.aux_uart[((off - 0x8c) / 4) as usize] = value; }
            // HP_ALIVE_SYS_HP_PAD_UART0..3_CTRL: only bit31 RW,reset1.
            // Configuration retained; deep-sleep/pad-clock wakeup is not modeled.
            0x2010 | 0x2014 | 0x2018 | 0x201c => {
                let bit = 1 << ((off - 0x2010) / 4); self.uart_pad_sleep = (self.uart_pad_sleep & !bit) | if value >> 31 != 0 { bit } else { 0 };
            }
            0x17c => self.ref_240m = value & 0x1ff,
            0x40 => self.modem = value & 1,
            0x68 => {
                // Selectors use actual powered/calibrated roots; selecting a stopped
                // source is legal but blocks access. Force-reset overrides remain absent.
                if value & 0x18 != 0 || (value >> 5) & 3 == 3 { return Err(Fault::Prohibited); }
                self.psram = value & 0x1ffff;
            },
            _ => return Err(Fault::Unmapped),
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn auxiliary_uart_clocks_have_independent_gates_reset_and_fractional_dividers() {
        let mut c = Clocks::default();
        for i in 0..3 { assert_eq!(c.aux_uart_clock(i), Ok((true,true,40000000))); }
        c.write(0x8c,0x81).unwrap(); assert_eq!(c.aux_uart_clock(0), Ok((false,true,40000000)));
        c.write(0x90,0x83 | 4).unwrap(); assert_eq!(c.aux_uart_clock(1), Ok((true,false,0)));
        c.write(0x94,0x83 | 1 << 8 | 1 << 16 | 2 << 24).unwrap(); assert_eq!(c.aux_uart_clock(2), Ok((true,true,16000000)));
        c.uart_write(UART0_CTRL,0x81).unwrap(); assert!(!c.uart_bus_on()); assert!(c.uart_core_on());
        for i in 0..4 { assert_eq!(c.read(0x2010 + i * 4),Ok(1 << 31)); }
        c.write(0x2014,u32::MAX >> 1).unwrap(); assert_eq!(c.read(0x2014),Ok(0)); assert_eq!(c.read(0x2010),Ok(1 << 31));
        assert_eq!(c.uart_sclk_hz(),40000000); assert_eq!(c.write(0x94,0x83 | 1 << 16),Err(Fault::Prohibited));
        assert_eq!(c.aux_uart_clock(2), Ok((true,true,16000000))); assert_eq!(c.aux_uart_clock(usize::MAX),Err(Fault::Unmapped));
    }

    #[test]
    fn calibration_requires_start_power_divider_and_elapsed_functional_quanta() {
        let mut c = Clocks::default(); c.lp_write(LP_BASE + 0x4c, 0x81).unwrap();
        c.set_cpll_power(true); c.advance_calibration(100); assert_eq!(c.read(0x174), Ok(0));
        c.write(0x174, 12).unwrap(); c.advance_calibration(100); assert_eq!(c.read(0x174), Ok(8)); // RO END cannot be injected.
        c.write(0x174, 0).unwrap(); c.advance_calibration(15); assert_eq!(c.read(0x174), Ok(0));
        c.advance_calibration(1); assert_eq!(c.read(0x174), Ok(4));
        c.write(0x174, 12).unwrap(); assert_eq!(c.read(0x174), Ok(12)); // STOP retains the completed result.
        c.set_cpll_power(false); c.advance_calibration(100); assert_eq!(c.read(0x174), Ok(8));
        c.write(0x174, 0).unwrap(); c.advance_calibration(100); assert_eq!(c.read(0x174), Ok(0));
        c.set_cpll_power(true); // loss cancelled the pending request; re-arm explicitly
        c.write(0x174, 0).unwrap(); c.advance_calibration(3); c.write(0x174, 8).unwrap();
        c.advance_calibration(100); assert_eq!(c.read(0x174), Ok(8));
        c.lp_write(LP_BASE + 0x4c, 0).unwrap(); c.write(0x174, 0).unwrap();
        c.advance_calibration(u32::MAX); assert_eq!(c.read(0x174), Ok(0));
        assert_eq!(c.write(0x174, 0x20), Err(Fault::Prohibited)); // SDIO PLL is not implemented.
    }
    #[test]
    fn cpll_clock_latch_uses_completed_ratio_and_preserves_active_clock_on_invalid_writes() {
        let mut c = Clocks::default(); c.lp_write(LP_BASE + 0x4c, 0x81).unwrap();
        c.write(0, 1).unwrap(); c.write(4, 3).unwrap(); c.write(8, 0).unwrap(); c.write(0xc, 0).unwrap();
        assert_eq!(c.write(0x14, 1), Err(Fault::Prohibited));
        c.set_cpll_power(true); c.write(0x174, 0).unwrap(); c.advance_calibration(16); c.write(0x174, 8).unwrap();
        c.write(0x14, 1).unwrap(); assert_eq!(c.rates(), Rates { cpu_hz: 80_000_000, mem_hz: 80_000_000, sys_hz: 80_000_000, apb_hz: 40_000_000 });
        assert_eq!(c.lp_write(LP_BASE + 0x4c, 0), Err(Fault::Prohibited));
        assert_eq!(c.write(0x174, 0), Err(Fault::Prohibited)); assert_eq!(c.rates().cpu_hz, 80_000_000);
        c.write(0, 0).unwrap(); c.write(4, 0).unwrap(); c.write(0x14, 1).unwrap();
        c.set_cpll_power(false); assert_eq!(c.rates().cpu_hz, 40_000_000);
    }
    #[test]
    fn rtc_fast_selector_has_real_gate_and_no_invented_crystal() {
        let mut c = Clocks::default(); assert_eq!(c.rtc_fast_hz(), 40_000_000); assert_eq!(c.rtc_slow_hz(), 136_000);
        c.lp_write(LP_BASE, 0).unwrap(); assert_eq!(c.rtc_fast_hz(), 17_500_000);
        c.set_fosc_power(false); assert_eq!(c.rtc_fast_hz(), 0);
        c.lp_write(LP_BASE, 1 << 26).unwrap(); assert_eq!(c.rtc_fast_hz(), 40_000_000);
        assert_eq!(c.lp_write(LP_BASE, 1 << 23), Err(Fault::Prohibited)); // no XTAL32K input
        assert_eq!(c.lp_write(LP_BASE, 2 << 26), Err(Fault::Prohibited));
        assert_eq!(c.lp_write(LP_BASE, 1 << 30), Err(Fault::Prohibited)); // force-on/sleep not modeled
        c.lp_write(LP_BASE + 0x48, 32).unwrap(); assert_eq!(c.lp_read(LP_BASE + 0x48), Ok(40));
    }
    #[test]
    fn xtal_dividers_latch_only_on_update_and_derive_all_domains() {
        let mut c = Clocks::default(); assert_eq!(c.rates(), Rates { cpu_hz: 40_000_000, mem_hz: 20_000_000, sys_hz: 13_333_333, apb_hz: 6_666_666 });
        c.write(4, 1).unwrap(); c.write(8, 0).unwrap(); c.write(0xc, 1).unwrap(); c.write(0x10, 3).unwrap();
        assert_eq!(c.rates().cpu_hz, 40_000_000); c.write(0x14, 1).unwrap(); assert_eq!(c.read(0x14), Ok(0));
        assert_eq!(c.rates(), Rates { cpu_hz: 20_000_000, mem_hz: 20_000_000, sys_hz: 10_000_000, apb_hz: 2_500_000 });
        c.write(4, 1 | 1 << 8 | 2 << 11).unwrap(); c.write(0x14, 1).unwrap(); assert_eq!(c.rates().cpu_hz, 16_000_000);
    }
    #[test]
    fn unsupported_source_and_invalid_fraction_do_not_change_active_clock() {
        let mut c = Clocks::default();
        for source in 1..=3 { c.write(0, source).unwrap(); assert_eq!(c.write(0x14, 1), Err(Fault::Prohibited)); assert_eq!(c.rates().cpu_hz, 40_000_000); }
        c.write(0, 0).unwrap(); c.write(4, 1 << 8).unwrap(); assert_eq!(c.write(0x14, 1), Err(Fault::Prohibited));
        assert_eq!(c.read(0x1c), Err(Fault::Unmapped)); assert_eq!(c.write(u32::MAX, 0), Err(Fault::Misaligned));
    }
}

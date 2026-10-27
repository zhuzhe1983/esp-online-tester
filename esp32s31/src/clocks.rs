//! S31 XTAL-root dividers and functional peripheral gates; PLL/RC sources remain unsupported.
//! ESP-IDF v6.1 fff9895c82d744c7237be8847347bdd1b07c6643:
//! soc/esp32s31/register/soc/hp_sys_clkrst_reg.h, esp_hal_clock/esp32s31/include/hal/clk_tree_ll.h,
//! and esp_hw_support/port/esp32s31/rtc_clk.c (40/20/40÷3/40÷6 MHz reset configuration).
use emu_core::Fault;
pub const BASE: u32 = 0x2058_7000;
pub const UART0_CTRL: u32 = BASE + 0x88;
pub const UART0_MEM: u32 = 0x2058_628c;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rates { pub cpu_hz: u64, pub mem_hz: u64, pub sys_hz: u64, pub apb_hz: u64 }
pub struct Clocks { pending: [u32; 5], active: [u32; 5], uart0: u32, uart_mem: u32, modem: u32, psram: u32 }
impl Default for Clocks {
    fn default() -> Self { Self { pending: [0, 0, 1, 2, 1], active: [0, 0, 1, 2, 1], uart0: 0x83, uart_mem: 0, modem: 1, psram: 0x181 } }
}
impl Clocks {
    pub fn is_mmio(address: u32) -> bool { (BASE..BASE + 0x18).contains(&address) || matches!((address & !3).wrapping_sub(BASE), 0x40 | 0x68) }
    pub fn uart_read(&self, address: u32) -> u32 { if address == UART0_CTRL { self.uart0 } else { self.uart_mem } }
    pub fn uart_write(&mut self, address: u32, value: u32) -> Result<(), Fault> {
        if address == UART0_CTRL {
            // v6.1 uart_ll.h: 0=XTAL, 1=RC_FAST, 2=PLL80M. Only the XTAL root
            // is implemented. FORCE_NORST/global reset interaction remains unsupported.
            if value & 0x70 != 0 { return Err(Fault::Prohibited); }
            let num = (value >> 16) & 255; let den = value >> 24;
            if num != 0 && (den == 0 || num >= den) { return Err(Fault::Prohibited); }
            self.uart0 = value;
        } else {
            // Low-power retention/shutdown is not modeled. Preserve mode configuration
            // while awake, but do not pretend entry to a powered-down memory state works.
            if value & 4 != 0 { return Err(Fault::Prohibited); }
            self.uart_mem = value & 15;
        }
        Ok(())
    }
    pub fn psram_on(&self) -> bool { self.psram & 0x167 == 0x101 }
    pub fn modem_on(&self) -> bool { self.modem != 0 }
    pub fn uart_bus_on(&self) -> bool { self.uart0 & 0xb == 3 }
    pub fn uart_core_on(&self) -> bool { self.uart0 & 0x87 == 0x83 }
    pub fn uart_sclk_hz(&self) -> u64 {
        if !self.uart_core_on() { return 0; }
        let integer = u64::from((self.uart0 >> 8) & 255) + 1;
        let num = u64::from((self.uart0 >> 16) & 255); let den = u64::from(self.uart0 >> 24);
        let den = if num == 0 { 1 } else { den };
        40_000_000 * den / (integer * den + num)
    }
    fn rates_of(regs: &[u32; 5]) -> Result<Rates, Fault> {
        // Mux1 is CPLL, mux2 RC_FAST, mux3 PLL_F240M per v6.1 LL. The generated
        // header's older “PLL_F300M” prose is not used to invent a fixed PLL rate.
        if regs[0] != 0 { return Err(Fault::Prohibited); }
        let div = u64::from((regs[1] & 255) + 1);
        let num = u64::from((regs[1] >> 8) & 7); let den = u64::from((regs[1] >> 11) & 7);
        if num != 0 && (den == 0 || num >= den) { return Err(Fault::Prohibited); }
        // SYS/APB fractional divider behavior is not used by the fixed LL and is
        // deliberately unsupported pending a documented constraint model.
        if regs[3] & !255 != 0 || regs[4] & !255 != 0 { return Err(Fault::Prohibited); }
        let den = if num == 0 { 1 } else { den };
        let cpu_den = div * den + num;
        let root = 40_000_000 * den; let mem_div = u64::from(regs[2] + 1);
        let sys_div = u64::from(regs[3] + 1); let apb_div = u64::from(regs[4] + 1);
        Ok(Rates { cpu_hz: root / cpu_den, mem_hz: root / (cpu_den * mem_div),
            sys_hz: root / (cpu_den * sys_div), apb_hz: root / (cpu_den * sys_div * apb_div) })
    }
    /// Computed functional domain rates; not an instruction-to-clock timing model.
    pub fn rates(&self) -> Rates { Self::rates_of(&self.active).unwrap_or(Rates { cpu_hz: 0, mem_hz: 0, sys_hz: 0, apb_hz: 0 }) }
    pub fn read(&self, off: u32) -> Result<u32, Fault> {
        if off & 3 != 0 { return Err(Fault::Misaligned); }
        match off { 0..=0x10 => Ok(self.pending[off as usize / 4]), 0x14 => Ok(0), 0x40 => Ok(self.modem), 0x68 => Ok(self.psram), _ => Err(Fault::Unmapped) }
    }
    pub fn write(&mut self, off: u32, value: u32) -> Result<(), Fault> {
        if off & 3 != 0 { return Err(Fault::Misaligned); }
        match off {
            0 => self.pending[0] = value & 3,
            4 | 0xc | 0x10 => self.pending[off as usize / 4] = value & 0x3fff,
            8 => self.pending[2] = value & 1,
            0x14 => if value & 1 != 0 {
                Self::rates_of(&self.pending)?;
                // Functional atomic latch; no fabricated PLL lock/busy poll or physical delay.
                self.active = self.pending;
            },
            // MODEM_CTRL0 has only the documented RW clock gate, reset1. This does
            // not implement RF or wireless operation.
            0x40 => self.modem = value & 1,
            0x68 => {
                // Functional XTAL PSRAM gate; PLL selection and force-reset override
                // require the unimplemented external-memory power/PLL sequencing.
                if value & 0x78 != 0 { return Err(Fault::Prohibited); }
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

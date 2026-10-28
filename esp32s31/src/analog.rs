//! S31 internal analog-I2C register transactions; no analog voltage, PLL lock or RF simulation.
//! ESP-IDF v6.1 fff9895c82d744c7237be8847347bdd1b07c6643:
//! soc/esp32s31/include/modem/{reg_base,i2c_ana_mst_reg,modem_lpcon_reg,modem_syscon_reg}.h,
//! esp_hal_regi2c/esp32s31/{regi2c_impl.c,include/hal/regi2c_ctrl_ll.h},
//! soc/esp32s31/{include/soc/{regi2c_saradc,regi2c_dig_reg}.h,register/soc/pmu_reg.h}.
use emu_core::Fault;
pub const BASE: u32 = 0x2010_f800;
const LP_RESET: u32 = 0x2010_f024;
const PERIF_POWER: u32 = 0x2070_4208;
const SYS_CLK: u32 = 0x2010_9c04;

pub struct Analog {
    pub lpcon: crate::modem_lpcon::ModemLpcon, source: u32, ctrl: [u32; 2], config: [u32; 2],
    select: [u32; 3], timing: [u32; 3], date: u32, bias: Option<[u8; 2]>, perif: Option<[u8; 2]>, perif_power: u32, dig: Option<[u8; 4]>, bod: Option<u8>, pub transactions: u64,
}

impl Default for Analog {
    fn default() -> Self { Self { lpcon: crate::modem_lpcon::ModemLpcon::default(), source: (1 << 21) | 2, ctrl: [0; 2], config: [0; 2],
        select: [0xe408, 0x2d, 4], timing: [0x42; 3], date: 0x2411290, bias: None, perif: None, perif_power: 1 << 30, dig: None, bod: None, transactions: 0 } }
}
impl Analog {
    pub fn master_enabled(&self) -> bool { self.lpcon.i2c_master_enabled() }
    pub fn is_mmio(address: u32) -> bool {
        (BASE..BASE + 0x40).contains(&address) || crate::modem_lpcon::ModemLpcon::is_mmio(address) || matches!(address & !3, SYS_CLK | PERIF_POWER)
    }
    /// The public BIAS field header specifies two data bytes but no silicon reset values.
    /// An explicit host seed is therefore required; zero is never assumed for an absent die.
    pub fn set_bias(&mut self, bytes: [u8; 2]) { self.bias = Some(bytes); }
    pub fn bias(&self) -> Option<[u8; 2]> { self.bias }
    /// Explicit register10 initial value from the caller, not an inferred analog reset.
    pub fn set_perif_glitch(&mut self, value: u8) { self.perif = Some([value, value]); }
    pub fn perif_glitch(&self) -> Option<u8> { self.perif.map(|pair| pair[1]) }
    /// DIG_REG registers5,7,13,14 in that order; header does not give reset bytes.
    pub fn set_dig_reg(&mut self, values: [u8; 4]) { self.dig = Some(values); }
    pub fn dig_reg(&self) -> Option<[u8; 4]> { self.dig }
    /// Internal BOD target0x61/register5. Only threshold bits2:0 are documented;
    /// require the caller's complete byte for read-modify-write,not a reset guess.
    pub fn set_bod_register(&mut self, value: u8) { self.bod = Some(value); }
    pub fn bod_register(&self) -> Option<u8> { self.bod }
    fn transaction(&mut self, host: usize, value: u32) -> Result<(), Fault> {
        // EN is mandatory in the official LL; FO prevents automatic idle gating, it
        // does not substitute for a disabled master. Only synchronous byte transactions
        // are modeled, not a 160 MHz waveform or an analog settling delay.
        if !self.lpcon.i2c_master_enabled() { return Err(Fault::Prohibited); }
        let slave = value & 255; let reg = ((value >> 8) & 255) as usize;
        // Internal BIAS, DIG and PERIF are separate targets. Public headers specify fields,
        // not their analog reset bytes; all require explicit host inputs.
        let slave_bit = match (slave, reg) { (0x61, 5) => 6, (0x6a, 0 | 1) => 4, (0x69, 10) => 9, (0x6d, 5 | 7 | 13 | 14) => 13, _ => return Err(Fault::Unmapped) };
        if self.select[1] & (1 << (slave_bit + 2)) != 0 || host != usize::from(self.select[2] & (1 << (slave_bit + 4)) == 0) { return Err(Fault::Prohibited); }
        let byte = if slave == 0x61 { self.bod.as_mut().ok_or(Fault::Prohibited)? }
            else if slave == 0x6a { &mut self.bias.as_mut().ok_or(Fault::Prohibited)?[reg] }
            else if slave == 0x6d {
                let index = match reg { 5 => 0, 7 => 1, 13 => 2, _ => 3 };
                &mut self.dig.as_mut().ok_or(Fault::Prohibited)?[index]
            } else {
                // PMU_ANA_PERI_PWR_CTRL: XPD30 and active-low reset release31.
                if self.perif_power != 0xc000_0000 { return Err(Fault::Prohibited); }
                &mut self.perif.as_mut().ok_or(Fault::Prohibited)?[1]
            };
        if value & (1 << 24) != 0 { *byte = (value >> 16) as u8; }
        self.ctrl[host] = (value & 0x0100_ffff) | u32::from(*byte) << 16;
        // BUSY is low only after the requested read/write above really completes.
        self.transactions += 1; Ok(())
    }
    pub fn read(&self, address: u32, parent_clock: bool) -> Result<u32, Fault> {
        if address == PERIF_POWER { return Ok(self.perif_power); }
        if crate::modem_lpcon::ModemLpcon::is_mmio(address) { return self.lpcon.read(address,parent_clock); }
        if !parent_clock { return Err(Fault::Prohibited); }
        match address {
            SYS_CLK => Ok(self.source),
            a if a >= BASE => match a - BASE {
                0 | 4 => Ok(self.ctrl[((a - BASE) / 4) as usize]),
                8 | 0xc => Ok(self.config[((a - BASE - 8) / 4) as usize]),
                0x18 | 0x1c | 0x20 => Ok(self.select[((a - BASE - 0x18) / 4) as usize]),
                0x24 | 0x28 | 0x2c => Ok(self.timing[((a - BASE - 0x24) / 4) as usize]),
                0x3c => Ok(self.date), _ => Err(Fault::Unmapped),
            },
            _ => Err(Fault::Unmapped),
        }
    }
    pub fn write(&mut self, address: u32, value: u32, parent_clock: bool) -> Result<(), Fault> {
        if address == PERIF_POWER {
            self.perif_power = value & 0xc000_0000;
            if self.perif_power != 0xc000_0000 {
                // Model reset/power loss returns the target to the caller's explicit
                // initialization profile; physical analog retention is not characterized.
                if let Some(pair) = self.perif.as_mut() { pair[1] = pair[0]; }
            }
            return Ok(());
        }
        if crate::modem_lpcon::ModemLpcon::is_mmio(address) {
            self.lpcon.write(address,value,parent_clock)?;
            if address == LP_RESET && value & 4 != 0 { self.ctrl = [0; 2]; self.config = [0; 2]; self.select = [0xe408, 0x2d, 4]; self.timing = [0x42; 3]; }
            return Ok(());
        }
        if !parent_clock { return Err(Fault::Prohibited); }
        match address {
            SYS_CLK => { if (value ^ self.source) & !(1 << 12) != 0 { return Err(Fault::Prohibited); } self.source = value; }
            a if a >= BASE => match a - BASE {
                0 | 4 => self.transaction(((a - BASE) / 4) as usize, value)?,
                8 | 0xc => {
                    // No public subfield semantics for this 24-bit sequencer control;
                    // preserve its disabled reset setting rather than invent a mode.
                    if value & 0xffffff != 0 { return Err(Fault::Prohibited); }
                }
                0x18 | 0x1c | 0x20 => self.select[((a - BASE - 0x18) / 4) as usize] = value & 0xffffff,
                0x24 | 0x28 => self.timing[((a - BASE - 0x24) / 4) as usize] = value & 0x7ff,
                0x2c => { if value & (1 << 11) != 0 { return Err(Fault::Prohibited); } self.timing[2] = value & 0x7ff; }
                0x3c => self.date = value & 0x1fff_ffff,
                _ => return Err(Fault::Unmapped),
            },
            _ => return Err(Fault::Unmapped),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    const LP_CLK: u32 = 0x2010_f018;
    #[test]
    fn bod_threshold_byte_requires_input_and_the_selected_master_route() {
        let mut a = super::Analog::default(); a.write(LP_CLK, 4, true).unwrap();
        assert_eq!(a.write(super::BASE + 4, 0x561, true), Err(emu_core::Fault::Prohibited)); assert_eq!(a.transactions, 0);
        a.set_bod_register(0xff); a.write(super::BASE + 4, 0x561, true).unwrap(); assert_eq!(a.read(super::BASE + 4, true), Ok(0xff0561));
        a.write(super::BASE + 4, 0x01fd0561, true).unwrap(); assert_eq!(a.bod_register(), Some(0xfd));
        a.write(super::BASE + 0x20, 0x404, true).unwrap(); assert_eq!(a.write(super::BASE + 4, 0x561, true), Err(emu_core::Fault::Prohibited));
        a.write(super::BASE, 0x561, true).unwrap(); assert_eq!(a.read(super::BASE, true), Ok(0xfd0561));
        a.write(LP_CLK, 0, true).unwrap(); assert_eq!(a.write(super::BASE, 0x01000561, true), Err(emu_core::Fault::Prohibited));
        assert_eq!(a.bod_register(), Some(0xfd)); assert_eq!(a.transactions, 3);
    }
    const LP_FORCE: u32 = 0x2010_f01c;
    use super::*;
    #[test]
    fn dig_data_needs_explicit_input_and_follows_its_own_master_route() {
        let mut a = Analog::default(); a.write(LP_CLK, 4, true).unwrap();
        assert_eq!(a.write(BASE + 4, 0xd6d, true), Err(Fault::Prohibited));
        a.set_dig_reg([0x11, 0x22, 0xff, 0x44]);
        for (reg, byte) in [(5, 0x11), (7, 0x22), (13, 0xff), (14, 0x44)] {
            a.write(BASE + 4, reg << 8 | 0x6d, true).unwrap();
            assert_eq!(a.read(BASE + 4, true), Ok(byte << 16 | reg << 8 | 0x6d));
        }
        a.write(BASE + 4, 0x01f30d6d, true).unwrap(); a.write(LP_RESET, 4, true).unwrap();
        assert_eq!(a.dig_reg(), Some([0x11, 0x22, 0xf3, 0x44]));
        a.write(BASE + 0x20, 4 | 1 << 17, true).unwrap();
        assert_eq!(a.write(BASE + 4, 0xd6d, true), Err(Fault::Prohibited));
        a.write(BASE, 0xd6d, true).unwrap(); assert_eq!(a.read(BASE, true), Ok(0xf30d6d));
        a.write(BASE + 0x1c, 1 << 15, true).unwrap();
        assert_eq!(a.write(BASE, 0x01000d6d, true), Err(Fault::Prohibited));
        assert_eq!(a.write(BASE, 0x66d, true), Err(Fault::Unmapped));
        assert_eq!(a.dig_reg(), Some([0x11, 0x22, 0xf3, 0x44])); assert_eq!(a.transactions, 6);
    }
    #[test]
    fn perif_target_requires_power_release_seed_and_route_and_retains_master_reset_data() {
        let mut a = Analog::default(); a.write(LP_CLK, 4, true).unwrap();
        assert_eq!(a.read(PERIF_POWER, false), Ok(1 << 30));
        assert_eq!(a.write(BASE + 4, 0xa69, true), Err(Fault::Prohibited));
        a.write(PERIF_POWER, 0xc000_0000, false).unwrap();
        assert_eq!(a.write(BASE + 4, 0xa69, true), Err(Fault::Prohibited));
        a.set_perif_glitch(0xf7); a.write(BASE + 4, 0xa69, true).unwrap();
        assert_eq!(a.read(BASE + 4, true), Ok(0xf70a69));
        a.write(BASE + 4, 0x01f20a69, true).unwrap();
        a.write(LP_RESET, 4, true).unwrap(); assert_eq!(a.perif_glitch(), Some(0xf2));
        a.write(BASE + 0x20, 0x2004, true).unwrap();
        assert_eq!(a.write(BASE + 4, 0xa69, true), Err(Fault::Prohibited));
        a.write(BASE, 0xa69, true).unwrap(); assert_eq!(a.read(BASE, true), Ok(0xf20a69));
        a.write(BASE + 0x1c, 0x800, true).unwrap();
        assert_eq!(a.write(BASE, 0xa69, true), Err(Fault::Prohibited));
        a.write(PERIF_POWER, 0x4000_0000, false).unwrap(); assert_eq!(a.perif_glitch(), Some(0xf7));
        a.write(BASE + 0x1c, 0, true).unwrap(); assert_eq!(a.write(BASE, 0xa69, true), Err(Fault::Prohibited));
        a.write(PERIF_POWER, 0x8000_0000, false).unwrap(); assert_eq!(a.write(BASE, 0xa69, true), Err(Fault::Prohibited));
        assert_eq!(a.transactions, 3); assert_eq!(a.write(BASE, 0xb69, true), Err(Fault::Unmapped));
    }
    #[test]
    fn forced_clock_and_routes_cannot_substitute_for_the_master_or_missing_data() {
        let mut a = Analog::default(); assert_eq!(a.read(SYS_CLK, true), Ok(0x200002)); a.set_bias([0x53, 0x27]);
        a.write(LP_FORCE, 4, true).unwrap();
        assert_eq!(a.write(BASE + 4, 0x6a, true), Err(Fault::Prohibited));
        a.write(LP_CLK, 4, true).unwrap();
        assert_eq!(a.write(BASE, 0x6a, true), Err(Fault::Prohibited));
        a.write(BASE + 4, 0x6a, true).unwrap(); assert_eq!(a.read(BASE + 4, true), Ok(0x53006a));
        a.write(BASE + 0x1c, 0x40, true).unwrap();
        assert_eq!(a.write(BASE + 4, 0x0100006a, true), Err(Fault::Prohibited));
        assert_eq!(a.bias(), Some([0x53, 0x27])); assert_eq!(a.transactions, 1);
        assert_eq!(a.read(BASE + 4, false), Err(Fault::Prohibited));
        a.write(BASE + 0x1c, 0, true).unwrap();
        for command in [0x66, 0x26a, 0xffff] { assert_eq!(a.write(BASE + 4, command, true), Err(Fault::Unmapped)); }
        assert_eq!(a.write(BASE + 0x10, 1, true), Err(Fault::Unmapped)); // Burst remains absent.
        assert_eq!(a.transactions, 1); assert_eq!(a.bias(), Some([0x53, 0x27]));
        assert_eq!(a.write(SYS_CLK, 0, true), Err(Fault::Prohibited)); // Don't silently reconfigure RF clocks.
    }
    #[test]
    fn read_only_busy_cannot_fabricate_a_pending_or_completed_transaction() {
        let mut a = Analog::default(); a.write(LP_CLK, 4, true).unwrap();
        assert_eq!(a.write(BASE + 4, 0x0200006a, true), Err(Fault::Prohibited));
        assert_eq!(a.read(BASE + 4, true), Ok(0)); assert_eq!(a.transactions, 0);
        a.set_bias([0x96, 0x61]); a.write(BASE + 4, 0x0200006a, true).unwrap();
        assert_eq!(a.read(BASE + 4, true), Ok(0x96006a)); assert_eq!(a.transactions, 1);
        a.write(LP_RESET, 4, true).unwrap(); assert_eq!(a.read(BASE + 4, true), Ok(0));
        assert_eq!(a.bias(), Some([0x96, 0x61])); // Master reset does not reset the analog target.
    }
}

//! S31 internal analog-I2C register transactions; no analog voltage, PLL lock or RF simulation.
//! ESP-IDF v6.1 fff9895c82d744c7237be8847347bdd1b07c6643:
//! soc/esp32s31/include/modem/{reg_base,i2c_ana_mst_reg,modem_lpcon_reg,modem_syscon_reg}.h,
//! esp_hal_regi2c/esp32s31/{regi2c_impl.c,include/hal/regi2c_ctrl_ll.h}.
use emu_core::Fault;
pub const BASE: u32 = 0x2010_f800;
const LP_CLK: u32 = 0x2010_f018;
const LP_FORCE: u32 = 0x2010_f01c;
const LP_RESET: u32 = 0x2010_f024;
const SYS_CLK: u32 = 0x2010_9c04;

pub struct Analog {
    lp_clock: u32, force: u32, source: u32, ctrl: [u32; 2], config: [u32; 2],
    select: [u32; 3], timing: [u32; 3], date: u32, bias: Option<[u8; 2]>, pub transactions: u64,
}

impl Default for Analog {
    fn default() -> Self { Self { lp_clock: 0, force: 0, source: (1 << 21) | 2, ctrl: [0; 2], config: [0; 2],
        select: [0xe408, 0x2d, 4], timing: [0x42; 3], date: 0x2411290, bias: None, transactions: 0 } }
}
impl Analog {
    pub fn is_mmio(address: u32) -> bool {
        (BASE..BASE + 0x40).contains(&address) || matches!(address & !3, LP_CLK | LP_FORCE | LP_RESET | SYS_CLK)
    }
    /// The public BIAS field header specifies two data bytes but no silicon reset values.
    /// An explicit host seed is therefore required; zero is never assumed for an absent die.
    pub fn set_bias(&mut self, bytes: [u8; 2]) { self.bias = Some(bytes); }
    pub fn bias(&self) -> Option<[u8; 2]> { self.bias }
    fn transaction(&mut self, host: usize, value: u32) -> Result<(), Fault> {
        // EN is mandatory in the official LL; FO prevents automatic idle gating, it
        // does not substitute for a disabled master. Only synchronous byte transactions
        // are modeled, not a 160 MHz waveform or an analog settling delay.
        if self.lp_clock & 4 == 0 { return Err(Fault::Prohibited); }
        let slave = value & 255; let reg = ((value >> 8) & 255) as usize;
        // regi2c_bias.h: 0x6a, register0 (1P6/1P1), register1 (1P1/2P2 PVT).
        if slave != 0x6a || reg > 1 { return Err(Fault::Unmapped); }
        if self.select[1] & (1 << 6) != 0 || host != usize::from(self.select[2] & (1 << 8) == 0) { return Err(Fault::Prohibited); }
        let bias = self.bias.as_mut().ok_or(Fault::Prohibited)?;
        if value & (1 << 24) != 0 { bias[reg] = (value >> 16) as u8; }
        self.ctrl[host] = (value & 0x0100_ffff) | u32::from(bias[reg]) << 16;
        // BUSY is low only after the requested read/write above really completes.
        self.transactions += 1; Ok(())
    }
    pub fn read(&self, address: u32, parent_clock: bool) -> Result<u32, Fault> {
        if !parent_clock { return Err(Fault::Prohibited); }
        match address {
            LP_CLK => Ok(self.lp_clock), LP_FORCE => Ok(self.force), LP_RESET => Ok(0), SYS_CLK => Ok(self.source),
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
        if !parent_clock { return Err(Fault::Prohibited); }
        match address {
            LP_CLK => { if value & 0xb != 0 { return Err(Fault::Prohibited); } self.lp_clock = value & 4; }
            LP_FORCE => { if value & 0x1b != 0 { return Err(Fault::Prohibited); } self.force = value & 4; }
            LP_RESET => {
                if value & 0x1b != 0 { return Err(Fault::Prohibited); }
                if value & 4 != 0 { self.ctrl = [0; 2]; self.config = [0; 2]; self.select = [0xe408, 0x2d, 4]; self.timing = [0x42; 3]; }
            }
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
    use super::*;
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

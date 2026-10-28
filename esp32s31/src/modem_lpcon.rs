//! S31 modem low-power clock configuration, not radio or network emulation.
//! Fixed IDF6.1 fff9895c82d744c7237be8847347bdd1b07c6643:
//! modem/{reg_base,modem_lpcon_reg}.h, hal/modem_lpcon_ll.h,
//! esp_hw_support/modem/port/esp32s31/modem_clock_impl.c.
use emu_core::Fault;
pub const BASE: u32 = 0x2010_f000;
pub struct ModemLpcon { test: u32, sources: [u32; 3], enable: u32, force: u32, state_map: u32, tick_target: u32 }
impl Default for ModemLpcon {
    fn default() -> Self { Self { test: 0, sources: [0; 3], enable: 0, force: 0, state_map: 0, tick_target: 39 } }
}
impl ModemLpcon {
    pub fn i2c_master_enabled(&self) -> bool { self.enable & 4 != 0 }
    pub fn is_mmio(address: u32) -> bool { (BASE..BASE + 0x5c).contains(&address) }
    pub fn read(&self, address: u32, bus_on: bool) -> Result<u32, Fault> {
        if !bus_on { return Err(Fault::Prohibited); }
        match address.wrapping_sub(BASE) {
            0 => Ok(self.test), 4 | 8 | 12 => Ok(self.sources[((address - BASE) / 4 - 1) as usize]),
            0x10 | 0x14 | 0x24 => Ok(0), 0x18 => Ok(self.enable), 0x1c => Ok(self.force),
            0x20 => Ok(self.state_map), 0x28 => Ok(self.tick_target), _ => Err(Fault::Unmapped),
        }
    }
    pub fn write(&mut self, address: u32, value: u32, bus_on: bool) -> Result<(), Fault> {
        if !bus_on { return Err(Fault::Prohibited); }
        match address.wrapping_sub(BASE) {
            0 => self.test = value & 1,
            4 | 8 | 12 => {
                // Only the existing XTAL and nominal RC_SLOW input paths. RC_FAST
                // digital distribution and external32K are not invented.
                if value & 10 != 0 { return Err(Fault::Prohibited); }
                self.sources[((address - BASE) / 4 - 1) as usize] = value & 0xffff;
            }
            // AON source forcing and 32K selection need their own source model.
            0x10 | 0x14 => { if value != 0 { return Err(Fault::Prohibited); } }
            0x18 => self.enable = value & 15, 0x1c => self.force = value & 31,
            // Retain documented PMU gating policy; no sleep/power-state transition
            // is synthesized. This is configuration, not a read-only ready word.
            0x20 => self.state_map = value & 0xffff0000,
            // The containing Analog device executes bit2 reset on its real sequencer.
            // Other reset targets (radio power,coex,BLE/DCMEM) are absent.
            0x24 => { if value & 0x1b != 0 { return Err(Fault::Prohibited); } }
            0x28 => self.tick_target = value & 63,
            _ => return Err(Fault::Unmapped),
        }
        Ok(())
    }
    /// Active-run software clock requests only. The policy above does not imply
    /// PMU sleep behavior, MAC activity, RF calibration or an available peer.
    /// channel0=BLE RTC,1=coex,2=Wi-Fi power, as the fixed register/LL names state.
    pub fn lp_hz(&self, channel: usize, bus_on: bool, slow_hz: u64) -> Result<u64, Fault> {
        let word = *self.sources.get(channel).ok_or(Fault::Unmapped)?;
        let mask = [8, 2, 1][channel];
        if !bus_on || self.enable & mask == 0 { return Ok(0); }
        let rate = match word & 15 { 0 => 0, 1 => slow_hz, 4 => 40_000_000, _ => return Err(Fault::Prohibited) };
        Ok(rate / u64::from((word >> 4) + 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn software_requests_require_real_source_and_independent_channel_divider() {
        let mut m = ModemLpcon::default(); assert_eq!(m.read(BASE + 0x28, true), Ok(39));
        m.write(BASE + 0x18, 11, true).unwrap(); assert_eq!(m.lp_hz(2, true, 136000), Ok(0));
        for (offset,value) in [(4,4 | 39 << 4),(8,1 | 3 << 4),(12,4 | 19 << 4)] { m.write(BASE + offset,value,true).unwrap(); }
        assert_eq!([m.lp_hz(0,true,136000),m.lp_hz(1,true,136000),m.lp_hz(2,true,136000)], [Ok(1000000),Ok(34000),Ok(2000000)]);
        m.write(BASE + 0x18,0,true).unwrap(); assert_eq!(m.lp_hz(2,true,136000),Ok(0));
        m.write(BASE + 0x1c,1,true).unwrap(); assert_eq!(m.lp_hz(2,true,136000),Ok(0)); // force prevents idle gating,not EN substitution
        m.write(BASE + 0x18,1,true).unwrap(); assert_eq!(m.lp_hz(2,true,136000),Ok(2000000));
        assert_eq!(m.lp_hz(1,true,136000),Ok(0)); assert_eq!(m.lp_hz(2,false,136000),Ok(0));
    }
    #[test]
    fn absent_bus_ambiguous_sources_and_unmodeled_resets_do_not_fake_radio_readiness() {
        let mut m=ModemLpcon::default(); assert_eq!(m.write(BASE+12,1,false),Err(Fault::Prohibited));
        assert_eq!(m.read(BASE+12,false),Err(Fault::Prohibited));
        assert_eq!(m.write(BASE+12,8,true),Err(Fault::Prohibited)); assert_eq!(m.read(BASE+12,true),Ok(0));
        m.write(BASE+12,5,true).unwrap(); m.write(BASE+0x18,1,true).unwrap(); assert_eq!(m.lp_hz(2,true,136000),Err(Fault::Prohibited));
        assert_eq!(m.write(BASE+0x24,1,true),Err(Fault::Prohibited)); assert_eq!(m.read(BASE+0x50,true),Err(Fault::Unmapped));
        assert_eq!(m.lp_hz(usize::MAX,true,136000),Err(Fault::Unmapped));
    }
}

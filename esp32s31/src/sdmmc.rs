//! S31 SDMMC slot 0: dedicated pads, clock/reset controls and the shared SD/IDMAC core.
//! ESP-IDF v6.1 fff9895c82d744c7237be8847347bdd1b07c6643:
//! soc/esp32s31/register/soc/{reg_base,sdmmc_reg,hp_sys_clkrst_reg,cnnt_sys_reg,
//! cnnt_io_mux_reg}.h, ld/esp32s31.peripherals.ld, include/soc/interrupts.h;
//! esp_hal_sd/esp32s31/include/{hal/sdmmc_ll,soc/sdmmc_pins}.h.
//! Commands and IDMAC transfers are immediate digital operations. No wire timing, DLL
//! calibration, UHS/DDR or slot-1 wiring is claimed. MPLL readiness comes from the SoC.
use emu_core::Fault;
use esp_periph::Device;
use esp_periph::sdmmc::{reg, SdCard, SdCardConfig, SdDevice, SdStore, Sdmmc, SdmmcLayout, SdmmcMemory};

pub const BASE: u32 = 0x2034_2000;
pub const SOURCE: usize = 3; // ETS_SDIO_HOST_INTR_SOURCE
pub const CLOCK: u32 = 0x2058_70c0;
pub const PHASE: u32 = 0x2058_70c4;
pub const MEMORY: u32 = 0x2035_9010;
pub const RESET: u32 = 0x2035_9038;
pub const PAD_BASE: u32 = 0x2058_8000;
pub const PAD_CTRL: u32 = PAD_BASE + 0x3f4;
pub const SLOT0_PINS: [u8; 6] = [24, 25, 20, 21, 22, 23]; // CLK, CMD, D0..3
const PHASE_CLOCKS: u32 = 7 << 22;

pub struct SdmmcS31 {
    pub host: Sdmmc,
    clock: u32, phase: u32, memory: u32, reset: u32,
    // Dedicated SDIO pad order: DATA0..3, CLK, CMD (not GPIO matrix entries).
    pads: [u32; 6], pad_ctrl: u32, pad_clock: u32, pad_date: u32,
}
impl Default for SdmmcS31 {
    fn default() -> Self {
        let mut host = Sdmmc::new(SdmmcLayout::Esp32s31); host.card_detect_n = 3;
        Self { host, clock: 0, phase: 0, memory: 6, reset: 0,
            pads: [0x5ac, 0x5ac, 0x5ac, 0x5ac, 0x528, 0x5ac], pad_ctrl: 0, pad_clock: 1, pad_date: 38_834_464 }
    }
}
impl SdmmcS31 {
    pub fn is_mmio(address: u32) -> bool {
        (BASE..BASE + 0x1000).contains(&address) || matches!(address, CLOCK | PHASE | MEMORY | RESET)
            || (PAD_BASE + 0x1c..=PAD_BASE + 0x30).contains(&address)
            || (PAD_BASE + 0x3f4..=PAD_BASE + 0x3fc).contains(&address)
    }
    fn core_offset(address: u32) -> Option<u32> {
        let off = address.checked_sub(BASE)?;
        (matches!(off, 0 | 8..=0x54 | 0x5c..=0x78 | 0x80..=0x98 | 0x100 | 0x10c..=0x110 | 0x200..=0x3fc | 0x800..=0x80c) && off & 3 == 0).then_some(off)
    }
    pub fn read(&mut self, address: u32) -> Result<u32, Fault> {
        if address & 3 != 0 { return Err(Fault::Unmapped); }
        if let Some(off) = Self::core_offset(address) { return Ok(self.host.read(off)); }
        match address {
            CLOCK => Ok(self.clock), PHASE => Ok(self.phase), MEMORY => Ok(self.memory), RESET => Ok(self.reset),
            PAD_CTRL => Ok(self.pad_ctrl),
            a if a == PAD_BASE + 0x3f8 => Ok(self.pad_clock),
            a if a == PAD_BASE + 0x3fc => Ok(self.pad_date),
            a if (PAD_BASE + 0x1c..=PAD_BASE + 0x30).contains(&a) => Ok(self.pads[((a - PAD_BASE - 0x1c) / 4) as usize]),
            _ => Err(Fault::Unmapped),
        }
    }
    pub fn write(&mut self, address: u32, value: u32) -> Result<(), Fault> {
        if address & 3 != 0 { return Err(Fault::Unmapped); }
        if let Some(off) = Self::core_offset(address) {
            // Holding reset keeps the core at its reset values, without resetting the card.
            if self.reset & (1 << 30) == 0 {
                // DLL_CAL_END is read-only. S31's HS DLL is not modeled.
                let v = if off == reg::DLL_CONF { value & !(1 << 1) } else { value };
                self.host.write(off, v);
            }
            return Ok(());
        }
        match address {
            CLOCK => self.clock = value & 0xfff,
            // EDGE_CFG_UPDATE is a write trigger, not readable state. The digital
            // model applies the requested phase/divider without analog settling.
            PHASE => self.phase = value & 0x01ff_fffe,
            MEMORY => self.memory = value & 15,
            RESET => { self.reset = value & 0xe000_0000; if value & (1 << 30) != 0 { self.host.reset_registers(); } }
            PAD_CTRL => self.pad_ctrl = value & 3,
            a if a == PAD_BASE + 0x3f8 => self.pad_clock = value & 1,
            a if a == PAD_BASE + 0x3fc => self.pad_date = value & 0x0fff_ffff,
            a if (PAD_BASE + 0x1c..=PAD_BASE + 0x30).contains(&a) => self.pads[((a - PAD_BASE - 0x1c) / 4) as usize] = value & 0x1fff,
            _ => return Err(Fault::Unmapped),
        }
        Ok(())
    }
    /// SoC reset: host/control registers reset; the external card and its image stay.
    pub fn reset(&mut self) {
        let devices = std::mem::take(&mut self.host.devices);
        let stats = std::mem::take(&mut self.host.stats);
        let log = self.host.logging();
        *self = Self::default(); self.host.devices = devices; self.host.stats = stats;
        self.host.debug(log); self.host.card_detect_n = if self.present() { 2 } else { 3 };
    }
    /// The image is caller-owned storage (FileStore or the browser's SharedSd). All guest
    /// writes/erases use that store directly; exporting it preserves changed card bytes.
    /// Validation finishes before replacing an attached card.
    pub fn attach(&mut self, image: Box<dyn SdStore>, config: SdCardConfig) -> Result<String, String> {
        if config.timed_bus || config.prg_ns != 0 { return Err("S31 SDMMC models immediate digital transfers, not wire timing".into()); }
        if !image.len().is_multiple_of(512 << 10) { return Err("an S31 SD card image must contain whole 512 KiB capacity units".into()); }
        let mut card = SdCard::with_config(image, config)?; let info = card.describe();
        card.set_log(self.host.logging()); self.host.reset_registers();
        self.host.devices.clear(); self.host.attach(Box::new(card));
        self.host.route = [None; 2]; self.host.card_detect_n = 2; Ok(info)
    }
    /// Disconnect the card without destroying its storage; the caller may retain the
    /// returned device. Any suspended transfer can no longer reach the removed card.
    pub fn eject(&mut self) -> Option<Box<dyn SdDevice>> {
        self.host.route = [None; 2]; self.host.card_detect_n = 3;
        if self.host.devices.is_empty() { None } else { Some(self.host.devices.remove(0)) }
    }
    pub fn present(&self) -> bool { !self.host.devices.is_empty() }
    pub fn pending(&self) -> bool { self.host.pending() }
    pub fn irq(&self) -> bool { self.reset & (1 << 30) == 0 && self.host.irq() }
    pub fn ciu_hz(&self, mpll_hz: Option<u64>) -> Option<u64> {
        // Only MPLL (selector 0) is supported by S31's v6.1 LL. Memory sleep under
        // software control blocks access; PMU-managed sleep belongs to the SoC.
        if self.clock & 13 != 9 || self.reset & (1 << 30) != 0
            || (self.memory & 12 == 12 && self.memory & 3 != 3) { return None; }
        if self.phase & PHASE_CLOCKS != PHASE_CLOCKS { return None; }
        let div = if self.clock & 2 != 0 { 1 } else { u64::from((self.phase >> 1) & 15) + 1 };
        mpll_hz.filter(|hz| *hz > 0).map(|hz| hz / div).filter(|hz| *hz > 0)
    }
    pub fn run(&mut self, memory: &mut dyn SdmmcMemory, now_ns: u64, board_slot: Option<([u8; 6], bool)>, mpll_hz: Option<u64>) {
        self.host.now_ns = now_ns; self.host.route = [None; 2]; self.host.ciu_hz = self.ciu_hz(mpll_hz).unwrap_or(0);
        if self.host.ciu_hz == 0 { return; } // A stopped controller cannot complete a command.
        let ctype = self.host.read(reg::CTYPE);
        let width = if ctype & (1 << 16) != 0 { 8 } else if ctype & 1 != 0 { 4 } else { 1 };
        let pads_ready = width <= 4 && self.pads.iter().all(|pad| pad & 1 == 0)
            && self.pads[5] & (1 << 8) != 0 && self.pads[..width.min(4)].iter().all(|pad| pad & (1 << 8) != 0);
        if board_slot == Some((SLOT0_PINS, true)) && self.present() && self.pad_ctrl & 1 != 0
            && pads_ready && self.host.read(reg::CLKENA) & 1 != 0 {
            self.host.route[0] = Some(0);
        }
        self.host.run(memory);
    }
}

#[cfg(test)]
mod tests;

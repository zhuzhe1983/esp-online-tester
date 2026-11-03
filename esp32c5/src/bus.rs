//! C5-specific map from ESP-IDF v5.5.4@735507283d5b2f9fb363a1901172dbd9e847945d:
//! soc/esp32c5/{include/soc/{soc,clic_reg,interrupts}.h,register/soc/{reg_base,gpio,uart,rmt}_reg.h}.
//! Unknown accesses fault instead of returning register RAM across absent devices.
use emu_core::{Bus, Fault};
use esp_periph::{Device, Gpio, RegRam, Uart, UartLayout, clic::ClicCore, rmt_compact::RmtCompact};
use esp_soc::{Board, NoBoard, pins::ChipPins};
pub const ROM_BASE: u32 = 0x4000_0000;
pub const ROM_SIZE: usize = 0x50000;
pub const SRAM_BASE: u32 = 0x4080_0000;
pub const SRAM_SIZE: usize = 0x60000;
pub const LP_BASE: u32 = 0x5000_0000;
pub const LP_SIZE: usize = 0x4000;
pub const CPU_HZ: u64 = 240_000_000; // nominal maximum; no dynamic clock or cycle-accuracy claim
pub const PINS: ChipPins = ChipPins { valid: (1 << 29) - 1, input_select: 0x100, output_mask: 0x3ff };
const SOURCES: usize = 84;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessFault { pub pc: u32, pub address: u32, pub write: bool, pub width: u8 }
pub struct C5Bus {
    pub rom: Vec<u8>, pub sram: Vec<u8>, pub lp_sram: Vec<u8>,
    pub uart: [Uart; 2], pub gpio: Gpio, pub io_mux: RegRam, pub rmt: RmtCompact,
    pub board: Board, pub clic: ClicCore, pub(crate) map: [u32; SOURCES], clic_config: u32, lp_ana_date: u32, pub(crate) reset_cause: u32,
    pub first_fault: Option<AccessFault>, pc: u32, pub cycles: u64,
    pub flash: Vec<u8>, pub(crate) app_off: Option<usize>, pub(crate) app_entry: Option<u32>,
    pub(crate) irq_dirty: bool, pub(crate) reset_requested: bool, pub(crate) misc: esp_periph::Misc,
    pub(crate) gpio_events: Option<Vec<(u64, u8, bool)>>, pub debug: esp_soc::DebugFlags,
}
impl Default for C5Bus { fn default() -> Self { Self::new() } }
impl C5Bus {
    pub fn new() -> Self {
        // C5 uart_reg.h: RX full/count 8 bits, CONF0_SYNC RXFIFO_RST bit22.
        let layout = UartLayout { thrhd_mask: 0xff, rxfifo_rst: 1 << 22, rxfifo_cnt_mask: 0xff };
        let mut gpio = Gpio::with_pins(29); gpio.input_select = 0x100; gpio.func_in_sel.fill(96); gpio.strap = 0;
        Self { rom: vec![0; ROM_SIZE], sram: vec![0; SRAM_SIZE], lp_sram: vec![0; LP_SIZE],
            uart: [Uart::new(layout), Uart::new(layout)], gpio, io_mux: RegRam::new(), rmt: RmtCompact::new(CPU_HZ),
            board: Box::new(NoBoard), clic: ClicCore::default(), map: [0; SOURCES], clic_config: 0, lp_ana_date: 36_774_528, reset_cause: 0x21, first_fault: None, pc: 0, cycles: 0,
            flash: Vec::new(), app_off: None, app_entry: None, irq_dirty: true, reset_requested: false, misc: Default::default(), gpio_events: None, debug: Default::default() }
    }
    pub fn attach_board(&mut self, board: Board) { self.board = board; self.publish_gpio(); }
    pub fn attach_board_devices(&mut self) { self.publish_gpio(); }
    pub(crate) fn publish_gpio(&mut self) {
        self.board.gpio_waveform_at(self.cycles, PINS.routes(&self.gpio, &self.io_mux));
        if let Some(events) = &mut self.gpio_events { events.extend(self.gpio.changes.drain(..).map(|(p, l)| (self.cycles, p, l))); }
        else { self.gpio.changes.clear(); }
        self.gpio.input_changes.clear();
    }
    fn range(address: u32, size: usize, base: u32, len: usize) -> Option<std::ops::Range<usize>> {
        let start = address.checked_sub(base)? as usize; let end = start.checked_add(size)?;
        (end <= len).then_some(start..end)
    }
    pub fn check_ram(&self, address: u32, size: usize) -> Result<(), String> {
        if Self::range(address, size, SRAM_BASE, SRAM_SIZE).is_some() || Self::range(address, size, LP_BASE, LP_SIZE).is_some() { Ok(()) }
        else { Err(format!("C5 SRAM range {address:#010x}+{size:#x} is unavailable")) }
    }
    pub fn load_ram(&mut self, address: u32, data: &[u8], size: usize) -> Result<(), String> {
        if data.len() > size { return Err("ELF file size exceeds memory size".into()); }
        self.check_ram(address, size)?;
        let (mem, range) = if let Some(r) = Self::range(address, size, SRAM_BASE, SRAM_SIZE) { (&mut self.sram, r) }
            else { (&mut self.lp_sram, Self::range(address, size, LP_BASE, LP_SIZE).ok_or("C5 LP SRAM range")?) };
        mem[range.clone()].fill(0); mem[range.start..range.start + data.len()].copy_from_slice(data); Ok(())
    }
    fn fail<T>(&mut self, address: u32, width: u8, write: bool, fault: Fault) -> Result<T, Fault> {
        self.first_fault.get_or_insert(AccessFault { pc: self.pc, address, write, width }); Err(fault)
    }
    // C5 GPIO has wider tables and relocated ENABLE/IN/STATUS/PIN registers.
    // Shared Gpio provides data/edge semantics only; no C6 addresses are decoded.
    fn gpio_offset(off: u32) -> Option<u32> {
        Some(match off {
            0x0 => 0x38, 0x4..=0x18 => off,
            0x34..=0x48 => off - 0x14, 0x64..=0x68 => off - 0x28,
            0x74..=0x88 => off - 0x30, 0xa4 => 0x5c, 0xac => 0x68,
            0xd4..=0x144 => off - 0x60,
            0x2d4..=0x4a4 => off - 0x180, // C5 header defines input signals 0..116
            0xad4..=0xb44 => off - 0x580,
            _ => return None,
        })
    }
    pub(crate) fn sources(&self) -> [(usize, bool); 4] { [(31, self.gpio.irq()), (47, self.uart[0].irq()), (48, self.uart[1].irq()), (55, self.rmt.rmt.irq())] }
    pub fn refresh_irq(&mut self, threshold: u32) -> Option<u32> {
        let mut input = 0;
        for (source, asserted) in self.sources() { let id = self.map[source] as usize; if asserted && (16..48).contains(&id) { input |= 1u64 << id; } }
        self.clic.set_input(input); self.clic.thresh = (threshold & 0xff) << 24;
        self.clic.candidate().filter(|&(id, _)| id >= 16).map(|(id, level)|
            (id as u32 - 16) | level << riscv_rv32::exec::CLIC_IRQ_LEVEL_SHIFT | if self.clic.vectored(id) { riscv_rv32::exec::CLIC_IRQ_SHV } else { 0 })
    }
    pub fn interrupt_taken(&mut self, line: u32) { self.clic.taken(line as usize + 16); }
    pub fn advance(&mut self, cycles: u64) {
        let before = self.rmt.rmt.irq();
        self.cycles = self.cycles.wrapping_add(cycles); self.rmt.tick(cycles);
        self.irq_dirty |= before != self.rmt.rmt.irq();
        for (ch, bits) in std::mem::take(&mut self.rmt.rmt.done) {
            for pin in PINS.routes(&self.gpio, &self.io_mux).output_pins(64 + ch as u32) { self.board.rmt_frame(pin, &bits); }
        }
    }
    fn mmio_read(&mut self, a: u32) -> Option<u32> {
        match a {
            // IDF v5.5.4 lp_analog_peri_reg.h DATE default; lp_clkrst_reg.h cause;
            // esp_rom/esp32c5/include/esp32c5/rom/rtc.h POWER_ON=1.
            0x600b_2ffc => Some(self.lp_ana_date), 0x600b_0410 => Some(self.reset_cause),
            0x6000_0000..=0x6000_0098 => Some(self.uart[0].read(a & 0xfff)),
            0x6000_1000..=0x6000_1098 => Some(self.uart[1].read(a & 0xfff)),
            0x6000_6000..=0x6000_6070 | 0x6000_6400..=0x6000_66fc => Some(self.rmt.read(a & 0xfff)),
            0x6009_0000..=0x6009_0074 => Some(self.io_mux.read(a & 0xfff)),
            0x6009_1000..=0x6009_1b44 => Self::gpio_offset(a & 0xfff).map(|o| self.gpio.read(o)),
            0x6001_0000..=0x6001_014c => Some(self.map[((a & 0xfff) / 4) as usize]),
            0x6001_0150..=0x6001_0158 => Some(self.sources().into_iter().filter(|&(s, on)| on && s / 32 == ((a - 0x6001_0150) / 4) as usize).fold(0, |v, (s, _)| v | 1 << (s % 32))),
            0x2080_0000 => Some(self.clic_config), 0x2080_0004 => Some(self.clic.read(4)),
            0x2080_1000..=0x2080_10bc => Some(self.clic.read(a - 0x2080_0000)),
            _ => None,
        }
    }
    fn mmio_write(&mut self, a: u32, value: u32, lanes: u32) -> bool {
        self.irq_dirty = true;
        if (0x2080_1000..=0x2080_10bc).contains(&a) { self.clic.write(a - 0x2080_0000, value, lanes); return true; }
        if lanes != 15 { return false; } // other subword MMIO deliberately unsupported
        match a {
            0x600b_2ffc => self.lp_ana_date = value & 0x7fff_ffff,
            // lp_clkrst_reg.h: CORE0_RESET_FLAG bit5 resets1; independent cause
            // clear bit29, flag SET bit30 and CLR bit31 are write triggers.
            0x600b_0410 => {
                if value & (1 << 29) != 0 { self.reset_cause &= !0x1f; }
                if value & (1 << 30) != 0 { self.reset_cause |= 1 << 5; }
                if value & (1 << 31) != 0 { self.reset_cause &= !(1 << 5); }
            }
            0x6000_0000..=0x6000_0098 => self.uart[0].write(a & 0xfff, value),
            0x6000_1000..=0x6000_1098 => self.uart[1].write(a & 0xfff, value),
            0x6000_6000..=0x6000_6070 | 0x6000_6400..=0x6000_66fc => { self.rmt.write(a & 0xfff, value); }
            0x6009_0000..=0x6009_0074 => { self.io_mux.write(a & 0xfff, value); if a & 0xfff >= 4 { self.gpio.set_pad(((a & 0xfff) / 4 - 1) as u8, value); } self.publish_gpio(); }
            0x6009_1000..=0x6009_1b44 => { if let Some(o) = Self::gpio_offset(a & 0xfff) { if o != 0x38 { self.gpio.write(o, value); } self.publish_gpio(); } else { return false; } }
            0x6001_0000..=0x6001_014c => self.map[((a & 0xfff) / 4) as usize] = value & 0x3f,
            0x6001_0150..=0x6001_0158 => {},
            0x2080_0000 => { self.clic_config = value & 0xf; self.clic.mnlbits = (value & 0xf) << 1; }
            _ => return false,
        }
        true
    }
    fn read(&mut self, address: u32, width: u8) -> Result<u32, Fault> {
        if address & (width as u32 - 1) != 0 { return self.fail(address, width, false, Fault::Misaligned); }
        for (base, mem) in [(ROM_BASE, &self.rom), (SRAM_BASE, &self.sram), (LP_BASE, &self.lp_sram)] {
            if let Some(r) = Self::range(address, width as usize, base, mem.len()) { let mut bytes = [0; 4]; bytes[..width as usize].copy_from_slice(&mem[r]); return Ok(u32::from_le_bytes(bytes)); }
        }
        if let Some(word) = self.mmio_read(address & !3) { return Ok((word >> ((address & 3) * 8)) & (u32::MAX >> (32 - width as u32 * 8))); }
        self.fail(address, width, false, Fault::Unmapped)
    }
    fn write(&mut self, address: u32, value: u32, width: u8) -> Result<(), Fault> {
        if address & (width as u32 - 1) != 0 { return self.fail(address, width, true, Fault::Misaligned); }
        for (base, mem) in [(SRAM_BASE, &mut self.sram), (LP_BASE, &mut self.lp_sram)] {
            if let Some(r) = Self::range(address, width as usize, base, mem.len()) { mem[r].copy_from_slice(&value.to_le_bytes()[..width as usize]); return Ok(()); }
        }
        let shift = address & 3;
        if self.mmio_write(address & !3, value << (shift * 8), ((1 << width) - 1) << shift) { return Ok(()); }
        self.fail(address, width, true, Fault::Unmapped)
    }
}
impl Bus for C5Bus {
    fn note_code_page(&mut self, _addr: u32) {} // interpreter-only; no decoded block cache
    fn note_pc(&mut self, pc: u32) { self.pc = pc; self.misc.cur_pc = pc; }
    fn block_break(&self) -> bool { self.irq_dirty }
    fn tick(&mut self, cycles: u32) -> u32 { self.advance(cycles as u64); u32::from(self.irq_dirty) }
    fn read8(&mut self, a: u32) -> Result<u8, Fault> { self.read(a, 1).map(|v| v as u8) }
    fn read16(&mut self, a: u32) -> Result<u16, Fault> { self.read(a, 2).map(|v| v as u16) }
    fn read32(&mut self, a: u32) -> Result<u32, Fault> { self.read(a, 4) }
    fn write8(&mut self, a: u32, v: u8) -> Result<(), Fault> { self.write(a, v as u32, 1) }
    fn write16(&mut self, a: u32, v: u16) -> Result<(), Fault> { self.write(a, v as u32, 2) }
    fn write32(&mut self, a: u32, v: u32) -> Result<(), Fault> { self.write(a, v, 4) }
    fn fetch(&mut self, pc: u32) -> Result<[u8; 4], Fault> {
        if ![(ROM_BASE, ROM_SIZE), (SRAM_BASE, SRAM_SIZE), (LP_BASE, LP_SIZE)].into_iter().any(|(base, len)| Self::range(pc, 2, base, len).is_some()) { return self.fail(pc, 2, false, Fault::Prohibited); }
        let lo = self.read16(pc)?; let hi = if lo & 3 == 3 { self.read16(pc.checked_add(2).ok_or(Fault::Unmapped)?)? } else { 0 };
        Ok((u32::from(lo) | u32::from(hi) << 16).to_le_bytes())
    }
}

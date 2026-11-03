//! Common Machine integration for explicit SRAM app-direct execution only.
use crate::{bus::{C5Bus, CPU_HZ, ROM_BASE, ROM_SIZE, SRAM_BASE, SRAM_SIZE, LP_BASE, LP_SIZE}, core};
use esp_soc::{BoardModel, Soc, SocBus};
use riscv_rv32::Cpu;
pub struct C5;
pub type Machine = esp_soc::Machine<C5>;
pub fn machine(mac: [u8; 6], flash_size: usize) -> Machine {
    let mut bus = C5Bus::new(); bus.set_flash_size(flash_size);
    let mut m = Machine::new(mac, bus); m.set_debug(&esp_soc::DebugFlags::from_env());
    m.dbg.stop_after_exceptions = 1; m
}
impl Soc for C5 {
    type Core = Cpu; type Bus = C5Bus;
    const NAME: &'static str = "esp32c5";
    const ROM_ELF: &'static str = "esp32c5_rev100_rom.elf";
    const CPU_HZ: u64 = CPU_HZ; const CORES: usize = 1; const IDLE_CHUNK: u64 = 64;
    const ROM_BOOT_DEFAULT: bool = false;
    const ROM_DATA_TABLE: &'static [&'static str] = &["_data_table_start"];
    const ROM_DATA_TABLE_END: &'static [&'static str] = &["_data_table_end"];
    const ROM_DATA_TABLE_STRIDE: u32 = 12;
    fn new_core(_i: usize) -> Cpu { core() }
    fn reset_core(c: &mut Cpu, _i: usize) { c.reset(); c.pc = ROM_BASE; }
    fn reset_core_with_bus(c: &mut Cpu, bus: &mut C5Bus, i: usize) {
        Self::reset_core(c, i); bus.irq_dirty = true;
        // Explicit app-direct mode's host restart recopies its selected image;
        // this is not a claim that the ROM bootloader executed during reset.
        if let Some(entry) = bus.app_entry { c.pc = entry; c.x[2] = SRAM_BASE + SRAM_SIZE as u32; }
    }
    fn boot_core(c: &mut Cpu, entry: u32) { Self::reset_core(c, 0); c.pc = entry; c.x[2] = SRAM_BASE + SRAM_SIZE as u32; }
    fn irqs(bus: &C5Bus, out: &mut [Option<u32>]) {
        out[0] = bus.clic.candidate().filter(|&(id, _)| id >= 16).map(|(id, level)|
            (id as u32 - 16) | level << riscv_rv32::exec::CLIC_IRQ_LEVEL_SHIFT | if bus.clic.vectored(id) { riscv_rv32::exec::CLIC_IRQ_SHV } else { 0 });
    }
    fn interrupt_taken(bus: &mut C5Bus, _core: usize, line: u32) { bus.interrupt_taken(line); bus.irq_dirty = true; }
    fn check_image(data: &[u8]) -> Result<(), String> { crate::image::parse(data).map(|_| ()) }
}
impl SocBus for C5Bus {
    fn cycles(&self) -> u64 { self.cycles }
    fn next_deadline(&self) -> Option<u64> { self.rmt.rmt.is_running().then_some(1) }
    fn irq_dirty(&mut self) -> &mut bool { &mut self.irq_dirty }
    fn refresh_irq(&mut self) -> bool { C5Bus::refresh_irq(self, 0); true } // core applies standard CSR threshold
    fn misc(&mut self) -> &mut esp_periph::Misc { &mut self.misc }
    fn load_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), String> {
        if data.len() <= ROM_SIZE && addr >= ROM_BASE && addr.checked_add(data.len() as u32).is_some_and(|end| end <= ROM_BASE + ROM_SIZE as u32) {
            let off = (addr - ROM_BASE) as usize; self.rom[off..off + data.len()].copy_from_slice(data); Ok(())
        } else { self.load_ram(addr, data, data.len()) }
    }
    fn write_flash(&mut self, offset: usize, data: &[u8]) -> Result<(), String> {
        let end = offset.checked_add(data.len()).filter(|&end| end <= self.flash.len()).ok_or("C5 app staging exceeds configured flash capacity")?;
        self.flash[offset..end].copy_from_slice(data); Ok(())
    }
    fn boot_app(&mut self, app_off: usize) -> Result<u32, String> {
        let data = self.flash.get(app_off..).ok_or("C5 app offset exceeds configured flash")?;
        let image = crate::image::parse(data)?;
        // Entire image is validated first; no guest SRAM changes on rejection.
        let copies: Vec<_> = image.segments.iter().map(|s| (s.load_addr, data[s.file_off as usize..(s.file_off + s.len) as usize].to_vec())).collect();
        for (addr, bytes) in copies { self.load_ram(addr, &bytes, bytes.len())?; }
        self.app_off = Some(app_off); self.app_entry = Some(image.entry); self.irq_dirty = true;
        Ok(image.entry)
    }
    fn reboot(&mut self, _mac: [u8; 6]) -> u32 {
        let mut fresh = C5Bus::new();
        std::mem::swap(&mut fresh.rom, &mut self.rom); std::mem::swap(&mut fresh.sram, &mut self.sram);
        std::mem::swap(&mut fresh.lp_sram, &mut self.lp_sram); std::mem::swap(&mut fresh.flash, &mut self.flash);
        std::mem::swap(&mut fresh.board, &mut self.board);
        fresh.gpio.restore_external(&self.gpio); fresh.gpio.strap = self.gpio.strap;
        fresh.cycles = self.cycles; fresh.debug = self.debug.clone(); fresh.reset_cause = self.reset_cause;
        fresh.gpio_events = self.gpio_events.as_ref().map(|_| Vec::new());
        fresh.app_off = self.app_off;
        if let Some(off) = self.app_off { if let Err(reason) = fresh.boot_app(off) { fresh.misc.log_unknown = true; eprintln!("[c5] app-direct restart rejected: {reason}"); } }
        fresh.publish_gpio(); *self = fresh; self.reset_cause & 0x1f
    }
    fn sw_reset(&self) -> bool { self.reset_requested }
    fn request_reset(&mut self, cause: u32) { self.reset_requested = true; self.reset_cause = cause & 0x1f | 1 << 5; }
    fn reset_cause(&self) -> u32 { self.reset_cause & 0x1f }
    fn last_fault(&self) -> Option<(u32, bool)> { self.first_fault.as_ref().map(|f| (f.address, f.write)) }
    fn console_take(&mut self) -> [Vec<u8>; 4] { [Vec::new(), std::mem::take(&mut self.uart[0].tx_out), std::mem::take(&mut self.uart[1].tx_out), Vec::new()] }
    fn serial_input(&mut self, _data: &[u8]) {} // USB Serial/JTAG is not implemented; never alias it to UART
    fn uart_input(&mut self, n: usize, data: &[u8]) { if let Some(uart) = self.uart.get_mut(n) { uart.host_input(data); self.irq_dirty = true; } }
    fn gpio_set_input(&mut self, pin: u8, level: bool) {
        if pin < 29 { self.irq_dirty |= self.gpio.set_input(pin, level); if let Some(events) = &mut self.gpio_events { events.push((self.cycles, pin, level)); } }
    }
    fn gpio_release_input(&mut self, pin: u8) { self.irq_dirty |= self.gpio.release_input(pin); }
    fn gpio_input(&self) -> u64 { self.gpio.input }
    fn gpio_state(&self, pin: u8) -> Option<esp_soc::GpioState> {
        (pin < 29).then(|| { let mux = self.io_mux.read(4 + 4 * u32::from(pin)); esp_soc::GpioState::new(self.gpio.out & (1 << pin) != 0, self.gpio.enable & (1 << pin) != 0, mux & (1 << 8) != 0, mux & (1 << 7) != 0) })
    }
    fn observe_gpio(&mut self, on: bool) { self.gpio_events = on.then(Vec::new); }
    fn take_gpio_events(&mut self) -> Vec<(u64, u8, bool)> { self.gpio_events.as_mut().map(std::mem::take).unwrap_or_default() }
    fn board(&mut self) -> &mut dyn BoardModel { &mut *self.board }
    fn board_ref(&self) -> &dyn BoardModel { &*self.board }
    fn audio(&self) -> (&[i16], u32) { (&[], 0) }
    fn irq_sources_of(&self, _core: usize, line: u32) -> Vec<usize> { self.sources().into_iter().filter(|&(source, active)| active && self.map[source] == line + 16).map(|(source, _)| source).collect() }
    fn set_debug(&mut self, f: &esp_soc::DebugFlags) { self.debug = f.clone(); }
    fn set_flash_size(&mut self, bytes: usize) { self.flash.resize(bytes.min(32 << 20), 0xff); }
    fn set_psram_size(&mut self, bytes: usize) -> Result<(), String> { if bytes == 0 { Ok(()) } else { Err("C5 SRAM app-direct profile has no PSRAM".into()) } }
    fn set_strap(&mut self, v: u32) { self.gpio.strap = v & 0xffff; }
    fn set_reset_cause(&mut self, cause: u32) { self.reset_cause = (self.reset_cause & !0x1f) | cause & 0x1f; }
    fn refresh_board_outputs(&mut self) { self.publish_gpio(); }
    fn report(&self) -> String { format!("[c5] SRAM app-direct; ROM/flash-MMU/IDF boot unimplemented; HP={SRAM_SIZE}B LP={LP_SIZE}B at{LP_BASE:#x}; {}", self.board.report()) }
}

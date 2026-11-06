//! ESP32-C61 Machine integration.
use crate::bus::{SocBus, FLASH_HIGH, FLASH_LOW, MMU_ENTRIES, MMU_VALID};
use crate::periph::{self, src};
use esp_periph::Misc;
use esp_soc::{BoardModel, Soc};
use riscv_rv32::Cpu;

pub struct C61;
pub type Machine = esp_soc::Machine<C61>;

pub fn machine(mac: [u8; 6], flash_size: usize) -> Machine { let mut m = Machine::new(mac, SocBus::new(flash_size, mac)); m.set_debug(&esp_soc::DebugFlags::from_env()); m }

impl Soc for C61 {
    type Core = Cpu;
    type Bus = SocBus;
    const NAME: &'static str = "esp32c61";
    const ROM_ELF: &'static str = "esp32c61_rev100_rom.elf";
    const CPU_HZ: u64 = periph::CPU_HZ;
    const CORES: usize = 1;
    const IDLE_CHUNK: u64 = 64;

    const ROM_DATA_TABLE: &'static [&'static str] = &["_data_table_start"];
    const ROM_DATA_TABLE_END: &'static [&'static str] = &["_data_table_end"];
    const ROM_DATA_TABLE_STRIDE: u32 = 12;
    fn new_core(_i: usize) -> Cpu { let mut c = Cpu::new_rv32imac(); c.clic = true; c.clic_std = true; c.user_counters = true; c }

    fn core_state(bus: &SocBus, _core: usize) -> esp_soc::CoreState { if bus.controller_powered { esp_soc::CoreState::Running } else { esp_soc::CoreState::Held } }
    fn reset_core(c: &mut Cpu, _i: usize) { Cpu::reset(c); }
    fn boot_core(c: &mut Cpu, entry: u32) {
        Cpu::reset(c);
        c.pc = entry;
        c.x[2] = 0x4084_E000;
    }
    fn irqs(bus: &SocBus, out: &mut [Option<u32>]) { out[0] = bus.periph.intc.clic.candidate().filter(|&(id, _)| id >= 16).map(|(id, level)| (id as u32 - 16) | level << riscv_rv32::exec::CLIC_IRQ_LEVEL_SHIFT | if bus.periph.intc.clic.vectored(id) { riscv_rv32::exec::CLIC_IRQ_SHV } else { 0 }); }
    fn interrupt_taken(bus: &mut SocBus, _core: usize, line: u32) { bus.periph.intc.clic.taken(line as usize + 16); bus.irq_dirty = true; }
    fn check_image(data: &[u8]) -> Result<(), String> {
        // ESP-IDF v5.5.4 bootloader_support/include/esp_app_format.h.
        let h = data.get(..24).ok_or("C61 requires a complete ESP image header")?;
        if u16::from_le_bytes([h[12], h[13]]) != 20 { return Err("Image is not ESP32-C61 (chip ID 20)".into()); }
        let min = u16::from_le_bytes([h[15], h[16]]); let max = u16::from_le_bytes([h[17], h[18]]);
        if min > 100 || (max != 0 && max < 100) { return Err("C61 models revision 1.0; the image excludes revision 100".into()); }
        esp_soc::image::parse(data).map(|_| ())
    }
}

impl esp_soc::SocBus for SocBus {
    fn attach_wifi(&mut self, cfg: esp_soc::wifi::ApConfig, nat: Option<esp_soc::nat::Nat>) -> Result<(), String> { self.periph.wifi_mac.link.attach_new(cfg, nat, &self.debug); Ok(()) }
    fn configure_softap_client(&mut self,cfg:Option<esp_soc::wifi::softap::ClientConfig>)->Result<(),String> {self.periph.wifi_mac.link.configure_softap(cfg)}
    fn poll_softap_client(&mut self) {self.periph.wifi_mac.link.poll_softap(self.cycles/(periph::CPU_HZ/1_000_000));}
    fn softap_client_status(&mut self)->String {self.periph.wifi_mac.link.softap_status()}
    fn take_softap_frames(&mut self)->Vec<Vec<u8>> {self.periph.wifi_mac.link.take_softap_frames()}
    fn receive_softap_frame(&mut self,frame:&[u8])->Result<(),String> {self.periph.wifi_mac.link.receive_softap_frame(frame,self.cycles/(periph::CPU_HZ/1_000_000))}
    fn set_ethernet_relay(&mut self, enabled: bool) -> Result<(), String> { self.periph.wifi_mac.link.set_relay(enabled); Ok(()) }
    fn take_ethernet_frames(&mut self) -> Vec<Vec<u8>> { self.periph.wifi_mac.link.take_relay_frames() }
    fn receive_ethernet_frame(&mut self, frame: &[u8]) -> Result<(), String> { self.periph.wifi_mac.link.receive_relay_frame(frame) }


    fn cycles(&self) -> u64 { self.cycles }
    fn next_deadline(&self) -> Option<u64> {
        let timer = match self.periph.cycles_until_timer() { u32::MAX => None, cycles => Some(cycles.max(1) as u64) };
        let board = self.board.next_deadline().map(|at| at.saturating_sub(self.cycles).max(1));
        timer.into_iter().chain(board).min()
    }
    fn irq_dirty(&mut self) -> &mut bool { &mut self.irq_dirty }
    fn refresh_irq(&mut self) -> bool { self.periph.refresh_lines(); true }

    fn misc(&mut self) -> &mut Misc { &mut self.periph.misc }
    fn load_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), String> { SocBus::load_bytes(self, addr, data) }
    fn write_flash(&mut self, offset: usize, data: &[u8]) -> Result<(), String> { SocBus::write_flash(self, offset, data) }

    fn boot_app(&mut self, app_off: usize) -> Result<u32, String> {
        let image = self.flash.get(app_off..).ok_or("app offset beyond flash")?;
        C61::check_image(image)?;
        let img = esp_soc::image::parse(image)?;
        let shift = self.page_shift();
        for s in &img.segments {
            let start = app_off + s.file_off as usize;
            let end = start + s.len as usize;
            if end > self.flash.len() { return Err("segment beyond flash".into()); }
            if (FLASH_LOW..FLASH_HIGH).contains(&s.load_addr) {
                let mask = (1u32 << shift) - 1;
                if (s.load_addr & mask) != (start as u32 & mask) {
                    return Err(format!("segment {:#x} not page-aligned with flash offset {:#x}", s.load_addr, start));
                }
                let window_end = FLASH_LOW + ((MMU_ENTRIES as u32) << shift);
                if !s.load_addr.checked_add(s.len).is_some_and(|end| end <= window_end) {
                    return Err("segment beyond the flash window".into());
                }
                let first_page = (start as u32) >> shift;
                let npages = ((s.load_addr & mask) + s.len + mask) >> shift;
                for i in 0..npages {
                    let idx = (((s.load_addr - FLASH_LOW) >> shift) + i) as usize;
                    self.mmu[idx] = MMU_VALID | ((first_page + i) & 0x1ff);
                }
            } else {
                let data = self.flash[start..end].to_vec();
                SocBus::load_bytes(self, s.load_addr, &data)?;
            }
        }
        Ok(img.entry)
    }

    fn reboot(&mut self, mac: [u8; 6]) -> u32 {

        let cause = self.periph.lpsys.reset_cause;
        let old = std::mem::replace(&mut self.periph, periph::Peripherals::new(mac));
        let p = &mut self.periph;
        p.efuse = old.efuse;
        p.wifi_mac.link = old.wifi_mac.link.surviving_reboot();
        p.adc.analog = old.adc.analog; p.adc.analog.set_stream_clock_offset(self.cycles);
        p.spi0.0.has_psram = !self.psram.is_empty(); p.spi1.0.has_psram = !self.psram.is_empty();

        p.misc.log_unknown = old.misc.log_unknown;
        p.usb.connected = old.usb.connected;

        p.lpsys = old.lpsys;
        p.lpsys.sw_reset = false;
        p.lpsys.reset_cause = cause;

        p.spi0.0.jedec = old.spi0.0.jedec;
        p.spi1.0.jedec = old.spi1.0.jedec;
        p.gpio.restore_external(&old.gpio);
        p.gpio.strap = old.gpio.strap;

        self.mmu = [0; MMU_ENTRIES];
        self.mmu_index = 0;
        self.mmu_power_ctrl = 0;
        self.attach_board_devices();
        for area in self.debug.iter() { esp_periph::Dispatch::debug(&mut self.periph, area, true); }
        self.periph.misc.log_all = self.debug.has("mmio");
        cause
    }
    fn sw_reset(&self) -> bool { self.periph.lpsys.sw_reset }
    fn request_reset(&mut self, cause: u32) { self.periph.lpsys.sw_reset = true; self.periph.lpsys.reset_cause = cause; }
    fn reset_cause(&self) -> u32 { self.periph.lpsys.reset_cause }
    fn last_fault(&self) -> Option<(u32, bool)> { self.last_fault }
    fn console_take(&mut self) -> [Vec<u8>; 4] { [std::mem::take(&mut self.periph.usb.tx_out), std::mem::take(&mut self.periph.uart[0].tx_out), std::mem::take(&mut self.periph.uart[1].tx_out), std::mem::take(&mut self.periph.uart[2].tx_out)] }
    fn serial_input(&mut self, data: &[u8]) {
        let before = self.periph.usb.irq();
        self.periph.usb.host_input(data);
        self.irq_dirty |= before != self.periph.usb.irq();
    }
    fn uart_input(&mut self, n: usize, data: &[u8]) {
        let Some(u) = self.periph.uart.get_mut(n) else { return };
        let before = u.irq();
        u.host_input(data);
        self.irq_dirty |= before != u.irq();
    }

    fn analog_set(&mut self, pin: u8, source: esp_periph::AnalogSource) { if crate::adc::PINS.contains(&pin) { self.periph.adc.analog.set(pin, source); } }
    fn adc_set_raw(&mut self, pin: u8, raw: u16) -> bool { crate::adc::PINS.contains(&pin) && self.periph.adc.analog.set_raw(pin, raw) }
    fn adc_observation(&self, pin: u8) -> Option<esp_periph::AdcObservation> { crate::adc::PINS.contains(&pin).then(|| self.periph.adc.analog.observation(pin)) }
    fn gpio_release_input(&mut self, pin: u8) {
        self.irq_dirty |= esp_soc::gpio::release_and_report(&mut self.periph.gpio, &mut self.gpio_events, self.cycles, pin);
    }
    fn gpio_set_input(&mut self, pin: u8, level: bool) {
        self.irq_dirty |= self.periph.gpio.set_input(pin, level);
        if let Some(ev) = &mut self.gpio_events { ev.push((self.cycles, pin, level)); }
    }
    fn set_flash_size(&mut self, bytes: usize) {
        self.flash = vec![0xff; bytes];
        let cap = bytes.trailing_zeros() as u8; self.periph.spi1.0.jedec[2] = cap; self.periph.spi0.0.jedec[2] = cap;
    }
    fn set_psram_size(&mut self, bytes: usize) -> Result<(), String> {
        if !matches!(bytes, 0 | 0x20_0000 | 0x40_0000 | 0x80_0000) { return Err("C61 Quad PSRAM supports 0, 2, 4 or 8 MiB".into()); }
        self.psram.resize(bytes, 0); self.periph.spi0.0.has_psram = bytes != 0; self.periph.spi1.0.has_psram = bytes != 0; Ok(())
    }
    fn attach_sd_card(&mut self, image: Box<dyn esp_periph::sdmmc::SdStore>, config: esp_periph::sdmmc::SdCardConfig) -> Result<String, String> { self.board.attach_spi_sd_card(image, config) }
    fn set_strap(&mut self, v: u32) { self.periph.gpio.strap = v; }
    fn set_reset_cause(&mut self, c: u32) { self.periph.lpsys.reset_cause = c; }
    fn set_debug(&mut self, f: &esp_soc::DebugFlags) {
        self.debug = f.clone();
        for area in f.iter() { esp_periph::Dispatch::debug(&mut self.periph, area, true); }
        self.periph.misc.log_all = f.has("mmio");
    }
    fn observe_gpio(&mut self, on: bool) { self.gpio_events = if on { Some(Vec::new()) } else { None }; }
    fn take_gpio_events(&mut self) -> Vec<(u64, u8, bool)> { self.gpio_events.as_mut().map(std::mem::take).unwrap_or_default() }
    fn gpio_input(&self) -> u64 { self.periph.gpio.input }
    fn gpio_state(&self, pin: u8) -> Option<esp_soc::GpioState> {
        if pin >= 30 { return None; }
        let mux = self.periph.io_mux.read(4 + u32::from(pin) * 4);
        Some(esp_soc::GpioState::new(
            self.periph.gpio.out & (1u64 << pin) != 0,
            self.periph.gpio.enable & (1u64 << pin) != 0,
            mux & (1 << 8) != 0,
            mux & (1 << 7) != 0,
        ))
    }
    fn pwm_output(&self, pin: u8) -> Option<(f64, u32)> { self.periph.pwm_output(pin) }

    fn board(&mut self) -> &mut dyn BoardModel { &mut *self.board }
    fn board_ref(&self) -> &dyn BoardModel { &*self.board }

    fn audio(&self) -> (&[i16], u32) { self.board.audio() }
    fn clear_audio_capture(&mut self) { self.board.clear_audio_capture(); }
    fn report(&self) -> String { format!("{}\n{}", self.board.report(), self.periph.wifi_mac.link.report()) }
    fn irq_sources_of(&self, _core: usize, line: u32) -> Vec<usize> { (0..src::COUNT).filter(|&s| self.periph.intmtx.map[s] == line + 16).collect() }

}
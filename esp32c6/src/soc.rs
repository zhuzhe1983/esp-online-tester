//! The ESP32-C6 as a `Soc`: one RV32IMAC core on `bus::SocBus`, a bare module (no board yet).
use crate::bus::{SocBus, FLASH_HIGH, FLASH_LOW, MMU_ENTRIES, MMU_VALID};
use crate::periph::{self, src};
use esp_periph::Misc;
use esp_soc::{BoardModel, Soc};
use riscv_rv32::Cpu;

pub struct C6;
pub type Machine = esp_soc::Machine<C6>;

pub fn machine(mac: [u8; 6], flash_size: usize) -> Machine { let mut m = Machine::new(mac, SocBus::new(flash_size, mac)); m.set_debug(&esp_soc::DebugFlags::from_env()); m }

impl Soc for C6 {
    type Core = Cpu;
    type Bus = SocBus;
    const NAME: &'static str = "esp32c6";
    const ROM_ELF: &'static str = "esp32c6_rev0_rom.elf";
    const CPU_HZ: u64 = periph::CPU_HZ;
    const CORES: usize = 1;
    const IDLE_CHUNK: u64 = 64;
    /// the C6 ROM's initialiser table: 12-byte (dst_start, dst_end, rom_src) entries
    const ROM_DATA_TABLE: &'static [&'static str] = &["_data_table_start"];
    const ROM_DATA_TABLE_END: &'static [&'static str] = &["_data_table_end"];
    const ROM_DATA_TABLE_STRIDE: u32 = 12;
    fn new_core(_i: usize) -> Cpu { Cpu::new_rv32imac() }
    fn function_hooks(bus: &SocBus) -> &[u32] { &bus.ble.hooks }
    fn function_hook(core: &mut Cpu, bus: &mut SocBus) -> bool { crate::ble::intercept(core, bus) }
    fn reset_core(c: &mut Cpu, _i: usize) { Cpu::reset(c); }
    fn boot_core(c: &mut Cpu, entry: u32) {
        Cpu::reset(c);
        c.pc = entry;
        c.x[2] = 0x4087_E000;                 // a stack the bootloader would have left us
    }
    fn irqs(bus: &SocBus, out: &mut [Option<u32>]) { out[0] = bus.periph.intc.lines.pending(); }
}

impl esp_soc::SocBus for SocBus {
    fn enable_ble(&mut self, elf: &esp_soc::elf::Elf) -> Result<(), String> { self.ble.enable(&elf.by_name, elf.symbol_sizes.get("esp_bt_controller_init").copied().unwrap_or(0), elf.idf_version().unwrap_or("")) }
    fn ble_enabled(&self) -> bool { !self.ble.hooks.is_empty() }
    fn ble_pending_commands(&self) -> usize { self.ble.session.pending_commands() }
    fn ble_command(&mut self, command: &str) -> Result<(), String> { self.ble.command(command, self.cycles) }
    fn ble_nearby(&mut self, list: Vec<esp_soc::ble::peer::Nearby>) -> Result<(), String> { self.ble.set_nearby(list, self.cycles) }
    fn attach_wifi(&mut self, cfg: esp_soc::wifi::ApConfig, nat: Option<esp_soc::nat::Nat>) -> Result<(), String> {
        self.periph.wifi_mac.link.attach_new(cfg, nat, &self.debug);
        Ok(())
    }
    fn set_ethernet_relay(&mut self, enabled: bool) -> Result<(), String> {
        self.periph.wifi_mac.link.set_relay(enabled);
        Ok(())
    }
    fn take_ethernet_frames(&mut self) -> Vec<Vec<u8>> {
        self.periph.wifi_mac.link.take_relay_frames()
    }
    fn receive_ethernet_frame(&mut self, frame: &[u8]) -> Result<(), String> {
        self.periph.wifi_mac.link.receive_relay_frame(frame)
    }
    fn cycles(&self) -> u64 { self.cycles }
    fn next_deadline(&self) -> Option<u64> {
        let timer = match self.periph.cycles_until_timer() { u32::MAX => None, cycles => Some(cycles.max(1) as u64) };
        let board = self.board.next_deadline().map(|at| at.saturating_sub(self.cycles).max(1));
        let deadline = timer.into_iter().chain(board).min();
        // work_pending is false on idle rounds; it covers both SPI and RX.
        if self.periph.work_pending && self.periph.optional.i2s0.rx_running() { Some(deadline.unwrap_or(u64::MAX).min(256)) } else { deadline }
    }
    fn irq_dirty(&mut self) -> &mut bool { &mut self.irq_dirty }
    fn refresh_irq(&mut self) -> bool { self.periph.refresh_lines(); true }
    fn take_host_event(&mut self) -> bool { self.periph.radio.take_tx_started() }
    fn misc(&mut self) -> &mut Misc { &mut self.periph.misc }
    fn load_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), String> { SocBus::load_bytes(self, addr, data) }
    fn write_flash(&mut self, offset: usize, data: &[u8]) -> Result<(), String> { SocBus::write_flash(self, offset, data) }
    /// Copy the RAM segments and map the flash-resident ones through the MMU, as the 2nd-stage
    /// bootloader would. One flat window for code and data makes this straightforward here;
    /// the system registers the bootloader would have set up are not preset, so this is a
    /// shortcut for firmware that does not depend on them.
    fn boot_app(&mut self, app_off: usize) -> Result<u32, String> {
        let image = self.flash.get(app_off..).ok_or("app offset beyond flash")?;
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
    /// Digital peripherals re-created; SRAM, the LP domain and the efuses kept.
    fn reboot(&mut self, mac: [u8; 6]) -> u32 {
        esp_soc::ble::restore_patched_flash(&mut self.ble.original_flash, &mut self.flash);
        self.ble.reset();
        let cause = self.periph.lpsys.reset_cause;
        let mut old = std::mem::replace(&mut self.periph, periph::Peripherals::new(mac));
        let p = &mut self.periph;
        p.efuse = old.efuse;
        p.optional.i2s0.keep_rx_input(&mut old.optional.i2s0);
        p.adc.analog = old.adc.analog;
        p.optional.sdio.stats = old.optional.sdio.stats;   // counts for the whole run
        p.adc.analog.set_stream_clock_offset(self.cycles); // the new peripheral clock starts at zero
        p.misc.log_unknown = old.misc.log_unknown;
        p.usb.connected = old.usb.connected;
        // The LP domain is not reset by a CPU or system reset: the STORE registers, the RTC
        // timer and the watchdog keep running, and the ROM reads the cause from LP_CLKRST.
        p.lpsys = old.lpsys;
        p.lpsys.sw_reset = false;
        p.lpsys.reset_cause = cause;
        p.assist_debug.saved_pc = old.misc.cur_pc;   // where the core was when the reset took effect
        // The flash chip is on the board, not in the chip: its JEDEC capacity survives a reset.
        p.spi0.0.jedec = old.spi0.0.jedec;
        p.spi1.0.jedec = old.spi1.0.jedec;
        p.gpio.restore_external(&old.gpio);
        p.gpio.strap = old.gpio.strap;      // strapping pins are board wiring, not chip state
        // The access point and the network behind it are the world outside the chip.
        p.wifi_mac.link = old.wifi_mac.link.surviving_reboot();
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
    fn console_take(&mut self) -> [Vec<u8>; 4] { [std::mem::take(&mut self.periph.usb.tx_out), std::mem::take(&mut self.periph.uart[0].tx_out), std::mem::take(&mut self.periph.uart[1].tx_out), Vec::new()] }
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
    fn analog_set(&mut self, pin: u8, src: esp_periph::AnalogSource) {
        let offset = self.peripheral_clock_offset();
        self.periph.adc.analog.set(pin, src);
        self.periph.adc.analog.set_stream_clock_offset(offset);
    }
    fn adc_set_raw(&mut self, pin: u8, raw: u16) -> bool { pin <= 6 && self.periph.adc.analog.set_raw(pin, raw) }
    fn adc_observation(&self, pin: u8) -> Option<esp_periph::AdcObservation> {
        (pin <= 6).then(|| self.periph.adc.analog.observation(pin))
    }
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
        if pin > 30 { return None; }
        let mux = self.periph.io_mux.read(4 + u32::from(pin) * 4);
        Some(esp_soc::GpioState::new(
            self.periph.gpio.out & (1u64 << pin) != 0,
            self.periph.gpio.enable & (1u64 << pin) != 0,
            mux & (1 << 8) != 0,
            mux & (1 << 7) != 0,
        ))
    }
    fn pwm_output(&self, pin: u8) -> Option<(f64, u32)> { self.periph.ledc.output(&self.periph.gpio, pin).or_else(|| self.periph.mcpwm.output(&self.periph.gpio, pin)) }

    fn board(&mut self) -> &mut dyn BoardModel { &mut *self.board }
    fn board_ref(&self) -> &dyn BoardModel { &*self.board }
    fn i2s_input(&mut self, port: usize) -> Option<&mut esp_periph::i2s::PcmInput> { if port == 0 { Some(self.periph.optional.i2s0.rx_input()) } else { None } }
    fn pcm_sources(&self) -> Option<&esp_periph::i2s::PcmSources> { self.periph.optional.i2s0.pcm_bank_ref() }
    fn attach_pcm_sources(&mut self) -> Option<&mut esp_periph::i2s::PcmSources> {
        self.flush_ticks();
        Some(esp_soc::soc::attach_pcm_sources(self.periph.optional.i2s0.pcm_bank(), self.cycles, periph::CPU_HZ))
    }
    fn audio(&self) -> (&[i16], u32) { (&[], 44100) }
    fn irq_sources_of(&self, _core: usize, line: u32) -> Vec<usize> { (0..src::COUNT).filter(|&s| self.periph.intmtx.map[s] == line).collect() }
    fn report(&self) -> String {
        let mut r = Vec::new();
        if self.periph.spi2.transfers > 0 { r.push(format!("[emu] spi2: {} transfers", self.periph.spi2.transfers)); }
        if self.periph.rmt.rmt.tx_count > 0 { r.push(format!("[emu] rmt: {} transmissions", self.periph.rmt.rmt.tx_count)); }
        if self.periph.radio.scans > 0 { r.push(format!("[emu] 802.15.4: {} energy scans, last channel {} = {} dBm", self.periph.radio.scans, self.periph.radio.channel(), self.periph.radio.ed_rss)); }
        let b = self.board.report(); if !b.is_empty() { r.push(b); }
        let wifi = self.periph.wifi_mac.link.report();
        if !wifi.is_empty() { r.push(wifi); }
        r.join("\n")
    }
}

//! The ESP32-S3 as a `Soc`: two LX7 cores on `bus::SocBus`, and the questions the machine asks
//! its bus (console, reset, app boot, board, audio, interrupt routing).
use crate::bus::{SocBus, DBUS_HIGH, DBUS_LOW, IBUS_HIGH, IBUS_LOW, MMU_ENTRIES, MMU_INVALID, SRC_FLASH};
use crate::periph::{self, NUM_SOURCES};
use esp_periph::Misc;
use esp_soc::{BoardModel, CoreState, Soc};
use xtensa_lx7::state::ps;
use xtensa_lx7::Cpu;

pub struct S3;
pub type Machine = esp_soc::Machine<S3>;

/// A machine with the default 8 MB flash / 2 MB PSRAM and no board (`bus.board` selects one).
pub fn machine(mac: [u8; 6]) -> Machine { let mut m = Machine::new(mac, SocBus::new(8 << 20, 2 << 20, mac)); m.set_debug(&esp_soc::DebugFlags::from_env()); m }

impl Soc for S3 {
    type Core = Cpu;
    type Bus = SocBus;
    const NAME: &'static str = "esp32s3";
    const ROM_ELF: &'static str = "esp32s3_rev0_rom.elf";
    const CPU_HZ: u64 = periph::CPU_HZ;
    const CORES: usize = 2;
    const IDLE_CHUNK: u64 = 64 * 8;
    const ROM_DATA_TABLE: &'static [&'static str] = &["_data_start"];
    fn new_core(i: usize) -> Cpu { Cpu::new(if i == 0 { 0xCDCD } else { 0xABAB }) }
    fn new_core_with_bus(i: usize, bus: &SocBus) -> Cpu {
        let mut core = Self::new_core(i);
        core.fetch_cache = bus.fetch_cache.clone();
        core
    }
    fn function_hooks(bus: &SocBus) -> &[u32] { &bus.ble.hooks }
    fn function_hook(core: &mut Cpu, bus: &mut SocBus) -> bool { crate::ble::intercept(core, bus) }
    fn reset_core(c: &mut Cpu, i: usize) { Cpu::reset(c); if i == 1 { c.prid = 0xABAB; } }
    #[inline]
    fn reset_core_with_bus(c: &mut Cpu, bus: &mut SocBus, i: usize) {
        Self::reset_core(c, i);
        // Reset clears external levels and edge history even if no source changed.
        // Force the existing routing refresh past its unchanged-source filter once.
        bus.irq_dirty = true;
        bus.periph.intmatrix_dirty = true;
    }
    fn boot_core(c: &mut Cpu, entry: u32) {
        Cpu::reset(c);
        c.pc = entry;
        c.ps = ps::WOE | ps::UM;      // windows enabled, user vector; INTLEVEL 0
        c.vecbase = 0x4000_0000;
        c.set_ar(1, 0x3FCE_B000);     // bootloader stack (in DRAM, app treats as free)
        c.set_ar(0, 0);
    }
    fn irqs(bus: &SocBus, out: &mut [u32]) { let (l0, l1) = bus.periph.cpu_lines_both(); out[0] = l0; out[1] = l1; }
    /// Core 0 is always running. SYSTEM_CORE_1_CONTROL_0 controls core 1's clock gate, reset,
    /// and run-stall state.
    fn core_state(bus: &SocBus, core: usize) -> CoreState {
        if core == 0 { return CoreState::Running; }
        let (clk, reset, stall) = bus.periph.core1_control();
        if reset { CoreState::Reset } else if clk && !stall { CoreState::Running } else { CoreState::Held }
    }
}

impl esp_soc::SocBus for SocBus {
    fn enable_ble(&mut self, elf: &esp_soc::elf::Elf) -> Result<(), String> { self.ble.enable(&elf.by_name, elf.symbol_sizes.get("esp_bt_controller_init").copied().unwrap_or(0), &<Self as esp_soc::ble::vhci::VhciBus>::abi()) }
    fn ble_enabled(&self) -> bool { !self.ble.hooks.is_empty() }
    fn ble_pending_commands(&self) -> usize { self.ble.session.pending_commands() }
    fn ble_command(&mut self, command: &str) -> Result<(), String> { self.ble.command(command, self.cycles, periph::CPU_HZ) }
    fn ble_nearby(&mut self, list: Vec<esp_soc::ble::peer::Nearby>) -> Result<(), String> { self.ble.set_nearby(list, self.cycles, periph::CPU_HZ) }
    fn attach_wifi(&mut self, cfg: esp_soc::wifi::ApConfig, nat: Option<esp_soc::nat::Nat>) -> Result<(), String> {
        self.flush_ticks();
        self.periph.wifi.link.attach_new(cfg, nat, &self.debug);
        self.refresh_tick_budget();
        Ok(())
    }
    fn set_ethernet_relay(&mut self, enabled: bool) -> Result<(), String> {
        self.flush_ticks();
        self.periph.wifi.link.set_relay(enabled);
        Ok(())
    }
    fn take_ethernet_frames(&mut self) -> Vec<Vec<u8>> {
        self.flush_ticks();
        self.periph.wifi.link.take_relay_frames()
    }
    fn receive_ethernet_frame(&mut self, frame: &[u8]) -> Result<(), String> {
        self.flush_ticks();
        self.periph.wifi.link.receive_relay_frame(frame)
    }
    fn cycles(&self) -> u64 { self.cycles }
    fn next_deadline(&self) -> Option<u64> { Some(SocBus::next_deadline(self)) }
    fn irq_dirty(&mut self) -> &mut bool { &mut self.irq_dirty }
    // Only the WASM JIT helpers currently stop before deferred device accesses.
    // `ESP32SIM_VQ_NATIVE` opts a native run in; it is only sound with `--no-jit`.
    fn can_defer(&self) -> bool { cfg!(target_arch = "wasm32") || std::env::var_os("ESP32SIM_VQ_NATIVE").is_some() }
    fn set_defer(&mut self, on: bool) { self.defer_mmio = on; self.mmio_deferred = false; }
    fn take_deferred(&mut self) -> bool { std::mem::take(&mut self.mmio_deferred) }
    fn refresh_irq(&mut self) -> bool {
        // Inputs are sampled at refresh boundaries. Clearing and reasserting a source
        // between samples cannot create a second edge at the CPU.
        let dirty = self.periph.lines_dirty() || self.periph.intmatrix_dirty;
        self.periph.intmatrix_dirty = false;
        dirty
    }
    fn flush_ticks(&mut self) { SocBus::flush_ticks(self) }
    fn touch_input(&mut self, x: u16, y: u16, down: bool) {
        self.board.touch_at(self.cycles, x, y, down);
        if self.board.next_deadline().is_some() { self.refresh_tick_budget(); }
    }
    fn misc(&mut self) -> &mut Misc { &mut self.periph.misc }
    fn load_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), String> { SocBus::load_bytes(self, addr, data) }
    fn write_flash(&mut self, offset: usize, data: &[u8]) -> Result<(), String> {
        let target = self.flash.get_mut(offset..).and_then(|tail| tail.get_mut(..data.len()))
            .ok_or("flash image too large")?;
        target.copy_from_slice(data);
        self.note_written(SRC_FLASH, offset, data.len());
        Ok(())
    }
    /// Copy IRAM/DRAM segments, map IROM/DROM through the MMU, as the 2nd-stage bootloader would.
    fn boot_app(&mut self, app_off: usize) -> Result<u32, String> {
        let image = self.flash.get(app_off..).ok_or("app offset beyond flash")?;
        let img = esp_soc::image::parse(image)?;
        self.periph.system.preset_after_bootloader();
        self.periph.rtc.preset_after_bootloader();
        for s in &img.segments {
            let start = app_off + s.file_off as usize;
            let end = start + s.len as usize;
            if end > self.flash.len() { return Err("segment beyond flash".into()); }
            let flash_mapped = (DBUS_LOW..DBUS_HIGH).contains(&s.load_addr) || (IBUS_LOW..IBUS_HIGH).contains(&s.load_addr);
            if flash_mapped {
                // esptool aligns segments so vaddr and flash offset agree modulo 64 KiB
                if (s.load_addr & 0xffff) != (start as u32 & 0xffff) { return Err(format!("segment {:#x} not page-aligned with flash offset {:#x}", s.load_addr, start)); }
                let window_end = if s.load_addr < DBUS_HIGH { DBUS_HIGH } else { IBUS_HIGH };
                if !s.load_addr.checked_add(s.len).is_some_and(|end| end <= window_end) {
                    return Err("segment beyond the flash window".into());
                }
                let first_page = (start as u32) >> 16;
                let npages = ((s.load_addr & 0xffff) + s.len + 0xffff) >> 16;
                for i in 0..npages {
                    let vpage = (((s.load_addr & 0x1FF_FFFF) >> 16) + i) as usize;
                    self.mmu[vpage] = first_page + i;
                }
                self.invalidate_tlb();
            } else {
                let data = self.flash[start..end].to_vec();
                SocBus::load_bytes(self, s.load_addr, &data)?;
            }
        }
        Ok(img.entry)
    }
    /// Digital peripherals re-initialised, cache MMU invalid; SRAM, RTC memories, efuses and the
    /// RTC-domain registers survive, as on silicon. Returns the cause the ROM will report.
    fn reboot(&mut self, mac: [u8; 6]) -> u32 {
        self.ble.restore_flash(&mut self.flash);
        self.ble.reset();
        self.flush_ticks();
        self.cancel_spi2_timing();
        let cause = self.periph.rtc.reset_cause;
        let mut old = std::mem::replace(&mut self.periph, periph::Peripherals::new(mac));
        let p = &mut self.periph;
        // The host AP and network survive a guest reboot; only the MAC and relay queues reset.
        p.wifi.link = old.wifi.link.surviving_reboot();
        // Host camera cadence and logging survive; guest capture state resets.
        p.lcd_cam.frame_cycles = old.lcd_cam.frame_cycles;
        p.efuse = old.efuse;
        p.i2s0.keep_rx_input(&mut old.i2s0); p.i2s1.keep_rx_input(&mut old.i2s1);
        p.rtc.analog = old.rtc.analog;
        p.rtc.analog.set_stream_clock_offset(self.cycles); // ticks flushed; the new peripheral clock starts at zero
        p.gpio.restore_external(&old.gpio);
        p.gpio.strap = old.gpio.strap;
        p.misc.log_unknown = old.misc.log_unknown;
        p.spi0.jedec = old.spi0.jedec; p.spi1.jedec = old.spi1.jedec;   // the flash chip is not reset: its ID keeps the --flash-mb capacity
        // Nor is the octal flash (it stays in its OPI mode until the ROM
        // resets it) or the PSRAM (MR2 is read-only and keeps its density).
        p.spi0.octal_flash = old.spi0.octal_flash; p.spi1.octal_flash = old.spi1.octal_flash; p.spi1.flash_cr2 = old.spi1.flash_cr2;
        p.spi0.psram_mr[2] = old.spi0.psram_mr[2]; p.spi1.psram_mr[2] = old.spi1.psram_mr[2];
        p.rtc.ram = old.rtc.ram; p.rtc.slow_ticks = old.rtc.slow_ticks; p.rtc.reset_cause = cause;
        p.usb.connected = old.usb.connected;            // the host side of the USB cable is not reset
        p.rtc.ram.write(0x38, cause | (cause << 6));
        p.rtc.ram.write(0x98, 0);                       // watchdog disarmed by the reset; the ROM re-arms it
        p.i2s0.pcm = old.i2s0.pcm; p.i2s0.frames_out = old.i2s0.frames_out; p.i2s1.pcm = old.i2s1.pcm; p.i2s1.frames_out = old.i2s1.frames_out;   // keep the captured audio continuous
        self.mmu = [MMU_INVALID; MMU_ENTRIES];
        self.invalidate_tlb();
        self.reset_approximate_cache();
        self.attach_board_devices();
        self.refresh_tick_budget();
        self.irq_dirty = true;
        for area in self.debug.iter() { esp_periph::Dispatch::debug(&mut self.periph, area, true); }
        self.periph.misc.log_all = self.debug.has("mmio");
        cause
    }
    fn sw_reset(&self) -> bool { self.periph.rtc.sw_reset }
    fn request_reset(&mut self, cause: u32) { self.periph.rtc.sw_reset = true; self.periph.rtc.reset_cause = cause; }
    fn reset_cause(&self) -> u32 { self.periph.rtc.reset_cause }
    fn last_fault(&self) -> Option<(u32, bool)> { self.last_fault }
    fn console_take(&mut self) -> [Vec<u8>; 4] {
        [std::mem::take(&mut self.periph.usb.tx_out), std::mem::take(&mut self.periph.uart[0].tx_out), std::mem::take(&mut self.periph.uart[1].tx_out), std::mem::take(&mut self.periph.uart[2].tx_out)]
    }
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
        self.periph.rtc.analog.set(pin, src);
        self.periph.rtc.analog.set_stream_clock_offset(offset);
    }
    fn adc_set_raw(&mut self, pin: u8, raw: u16) -> bool { (1..=20).contains(&pin) && self.periph.rtc.analog.set_raw(pin, raw) }
    fn adc_observation(&self, pin: u8) -> Option<esp_periph::AdcObservation> {
        ((1..=20).contains(&pin)).then(|| self.periph.rtc.analog.observation(pin))
    }
    fn gpio_release_input(&mut self, pin: u8) {
        self.irq_dirty |= esp_soc::gpio::release_and_report(&mut self.periph.gpio, &mut self.gpio_events, self.cycles, pin);
        self.refresh_tick_budget();
    }
    fn gpio_set_input(&mut self, pin: u8, level: bool) {
        self.irq_dirty |= self.periph.gpio.set_input(pin, level);
        // Host edges queue PCNT work without going through the MMIO refresh hook.
        // Recompute the threshold without dropping cycles already pending.
        self.refresh_tick_budget();
        if let Some(ev) = &mut self.gpio_events { ev.push((self.cycles, pin, level)); }
    }
    fn set_flash_size(&mut self, bytes: usize) {
        self.flash = vec![0xff; bytes];
        let cap = esp_periph::flash_jedec(self.periph.spi1.octal_flash, bytes)[2]; self.periph.spi1.jedec[2] = cap; self.periph.spi0.jedec[2] = cap;   // octal capacity codes
        self.rebuild_page_table();
    }
    /// The array, and the octal PSRAM's MR2 density with it (ESP-IDF sizes the
    /// PSRAM from MR2 alone).
    fn set_psram_size(&mut self, bytes: usize) -> Result<(), String> {
        self.psram = vec![0; bytes];
        self.periph.spi0.set_psram_size(bytes); self.periph.spi1.set_psram_size(bytes);
        self.rebuild_page_table();
        Ok(())
    }
    fn set_strap(&mut self, v: u32) { self.periph.gpio.strap = v; }
    fn set_reset_cause(&mut self, c: u32) { self.periph.rtc.ram.write(0x38, c | (c << 6)); }
    fn report(&self) -> String {
        let p = &self.periph;
        let mut s = format!("[emu] i2s frames out: {} (i2s0 @ {} Hz) {} (i2s1 @ {} Hz)\n", p.i2s0.frames_out, p.i2s0.sample_rate, p.i2s1.frames_out, p.i2s1.sample_rate);
        { let r = self.board.report(); if !r.is_empty() { s += &r; s += "\n"; } }
        { let w = p.wifi.link.report(); if !w.is_empty() { s += &w; s += "\n"; } }
        { let (a, sh, r) = (&p.aes, &p.sha, &p.rsa);
          if a.blocks + sh.blocks + r.ops > 0 { s += &format!("[emu] crypto: {} AES blocks, {} SHA blocks, {} RSA/MPI operations\n", a.blocks, sh.blocks, r.ops); } }
        if p.lcd_cam.lcd_frames > 0 { s += &format!("[emu] lcd: {} RGB frames\n", p.lcd_cam.lcd_frames); }
        if p.lcd_cam.frames + p.lcd_cam.dropped > 0 { s += &format!("[emu] camera: {} VSYNCs, {} frames with dropped data\n", p.lcd_cam.frames, p.lcd_cam.dropped); }
        if p.rmt.tx_count > 0 { s += &format!("[emu] rmt tx {}\n", p.rmt.tx_count); }
        s.trim_end().to_string()
    }
    fn set_debug(&mut self, f: &esp_soc::DebugFlags) {
        self.debug = f.clone();
        for area in f.iter() { esp_periph::Dispatch::debug(&mut self.periph, area, true); }
        self.periph.misc.log_all = f.has("mmio");
    }
    fn observe_gpio(&mut self, on: bool) { self.gpio_events = if on { Some(Vec::new()) } else { None }; }
    fn take_gpio_events(&mut self) -> Vec<(u64, u8, bool)> { self.gpio_events.as_mut().map(std::mem::take).unwrap_or_default() }
    fn gpio_input(&self) -> u64 { self.periph.gpio.input }
    fn gpio_state(&self, pin: u8) -> Option<esp_soc::GpioState> {
        if pin > 48 || (22..=25).contains(&pin) { return None; }
        let mux = self.periph.io_mux.read(4 + u32::from(pin) * 4);
        Some(esp_soc::GpioState::new(
            self.periph.gpio.out & (1u64 << pin) != 0,
            self.periph.gpio.enable & (1u64 << pin) != 0,
            mux & (1 << 8) != 0,
            mux & (1 << 7) != 0,
        ))
    }
    fn pwm_output(&self, pin: u8) -> Option<(f64, u32)> { self.periph.ledc.output(&self.periph.gpio, pin).or_else(|| self.periph.mcpwm.iter().find_map(|pwm| pwm.output(&self.periph.gpio, pin))) }

    fn board(&mut self) -> &mut dyn BoardModel { &mut *self.board }
    fn board_ref(&self) -> &dyn BoardModel { &*self.board }
    fn i2s_input(&mut self, port: usize) -> Option<&mut esp_periph::i2s::PcmInput> { match port { 0 => Some(self.periph.i2s0.rx_input()), 1 => Some(self.periph.i2s1.rx_input()), _ => None } }
    fn pcm_sources(&self) -> Option<&esp_periph::i2s::PcmSources> { self.periph.i2s0.pcm_bank_ref() }
    fn attach_pcm_sources(&mut self) -> Option<&mut esp_periph::i2s::PcmSources> {
        self.flush_ticks();
        Some(esp_soc::soc::attach_pcm_sources(self.periph.i2s0.pcm_bank(), self.cycles, periph::CPU_HZ))
    }
    fn audio(&self) -> (&[i16], u32) { let a = self.periph.audio(); (&a.pcm, a.sample_rate) }
    fn camera_frames(&self) -> u64 { self.periph.lcd_cam.frames }
    fn irq_sources_of(&self, core: usize, line: u32) -> Vec<usize> { (0..NUM_SOURCES).filter(|&s| self.periph.intmatrix.map[core][s] == line).collect() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_routes_sampled_edges_and_nmi_to_machine_cores() {
        for irq in [10, 14, 22, 28, 30] {
            let mut m = Machine::new([0; 6], SocBus::new(8 << 20, 2 << 20, [0; 6]));
            m.bus.periph.intmatrix.map[0][periph::SRC_FROM_CPU0] = irq;
            m.bus.periph.intmatrix_dirty = true;
            m.cores[0].ps = if irq == 14 { ps::EXCM | 15 } else { 0 };
            m.cores[0].intenable = if irq == 14 { 0 } else { 1 << irq };
            m.sync_irq();
            m.bus.periph.system.write(0x30, 1);
            m.sync_irq();
            assert_ne!(m.cores[0].check_interrupts_pending() & (1 << irq), 0);
            assert!(m.cores[0].check_interrupts().is_some());
            // Acknowledgment while the source remains high cannot manufacture another edge.
            m.cores[0].interrupt &= !(1 << irq);
            m.sync_irq();
            assert_eq!(m.cores[0].interrupt & (1 << irq), 0);
            m.bus.periph.system.write(0x30, 0);
            m.sync_irq();
            m.bus.periph.system.write(0x30, 1);
            m.sync_irq();
            assert_ne!(m.cores[0].interrupt & (1 << irq), 0);
        }
    }

    #[test]
    fn primary_core_is_not_controlled_by_core_one_reset_registers() {
        let bus = SocBus::new(8 << 20, 2 << 20, [0; 6]);
        assert_eq!(S3::core_state(&bus, 0), CoreState::Running);
        assert_eq!(S3::core_state(&bus, 1), CoreState::Held);
    }
}

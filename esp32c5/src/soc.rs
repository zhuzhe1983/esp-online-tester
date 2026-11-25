//! Common Machine integration for SRAM apps and audited MP ROM bring-up.
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
/// Real CPU-context monitoring for experimental ROM execution. The existing
/// SRAM constructor keeps its unobserved execution path.
pub fn rom_machine(mac:[u8;6],flash_size:usize)->Machine{
    let mut m=machine(mac,flash_size);let monitor=m.bus.cpu_monitor();
    m.add_observer(Box::new(crate::cpu_monitor::CpuObserver(monitor)));m
}
/// Validate the MP layout and reconstruct only this ELF's declared ROM-side
/// initializer bytes. The default ELF staging path eagerly fills SRAM; C5's
/// Machine override uses this loader for actual guest reset-loop work.
pub fn load_rom(m:&mut Machine,bytes:&[u8])->Result<(),String>{
    let (elf,image)=crate::rom::decode(bytes)?;
    if m.bus.reset_requested{return Err("C5 ROM replacement requires applying the pending reset first".into());}
    m.cores[0].c5_machine_context().map_err(|e|format!("C5 cold ROM needs supported Machine context: {e:?}"))?;
    m.bus.select_validated_rom_profile()?;
    m.cores[0].enable_c5_memory_protection().map_err(|e|format!("C5 CPU memory policy unavailable: {e:?}"))?;
    if !m.has_observer("c5-hardware-monitor"){
        let monitor=m.bus.cpu_monitor();m.add_observer(Box::new(crate::cpu_monitor::CpuObserver(monitor)));
    }
    m.bus.rom.copy_from_slice(&image);m.bus.rom_boot=true;m.bus.app_off=None;m.bus.app_entry=None;
    m.bus.first_fault=None;m.bus.reset_requested=false;m.bus.wifi_compat=None;m.symbols.extend(elf.symbols);Ok(())
}
impl Soc for C5 {
    type Core = Cpu; type Bus = C5Bus;
    const NAME: &'static str = "esp32c5";
    const BOOTLOADER_OFFSET:usize=0x2000;
    const ROM_ELF: &'static str = "esp32c5_rev100_rom.elf";
    fn load_rom_override(m:&mut Machine,bytes:&[u8])->Option<Result<(),String>>{Some(load_rom(m,bytes))}
    const CPU_HZ: u64 = CPU_HZ; const CORES: usize = 1; const IDLE_CHUNK: u64 = 64;
    const ROM_BOOT_DEFAULT: bool = false;
    const EXCEPTION_BUDGET_AFTER_RESET: Option<u64> = Some(1);
    const ROM_DATA_TABLE: &'static [&'static str] = &["_data_table_start"];
    const ROM_DATA_TABLE_END: &'static [&'static str] = &["_data_table_end"];
    const ROM_DATA_TABLE_STRIDE: u32 = 12;
    fn function_hooks(bus:&C5Bus)->&[u32]{crate::wifi_compat::hooks(bus)}
    fn function_hook(cpu:&mut Cpu,bus:&mut C5Bus)->bool{crate::wifi_compat::intercept(cpu,bus)}
    fn new_core(_i: usize) -> Cpu { core() }
    fn reset_core(c: &mut Cpu, _i: usize) { c.reset(); c.pc = ROM_BASE; }
    fn reset_core_with_bus(c: &mut Cpu, bus: &mut C5Bus, i: usize) {
        Self::reset_core(c, i); bus.irq_dirty = true;
        if let Some(m)=&bus.monitor{m.borrow_mut().cpu_reset();}
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
    fn observer_stopped(bus:&mut C5Bus){if let Some(m)=&bus.monitor{m.borrow_mut().end();}}
    fn check_image(data: &[u8]) -> Result<(), String> { crate::image::parse(data).map(|_| ()) }
    fn check_image_at(bus:&C5Bus,kind:esp_soc::LoadKind,offset:usize,data:&[u8])->Option<Result<(),String>>{
        use esp_soc::LoadKind;
        match kind{
            LoadKind::Flash=>Some(crate::image::discover_merged(data,bus.flash.len()).map(|_|())),
            LoadKind::Bootloader=>Some(crate::image::validate_at(data,bus.flash.len(),offset).map(|_|())),
            LoadKind::App if bus.has_validated_rom()=>Some(crate::image::validate_at(data,bus.flash.len(),offset).map(|_|())),
            _=>None,
        }
    }
}
impl SocBus for C5Bus {
    fn cycles(&self) -> u64 { self.cycles }
    fn next_deadline(&self) -> Option<u64> { (self.rmt_clocked()&&self.rmt.rmt.is_running()).then_some(1).into_iter().chain(self.boot_analog.as_ref().and_then(|a|a.deadline()).map(|d|d.saturating_sub(self.cycles).max(1))).chain(self.boot_clock.deadline().map(|d|d.saturating_sub(self.cycles).max(1))).chain(self.lp_wdt.deadline.map(|d|d.saturating_sub(self.cycles).max(1))).chain(self.timg_wdt.deadline().map(|d|d.saturating_sub(self.cycles).max(1))).chain(self.boot_security.as_ref().and_then(|s|s.deadline()).map(|d|d.saturating_sub(self.cycles).max(1))).chain(self.boot_saradc.as_ref().and_then(|s|s.deadline()).map(|d|d.saturating_sub(self.cycles).max(1))).chain(self.transport.as_ref().and_then(|t|t.deadline).map(|d|d.saturating_sub(self.cycles).max(1))).chain(self.i2c.as_ref().and_then(|i|i.deadline).map(|d|d.saturating_sub(self.cycles).max(1))).chain(self.systimer.as_ref().and_then(|s|s.deadline).map(|d|d.saturating_sub(self.cycles).max(1))).min() }
    fn irq_dirty(&mut self) -> &mut bool { &mut self.irq_dirty }
    fn refresh_irq(&mut self) -> bool { C5Bus::refresh_irq(self, 0); true } // core applies standard CSR threshold
    fn misc(&mut self) -> &mut esp_periph::Misc { &mut self.misc }
    fn load_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), String> {
        if data.len() <= ROM_SIZE && addr >= ROM_BASE && addr.checked_add(data.len() as u32).is_some_and(|end| end <= ROM_BASE + ROM_SIZE as u32) {
            let off = (addr - ROM_BASE) as usize; self.rom[off..off + data.len()].copy_from_slice(data); self.rom_boot=true; self.deselect_validated_rom_profile(); Ok(())
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
        self.rom_boot=false;self.deselect_validated_rom_profile();self.wifi_compat=None;
        self.first_fault = None; self.reset_requested = false;self.watchdog_reset_at=None;
        Ok(image.entry)
    }
    fn reboot(&mut self, _mac: [u8; 6]) -> u32 {
        // TRM1.1 Table9.1-1: CPU-only resets must not reconstruct HP, LP or
        // analog peripherals. CLIC is core-local; matrix routes are retained.
        let cause=self.reset_cause&0x1f;
        if let Some(w)=&mut self.wifi_compat{w.reset();}
        let reset_at=self.watchdog_reset_at.take().unwrap_or(self.cycles);let cpu=matches!(cause,0x0b..=0x0d|0x11|0x18|0x1a);
        if cpu{
            self.lp_wdt.reset_domains(reset_at,false,false);self.timg_wdt.reset_domains(reset_at,false,false,false);
            self.lp_timer.reset_domains(reset_at,false,false);
            self.reset_cpu_domain();
            if let Some(off)=self.app_off{if let Err(reason)=self.boot_app(off){self.misc.log_unknown=true;eprintln!("[c5] app-direct restart rejected: {reason}");}}
            self.pump_boot_watchdogs();
            self.pump_boot_security();
            self.pump_boot_saradc();
            if !self.reset_requested{self.lp_timer.pump(self.cycles);}
            return cause;
        }
        let mut fresh = C5Bus::new();
        std::mem::swap(&mut fresh.rom, &mut self.rom); std::mem::swap(&mut fresh.sram, &mut self.sram);
        std::mem::swap(&mut fresh.lp_sram, &mut self.lp_sram); std::mem::swap(&mut fresh.flash, &mut self.flash);
        std::mem::swap(&mut fresh.psram, &mut self.psram); fresh.mspi.preserve_psram(&self.mspi);
        fresh.xtal_hz = self.xtal_hz; fresh.mspi.set_xtal_hz(self.xtal_hz);
        std::mem::swap(&mut fresh.board, &mut self.board);
        fresh.gpio.restore_external(&self.gpio); fresh.gpio.strap = self.gpio.strap;
        fresh.cycles = self.cycles; fresh.debug = self.debug.clone(); fresh.reset_cause = self.reset_cause;
        fresh.lp_store=self.lp_store;
        fresh.rom_boot=self.rom_boot;fresh.wifi_compat=self.wifi_compat.take();
        // C5 TRM1.1 Table9.1-1.
        let chip=cause==1;let lp=chip||matches!(cause,0x10|0x12|0x19);
        // LPPERI gates/configuration and its declared virtual RNG state are
        // LP-domain state, just like the other half of the LP_IO close loop.
        // An HP-only reset must not reopen LP_IO/RNG or reseed the source.
        if !lp{std::mem::swap(&mut fresh.lp_peri,&mut self.lp_peri);}
        if self.has_validated_rom(){
            let mut w=std::mem::replace(&mut self.lp_wdt,crate::lp_wdt::LpWdt::new());
            w.reset_domains(reset_at,lp,chip);fresh.lp_wdt=w;
        }
        let mut tg=std::mem::replace(&mut self.timg_wdt,crate::timg_wdt::TimgWdt::new());
        tg.reset_domains(reset_at,true,lp,chip);fresh.timg_wdt=tg;
        let mut timer=std::mem::replace(&mut self.lp_timer,crate::lp_timer::LpTimer::new(reset_at));
        timer.reset_domains(reset_at,lp,chip);fresh.lp_timer=timer;
        if let Some(mut b)=self.boot_bias.take(){b.reset_domains(lp,chip);fresh.boot_bias=Some(b);}
        if let Some(mut p)=self.boot_pmu.take(){p.reset_domains(lp,chip);fresh.boot_pmu=Some(p);}
        if let Some(mut a)=self.boot_apm.take(){a.reset_domains(true,lp,chip);fresh.boot_apm=Some(a);}
        if let Some(mut c)=self.boot_peripheral_clock.take(){c.reset_domains(true,lp,chip);fresh.boot_peripheral_clock=Some(c);}
        if let Some(mut b)=self.boot_brownout.take(){b.reset_domains(true,lp,chip);fresh.boot_brownout=Some(b);}
        if let Some(mut s)=self.software_interrupt.take(){s.reset_domains(true,chip);fresh.software_interrupt=Some(s);}
        if let Some(mut s)=self.boot_security.take(){s.reset_domains(reset_at,true,chip);fresh.boot_security=Some(s);}
        if let Some(mut c)=self.modem_lp_clock.take(){c.reset_domains(true,chip);fresh.modem_lp_clock=Some(c);}
        if let Some(mut c)=self.modem_wifi_clock.take(){c.reset_domains(true,chip);fresh.modem_wifi_clock=Some(c);}
        if self.boot_clock.virtual_profile_enabled(){
            let mut c=std::mem::replace(&mut self.boot_clock,crate::boot_clock::BootClock::new());
            c.reset_domains(self.cycles,true,lp,chip);fresh.boot_clock=c;
        }
        if let Some(mut a)=self.boot_analog.take(){
            // Brownout's configured domain and unknown
            // causes retain the prior HP-only fallback, not a precision claim.
            a.reset_domains(!cpu,lp,chip);fresh.boot_analog=Some(a);
        }
        if let Some(mut s)=self.boot_saradc.take(){s.reset_domains(self.cycles,true,lp,chip);fresh.boot_saradc=Some(s);}
        if self.mspi.mp_rom_compat(){fresh.mspi.enable_mp_rom_compat();}
        if chip&&fresh.has_validated_rom(){
            let inputs=fresh.timg_wdt_inputs();if let Err(reason)=fresh.timg_wdt.enable_cold_spi_boot(reset_at,inputs){
                fresh.deselect_validated_rom_profile();eprintln!("[c5] cold reset profile rejected: {reason}");
            }
            else if let Some(s)=&mut fresh.boot_saradc{if let Err(reason)=s.enable_virtual_profile(fresh.cycles,[2048,2048]){
                fresh.deselect_validated_rom_profile();eprintln!("[c5] cold ADC profile rejected: {reason}");
            }}
        }
        // Preserve the observer's shared handle across resets. HP Core Reset
        // causes retain PC/SP captures; System/LP and Chip retention is not inferred.
        if let Some(m)=self.monitor.take(){
            {let mut state=m.borrow_mut();if matches!(cause,3|5|7|8|9|0x14..=0x16){state.core_reset();}else{*state=crate::cpu_monitor::CpuMonitor::new();}}
            fresh.monitor=Some(m);
        }
        fresh.gpio_events = self.gpio_events.as_ref().map(|_| Vec::new());
        fresh.app_off = self.app_off;
        if let Some(off) = self.app_off { if let Err(reason) = fresh.boot_app(off) { fresh.misc.log_unknown = true; eprintln!("[c5] app-direct restart rejected: {reason}"); } }
        // A reset of the LP mux is effective at the reset event, before any
        // late host catch-up can run the SWD pulse under the previous source.
        if fresh.boot_clock.virtual_profile_enabled(){fresh.lp_wdt.set_fast_clock(reset_at,fresh.boot_clock.rtc_fast_hz());}
        fresh.pump_boot_clock();fresh.pump_boot_security();fresh.pump_boot_saradc();fresh.attach_i2c_devices();fresh.publish_gpio(); *self = fresh;cause
    }
    fn sw_reset(&self) -> bool { self.reset_requested }
    fn request_reset(&mut self, cause: u32) { self.reset_requested = true; self.reset_cause = cause & 0x1f | 1 << 5;self.watchdog_reset_at=None; }
    fn reset_cause(&self) -> u32 { self.reset_cause & 0x1f }
    fn last_fault(&self) -> Option<(u32, bool)> { self.first_fault.as_ref().map(|f| (f.address, f.write)) }
    fn console_take(&mut self) -> [Vec<u8>; 4] { [Vec::new(), std::mem::take(&mut self.uart[0].tx_out), std::mem::take(&mut self.uart[1].tx_out), Vec::new()] }
    fn serial_input(&mut self, _data: &[u8]) {} // USB Serial/JTAG is not implemented; never alias it to UART
    fn uart_input(&mut self, n: usize, data: &[u8]) { if self.uart_receiving(n) { self.uart[n].host_input(data); self.irq_dirty = true; } }
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
    fn irq_sources_of(&self, _core: usize, line: u32) -> Vec<usize> { self.sources().into_iter().chain(self.transport.as_ref().into_iter().flat_map(|t|t.sources())).filter(|&(source, active)| active && self.map[source] == line + 16).map(|(source, _)| source).collect() }
    fn set_debug(&mut self, f: &esp_soc::DebugFlags) { self.debug = f.clone(); }
    fn set_flash_size(&mut self, bytes: usize) { self.flash.resize(bytes.min(32 << 20), 0xff); }
    fn set_psram_size(&mut self, bytes: usize) -> Result<(), String> { self.configure_psram(bytes) }
    fn set_strap(&mut self, v: u32) { self.gpio.strap = v & 0x7f; }
    fn set_reset_cause(&mut self, cause: u32) { self.reset_cause = (self.reset_cause & !0x1f) | cause & 0x1f; }
    fn attach_sd_card(&mut self,image:Box<dyn esp_periph::sdmmc::SdStore>,config:esp_periph::sdmmc::SdCardConfig)->Result<String,String>{self.board.attach_spi_sd_card(image,config)}
    fn refresh_board_outputs(&mut self) { self.publish_gpio(); }
    fn touch_input(&mut self,x:u16,y:u16,down:bool){self.board.touch_at(self.cycles,x,y,down);self.irq_dirty|=esp_soc::gpio::drain_board_inputs(&mut *self.board,&mut self.gpio,&mut self.gpio_events,self.cycles);}
    fn report(&self) -> String { format!("[c5] {}; functional MSPI/MMU; PSRAM={}B Quad requires guest pads/QPI/phases/maps; factory startup incomplete; HP={SRAM_SIZE}B LP={LP_SIZE}B at{LP_BASE:#x}; {}{}{}{}{}{}; {}",if self.rom_boot{"ROM bring-up"}else{"SRAM app-direct"},self.mspi.psram_size(),self.transport.as_ref().map_or_else(||"SPI2/GDMA/LEDC unused".into(),|t|t.report()),self.systimer.as_ref().map_or_else(String::new,|s|format!("; SYSTIMER ETM source events={:?} (fabric unavailable)",s.etm_events())),self.boot_analog.as_ref().map_or_else(String::new,|a|format!("; {}",a.report())),{let mut w=self.lp_wdt.report();if self.timg_wdt.virtual_profile_enabled(){w.push_str("; ");w.push_str(&self.timg_wdt.report());}
        if let Some(s)=&self.boot_saradc{w.push_str("; ");w.push_str(&s.report());}
        if self.lp_timer.virtual_profile_enabled(){w.push_str("; ");w.push_str(&self.lp_timer.report());}
        if let Some(p)=&self.boot_pmu{w.push_str("; ");w.push_str(&p.report());}
        if let Some(a)=&self.boot_apm{w.push_str("; ");w.push_str(&a.report());}
        if let Some(c)=&self.boot_peripheral_clock{w.push_str("; ");w.push_str(&c.report());}
        if let Some(b)=&self.boot_brownout{w.push_str("; ");w.push_str(&b.report());}
        if let Some(s)=&self.software_interrupt{w.push_str("; ");w.push_str(&s.report());}
        if let Some(s)=&self.boot_security{w.push_str("; ");w.push_str(&s.report());}
        if self.boot_clock.virtual_profile_enabled(){w.push_str("; ");w.push_str(&self.cache.error_report());}
        if let Some(c)=&self.modem_lp_clock{w.push_str("; ");w.push_str(&c.report(self.modem_clock_inputs()));}
        if let Some(c)=&self.modem_wifi_clock{w.push_str("; ");w.push_str(&c.report());}
        if let Some(e)=self.memory_policy_unsupported(){w.push_str(&format!("; CPU_memory_policy_model_limit={e:?}"));}
        if w.is_empty(){w}else{format!("; {w}")}},{let c=self.boot_clock.report();if c.is_empty(){c}else{format!("; {c}")}},self.boot_bias.as_ref().map_or_else(String::new,|b|format!("; {}",b.report())),self.board.report()) }
}

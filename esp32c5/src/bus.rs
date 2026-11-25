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
// Actual original C5 WiFi first access after CLK_CONF1. No public whole-word
// semantics/POR is qualified: stop the model, never return guessed MAC data.
const ORIGINAL_WIFI_MAC_BOUNDARY:u32=0x600a_d858;
const LP_AON_GPIO_MUX:u32=0x600b_1028;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessFault { pub pc: u32, pub address: u32, pub write: bool, pub width: u8 }
pub struct C5Bus {
    pub rom: Vec<u8>, pub sram: Vec<u8>, pub lp_sram: Vec<u8>,
    pub uart: [Uart; 2], pub gpio: Gpio, pub io_mux: RegRam, pub rmt: RmtCompact,
    pub board: Board, pub clic: ClicCore, pub(crate) map: [u32; SOURCES], clic_config: u32, lp_ana_date: u32, pub(crate) reset_cause: u32,
    pub first_fault: Option<AccessFault>, pc: u32, pub cycles: u64,
    pub flash: Vec<u8>, pub psram: Vec<u8>, pub(crate) mspi: crate::mspi::Mspi,
    pub(crate) xtal_hz: u32, pub(crate) app_off: Option<usize>, pub(crate) app_entry: Option<u32>,
    pub(crate) irq_dirty: bool, pub(crate) reset_requested: bool, pub(crate) watchdog_reset_at:Option<u64>,pub(crate) misc: esp_periph::Misc,
    pub(crate) gpio_events: Option<Vec<(u64, u8, bool)>>, pub debug: esp_soc::DebugFlags,
    pub(crate) transport: Option<Box<crate::transport::Transport>>,
    pub(crate) lp_peri:crate::lp_peri::LpPeri,
    pub(crate) lp_wdt:crate::lp_wdt::LpWdt,
    pub(crate) timg_wdt:crate::timg_wdt::TimgWdt,
    pub(crate) lp_timer:crate::lp_timer::LpTimer,
    pub(crate) i2c:Option<Box<crate::i2c::C5I2c>>,
    pub(crate) cache:crate::cache::CacheControls,
    sha:Option<Box<crate::sha::C5Sha>>,
    pub(crate) boot_analog:Option<Box<crate::boot_analog::C5BootAnalog>>,
    pub(crate) boot_saradc:Option<Box<crate::boot_saradc::BootSarAdc>>,
    pub(crate) boot_pmu:Option<Box<crate::boot_pmu::BootPmu>>,
    pub(crate) boot_apm:Option<Box<crate::boot_apm::BootApm>>,
    pub(crate) boot_peripheral_clock:Option<Box<crate::boot_peripheral_clock::BootPeripheralClock>>,
    pub(crate) boot_brownout:Option<Box<crate::boot_brownout::BootBrownout>>,
    pub(crate) boot_security:Option<Box<crate::boot_security::BootSecurity>>,
    pub(crate) software_interrupt:Option<Box<crate::software_interrupt::SoftwareInterrupt>>,
    pub(crate) boot_clock:crate::boot_clock::BootClock,
    pub(crate) modem_lp_clock:Option<Box<crate::modem_lp_clock::ModemLpClock>>,
    pub(crate) modem_wifi_clock:Option<Box<crate::modem_wifi_clock::ModemWifiClock>>,
    pub(crate) boot_bias:Option<Box<crate::boot_bias::C5BootBias>>,
    uart_conf:[u32;2],uart_clock:[u32;2],uart_power:[u32;2],
    efuse:crate::efuse::C5Efuse,
    usb:crate::usb_detached::DetachedUsb,
    pub(crate) lp_store:[u32;10],
    pub(crate) systimer:Option<Box<crate::systimer::C5Systimer>>,
    pub(crate) rom_boot:bool,
    pub(crate) monitor:Option<crate::cpu_monitor::Monitor>,
    pub(crate) wifi_compat:Option<Box<crate::wifi_compat::WifiCompat>>,
}
impl Default for C5Bus { fn default() -> Self { Self::new() } }
impl C5Bus {
    pub fn new() -> Self {
        // C5 uart_reg.h: RX full/count 8 bits, CONF0_SYNC RXFIFO_RST bit22.
        let layout = UartLayout { thrhd_mask: 0xff, rxfifo_rst: 1 << 22, rxfifo_cnt_mask: 0xff, autobaud_en: 0, autobaud_regs: 0, autobaud_clock_hz: 0 };
        let mut gpio = Gpio::with_pins(29); gpio.input_select = 0x100; gpio.func_in_sel.fill(96);
        // C5 TRM1.1 10.2.1/10.2.2: reset pull-ups on GPIO27/28 give SPI boot.
        // Floating strap inputs use an explicit virtual LOW, not a measured die.
        gpio.strap = 0x18;
        // C5 gpio[29] begins at physical IO_MUX+0. PinRoutes stores its shared
        // internal view at +4; this private offset is not a hardware alias.
        let mut io_mux = RegRam::new();
        for pin in 0..29 { io_mux.write(4 + 4 * pin, 0x1800); gpio.set_pad(pin as u8, 0x1800); }
        io_mux.write(0x200, 36_770_416);
        Self { boot_bias:None, boot_analog:None,boot_saradc:None,boot_pmu:None,boot_apm:None,boot_peripheral_clock:None,boot_brownout:None,boot_security:None,software_interrupt:None, sha:None, rom: vec![0; ROM_SIZE], sram: vec![0; SRAM_SIZE], lp_sram: vec![0; LP_SIZE],
            uart: [Uart::new(layout), Uart::new(layout)], gpio, io_mux, rmt: RmtCompact::new(CPU_HZ),
            board: Box::new(NoBoard), clic: ClicCore::default(), map: [0; SOURCES], clic_config: 0, lp_ana_date: 36_774_528, reset_cause: 0x21, first_fault: None, pc: 0, cycles: 0,
            flash: Vec::new(), psram: Vec::new(), mspi: crate::mspi::Mspi::new(), xtal_hz: 48_000_000,
            app_off: None, app_entry: None, irq_dirty: true, reset_requested: false,watchdog_reset_at:None, misc: Default::default(), gpio_events: None, debug: Default::default(), transport: None, lp_peri:crate::lp_peri::LpPeri::new(),lp_wdt:crate::lp_wdt::LpWdt::new(),timg_wdt:crate::timg_wdt::TimgWdt::new(),lp_timer:crate::lp_timer::LpTimer::new(0),i2c:None,cache:crate::cache::CacheControls::new(),boot_clock:crate::boot_clock::BootClock::new(),modem_lp_clock:None,modem_wifi_clock:None,uart_conf:[1;2],uart_clock:[1<<22;2],uart_power:[2;2],efuse:crate::efuse::C5Efuse::new(),usb:crate::usb_detached::DetachedUsb::new(),lp_store:[0;10],systimer:None,rom_boot:false,monitor:None,wifi_compat:None }
    }
    /// Explicit nominal board crystal; RC_FAST and PLL remain unmodeled here.
    pub fn set_xtal_mhz(&mut self, mhz: u32) -> Result<(), String> {
        if mhz != 48 { return Err("C5 current source-backed clock subset requires 48 MHz XTAL".into()); }
        self.xtal_hz = mhz * 1_000_000; self.mspi.set_xtal_hz(self.xtal_hz); Ok(())
    }
    pub fn configure_psram(&mut self, bytes: usize) -> Result<(), String> {
        self.mspi.configure_psram(bytes)?; self.psram.resize(bytes, 0); Ok(())
    }
    /// Selected audited mask-ROM profile, cleared by SRAM load or raw ROM overwrite.
    pub fn has_validated_rom(&self)->bool{self.rom_boot&&self.mspi.mp_rom_compat()}
    pub fn attach_board(&mut self, board: Board) { self.board = board; self.attach_board_devices(); }
    pub fn attach_board_devices(&mut self) {
        self.attach_i2c_devices();
        for (pin, level) in self.board.input_levels() { self.gpio.set_input(pin, level); }
        self.publish_gpio();
    }
    pub(crate) fn attach_i2c_devices(&mut self){
        if let Some(i)=&mut self.i2c{i.clear_devices();}
        for(host,address,device)in self.board.i2c_devices(){if host==0{self.i2c.get_or_insert_with(||Box::new(crate::i2c::C5I2c::new(self.cycles))).attach(address,device);}}
    }
    pub(crate) fn publish_gpio(&mut self) {
        self.board.gpio_waveform_at(self.cycles, PINS.routes(&self.gpio, &self.io_mux));
        if let Some(events) = &mut self.gpio_events { events.extend(self.gpio.changes.drain(..).map(|(p, l)| (self.cycles, p, l))); }
        else { self.gpio.changes.clear(); }
        self.gpio.input_changes.clear();
        if let Some(mut t)=self.transport.take(){t.publish_pwm(self);self.transport=Some(t);}
        self.irq_dirty|=esp_soc::gpio::drain_board_inputs(&mut *self.board,&mut self.gpio,&mut self.gpio_events,self.cycles);
    }
    fn range(address: u32, size: usize, base: u32, len: usize) -> Option<std::ops::Range<usize>> {
        let start = address.checked_sub(base)? as usize; let end = start.checked_add(size)?;
        (end <= len).then_some(start..end)
    }
    pub fn check_ram(&self, address: u32, size: usize) -> Result<(), String> {
        if Self::range(address, size, SRAM_BASE, SRAM_SIZE).is_some() || Self::range(address, size, LP_BASE, LP_SIZE).is_some() { Ok(()) }
        else { Err(format!("C5 SRAM range {address:#010x}+{size:#x} is unavailable")) }
    }
    pub(crate) fn compat_check_span(&mut self,address:u32,size:usize,write:bool)->Result<(),String>{
        self.check_ram(address,size)?;
        if !self.cpu_apm_supported(){return Err("compat output APM context is unavailable".into());}
        let kind=if write{riscv_rv32::c5_memory_protection::AccessKind::Store}else{riscv_rv32::c5_memory_protection::AccessKind::Load};
        for i in 0..size{self.cpu_memory_check(address+i as u32,1,kind).map_err(|_|"compat guest memory policy rejected")?;}Ok(())
    }
    pub(crate) fn compat_check_write(&mut self,address:u32,size:usize)->Result<(),String>{self.compat_check_span(address,size,true)}
    pub(crate) fn compat_read(&mut self,address:u32,size:usize)->Result<Vec<u8>,String>{
        self.compat_check_span(address,size,false)?;
        if let Some(r)=Self::range(address,size,SRAM_BASE,SRAM_SIZE){Ok(self.sram[r].to_vec())}
        else if let Some(r)=Self::range(address,size,LP_BASE,LP_SIZE){Ok(self.lp_sram[r].to_vec())}else{Err("compat backing unavailable".into())}
    }
    pub(crate) fn compat_write(&mut self,address:u32,data:&[u8])->Result<(),String>{
        self.compat_check_span(address,data.len(),true)?;
        if let Some(r)=Self::range(address,data.len(),SRAM_BASE,SRAM_SIZE){self.sram[r].copy_from_slice(data);}
        else if let Some(r)=Self::range(address,data.len(),LP_BASE,LP_SIZE){self.lp_sram[r].copy_from_slice(data);}else{return Err("compat backing unavailable".into());}Ok(())
    }
    pub(crate) fn compat_mac(&self)->[u8;6]{self.efuse.logical_mac()}
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
    pub(crate) fn sources(&self) -> [(usize, bool); 17] {
        let timer=self.systimer.as_ref().map_or([(61,false),(62,false),(63,false)],|s|s.sources());
        let watchdog=self.timg_wdt.sources();
        [(14,self.efuse.irq()),(15,self.boot_brownout.as_ref().is_some_and(|b|b.irq())),(18,self.lp_wdt.irq()),(23,self.software_interrupt.as_ref().is_some_and(|s|s.irq())),self.monitor.as_ref().map_or((27,false),|m|m.borrow().regs.source()),self.cache.source(),(31, self.gpio.irq()), (44, self.mspi.irq()), (47, self.uart[0].irq()), (48, self.uart[1].irq()), (55, self.rmt_clocked()&&self.rmt.rmt.irq()),(56,self.i2c.as_ref().is_some_and(|i|i.irq())),watchdog[0],watchdog[1],timer[0],timer[1],timer[2]]
    }
    pub fn refresh_irq(&mut self, threshold: u32) -> Option<u32> {
        let mut input = 0;
        for (source, asserted) in self.sources() { let id = self.map[source] as usize; if asserted && (16..48).contains(&id) { input |= 1u64 << id; } }
        if let Some(t)=&self.transport {for(source,asserted)in t.sources(){let id=self.map[source] as usize;if asserted&&(16..48).contains(&id){input|=1u64<<id;}}}
        self.clic.set_input(input); self.clic.thresh = (threshold & 0xff) << 24;
        self.clic.candidate().filter(|&(id, _)| id >= 16).map(|(id, level)|
            (id as u32 - 16) | level << riscv_rv32::exec::CLIC_IRQ_LEVEL_SHIFT | if self.clic.vectored(id) { riscv_rv32::exec::CLIC_IRQ_SHV } else { 0 })
    }
    pub fn interrupt_taken(&mut self, line: u32) { self.clic.taken(line as usize + 16); }
    pub(crate) fn reset_cpu_domain(&mut self){
        self.clic=ClicCore::default();self.clic_config=0;self.first_fault=None;self.reset_requested=false;self.watchdog_reset_at=None;self.irq_dirty=true;
        if let Some(m)=&self.monitor{m.borrow_mut().cpu_reset();}
    }
    pub fn advance(&mut self, cycles: u64) {
        self.cycles = self.cycles.wrapping_add(cycles);
        if self.rmt_clocked(){
            let before=self.rmt.rmt.irq();self.rmt.tick(cycles);
            self.irq_dirty|=before!=self.rmt.rmt.irq();
            for (ch,bits) in std::mem::take(&mut self.rmt.rmt.done){
                for pin in PINS.routes(&self.gpio,&self.io_mux).output_pins(64+ch as u32){self.board.rmt_frame(pin,&bits);}
            }
        }
        if self.boot_analog.as_ref().is_some_and(|a|a.deadline().is_some_and(|d|self.cycles>=d)){self.pump_boot_analog();}
        if self.boot_clock.deadline().is_some_and(|d|self.cycles>=d){self.pump_boot_clock();}
        if self.lp_wdt.deadline.is_some_and(|d|self.cycles>=d)||self.timg_wdt.deadline().is_some_and(|d|self.cycles>=d){self.pump_boot_watchdogs();}
        if self.boot_security.as_ref().is_some_and(|s|s.deadline().is_some_and(|d|self.cycles>=d)){self.pump_boot_security();}
        if self.boot_saradc.as_ref().is_some_and(|s|s.deadline().is_some_and(|d|self.cycles>=d)){self.pump_boot_saradc();}
        if self.transport.as_ref().is_some_and(|t|t.deadline.is_some_and(|d|self.cycles>=d)){
            let mut t=self.transport.take().unwrap();t.publish_pwm(self);t.pump(self);self.transport=Some(t);self.irq_dirty=true;
        }
        if self.i2c.as_ref().is_some_and(|i|i.deadline.is_some_and(|d|self.cycles>=d)){let mut i=self.i2c.take().unwrap();i.sync(self);self.i2c=Some(i);self.irq_dirty=true;}
        if self.systimer.as_ref().is_some_and(|s|s.deadline.is_some_and(|d|self.cycles>=d)){self.systimer.as_mut().unwrap().pump(self.cycles);self.irq_dirty=true;}
    }
    pub(crate) fn timg_wdt_inputs(&self)->crate::timg_wdt::ClockInputs{
        let tg0=self.boot_clock.timg0_control();let tg1=self.boot_clock.timg1_control();
        crate::timg_wdt::ClockInputs{xtal_hz:Some(self.xtal_hz),apb_clocked:[tg0&1!=0,tg1&1!=0],in_reset:[tg0&2!=0,tg1&2!=0]}
    }
    // The strict cold-ROM profile only qualifies disabled RMT PCR states.
    // The unselected SRAM runtime retains its existing bare RMT behavior.
    pub(crate) fn rmt_clocked(&self)->bool{self.boot_peripheral_clock.as_ref().is_none_or(|c|c.rmt_clocked())}
    fn rmt_address(a:u32)->bool{matches!(a,0x6000_6000..=0x6000_6070|0x6000_6400..=0x6000_66fc)}
    fn cache_error_word(a:u32)->bool{matches!(a,0x600c_83d0|0x600c_8164|0x600c_8168|0x600c_816c|0x600c_8170|0x600c_8174)}
    fn usb_monitor_word(a:u32)->bool{matches!(a,0x6000_f008|0x6000_f00c|0x6000_f010|0x6000_f014)}
    pub(crate) fn usb_monitor_clock_qualified(&self)->bool{
        // Fixed C5 PCR_USB+94 named cold POR: CLK_EN1/RST_EN0. This
        // profile accepts no PCR_USB writes, so neither is silently changed.
        // Real APB and PLL48-tap observations are additional prerequisites;
        // none of these establishes PHY readiness or an attached USB host.
        self.has_validated_rom()&&self.boot_clock.virtual_profile_enabled()&&!self.reset_requested&&
            self.boot_clock.apb_hz().is_some()&&self.boot_clock.pll_tap_hz(48)==Some(48_000_000)
    }
    fn refresh_boot_brownout(&mut self)->bool{
        let threshold=self.boot_analog.as_ref().and_then(|a|a.bod_threshold());
        self.boot_brownout.as_mut().is_none_or(|b|b.observe_threshold(threshold))
    }
    fn cache_cpu_access(&mut self,instruction:bool)->Result<(),Fault>{
        if self.cache.cpu_allowed(instruction){return Ok(());}
        // Only a genuine observed CPU request can feed the shared-cache fail
        // tracer. PMA/APM already ran; host inspection and DMA do not use it.
        if self.boot_clock.virtual_profile_enabled()&&self.monitor.as_ref().is_some_and(|m|m.borrow().machine_context().is_some()){
            let irq=self.cache.irq();let result=self.cache.observe_cpu_failure(instruction);self.irq_dirty|=irq!=self.cache.irq();
            result.map_err(|_|Fault::Unsupported)?;
        }
        Err(Fault::Prohibited)
    }
    pub(crate) fn pump_boot_watchdogs(&mut self){
        // A dispatched reset is a boundary, not permission to drain the next
        // stage or overwrite the winning cause before reboot applies domains.
        if self.reset_requested{return;}
        let irq=self.lp_wdt.irq();let timg_irq=self.timg_wdt.sources();
        let event=if self.timg_wdt.virtual_profile_enabled(){
            // Preview two fixed-size controllers, then commit only through
            // the first reset. A late host advance must not move retained LP
            // state past an earlier HP reset. No per-stage catch-up loop.
            let inputs=self.timg_wdt_inputs();let mut lp=self.lp_wdt.clone();let mut tg=self.timg_wdt.clone();
            lp.pump(self.cycles);tg.pump(self.cycles,inputs);
            let a=lp.take_reset_event().map(|(cause,at)|(cause.reason(),at));let b=tg.take_reset_event();
            // TRM1.1 Table9.1-1: SWD/RWDT system/core outrank MWDT0
            // Core at equal times; either MWDT Core outranks RWDT CPU.
            // The TIMG owner already resolves a same-cycle TG0/TG1 tie.
            let event=match(a,b){(Some(a),Some(b))=>Some(if b.1<a.1||b.1==a.1&&a.0==0x0d{b}else{a}),(a,b)=>a.or(b)};
            if let Some((_,at))=event{
                self.lp_wdt.pump(at);self.timg_wdt.pump(at,inputs);
                self.lp_wdt.take_reset_event();self.timg_wdt.take_reset_event();
            }else{self.lp_wdt=lp;self.timg_wdt=tg;}event
        }else{self.lp_wdt.pump(self.cycles);self.lp_wdt.take_reset_event().map(|(cause,at)|(cause.reason(),at))};
        self.irq_dirty|=irq!=self.lp_wdt.irq()||timg_irq!=self.timg_wdt.sources();
        if let Some((cause,at))=event{
            // Finish only an OCODE event strictly before this reset. A late
            // service must neither lose an earlier DONE nor create a later
            // one. Reset wins at the same nominal cycle; feature-off is idle.
            if let Some(a)=self.boot_analog.as_mut().filter(|a|a.virtual_ocode_enabled()){a.pump(at.saturating_sub(1));}
            let inputs=self.security_inputs();
            if let Some(s)=self.boot_security.as_mut().filter(|s|s.deadline().is_some_and(|d|d<at)){s.pump(at.saturating_sub(1),inputs);}
            self.reset_requested=true;self.reset_cause=u32::from(cause)|1<<5;self.watchdog_reset_at=Some(at);
        }
    }
    fn i2c_read(&mut self,a:u32)->Option<u32>{let mut i=self.i2c.take().unwrap_or_else(||Box::new(crate::i2c::C5I2c::new(self.cycles)));let v=i.read(a,self);self.i2c=Some(i);self.irq_dirty=true;v}
    fn i2c_write(&mut self,a:u32,v:u32)->bool{let mut i=self.i2c.take().unwrap_or_else(||Box::new(crate::i2c::C5I2c::new(self.cycles)));let ok=i.write(a,v,self);self.i2c=Some(i);self.irq_dirty=true;ok}
    fn transport_read(&mut self,a:u32)->Option<u32>{let mut t=self.transport.take().unwrap_or_else(||Box::new(crate::transport::Transport::new(self.cycles)));t.set_clock_sources(self.cycles,self.boot_clock.rc_fast_hz(),self.clock_inputs().pll_ready,self.boot_clock.pll_div_gate_mask().unwrap_or(0x1ff));let v=t.read(a,self);t.pump(self);self.transport=Some(t);self.irq_dirty=true;v}
    fn transport_write(&mut self,a:u32,value:u32)->bool{let mut t=self.transport.take().unwrap_or_else(||Box::new(crate::transport::Transport::new(self.cycles)));t.set_clock_sources(self.cycles,self.boot_clock.rc_fast_hz(),self.clock_inputs().pll_ready,self.boot_clock.pll_div_gate_mask().unwrap_or(0x1ff));let ok=t.write(a,value,self);if ok{t.pump(self);}self.transport=Some(t);ok}
    fn source_status(&self,word:usize)->u32{
        let mut v=self.sources().into_iter().filter(|&(s,on)|on&&s/32==word).fold(0,|v,(s,_)|v|1<<(s%32));
        if let Some(t)=&self.transport{v|=t.sources().into_iter().filter(|&(s,on)|on&&s/32==word).fold(0,|v,(s,_)|v|1<<(s%32));}v
    }
    // C5 IDF5.5.4 pcr_reg.h UART0 +0/4/8 and UART1 +c/10/14.
    // Functional reset reconstructs the controller; already transmitted host
    // console bytes are history, not UART RAM. Powered-down RAM is unsupported.
    fn uart_pcr_read(&self,off:u32)->Option<u32>{let n=(off/12) as usize;if n>=2{return None;}Some(match off%12{
        0=>self.uart_conf[n]|if self.uart_conf[n]&2==0{4}else{0},4=>self.uart_clock[n],8=>self.uart_power[n],_=>return None,
    })}
    fn uart_pcr_write(&mut self,off:u32,value:u32)->bool{let n=(off/12) as usize;if n>=2{return false;}match off%12{
        0=>{
            if value&2!=0{
                let history=std::mem::take(&mut self.uart[n].tx_out);
                self.uart[n]=Uart::new(UartLayout{thrhd_mask:0xff,rxfifo_rst:1<<22,rxfifo_cnt_mask:0xff,autobaud_en:0,autobaud_regs:0,autobaud_clock_hz:0});
                self.uart[n].tx_out=history;
            }self.uart_conf[n]=value&3;
        },
        4=>{if value&(3<<20)!=0{return false;}self.uart_clock[n]=value&0x7f_ffff;},
        8=>{if value&4!=0{return false;}self.uart_power[n]=value&6;},_=>return false,
    }true}
    pub(crate) fn uart_receiving(&self,n:usize)->bool{n<2&&self.uart_conf[n]&2==0&&self.uart_clock[n]&(1<<22)!=0}
    fn uart_access(&self,n:usize,off:u32)->bool{self.uart_conf[n]&3==1&&(off!=0||self.uart_receiving(n))}
    fn systimer_read(&mut self,a:u32)->Option<u32>{let now=self.cycles;let s=self.systimer.get_or_insert_with(||Box::new(crate::systimer::C5Systimer::new(now)));let v=s.read(a,now);self.irq_dirty=true;v}
    fn systimer_write(&mut self,a:u32,v:u32)->bool{let now=self.cycles;let s=self.systimer.get_or_insert_with(||Box::new(crate::systimer::C5Systimer::new(now)));let ok=s.write(a,v,now);self.irq_dirty=true;ok}
    pub(crate) fn cpu_monitor(&mut self)->crate::cpu_monitor::Monitor{
        self.monitor.get_or_insert_with(||std::rc::Rc::new(std::cell::RefCell::new(crate::cpu_monitor::CpuMonitor::new()))).clone()
    }
    fn cpu_apm_supported(&self)->bool{
        let Some(a)=&self.boot_apm else{return true;};
        // No context means a host/component inspection, not a CPU world.
        // Actual guest instructions must come from the constrained observer.
        self.monitor.as_ref().and_then(|m|m.borrow().machine_context()).is_none_or(|machine|a.access_supported(0,machine))
    }
    pub(crate) fn dma_apm_supported(&self)->bool{
        self.boot_apm.as_ref().is_none_or(|a|a.all_filters_disabled()&&a.all_masters_tee_selected())
    }
    fn apm_cpu_context(&self)->bool{self.monitor.as_ref().and_then(|m|m.borrow().machine_context())==Some(true)}
    pub fn memory_policy_unsupported(&self)->Option<(riscv_rv32::c5_memory_protection::Access,riscv_rv32::c5_memory_protection::UnsupportedReason)>{
        self.monitor.as_ref().and_then(|m|m.borrow().memory_unsupported())
    }
    fn cpu_memory_check(&self,address:u32,width:u8,kind:riscv_rv32::c5_memory_protection::AccessKind)->Result<(),Fault>{
        self.monitor.as_ref().map_or(Ok(()),|m|m.borrow_mut().check_memory(riscv_rv32::c5_memory_protection::Access{address,width,kind}))
    }
    fn monitor_access(&mut self,address:u32,width:u8,write:bool){
        let Some(m)=&self.monitor else{return;};
        // C5 CPU data bus memory windows versus its HP peripheral window. CLIC
        // core-local traffic is excluded. DMA uses DmaMemory, never this hook.
        let bus=if (0x6000_0000..0x6010_0000).contains(&address){crate::bus_monitor::CpuBus::Peripheral}
            else if [(ROM_BASE,ROM_SIZE),(SRAM_BASE,SRAM_SIZE),(LP_BASE,LP_SIZE),(0x4200_0000,0x200_0000)].into_iter().any(|(b,n)|Self::range(address,width as usize,b,n).is_some()){crate::bus_monitor::CpuBus::Data}
            else{return;};
        let mut m=m.borrow_mut();let before=m.regs.irq();m.access(bus,address,width,write);self.irq_dirty|=before!=m.regs.irq();
    }
    pub(crate) fn clock_inputs(&self)->crate::clock_tree::ClockInputs{
        self.boot_analog.as_ref().map_or_else(crate::clock_tree::ClockInputs::default,|a|crate::clock_tree::ClockInputs{
            pll_ready:a.pll_ready(),rc_fast_powered:a.rc_fast_powered(),slow_dcap:a.slow_dcap(),
        })
    }
    pub(crate) fn security_inputs(&self)->crate::boot_security::Inputs{
        use crate::boot_security::Inputs;
        if !self.has_validated_rom()||!self.boot_clock.virtual_profile_enabled()||self.reset_requested{return Inputs::default();}
        let(dpa,encrypted,forced)=self.efuse.boot_security_policy();
        // Explicit cold/ideal-awake virtual HP-domain availability, not PMU
        // target fields proving physical power/settling. Unknown owners fail
        // closed; APB/source and KM SRAM/gate/reset remain separate guards.
        let domain=self.boot_analog.is_some()&&self.boot_pmu.as_ref().is_some_and(|p|
            p.read(0x600b_0000)==Some(0)&&p.read(0x600b_0004)==Some(u32::MAX)&&p.read(0x600b_0008)==Some(u32::MAX));
        Inputs{sec_source_hz:self.boot_clock.sec_source_hz(),apb_available:self.boot_clock.apb_hz().is_some(),crypto_domain_available:domain,
            efuse_dpa_level:Some(dpa),flash_encryption_enabled:Some(encrypted),force_km_flash:Some(forced)}
    }
    fn pump_boot_security_to(&mut self,cycle:u64)->bool{
        if self.boot_security.is_none(){return true;}let inputs=self.security_inputs();
        self.boot_security.as_mut().is_none_or(|s|s.pump(cycle,inputs))
    }
    pub(crate) fn pump_boot_security(&mut self)->bool{
        self.pump_boot_watchdogs();if self.reset_requested{return true;}self.pump_boot_security_to(self.cycles)
    }
    pub(crate) fn modem_clock_inputs(&self)->crate::modem_lp_clock::ClockInputs{
        crate::modem_lp_clock::ClockInputs{rc_slow_hz:self.boot_clock.rc_slow_osc_hz(),rc_fast_hz:self.boot_clock.rc_fast_osc_hz(),xtal_hz:Some(self.xtal_hz)}
    }
    /// Native diagnostics only; pure state snapshots never issue guest MMIO.
    pub fn psram_command_debug(&self)->String{
        let pads=[15,17,18,20,21,22].map(|pin|(pin,self.io_mux.read(4+pin*4)));
        format!("{} iomux={pads:08x?} psram0={:02x?}",self.mspi.psram_command_debug(),self.psram.get(..4))
    }
    /// Cold mapping/gate snapshot only; it does not perform a CPU memory access
    /// or establish the caller's APM/PMA/PMP permissions.
    pub fn mapped_memory_debug(&self,address:u32)->String{
        format!("{} cache_fetch={} cache_data={} flash_bytes={} psram_bytes={}",self.mspi.mapping_debug(address),self.cache.cpu_allowed(true),self.cache.cpu_allowed(false),self.flash.len(),self.psram.len())
    }
    /// Bounded host backing inspection: no guest load, CPU permission claim,
    /// cache request or mutation. Only the hash and first16 bytes are returned.
    pub fn mapped_backing_debug(&self,address:u32,len:usize)->Option<String>{
        if len==0||len>65536{return None;}
        let last=address.checked_add(u32::try_from(len-1).ok()?)?;let (ram,offset)=self.mspi.translate(address)?;
        for a in address..=last{if self.mspi.translate(a)!=Some((ram,offset.checked_add((a-address)as usize)?)){return None;}}
        let memory=if ram{&self.psram}else{&self.flash};let data=memory.get(offset..offset.checked_add(len)?)?;
        let hash=crate::image::sha256(data).iter().map(|b|format!("{b:02x}")).collect::<String>();
        Some(format!("host_backing_only address={address:#010x} len={len} psram={ram} physical_offset={offset:#x} sha256={hash} prefix={:02x?}",&data[..data.len().min(16)]))
    }
    pub fn spi_transport_debug(&self)->String{self.transport.as_ref().map_or_else(||"SPI2=uninitialized".into(),|t|t.spi_debug(self))}
    /// Cold native observation only; no MMIO or FIFO/interrupt mutation.
    pub fn detached_usb_debug(&self)->String{
        format!("qualified={} raw_without_clock_guard={:?} raw_qualified={:?} ena={:?} tx_data_free={:?} APB={:?} PLL48={:?} strict={}",
            self.usb_monitor_clock_qualified(),self.usb.read_monitor(8,true),self.usb.read_monitor(8,self.usb_monitor_clock_qualified()),
            self.usb.read_monitor(0x10,true),self.usb.read(4),self.boot_clock.apb_hz(),self.boot_clock.pll_tap_hz(48),self.has_validated_rom())
    }
    fn saradc_inputs(&self)->crate::boot_saradc::ClockInputs{
        crate::boot_saradc::ClockInputs{xtal_hz:Some(self.xtal_hz),reserved_path_ready:self.boot_analog.as_ref().is_some_and(|a|a.sar_reserved_path_ready())}
    }
    pub(crate) fn pump_boot_saradc(&mut self)->bool{
        if self.reset_requested{return true;}let inputs=self.saradc_inputs();
        self.boot_saradc.as_mut().is_none_or(|s|s.pump(self.cycles,inputs))
    }
    fn saradc_read(&mut self,address:u32)->Option<u32>{
        self.pump_boot_watchdogs();if self.reset_requested{return None;}let inputs=self.saradc_inputs();
        self.boot_saradc.get_or_insert_with(||Box::new(crate::boot_saradc::BootSarAdc::new())).read_at(address,self.cycles,inputs)
    }
    fn saradc_write(&mut self,address:u32,value:u32)->bool{
        self.pump_boot_watchdogs();if self.reset_requested{return false;}let inputs=self.saradc_inputs();
        self.boot_saradc.get_or_insert_with(||Box::new(crate::boot_saradc::BootSarAdc::new())).write_at(address,value,self.cycles,inputs)
    }
    fn refresh_clock_sources(&mut self){
        // Normal clock publication must not cross an unapplied reset. Raw
        // deselection still withdraws cached sources after disabling profile.
        if self.boot_clock.virtual_profile_enabled(){self.pump_boot_watchdogs();if self.reset_requested{return;}}
        let rc=self.boot_clock.rc_fast_hz();let pll=self.clock_inputs().pll_ready;self.mspi.set_clock_sources(rc,pll);
        if let Some(mut t)=self.transport.take(){t.set_clock_sources(self.cycles,rc,pll,self.boot_clock.pll_div_gate_mask().unwrap_or(0x1ff));t.publish_pwm(self);self.transport=Some(t);self.irq_dirty=true;}
        if self.boot_clock.virtual_profile_enabled(){
            let fast=self.boot_clock.rtc_fast_hz();self.lp_wdt.set_fast_clock(self.cycles,fast);
            let slow=self.boot_clock.rtc_slow_hz();self.lp_wdt.set_slow_clock(self.cycles,slow);self.lp_timer.set_slow_clock(self.cycles,slow);
            let xtal=self.boot_clock.cpu_uses_xtal();if let Some(a)=self.boot_analog.as_mut().filter(|a|a.virtual_ocode_enabled()){a.observe_ocode_cpu_xtal(self.cycles,xtal);}
        }
        self.pump_boot_security_to(self.cycles);
    }
    pub(crate) fn pump_boot_clock(&mut self){
        if !self.boot_clock.virtual_profile_enabled(){return;}
        self.pump_boot_watchdogs();if self.reset_requested{return;}
        let inputs=self.clock_inputs();self.boot_clock.pump(self.cycles,inputs);self.refresh_clock_sources();
    }
    pub(crate) fn select_validated_rom_profile(&mut self)->Result<(),String>{
        self.pump_boot_watchdogs();if self.reset_requested{return Err("C5 ROM profile requires applying the pending reset first".into());}
        if self.boot_peripheral_clock.is_none()&&(self.rmt.rmt.is_running()||self.rmt.rmt.irq()||!self.rmt.rmt.done.is_empty()){
            return Err("C5 cold ROM requires an idle legacy RMT controller".into());
        }
        let mut lp=self.lp_wdt.clone();let mut tg=self.timg_wdt.clone();
        let mut timer=self.lp_timer.clone();timer.enable_virtual_profile(self.cycles);
        let mut sar=self.boot_saradc.as_deref().cloned().unwrap_or_else(crate::boot_saradc::BootSarAdc::new);
        if !lp.enable_virtual_rwdt(self.cycles,self.efuse.wdt_delay_sel()){return Err("C5 RTC watchdog state cannot select the validated-ROM profile".into());}
        tg.enable_cold_spi_boot(self.cycles,self.timg_wdt_inputs())?;
        // Explicit ideal raw codes for the internal RNG compatibility path.
        // They have no voltage, calibration, noise or entropy interpretation.
        sar.enable_virtual_profile(self.cycles,[2048,2048])?;
        self.lp_wdt=lp;self.timg_wdt=tg;self.lp_timer=timer;self.boot_saradc=Some(Box::new(sar));
        self.boot_apm.get_or_insert_with(||Box::new(crate::boot_apm::BootApm::new()));
        self.boot_peripheral_clock.get_or_insert_with(||Box::new(crate::boot_peripheral_clock::BootPeripheralClock::new()));
        self.boot_brownout.get_or_insert_with(||Box::new(crate::boot_brownout::BootBrownout::new())).enable_ideal_usb3v3();
        self.software_interrupt.get_or_insert_with(||Box::new(crate::software_interrupt::SoftwareInterrupt::new()));
        self.boot_security.get_or_insert_with(||Box::new(crate::boot_security::BootSecurity::new()));
        // LP_CLKRST+0x14 CPU-reset permission remains at its pinned header POR
        // value0; writes to that otherwise unmodeled control still reject.
        self.lp_wdt.set_cpu_reset_enabled(false);
        self.mspi.enable_mp_rom_compat();self.lp_wdt.enable_virtual_swd(self.cycles);self.boot_clock.enable_virtual_profile(self.cycles);
        self.boot_analog.get_or_insert_with(||Box::new(crate::boot_analog::C5BootAnalog::virtual_usb3v3())).enable_virtual_pll(self.cycles);
        self.boot_analog.as_mut().unwrap().enable_virtual_sar();
        self.boot_analog.as_mut().unwrap().enable_virtual_ocode_ideal(self.cycles);
        self.boot_analog.as_mut().unwrap().enable_virtual_bod_threshold();
        self.pump_boot_clock();self.irq_dirty=true;Ok(())
    }
    pub(crate) fn deselect_validated_rom_profile(&mut self){
        self.wifi_compat=None;
        self.boot_security=None;
        if let Some(b)=&mut self.boot_brownout{b.disable_ideal_profile();}self.boot_brownout=None;
        self.mspi.disable_mp_rom_compat();self.lp_wdt.disable_virtual_swd();self.lp_wdt.disable_virtual_rwdt();self.timg_wdt.disable_virtual_profile();self.boot_clock.disable_virtual_profile();
        self.lp_timer.disable_virtual_profile();
        self.boot_apm=None;
        self.boot_peripheral_clock=None;
        self.software_interrupt=None;
        self.modem_lp_clock=None;
        self.modem_wifi_clock=None;
        if let Some(s)=&mut self.boot_saradc{s.disable_virtual_profile();}
        if let Some(a)=&mut self.boot_analog{a.disable_virtual_bod_threshold();a.disable_virtual_pll();a.disable_virtual_sar();a.disable_virtual_ocode();}self.refresh_clock_sources();self.irq_dirty=true;
    }
    pub(crate) fn pump_boot_analog(&mut self){
        self.pump_boot_clock();if self.reset_requested{return;}
        if let Some(a)=&mut self.boot_analog{a.pump(self.cycles);}self.pump_boot_clock();
    }
    fn boot_analog_read(&mut self,address:u32)->Option<u32>{
        self.pump_boot_clock();if self.reset_requested||!self.pump_boot_saradc(){return None;}
        let v=self.boot_analog.get_or_insert_with(||Box::new(crate::boot_analog::C5BootAnalog::virtual_usb3v3())).read_at(address,self.cycles);
        self.pump_boot_clock();if self.pump_boot_saradc(){v}else{None}
    }
    fn boot_analog_write(&mut self,address:u32,value:u32)->bool{
        self.pump_boot_clock();if self.reset_requested||!self.pump_boot_saradc(){return false;}
        // A live reserved ADC route is part of this bounded profile. Reject
        // an unsupported route withdrawal before publishing its new clock
        // observations; preserve elapsed sampling before the attempted write.
        let before=(self.boot_saradc.as_ref().is_some_and(|s|s.virtual_timer_enabled())||self.boot_brownout.is_some()).then(||(self.boot_analog.clone(),self.boot_saradc.clone(),self.boot_brownout.clone()));
        let ok=self.boot_analog.get_or_insert_with(||Box::new(crate::boot_analog::C5BootAnalog::virtual_usb3v3())).write_at(address,value,self.cycles);
        let known=self.refresh_boot_brownout()&&self.pump_boot_saradc();if !known{if let Some((a,s,b))=before{self.boot_analog=a;self.boot_saradc=s;self.boot_brownout=b;}return false;}
        self.pump_boot_clock();ok
    }
    fn mmio_read(&mut self, a: u32) -> Option<u32> {
        // C5 TRM1.1 8.6.2 / IDF5.5.3,5.5.4 lp_aon_reg.h: SEL[7:0]
        // resets0 (HP). Only that inactive-LP subset exists; no LP pad engine.
        if a==LP_AON_GPIO_MUX&&self.has_validated_rom(){self.pump_boot_watchdogs();return (!self.reset_requested).then_some(0);}
        if a==ORIGINAL_WIFI_MAC_BOUNDARY&&self.has_validated_rom(){self.pump_boot_watchdogs();return None;}
        if crate::modem_wifi_clock::ModemWifiClock::contains(a){
            self.pump_boot_watchdogs();if self.reset_requested||!self.has_validated_rom(){return None;}
            let apb=self.boot_analog.as_ref().is_some_and(|p|p.modem_apb_accessible());if !apb{return None;}
            return self.modem_wifi_clock.get_or_insert_with(||Box::new(crate::modem_wifi_clock::ModemWifiClock::new())).read(a,apb);
        }
        if Self::usb_monitor_word(a)||a==0x6009_6094{
            self.pump_boot_watchdogs();if self.reset_requested||!self.has_validated_rom()||a==0x6009_6094{return None;}
            return self.usb.read_monitor(a-0x6000_f000,self.usb_monitor_clock_qualified());
        }
        if crate::boot_security::BootSecurity::contains(a){
            self.pump_boot_watchdogs();if self.reset_requested||!self.has_validated_rom(){return None;}
            let inputs=self.security_inputs();return self.boot_security.as_mut()?.read_at(a,self.cycles,inputs);
        }
        if crate::boot_brownout::BootBrownout::contains(a){
            self.pump_boot_watchdogs();if self.reset_requested||!self.has_validated_rom()||!self.refresh_boot_brownout(){return None;}
            return self.boot_brownout.as_ref()?.read(a);
        }
        if self.boot_clock.virtual_profile_enabled()&&(a==0x6009_6104||(0x600c_8000..=0x600c_83fc).contains(&a)){
            self.pump_boot_watchdogs();if self.reset_requested{return None;}
            return if a==0x6009_6104{Some(self.cache.read_pcr())}else{self.cache.read(a-0x600c_8000)};
        }
        if crate::software_interrupt::SoftwareInterrupt::contains(a){
            self.pump_boot_watchdogs();if self.reset_requested||!self.has_validated_rom(){return None;}
            return self.software_interrupt.as_ref()?.read(a);
        }
        if crate::boot_peripheral_clock::BootPeripheralClock::contains(a){
            self.pump_boot_watchdogs();if self.reset_requested||!self.has_validated_rom(){return None;}
            return self.boot_peripheral_clock.as_ref()?.read(a);
        }
        if crate::modem_lp_clock::ModemLpClock::contains(a){
            self.pump_boot_watchdogs();if self.reset_requested||!self.has_validated_rom(){return None;}
            let apb=self.boot_analog.as_ref().is_some_and(|p|p.lp_apb_accessible());if !apb{return None;}
            return self.modem_lp_clock.get_or_insert_with(||Box::new(crate::modem_lp_clock::ModemLpClock::new())).read(a,apb);
        }
        if crate::boot_apm::BootApm::contains(a){self.pump_boot_watchdogs();if self.reset_requested{return None;}return self.boot_apm.as_ref()?.read(a,self.apm_cpu_context());}
        if crate::boot_pmu::BootPmu::contains(a){
            if !self.has_validated_rom(){return None;}self.pump_boot_watchdogs();if self.reset_requested{return None;}
            self.pump_boot_security_to(self.cycles);let value=self.boot_pmu.get_or_insert_with(||Box::new(crate::boot_pmu::BootPmu::new())).read(a);
            self.pump_boot_security_to(self.cycles);return value;
        }
        if crate::lp_timer::LpTimer::contains(a){self.pump_boot_watchdogs();if self.reset_requested{return None;}return self.lp_timer.read_at(a,self.cycles);}
        if crate::boot_saradc::BootSarAdc::contains(a){return self.saradc_read(a);}
        if crate::timg_wdt::TimgWdt::contains(a){self.pump_boot_watchdogs();if self.reset_requested{return None;}let inputs=self.timg_wdt_inputs();return self.timg_wdt.read_at(a,self.cycles,inputs);}
        if crate::boot_bias::C5BootBias::contains(a){return self.boot_bias.get_or_insert_with(||Box::new(crate::boot_bias::C5BootBias::virtual_usb3v3())).read(a);}
        if crate::boot_clock::BootClock::contains(a)&&self.boot_clock.virtual_profile_enabled(){self.pump_boot_watchdogs();if self.reset_requested{return None;}let inputs=self.clock_inputs();let value=self.boot_clock.read_at(a,self.cycles,inputs);self.refresh_clock_sources();return value;}
        if matches!(a,0x600b_00ac|0x600b_00cc|0x600a_f818|0x600b_000c|0x600b_00dc|0x600b_00d0|0x600a_9c0c|0x600a_f020|0x6009_6174|0x600a_f018|0x600a_f01c|0x600a_9c04|0x600b_0158|0x600b_2c04..=0x600b_2c0c|0x600a_f800|0x600a_f804|0x600a_f81c|0x600a_f820){return self.boot_analog_read(a);}
        if matches!(a,0x6008_9000..=0x6008_90fc|0x6009_60d0|0x6009_60e4|0x6009_60e8|0x6009_60ec){return self.sha.get_or_insert_with(||Box::new(crate::sha::C5Sha::new())).read(a);}
        if a==0x6009_6104{return Some(self.cache.read_pcr());}
        if crate::bus_monitor::C5BusMonitor::contains(a){return self.cpu_monitor().borrow().regs.read(a);}
        if crate::systimer::C5Systimer::contains(a){return self.systimer_read(a);}
        if crate::transport::Transport::contains(a){return self.transport_read(a);}
        if crate::i2c::C5I2c::contains(a){return self.i2c_read(a);}
        match a {
            0x6000_2000..=0x6000_33fc | 0x6009_6018..=0x6009_601c => self.mspi.read(a),
            0x600b_2800..=0x600b_2bfc=>{
                if self.boot_clock.virtual_profile_enabled(){self.pump_boot_watchdogs();if self.reset_requested{return None;}}
                self.lp_peri.read(a-0x600b_2800)
            },
            0x600b_1c00..=0x600b_1ffc=>{self.pump_boot_watchdogs();self.lp_wdt.read(a-0x600b_1c00)},
            0x600c_8000..=0x600c_83fc=>self.cache.read(a-0x600c_8000),
            0x600b_4800..=0x600b_49fc=>self.efuse.read(a-0x600b_4800),
            0x6000_f000..=0x6000_f004=>self.usb.read(a-0x6000_f000),
            // C5 LP_AON_STORE0..9 are retained software scratch words, not engine status.
            0x600b_1000..=0x600b_1024=>Some(self.lp_store[((a-0x600b_1000)/4) as usize]),
            // C5 IDF5.5.4 pmu_reg.h +0xe4: WT pad-hold commands self-clear.
            // Only release of the POR-unheld pads is supported; setting a hold
            // must fault until its physical pad latches are implemented.
            0x600b_00e4=>Some(0),
            // Pinned C5 PMU_HP_INT_ENA / LP_ANA_LP_INT_ENA reset0.
            // Only their disabled subset exists; BOD mode0/sleep events do not.
            0x600b_0168|0x600b_2c28|0x600b_2c2c=>Some(0),
            // LP_AON_GPIO_HOLD0 POR is zero. Holding pads is not implemented;
            // an explicit nonzero hold request rejects rather than disappearing.
            0x600b_102c=>Some(0),
            0x6009_6000..=0x6009_6014=>self.uart_pcr_read(a-0x6009_6000),
            0x6009_6110..=0x6009_6120=>self.boot_clock.read(a-0x6009_6000,self.xtal_hz),
            // IDF v5.5.4 lp_analog_peri_reg.h DATE default; lp_clkrst_reg.h cause;
            // esp_rom/esp32c5/include/esp32c5/rom/rtc.h POWER_ON=1.
            0x600b_2ffc => Some(self.lp_ana_date), 0x600b_0410 => Some(self.reset_cause),
            // C5 IDF5.5.3/5.5.4 lp_clkrst_reg.h +0: active POR mux is
            // RC_SLOW(0), XTAL/2 FAST(1). Selection only, not calibration.
            0x600b_0400=>Some(4),
            0x6000_0000..=0x6000_0098 if self.uart_access(0,a&0xfff)=>Some(self.uart[0].read(a&0xfff)),
            0x6000_1000..=0x6000_1098 if self.uart_access(1,a&0xfff)=>Some(self.uart[1].read(a&0xfff)),
            0x6000_6000..=0x6000_6070 | 0x6000_6400..=0x6000_66fc if self.rmt_clocked()=>Some(self.rmt.read(a&0xfff)),
            0x6009_0000..=0x6009_0070 | 0x6009_01fc => Some(self.io_mux.read((a & 0xfff) + 4)),
            0x6009_1000..=0x6009_1b44 => Self::gpio_offset(a & 0xfff).map(|o| self.gpio.read(o)),
            0x6001_0000..=0x6001_014c => Some(self.map[((a & 0xfff) / 4) as usize]),
            0x6001_0150..=0x6001_0158 => Some(self.source_status(((a-0x6001_0150)/4) as usize)),
            0x2080_0000 => Some(self.clic_config), 0x2080_0004 => Some(self.clic.read(4)),
            0x2080_1000..=0x2080_10bc => Some(self.clic.read(a - 0x2080_0000)),
            _ => None,
        }
    }
    fn mmio_write(&mut self, a: u32, value: u32, lanes: u32) -> bool {
        if a==LP_AON_GPIO_MUX&&self.has_validated_rom(){self.pump_boot_watchdogs();return !self.reset_requested&&lanes==15&&value==0;}
        self.irq_dirty = true;
        if (0x2080_1000..=0x2080_10bc).contains(&a) { self.clic.write(a - 0x2080_0000, value, lanes); return true; }
        if a==ORIGINAL_WIFI_MAC_BOUNDARY&&self.has_validated_rom(){self.pump_boot_watchdogs();return false;}
        if crate::modem_wifi_clock::ModemWifiClock::contains(a){
            self.pump_boot_watchdogs();if self.reset_requested||!self.has_validated_rom()||lanes!=15{return false;}
            let apb=self.boot_analog.as_ref().is_some_and(|p|p.modem_apb_accessible());if !apb{return false;}
            return self.modem_wifi_clock.get_or_insert_with(||Box::new(crate::modem_wifi_clock::ModemWifiClock::new())).write(a,value,apb);
        }
        if Self::usb_monitor_word(a)||a==0x6009_6094{
            self.pump_boot_watchdogs();if self.reset_requested||!self.has_validated_rom()||lanes!=15||a==0x6009_6094{return false;}
            let qualified=self.usb_monitor_clock_qualified();return self.usb.write_monitor(a-0x6000_f000,value,qualified);
        }
        if crate::boot_security::BootSecurity::contains(a){
            self.pump_boot_watchdogs();if self.reset_requested||!self.has_validated_rom()||lanes!=15{return false;}
            let inputs=self.security_inputs();return self.boot_security.as_mut().is_some_and(|s|s.write_at(a,value,self.cycles,inputs));
        }
        if crate::modem_lp_clock::ModemLpClock::contains(a){
            self.pump_boot_watchdogs();if self.reset_requested||!self.has_validated_rom()||lanes!=15{return false;}
            let apb=self.boot_analog.as_ref().is_some_and(|p|p.lp_apb_accessible());if !apb{return false;}
            return self.modem_lp_clock.get_or_insert_with(||Box::new(crate::modem_lp_clock::ModemLpClock::new())).write(a,value,apb);
        }
        if a==0x600a_f018{
            // The named LPCON gate request must not pass an earlier reset,
            // including when a partial write will itself be unsupported.
            self.pump_boot_watchdogs();if self.reset_requested||lanes!=15{return false;}
            return self.boot_analog_write(a,value);
        }
        if lanes != 15 { return false; } // other subword MMIO deliberately unsupported
        if crate::boot_brownout::BootBrownout::contains(a){
            self.pump_boot_watchdogs();if self.reset_requested||!self.has_validated_rom()||!self.refresh_boot_brownout(){return false;}
            return self.boot_brownout.as_mut().is_some_and(|b|b.write(a,value));
        }
        if self.boot_clock.virtual_profile_enabled()&&(a==0x6009_6104||(0x600c_8000..=0x600c_83fc).contains(&a)){
            self.pump_boot_watchdogs();if self.reset_requested{return false;}
            return if a==0x6009_6104{self.cache.write_pcr(value)}else{self.cache.write(a-0x600c_8000,value)};
        }
        if crate::software_interrupt::SoftwareInterrupt::contains(a){
            self.pump_boot_watchdogs();if self.reset_requested||!self.has_validated_rom(){return false;}
            return self.software_interrupt.as_mut().is_some_and(|s|s.write(a,value));
        }
        if crate::boot_peripheral_clock::BootPeripheralClock::contains(a){
            self.pump_boot_watchdogs();if self.reset_requested||!self.has_validated_rom(){return false;}
            return self.boot_peripheral_clock.as_mut().is_some_and(|c|c.write(a,value));
        }
        if crate::boot_apm::BootApm::contains(a){self.pump_boot_watchdogs();if self.reset_requested{return false;}let machine=self.apm_cpu_context();return self.boot_apm.as_mut().is_some_and(|p|p.write(a,value,machine));}
        if crate::boot_pmu::BootPmu::contains(a){
            if !self.has_validated_rom(){return false;}self.pump_boot_watchdogs();if self.reset_requested{return false;}
            self.pump_boot_security_to(self.cycles);let ok=self.boot_pmu.get_or_insert_with(||Box::new(crate::boot_pmu::BootPmu::new())).write(a,value);
            self.pump_boot_security_to(self.cycles);return ok;
        }
        if crate::lp_timer::LpTimer::contains(a){self.pump_boot_watchdogs();return !self.reset_requested&&self.lp_timer.write_at(a,value,self.cycles);}
        if crate::boot_saradc::BootSarAdc::contains(a){return self.saradc_write(a,value);}
        if crate::timg_wdt::TimgWdt::contains(a){self.pump_boot_watchdogs();if self.reset_requested{return false;}let inputs=self.timg_wdt_inputs();return self.timg_wdt.write_at(a,value,self.cycles,inputs);}
        if crate::boot_bias::C5BootBias::contains(a){
            self.pump_boot_watchdogs();if self.reset_requested{return false;}let awake=self.has_validated_rom();
            let bias=self.boot_bias.get_or_insert_with(||Box::new(crate::boot_bias::C5BootBias::virtual_usb3v3()));
            return if awake{bias.write_awake_config(a,value)}else{bias.write(a,value)};
        }
        if crate::boot_clock::BootClock::contains(a)&&self.boot_clock.virtual_profile_enabled(){self.pump_boot_watchdogs();if self.reset_requested{return false;}let inputs=self.clock_inputs();let ok=self.boot_clock.write_at(a,value,self.cycles,inputs);self.refresh_clock_sources();return ok;}
        if matches!(a,0x600b_00ac|0x600b_00cc|0x600a_f818|0x600b_000c|0x600b_00dc|0x600b_00d0|0x600a_9c0c|0x600a_f020|0x6009_6174|0x600a_f018|0x600a_f01c|0x600a_9c04|0x600b_0158|0x600b_2c04..=0x600b_2c0c|0x600a_f800|0x600a_f804|0x600a_f81c|0x600a_f820){return self.boot_analog_write(a,value);}
        if matches!(a,0x6008_9000..=0x6008_90fc|0x6009_60d0|0x6009_60e4|0x6009_60e8|0x6009_60ec){return self.sha.get_or_insert_with(||Box::new(crate::sha::C5Sha::new())).write(a,value);}
        if a==0x6009_6104{return self.cache.write_pcr(value);}
        if crate::bus_monitor::C5BusMonitor::contains(a){return self.cpu_monitor().borrow_mut().regs.write(a,value);}
        if crate::systimer::C5Systimer::contains(a){return self.systimer_write(a,value);}
        if crate::transport::Transport::contains(a){return self.transport_write(a,value);}
        if crate::i2c::C5I2c::contains(a){return self.i2c_write(a,value);}
        match a {
            0x600b_0168|0x600b_2c28=>return value==0,
            0x600b_2c2c=>return value&!0x8000_0000==0, // WT clear; no enabled BOD0 source
            // Only the existing active LP mux is implemented; alternate
            // sources/dividers cannot silently desynchronize SWD pulse time.
            0x600b_0400=>return value==4,
            0x6000_2000..=0x6000_33fc | 0x6009_6018..=0x6009_601c => return self.mspi.write(a, value, &mut self.flash, &mut self.psram, false),
            0x600b_2800..=0x600b_2bfc=>{
                if self.boot_clock.virtual_profile_enabled(){self.pump_boot_watchdogs();if self.reset_requested{return false;}}
                return self.lp_peri.write(a-0x600b_2800,value);
            },
            0x600b_1c00..=0x600b_1ffc=>{self.pump_boot_watchdogs();return !self.reset_requested&&self.lp_wdt.write(a-0x600b_1c00,value);},
            0x600c_8000..=0x600c_83fc=>return self.cache.write(a-0x600c_8000,value),
            0x600b_4800..=0x600b_49fc=>return self.efuse.write(a-0x600b_4800,value),
            0x6000_f000..=0x6000_f004=>return self.usb.write(a-0x6000_f000,value),
            0x600b_1000..=0x600b_1024=>self.lp_store[((a-0x600b_1000)/4) as usize]=value,
            0x600b_00e4=>return value&!0xa000_0000==0,
            0x600b_102c=>return value==0,
            0x6009_6000..=0x6009_6014=>return self.uart_pcr_write(a-0x6009_6000,value),
            0x6009_6110..=0x6009_6120=>return self.boot_clock.write(a-0x6009_6000,value),
            0x600b_2ffc => self.lp_ana_date = value & 0x7fff_ffff,
            // lp_clkrst_reg.h: CORE0_RESET_FLAG bit5 resets1; independent cause
            // clear bit29, flag SET bit30 and CLR bit31 are write triggers.
            0x600b_0410 => {
                if value & (1 << 29) != 0 { self.reset_cause &= !0x1f; }
                if value & (1 << 30) != 0 { self.reset_cause |= 1 << 5; }
                if value & (1 << 31) != 0 { self.reset_cause &= !(1 << 5); }
            }
            0x6000_0000..=0x6000_0098 if self.uart_access(0,a&0xfff)=>self.uart[0].write(a&0xfff,value),
            0x6000_1000..=0x6000_1098 if self.uart_access(1,a&0xfff)=>self.uart[1].write(a&0xfff,value),
            0x6000_6000..=0x6000_6070 | 0x6000_6400..=0x6000_66fc if self.rmt_clocked()=>{self.rmt.write(a&0xfff,value);}
            0x6009_0000..=0x6009_0070 => {
                let pin = ((a & 0xfff) / 4) as u8; let value = value & 0x3ffff;
                self.io_mux.write((a & 0xfff) + 4, value); self.gpio.set_pad(pin, value);
                self.mspi.set_pad(pin, (value >> 12) & 7); self.publish_gpio();
            }
            0x6009_01fc => self.io_mux.write(0x200, value & 0x0fff_ffff),
            0x6009_1000..=0x6009_1b44 => { if let Some(o) = Self::gpio_offset(a & 0xfff) { if o != 0x38 { self.gpio.write(o, value); } self.publish_gpio(); } else { return false; } }
            0x6001_0000..=0x6001_014c => self.map[((a & 0xfff) / 4) as usize] = value & 0x3f,
            0x6001_0150..=0x6001_0158 => {},
            0x2080_0000 => { self.clic_config = value & 0xf; self.clic.mnlbits = (value & 0xf) << 1; }
            _ => return false,
        }
        true
    }
    // C5 TRM1.1 HP2.12.1.2 p123 permits ordinary unaligned split loads/stores;
    // Memory6.3.3.1 p242 places external Flash/RAM on the cache/MMU data path.
    // Qualify one complete backing, conservatively within one current MMU page.
    // This models functional values, not physical split latency/atomicity or
    // asynchronous bus-error timing. Whole-span APM/PMA/PMP and the existing
    // cache/MSPI/Quad die gates below still precede memory effects.
    fn ordinary_data_span(&self,address:u32,width:u8,write:bool)->bool{
        let internal=[(SRAM_BASE,self.sram.len()),(LP_BASE,self.lp_sram.len()),(ROM_BASE,if write{0}else{self.rom.len()})]
            .into_iter().any(|(base,len)|Self::range(address,width as usize,base,len).is_some());
        internal||self.mspi.translate_data_span(address,width).is_some_and(|(ram,offset)|
            (!write||ram)&&offset.checked_add(width as usize).is_some_and(|end|end<=if ram{self.psram.len()}else{self.flash.len()}))
    }
    fn read(&mut self, address: u32, width: u8,instruction:bool) -> Result<u32, Fault> {
        if address & (width as u32 - 1) != 0 {
            if instruction||!self.has_validated_rom(){return self.fail(address,width,false,Fault::Misaligned);}
            if !self.ordinary_data_span(address,width,false){return self.fail(address,width,false,Fault::Unsupported);}
        }
        if !self.cpu_apm_supported(){return self.fail(address,width,false,Fault::Prohibited);}
        let kind=if instruction{riscv_rv32::c5_memory_protection::AccessKind::Fetch}else{riscv_rv32::c5_memory_protection::AccessKind::Load};
        if let Err(f)=self.cpu_memory_check(address,width,kind){return self.fail(address,width,false,f);}
        if !instruction{self.monitor_access(address,width,false);}
        for (base, mem) in [(ROM_BASE, &self.rom), (SRAM_BASE, &self.sram), (LP_BASE, &self.lp_sram)] {
            if let Some(r) = Self::range(address, width as usize, base, mem.len()) { let mut bytes = [0; 4]; bytes[..width as usize].copy_from_slice(&mem[r]); return Ok(u32::from_le_bytes(bytes)); }
        }
        if (0x4200_0000..0x4400_0000).contains(&address){if let Err(f)=self.cache_cpu_access(instruction){return self.fail(address,width,false,f);}}
        if let Some((psram, offset)) = self.mspi.translate(address) {
            let memory = if psram { &self.psram } else { &self.flash };
            if (!psram || self.mspi.psram_access(false)) && offset.checked_add(width as usize).is_some_and(|end| end <= memory.len()) {
                let mut bytes = [0; 4]; bytes[..width as usize].copy_from_slice(&memory[offset..offset + width as usize]);
                return Ok(u32::from_le_bytes(bytes));
            }
        }
        if let Some(word) = self.mmio_read(address & !3) { return Ok((word >> ((address & 3) * 8)) & (u32::MAX >> (32 - width as u32 * 8))); }
        let fault=if Self::rmt_address(address&!3)&&!self.rmt_clocked()||Self::cache_error_word(address&!3)||self.boot_brownout.is_some()&&crate::boot_brownout::BootBrownout::contains(address&!3)||self.timg_wdt.virtual_profile_enabled()&&crate::timg_wdt::TimgWdt::contains(address&!3)||self.boot_clock.virtual_profile_enabled()&&crate::boot_clock::BootClock::contains(address&!3)||self.boot_security.is_some()&&crate::boot_security::BootSecurity::contains(address&!3)||self.has_validated_rom()&&(matches!(address&!3,ORIGINAL_WIFI_MAC_BOUNDARY|LP_AON_GPIO_MUX)||crate::modem_wifi_clock::ModemWifiClock::contains(address&!3)||crate::modem_lp_clock::ModemLpClock::contains(address&!3)||Self::usb_monitor_word(address&!3)||matches!(address&!3,0x6009_6094|0x600a_f018)){Fault::Unsupported}else{Fault::Unmapped};
        self.fail(address,width,false,fault)
    }
    fn write(&mut self, address: u32, value: u32, width: u8) -> Result<(), Fault> {
        if address & (width as u32 - 1) != 0 {
            if !self.has_validated_rom(){return self.fail(address,width,true,Fault::Misaligned);}
            if !self.ordinary_data_span(address,width,true){return self.fail(address,width,true,Fault::Unsupported);}
        }
        if !self.cpu_apm_supported(){return self.fail(address,width,true,Fault::Prohibited);}
        if let Err(f)=self.cpu_memory_check(address,width,riscv_rv32::c5_memory_protection::AccessKind::Store){return self.fail(address,width,true,f);}
        self.monitor_access(address,width,true);
        for (base, mem) in [(SRAM_BASE, &mut self.sram), (LP_BASE, &mut self.lp_sram)] {
            if let Some(r) = Self::range(address, width as usize, base, mem.len()) { mem[r].copy_from_slice(&value.to_le_bytes()[..width as usize]); return Ok(()); }
        }
        if (0x4200_0000..0x4400_0000).contains(&address){if let Err(f)=self.cache_cpu_access(false){return self.fail(address,width,true,f);}}
        if let Some((true, offset)) = self.mspi.translate(address) {
            if self.mspi.psram_access(true) && offset.checked_add(width as usize).is_some_and(|end| end <= self.psram.len()) {
                self.psram[offset..offset + width as usize].copy_from_slice(&value.to_le_bytes()[..width as usize]); return Ok(());
            }
        }
        let shift = address & 3;
        if self.mmio_write(address & !3, value << (shift * 8), ((1 << width) - 1) << shift) { return Ok(()); }
        let fault=if Self::rmt_address(address&!3)&&!self.rmt_clocked()||Self::cache_error_word(address&!3)||self.boot_brownout.is_some()&&crate::boot_brownout::BootBrownout::contains(address&!3)||self.timg_wdt.virtual_profile_enabled()&&crate::timg_wdt::TimgWdt::contains(address&!3)||self.boot_clock.virtual_profile_enabled()&&crate::boot_clock::BootClock::contains(address&!3)||self.boot_security.is_some()&&crate::boot_security::BootSecurity::contains(address&!3)||self.has_validated_rom()&&(matches!(address&!3,ORIGINAL_WIFI_MAC_BOUNDARY|LP_AON_GPIO_MUX)||crate::modem_wifi_clock::ModemWifiClock::contains(address&!3)||crate::modem_lp_clock::ModemLpClock::contains(address&!3)||Self::usb_monitor_word(address&!3)||matches!(address&!3,0x6009_6094|0x600a_f018)){Fault::Unsupported}else{Fault::Unmapped};
        self.fail(address,width,true,fault)
    }
}
impl Bus for C5Bus {
    fn check_atomic32(&mut self,a:u32,read:bool,write:bool)->Result<(),Fault>{
        if self.boot_apm.is_none()&&self.monitor.as_ref().is_none_or(|m|!m.borrow().policy_active()){return Ok(());}
        if a&3!=0{return self.fail(a,4,write,Fault::Misaligned);}
        if !read&&!write||!self.apm_cpu_context()||self.reset_requested{return self.fail(a,4,write,Fault::Prohibited);}
        use riscv_rv32::c5_memory_protection::AccessKind;
        let kind=match(read,write){(true,false)=>AccessKind::AtomicRead,(false,true)=>AccessKind::AtomicWrite,_=>AccessKind::AtomicRmw};
        if let Err(f)=self.cpu_memory_check(a,4,kind){return self.fail(a,4,write,f);}
        let internal=[(SRAM_BASE,self.sram.len()),(LP_BASE,self.lp_sram.len())].into_iter().any(|(base,n)|Self::range(a,4,base,n).is_some());
        let external=self.mspi.translate(a).is_some_and(|(ram,off)|ram&&off.checked_add(4).is_some_and(|end|end<=self.psram.len())&&(!read||self.mspi.psram_access(false))&&(!write||self.mspi.psram_access(true)));
        if external{if let Err(f)=self.cache_cpu_access(false){return self.fail(a,4,write,f);}}
        if internal||external{Ok(())}else{self.fail(a,4,write,Fault::Prohibited)}
    }
    fn note_code_page(&mut self, _addr: u32) {} // interpreter-only; no decoded block cache
    fn note_pc(&mut self, pc: u32) { self.pc = pc; self.misc.cur_pc = pc; }
    fn block_break(&self) -> bool { self.irq_dirty }
    fn tick(&mut self, cycles: u32) -> u32 { self.advance(cycles as u64); u32::from(self.irq_dirty) }
    fn read8(&mut self, a: u32) -> Result<u8, Fault> { self.read(a, 1,false).map(|v| v as u8) }
    fn read16(&mut self, a: u32) -> Result<u16, Fault> { self.read(a, 2,false).map(|v| v as u16) }
    fn read32(&mut self, a: u32) -> Result<u32, Fault> { self.read(a, 4,false) }
    fn write8(&mut self, a: u32, v: u8) -> Result<(), Fault> { self.write(a, v as u32, 1) }
    fn write16(&mut self, a: u32, v: u16) -> Result<(), Fault> { self.write(a, v as u32, 2) }
    fn write32(&mut self, a: u32, v: u32) -> Result<(), Fault> { self.write(a, v, 4) }
    fn fetch(&mut self, pc: u32) -> Result<[u8; 4], Fault> {
        if self.wifi_compat.as_ref().is_some_and(|w|w.limited()){return Err(Fault::Unsupported);}
        if ![(ROM_BASE, ROM_SIZE), (SRAM_BASE, SRAM_SIZE), (LP_BASE, LP_SIZE), (0x4200_0000, 0x200_0000)].into_iter().any(|(base, len)| Self::range(pc, 2, base, len).is_some()) { return self.fail(pc, 2, false, Fault::Prohibited); }
        let lo = self.read(pc,2,true)? as u16; let hi = if lo & 3 == 3 {
            if let Err(f)=self.cpu_memory_check(pc,4,riscv_rv32::c5_memory_protection::AccessKind::Fetch){return self.fail(pc,4,false,f);}
            self.read(pc.checked_add(2).ok_or(Fault::Unmapped)?,2,true)? as u16
        } else { 0 };
        Ok((u32::from(lo) | u32::from(hi) << 16).to_le_bytes())
    }
}

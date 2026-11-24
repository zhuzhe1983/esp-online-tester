//! Common Machine integration for the explicit S2 SRAM app-direct contract.
use crate::{S2Core, bus::{S2Bus, CPU_HZ, DRAM_BASE, SRAM_SIZE, MAX_STAGING_FLASH, valid_pin}};
use emu_core::Core;
use esp_soc::{BoardModel, Soc, SocBus};

pub struct S2;
pub type Machine = esp_soc::Machine<S2>;
pub fn machine(mac: [u8;6], staging_flash_size: usize) -> Machine {
    let mut bus=S2Bus::new(); bus.set_flash_size(staging_flash_size);
    let mut m=Machine::new(mac,bus); m.set_debug(&esp_soc::DebugFlags::from_env());
    m.dbg.stop_after_exceptions=1; m
}
impl Soc for S2 {
    type Core=S2Core; type Bus=S2Bus;
    const NAME:&'static str="esp32s2";
    const ROM_ELF:&'static str=""; // No ROM loading or ROM boot implementation.
    const CPU_HZ:u64=CPU_HZ; const CORES:usize=1; const IDLE_CHUNK:u64=64;
    const ROM_BOOT_DEFAULT:bool=false;
    const EXCEPTION_BUDGET_AFTER_RESET:Option<u64>=Some(1);
    const ROM_DATA_TABLE:&'static[&'static str]=&[];
    fn new_core(_i:usize)->S2Core {S2Core::new()}
    fn reset_core(c:&mut S2Core,_i:usize) {Core::reset(c);}
    fn reset_core_with_bus(c:&mut S2Core,bus:&mut S2Bus,i:usize) {
        Self::reset_core(c,i); bus.irq_dirty=true;
        if let Some(entry)=bus.app_entry {Self::boot_core(c,entry);}
    }
    fn boot_core(c:&mut S2Core,entry:u32) {
        Self::reset_core(c,0); c.pc=entry;
        c.ps=xtensa_lx7::state::ps::WOE | xtensa_lx7::state::ps::UM;
        c.set_ar(1,DRAM_BASE+SRAM_SIZE as u32); c.set_ar(0,0);
    }
    fn irqs(_bus:&S2Bus,out:&mut[u32]) {out.fill(0);}
    fn check_image(data:&[u8])->Result<(),String> {crate::image::parse(data).map(|_|())}
}
impl SocBus for S2Bus {
    fn cycles(&self)->u64 {self.cycles}
    fn irq_dirty(&mut self)->&mut bool {&mut self.irq_dirty}
    fn refresh_irq(&mut self)->bool {false} // Polled UART/GPIO only; no interrupt matrix.
    fn misc(&mut self)->&mut esp_periph::Misc {&mut self.misc}
    fn load_bytes(&mut self,addr:u32,bytes:&[u8])->Result<(),String> {self.load_ram(addr,bytes,bytes.len())}
    fn write_flash(&mut self,offset:usize,bytes:&[u8])->Result<(),String> {
        let end=offset.checked_add(bytes.len()).filter(|&end|end<=self.flash.len()).ok_or("S2 image staging exceeds configured capacity")?;
        self.flash[offset..end].copy_from_slice(bytes); Ok(())
    }
    fn boot_app(&mut self,app_off:usize)->Result<u32,String> {
        let bytes=self.flash.get(app_off..).ok_or("S2 app offset exceeds image staging capacity")?;
        let image=crate::image::parse(bytes)?;
        // Validate the entire image first. Every copy is already range-checked,
        // including physical alias overlap; rejected images change no SRAM.
        let copies:Vec<_>=image.segments.iter().map(|s|(s.load_addr,bytes[s.file_off as usize..(s.file_off+s.len)as usize].to_vec())).collect();
        for(addr,bytes)in copies {self.load_ram(addr,&bytes,bytes.len())?;}
        self.app_off=Some(app_off); self.app_entry=Some(image.entry);
        self.first_fault=None; self.reset_requested=false; self.irq_dirty=true;
        Ok(image.entry)
    }
    fn reboot(&mut self,_mac:[u8;6])->u32 {
        let mut fresh=S2Bus::new();
        fresh.psram.configure(self.psram.len()).expect("previously validated S2 capacity");
        std::mem::swap(&mut fresh.flash,&mut self.flash); std::mem::swap(&mut fresh.board,&mut self.board);
        fresh.gpio.restore_external(&self.gpio); fresh.gpio.strap=self.gpio.strap;
        fresh.cycles=self.cycles; fresh.debug=self.debug.clone(); fresh.reset_cause=self.reset_cause;
        fresh.gpio_events=self.gpio_events.as_ref().map(|_|Vec::new());
        // Deterministic host restart: clear diagnostic SRAM and recopy the image.
        // This is explicitly not the silicon reset's ROM/retention behavior.
        if let Some(off)=self.app_off {if let Err(reason)=fresh.boot_app(off) {eprintln!("[s2] app-direct restart rejected: {reason}");}}
        fresh.publish_gpio(); *self=fresh; self.reset_cause
    }
    fn sw_reset(&self)->bool {self.reset_requested}
    fn request_reset(&mut self,cause:u32) {self.reset_requested=true; self.reset_cause=cause&0x3f;}
    fn reset_cause(&self)->u32 {self.reset_cause}
    fn last_fault(&self)->Option<(u32,bool)> {self.first_fault.as_ref().map(|f|(f.address,f.write))}
    fn console_take(&mut self)->[Vec<u8>;4] {[Vec::new(),std::mem::take(&mut self.uart[0].tx_out),std::mem::take(&mut self.uart[1].tx_out),Vec::new()]}
    fn serial_input(&mut self,_bytes:&[u8]) {} // USB OTG is absent; never silently route USB to UART.
    fn uart_input(&mut self,n:usize,bytes:&[u8]) {if let Some(uart)=self.uart.get_mut(n) {uart.host_input(bytes); self.irq_dirty=true;}}
    fn gpio_set_input(&mut self,pin:u8,high:bool) {
        if valid_pin(pin) {self.irq_dirty|=self.gpio.set_input(pin,high); if let Some(events)=&mut self.gpio_events {events.push((self.cycles,pin,high));}}
    }
    fn gpio_release_input(&mut self,pin:u8) {if valid_pin(pin) {self.irq_dirty|=self.gpio.release_input(pin);}}
    fn gpio_input(&self)->u64 {self.gpio.input & crate::bus::VALID_GPIO}
    fn gpio_state(&self,pin:u8)->Option<esp_soc::GpioState> {
        valid_pin(pin).then(||{let mux=self.io_mux.read(4+4*u32::from(pin));
            esp_soc::GpioState::new(self.gpio.out&(1u64<<pin)!=0,self.gpio.enable&(1u64<<pin)!=0,mux&(1<<8)!=0,mux&(1<<7)!=0)})
    }
    fn observe_gpio(&mut self,on:bool) {self.gpio_events=on.then(Vec::new);}
    fn take_gpio_events(&mut self)->Vec<(u64,u8,bool)> {self.gpio_events.as_mut().map(std::mem::take).unwrap_or_default()}
    fn board(&mut self)->&mut dyn BoardModel {&mut *self.board}
    fn board_ref(&self)->&dyn BoardModel {&*self.board}
    fn audio(&self)->(&[i16],u32) {(&[],0)}
    fn irq_sources_of(&self,_core:usize,_line:u32)->Vec<usize> {Vec::new()}
    fn set_debug(&mut self,f:&esp_soc::DebugFlags) {self.debug=f.clone();}
    fn set_flash_size(&mut self,bytes:usize) {self.flash.resize(bytes.min(MAX_STAGING_FLASH),0xff);}
    fn set_psram_size(&mut self,bytes:usize)->Result<(),String> {self.psram.configure(bytes)}
    fn set_strap(&mut self,value:u32) {self.gpio.strap=value&0xffff;}
    fn set_reset_cause(&mut self,cause:u32) {self.reset_cause=cause&0x3f;}
    fn refresh_board_outputs(&mut self) {self.publish_gpio();}
    fn report(&self)->String {format!("[s2] SRAM app-direct diagnostic rev0; one integer core, JIT off; {SRAM_SIZE}B shared IRAM/DRAM aliases; polled UART/GPIO; {}B data-only PSRAM with64KiB MMU; ROM/flash mapping/PSRAM SPI init/cache timing/RTC/IRQ/USB/Wi-Fi/IDF boot unimplemented; {}",self.psram.len(),self.board.report())}
}

//! Common Machine adapter for the independent H2 SRAM-only runtime.
use crate::{bus::{H2Bus,CPU_HZ,ROM_BASE,SRAM_BASE,SRAM_SIZE,FLASH_SIZE},core};
use esp_soc::{BoardModel,Soc,SocBus};
use riscv_rv32::Cpu;
pub struct H2;
pub type Machine=esp_soc::Machine<H2>;
pub fn machine(mac:[u8;6],flash_size:usize)->Machine{let mut bus=H2Bus::new();bus.set_flash_size(flash_size);let mut m=Machine::new(mac,bus);m.set_debug(&esp_soc::DebugFlags::from_env());m.dbg.stop_after_exceptions=1;m}
impl Soc for H2{
 type Core=Cpu;type Bus=H2Bus;
 const NAME:&'static str="esp32h2";
 // No bundled/validated H2 ROM: callers must use explicit app-direct loading.
 const ROM_ELF:&'static str="";
 const CPU_HZ:u64=CPU_HZ;const CORES:usize=1;const IDLE_CHUNK:u64=64;
 const ROM_BOOT_DEFAULT:bool=false;const EXCEPTION_BUDGET_AFTER_RESET:Option<u64>=Some(1);
 const ROM_DATA_TABLE:&'static[&'static str]=&[];
 fn new_core(_i:usize)->Cpu{core()}
 fn reset_core(c:&mut Cpu,_i:usize){c.reset();c.pc=ROM_BASE;}
 fn reset_core_with_bus(c:&mut Cpu,b:&mut H2Bus,i:usize){Self::reset_core(c,i);b.irq_dirty=true;if let Some(entry)=b.app_entry{c.pc=entry;c.x[2]=SRAM_BASE+SRAM_SIZE as u32;}}
 fn boot_core(c:&mut Cpu,entry:u32){Self::reset_core(c,0);c.pc=entry;c.x[2]=SRAM_BASE+SRAM_SIZE as u32;}
 fn irqs(_b:&H2Bus,out:&mut[Option<u32>]){out[0]=None;}
 fn check_image(data:&[u8])->Result<(),String>{crate::image::parse(data).map(|_|())}
}
impl SocBus for H2Bus{
 fn cycles(&self)->u64{self.cycles}
 fn next_deadline(&self)->Option<u64>{self.rmt.busy().then_some(1)}
 fn irq_dirty(&mut self)->&mut bool{&mut self.irq_dirty}
 fn refresh_irq(&mut self)->bool{self.irq_dirty=false;false}
 fn misc(&mut self)->&mut esp_periph::Misc{&mut self.misc}
 fn load_bytes(&mut self,a:u32,d:&[u8])->Result<(),String>{self.load_ram(a,d,d.len())}
 fn write_flash(&mut self,offset:usize,data:&[u8])->Result<(),String>{
  if let Some(e)=&self.config_error{return Err(e.clone());}
  let end=offset.checked_add(data.len()).filter(|&e|e<=self.flash.len()).ok_or("H2 image staging exceeds configured board capacity")?;self.flash[offset..end].copy_from_slice(data);Ok(())
 }
 fn boot_app(&mut self,off:usize)->Result<u32,String>{
  if let Some(e)=&self.config_error{return Err(e.clone());}
  let data=self.flash.get(off..).ok_or("H2 app offset exceeds configured flash")?;let image=crate::image::parse(data)?;
  if (1usize<<20).checked_shl(u32::from(data[3]>>4)).is_none_or(|bytes|bytes>self.flash.len()) {return Err("H2 image Flash header exceeds this board capacity".into());}
  let copies:Vec<_>=image.segments.iter().map(|s|(s.load_addr,data[s.file_off as usize..s.file_off as usize+s.len as usize].to_vec())).collect();
  // Validate every segment, checksum and revision before touching guest SRAM.
  for(a,d)in copies{self.load_ram(a,&d,d.len())?;}
  self.app_off=Some(off);self.app_entry=Some(image.entry);self.first_fault=None;self.reset_requested=false;self.irq_dirty=true;Ok(image.entry)
 }
 fn reboot(&mut self,_mac:[u8;6])->u32{
  let mut next=H2Bus::new();std::mem::swap(&mut next.flash,&mut self.flash);std::mem::swap(&mut next.sram,&mut self.sram);std::mem::swap(&mut next.lp_sram,&mut self.lp_sram);std::mem::swap(&mut next.board,&mut self.board);
  next.gpio.restore_external(&self.gpio);next.gpio.strap=self.gpio.strap;next.cycles=self.cycles;next.debug=self.debug.clone();next.config_error=self.config_error.clone();next.reset_cause=self.reset_cause;next.gpio_events=self.gpio_events.as_ref().map(|_|vec![]);
  if let Some(off)=self.app_off{if let Err(reason)=next.boot_app(off){next.config_error=Some(format!("H2 app-direct restart rejected:{reason}"));}}
  next.publish_gpio();*self=next;self.reset_cause
 }
 fn sw_reset(&self)->bool{self.reset_requested}
 fn request_reset(&mut self,cause:u32){self.reset_requested=true;self.reset_cause=cause&0x1f;}
 fn reset_cause(&self)->u32{self.reset_cause}
 fn last_fault(&self)->Option<(u32,bool)>{self.first_fault.as_ref().map(|f|(f.address,f.write))}
 fn console_take(&mut self)->[Vec<u8>;4]{[vec![],std::mem::take(&mut self.uart[0].tx_out),std::mem::take(&mut self.uart[1].tx_out),vec![]]}
 fn serial_input(&mut self,_data:&[u8]){} // USB Serial/JTAG is not UART0
 fn uart_input(&mut self,n:usize,data:&[u8]){if matches!(self.board.name(),"m5-unit-gateway-h2"|"m5-module-gateway-h2") && (n!=0||self.uart_route(n).rx_pin!=Some(23)||!self.uart_route(n).matches_baud(115200)) {return;}
        if n<2&&self.uart_on(n)&&self.uart[n].read(0x20)&((1<<20)|(1<<22))==1<<20{self.uart[n].host_input(data);}}
 fn gpio_set_input(&mut self,p:u8,l:bool){if Self::valid_pad(p){self.gpio.set_input(p,l);if let Some(events)=&mut self.gpio_events{events.push((self.cycles,p,l));}}}
 fn gpio_release_input(&mut self,p:u8){if Self::valid_pad(p){self.gpio.release_input(p);}}
 fn gpio_input(&self)->u64{self.gpio.input&crate::bus::VALID_PADS}
 fn gpio_state(&self,p:u8)->Option<esp_soc::GpioState>{Self::valid_pad(p).then(||{let mux=self.io_mux.read(4+4*u32::from(p));esp_soc::GpioState::new(self.gpio.out&(1<<p)!=0,self.gpio.enable&(1<<p)!=0,mux&(1<<8)!=0,mux&(1<<7)!=0)})}
 fn observe_gpio(&mut self,on:bool){self.gpio_events=on.then(Vec::new);}
 fn take_gpio_events(&mut self)->Vec<(u64,u8,bool)>{self.gpio_events.as_mut().map(std::mem::take).unwrap_or_default()}
 fn board(&mut self)->&mut dyn BoardModel{&mut*self.board}
 fn board_ref(&self)->&dyn BoardModel{&*self.board}
 fn audio(&self)->(&[i16],u32){(&[],0)}
 fn irq_sources_of(&self,_core:usize,_line:u32)->Vec<usize>{vec![]}
 fn set_debug(&mut self,f:&esp_soc::DebugFlags){self.debug=f.clone();}
 fn set_flash_size(&mut self,bytes:usize){self.config_error=(![2<<20,FLASH_SIZE].contains(&bytes)).then(||format!("H2 staging supports source-backed 2/4MiB profiles; requested{bytes}bytes"));if self.config_error.is_none() {self.flash.resize(bytes,0xff);}}
 fn set_psram_size(&mut self,bytes:usize)->Result<(),String>{if bytes==0{Ok(())}else{Err("H2 boards have no supported PSRAM".into())}}
 fn set_strap(&mut self,v:u32){self.gpio.strap=v&0xffff;}
 fn set_reset_cause(&mut self,cause:u32){self.reset_cause=cause&0x1f;}
 fn refresh_board_outputs(&mut self){self.publish_gpio();}
 fn report(&self)->String{format!("[h2] independent chip16 SRAM app-direct,96MHz nominal,320KiB HP/4KiB LP,{}MiB staging; UART FIFO polling + GPIO19 + finite XTAL32MHz RMT; ROM/cache/interrupt routing/USB/BLE/802.15.4 unimplemented; {}",self.flash.len()>>20,self.board.report())}
}

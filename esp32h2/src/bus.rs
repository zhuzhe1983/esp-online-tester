//! Independent H2 map, fixed ESP-IDF v5.5.1 and H2 TRM v1.2.
//! Only SRAM, GPIO, UART polling, finite RMT and their PCR gates are implemented.
//! Absent addresses fault; there is no C6 register alias or simulated ROM boot.
use emu_core::{Bus, Fault};
use esp_periph::{Gpio, RegRam, Uart, UartLayout};
use esp_soc::{Board, NoBoard, pins::ChipPins};
use crate::rmt::H2Rmt;
pub const ROM_BASE:u32=0x4000_0000;
pub const ROM_SIZE:usize=0x20000;
pub const SRAM_BASE:u32=0x4080_0000;
pub const SRAM_SIZE:usize=0x50000;
pub const LP_BASE:u32=0x5000_0000;
pub const LP_SIZE:usize=0x1000;
pub const FLASH_SIZE:usize=4<<20;
pub const CPU_HZ:u64=96_000_000;
pub const XTAL_HZ:u64=32_000_000;
// 28 register positions, but H2 TRM 6.17.2 exposes only19 physical GPIOs.
// IDF SOC_GPIO_VALID_GPIO_MASK covers internal positions too; do not expose the
// unavailable6/7 and15..21 as usable pads on these FH4S/MINI-1 boards.
pub const VALID_PADS:u64=0x3f|(0x7f<<8)|(0x3f<<22);
pub const PINS:ChipPins=ChipPins{valid:VALID_PADS,input_select:0x80,output_mask:0x1ff};
const GPIO_BASE:u32=0x6009_1000;
const MUX_BASE:u32=0x6009_0000;
const RMT_BASE:u32=0x6000_7000;
const PCR_BASE:u32=0x6009_6000;
const UART_CONF0:u32=0x0010_001c;
const UART_INTERRUPTS:u32=1|2|(1<<4)|(1<<14); // FIFO polling; no timeout/line simulation
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct AccessFault{pub pc:u32,pub address:u32,pub write:bool,pub width:u8}
pub struct H2Bus{
 pub sram:Vec<u8>,pub lp_sram:Vec<u8>,pub flash:Vec<u8>,
 pub gpio:Gpio,pub io_mux:RegRam,pub uart:[Uart;2],pub board:Board,
 pub first_fault:Option<AccessFault>,pub cycles:u64,pub debug:esp_soc::DebugFlags,
 pub(crate) rmt:H2Rmt,pub(crate) irq_dirty:bool,pub(crate) misc:esp_periph::Misc,
 pub(crate) app_off:Option<usize>,pub(crate) app_entry:Option<u32>,
 pub(crate) config_error:Option<String>,pub(crate) gpio_events:Option<Vec<(u64,u8,bool)>>,
 pub(crate) reset_requested:bool,pub(crate) reset_cause:u32,
 pc:u32,uart_conf:[u32;2],uart_clock:[u32;2],rmt_conf:u32,rmt_clock:u32,
 rmt_phase:u128,rmt_routes:[u64;2],rmt_invalid:[bool;2],
}
impl Default for H2Bus{fn default()->Self{Self::new()}}
impl H2Bus{
 fn new_uart()->Uart{
  let mut u=Uart::new(UartLayout{thrhd_mask:0xff,rxfifo_rst:1<<22,rxfifo_cnt_mask:0xff,autobaud_en:0,autobaud_regs:0,autobaud_clock_hz:0});
  u.write(0x14,694);u.write(0x20,UART_CONF0);u.write(0x24,0x6060);u
 }
 pub fn new()->Self{
  let mut gpio=Gpio::with_pins(28);gpio.func_out_sel.fill(128);gpio.strap=0;
  let mut io_mux=RegRam::new();
  for pin in 0..28{if VALID_PADS&(1<<pin)!=0{io_mux.write(4+pin*4,0xb00);gpio.set_pad(pin as u8,0xb00);}}
  Self{sram:vec![0;SRAM_SIZE],lp_sram:vec![0;LP_SIZE],flash:vec![0xff;FLASH_SIZE],gpio,io_mux,uart:[Self::new_uart(),Self::new_uart()],board:Box::new(NoBoard),first_fault:None,cycles:0,debug:Default::default(),rmt:Default::default(),irq_dirty:true,misc:Default::default(),app_off:None,app_entry:None,config_error:None,gpio_events:None,reset_requested:false,reset_cause:1,pc:0,uart_conf:[1;2],uart_clock:[0x700000;2],rmt_conf:1,rmt_clock:0x301000,rmt_phase:0,rmt_routes:[0;2],rmt_invalid:[false;2]}
 }
 pub fn attach_board(&mut self,board:Board){self.board=board;self.attach_board_devices();}
 pub fn attach_board_devices(&mut self){for(p,l)in self.board.input_levels(){if Self::valid_pad(p){self.gpio.set_input(p,l);}}self.publish_gpio();}
 pub fn valid_pad(pin:u8)->bool{pin<28&&VALID_PADS&(1u64<<pin)!=0}
 pub(crate) fn publish_gpio(&mut self){
  for ch in 0..2{if self.rmt.running(ch)&&self.route(ch)!=self.rmt_routes[ch]{self.rmt_invalid[ch]=true;}}
  self.board.gpio_waveform_at(self.cycles,PINS.routes(&self.gpio,&self.io_mux));
  if let Some(events)=&mut self.gpio_events{events.extend(self.gpio.changes.drain(..).filter(|(p,_)|Self::valid_pad(*p)).map(|(p,l)|(self.cycles,p,l)));}else{self.gpio.changes.clear();}
  self.gpio.input_changes.clear();
 }
 fn route(&self,ch:usize)->u64{PINS.routes(&self.gpio,&self.io_mux).output_pins(71+ch as u32).fold(0,|mask,p|mask|(1<<p))}
 fn range(a:u32,n:usize,base:u32,len:usize)->Option<std::ops::Range<usize>>{let start=a.checked_sub(base)?as usize;let end=start.checked_add(n)?;(end<=len).then_some(start..end)}
 pub fn check_ram(&self,a:u32,n:usize)->Result<(),String>{if Self::range(a,n,SRAM_BASE,SRAM_SIZE).is_some()||Self::range(a,n,LP_BASE,LP_SIZE).is_some(){Ok(())}else{Err(format!("H2 SRAM range{a:#010x}+{n:#x} is unavailable"))}}
 pub fn load_ram(&mut self,a:u32,data:&[u8],size:usize)->Result<(),String>{
  if data.len()>size{return Err("H2 file bytes exceed segment memory size".into());}self.check_ram(a,size)?;
  let(mem,r)=if let Some(r)=Self::range(a,size,SRAM_BASE,SRAM_SIZE){(&mut self.sram,r)}else{(&mut self.lp_sram,Self::range(a,size,LP_BASE,LP_SIZE).ok_or("H2 LP SRAM range")?)};
  mem[r.clone()].fill(0);mem[r.start..r.start+data.len()].copy_from_slice(data);Ok(())
 }
 fn fail<T>(&mut self,a:u32,w:u8,write:bool,f:Fault)->Result<T,Fault>{self.first_fault.get_or_insert(AccessFault{pc:self.pc,address:a,write,width:w});Err(f)}
 fn ram_read(&self,a:u32,n:usize)->Option<u32>{
  let(mem,r)=if let Some(r)=Self::range(a,n,SRAM_BASE,SRAM_SIZE){(&self.sram,r)}else{(&self.lp_sram,Self::range(a,n,LP_BASE,LP_SIZE)?)};
  let mut bytes=[0;4];bytes[..n].copy_from_slice(&mem[r]);Some(u32::from_le_bytes(bytes))
 }
 fn ram_write(&mut self,a:u32,n:usize,v:u32)->bool{
  let(mem,r)=if let Some(r)=Self::range(a,n,SRAM_BASE,SRAM_SIZE){(&mut self.sram,r)}else if let Some(r)=Self::range(a,n,LP_BASE,LP_SIZE){(&mut self.lp_sram,r)}else{return false;};mem[r].copy_from_slice(&v.to_le_bytes()[..n]);true
 }
 pub fn uart_route(&self,n:usize)->esp_soc::uart::UartRoute {
  if n>=2 {return esp_soc::uart::UartRoute::new(n,0,None,None);}
  // This H2 subset accepts only PCR source3 (32MHz XTAL); shared UART
  // decoder source3 is40MHz, so rescale its fractional-divider result.
  let baud=self.uart_on(n).then(||self.uart[n].baud(self.uart_clock[n],32_000_000)).flatten().map(|v|(u64::from(v)*4/5)as u32);
  esp_soc::uart::UartPins::H2.route(n,&self.gpio,&self.io_mux,baud)
 }
 pub(crate) fn uart_on(&self,n:usize)->bool{self.uart_conf[n]&3==1&&self.uart_clock[n]&(1<<22)!=0}
 fn pcr_read(&self,o:u32)->Result<u32,Fault>{match o{0|0xc=>{let c=self.uart_conf[(o/12)as usize];Ok(c|if c&2==0{4}else{0})},4|0x10=>Ok(self.uart_clock[((o-4)/12)as usize]),0x34=>Ok(self.rmt_conf|if self.rmt_conf&2==0{4}else{0}),0x38=>Ok(self.rmt_clock),_=>Err(Fault::Unmapped)}}
 fn pcr_write(&mut self,o:u32,v:u32)->Result<(),Fault>{
  match o{
   0|0xc=>{if v&!7!=0{return Err(Fault::Prohibited);}let n=(o/12)as usize;if v&2!=0{self.uart[n]=Self::new_uart();}self.uart_conf[n]=v&3;},
   4|0x10=>{if v&!0x7fffff!=0||(v>>20)&3!=3{return Err(Fault::Prohibited);}self.uart_clock[((o-4)/12)as usize]=v;},
   0x34=>{if v&!7!=0{return Err(Fault::Prohibited);}
 if v&2!=0{self.rmt.reset();self.rmt_phase=0;self.rmt_invalid=[false;2];}self.rmt_conf=v&3;},
   0x38=>{
    if v&!0x3fffff!=0||(v&63!=0&&(v>>6)&63==0){return Err(Fault::Prohibited);}
    if self.rmt.busy()&&(v^self.rmt_clock)&!(1<<21)!=0{return Err(Fault::Prohibited);}
    if (v^self.rmt_clock)&!(1<<21)!=0{self.rmt_phase=0;}self.rmt_clock=v;
   },
   _=>return Err(Fault::Unmapped),
  }Ok(())
 }
 fn uart_read(&mut self,n:usize,o:u32)->Result<u32,Fault>{
  if !self.uart_on(n){return Err(Fault::Prohibited);}
  match o{0 if self.uart[n].read(0x20)&(1<<20)==0=>Err(Fault::Prohibited),0|0x14|0x1c|0x20|0x24|0x98=>Ok(self.uart[n].read(o)),4|8|0xc=>Ok(self.uart[n].read(o)&UART_INTERRUPTS),0x10=>Ok(0),_=>Err(Fault::Unmapped)}
 }
 fn uart_write(&mut self,n:usize,o:u32,v:u32)->Result<(),Fault>{
  if !self.uart_on(n){return Err(Fault::Prohibited);}
  match o{
   0=>{if self.uart[n].read(0x20)&(1<<20)==0{return Err(Fault::Prohibited);}self.uart[n].write(0,v&255);if self.board.uses_uart_pins(){self.board.uart_tx(self.cycles,self.uart_route(n),v as u8);}},
   0xc|0x10=>{if v&!UART_INTERRUPTS!=0{return Err(Fault::Prohibited);}self.uart[n].write(o,v);},
   0x14=>{if v&!0x00f00fff!=0{return Err(Fault::Prohibited);}self.uart[n].write(o,v);},
   0x20=>{let commands=(1<<22)|(1<<23);if v&!(commands|(1<<20))!=0x1c{return Err(Fault::Prohibited);}self.uart[n].write(o,v&!(1<<23));},
   0x24=>{if v&!0xffff!=0||v&255==0||v&255>128||(v>>8)&255==0{return Err(Fault::Prohibited);}self.uart[n].write(o,v);},
   0x98=>{if v&!1!=0{return Err(Fault::Prohibited);}},
   _=>return Err(Fault::Unmapped),
  }Ok(())
 }
 fn gpio_read(&mut self,o:u32)->Result<u32,Fault>{
  match o{4|0x20|0x44=>Ok(self.gpio.read(o)&VALID_PADS as u32),0x3c=>{let enabled=(0..28).filter(|p|Self::valid_pad(*p)&&self.io_mux.read(4+u32::from(*p)*4)&(1<<9)!=0).fold(0u32,|m,p|m|(1<<p));Ok(self.gpio.read(o)&enabled)},0x38=>Ok(self.gpio.strap&0xffff),8|0xc|0x24|0x28|0x48|0x4c=>Ok(0),0x74..=0xe0 if Self::valid_pad(((o-0x74)/4)as u8)=>Ok(self.gpio.read(o)),0x154..=0x350=>Ok(self.gpio.read(o)),0x554..=0x5c0 if Self::valid_pad(((o-0x554)/4)as u8)=>Ok(self.gpio.read(o)),_=>Err(Fault::Unmapped)}
 }
 fn gpio_write(&mut self,o:u32,v:u32)->Result<(),Fault>{
  match o{4|8|0xc|0x20|0x24|0x28|0x44|0x48|0x4c=>self.gpio.write(o,v&VALID_PADS as u32),0x74..=0xe0 if Self::valid_pad(((o-0x74)/4)as u8)=>{if v&!0x39f!=0{return Err(Fault::Prohibited);}self.gpio.write(o,v);},0x154..=0x350=>{if v&!0xff!=0{return Err(Fault::Prohibited);}self.gpio.write(o,v);},0x554..=0x5c0 if Self::valid_pad(((o-0x554)/4)as u8)=>{if v&!0x7ff!=0{return Err(Fault::Prohibited);}self.gpio.write(o,v);},_=>return Err(Fault::Unmapped)}self.publish_gpio();Ok(())
 }
 fn mmio_read(&mut self,a:u32)->Result<u32,Fault>{match a{
  0x60000000..=0x60001fff=>self.uart_read(((a>>12)&1)as usize,a&0xfff),
  RMT_BASE..=0x600077ff if self.rmt_conf&3==1=>self.rmt.read(a-RMT_BASE),
  GPIO_BASE..=0x60091fff=>self.gpio_read(a-GPIO_BASE),
  0x60090004..=0x60090070 if Self::valid_pad(((a-MUX_BASE-4)/4)as u8)=>Ok(self.io_mux.read(a-MUX_BASE)),
  PCR_BASE..=0x60096fff=>self.pcr_read(a-PCR_BASE),
  _=>Err(Fault::Unmapped),
 }}
 fn mmio_write(&mut self,a:u32,v:u32)->Result<(),Fault>{match a{
  0x60000000..=0x60001fff=>self.uart_write(((a>>12)&1)as usize,a&0xfff,v),
  RMT_BASE..=0x600077ff if self.rmt_conf&3==1=>{
   let o=a-RMT_BASE;let start=matches!(o,0x10|0x14)&&v&1!=0;
   if start&&((self.rmt_clock>>20)&1!=0||self.rmt_clock&(1<<21)==0){return Err(Fault::Prohibited);}
   self.rmt.write(o,v)?;
   if start{let ch=((o-0x10)/4)as usize;self.rmt_routes[ch]=self.route(ch);self.rmt_invalid[ch]=false;}Ok(())
  },
  GPIO_BASE..=0x60091fff=>self.gpio_write(a-GPIO_BASE,v),
  0x60090004..=0x60090070 if Self::valid_pad(((a-MUX_BASE-4)/4)as u8)=>{
   // Active digital GPIO/MUX only. Sleep,glitch filter and hysteresis are not emulated.
   if v&!0x7f80!=0{return Err(Fault::Prohibited);}self.io_mux.write(a-MUX_BASE,v);self.gpio.set_pad(((a-MUX_BASE-4)/4)as u8,v);self.publish_gpio();Ok(())
  },
  PCR_BASE..=0x60096fff=>self.pcr_write(a-PCR_BASE,v),
  _=>Err(Fault::Unmapped),
 }}
 fn read(&mut self,a:u32,w:u8)->Result<u32,Fault>{
  if !a.is_multiple_of(u32::from(w)){return self.fail(a,w,false,Fault::Misaligned);}
 if let Some(v)=self.ram_read(a,w as usize){return Ok(v);}
  if w!=4{return self.fail(a,w,false,Fault::Prohibited);}match self.mmio_read(a){Ok(v)=>Ok(v),Err(e)=>self.fail(a,w,false,e)}
 }
 fn write(&mut self,a:u32,v:u32,w:u8)->Result<(),Fault>{
  if !a.is_multiple_of(u32::from(w)){return self.fail(a,w,true,Fault::Misaligned);}
 if self.ram_write(a,w as usize,v){return Ok(());}
  if w!=4{return self.fail(a,w,true,Fault::Prohibited);}match self.mmio_write(a,v){Ok(())=>Ok(()),Err(e)=>self.fail(a,w,true,e)}
 }
 pub fn advance(&mut self,cycles:u64){
  self.cycles=self.cycles.saturating_add(cycles);
  if self.rmt.busy()&&self.rmt_conf&3==1&&self.rmt_clock&(1<<21)!=0&&self.rmt_clock&(1<<20)==0{
   let a=u128::from(self.rmt_clock&63);let b=u128::from((self.rmt_clock>>6)&63).max(1);let integer=u128::from((self.rmt_clock>>12)&255)+1;
   let denominator=u128::from(CPU_HZ)*(integer*b+a);let numerator=self.rmt_phase+u128::from(cycles)*u128::from(XTAL_HZ)*b;
   let ticks=(numerator/denominator)as u64;self.rmt_phase=numerator%denominator;
   if ticks!=0{self.rmt.advance(ticks);}
   for(ch,bits)in std::mem::take(&mut self.rmt.done){if !self.rmt_invalid[ch]&&self.route(ch)==self.rmt_routes[ch]{let mut pads=self.rmt_routes[ch];while pads!=0{let pin=pads.trailing_zeros()as u8;self.board.rmt_frame(pin,&bits);pads&=pads-1;}}}
  }
 }
}
impl Bus for H2Bus{
 fn note_code_page(&mut self,_page:u32){} // no decoded block cache or JIT mapping
 fn note_pc(&mut self,pc:u32){self.pc=pc;self.misc.cur_pc=pc;}
 fn block_break(&self)->bool{self.irq_dirty}
 fn tick(&mut self,cycles:u32)->u32{self.advance(u64::from(cycles));u32::from(self.irq_dirty)}
 fn read8(&mut self,a:u32)->Result<u8,Fault>{self.read(a,1).map(|v|v as u8)}
 fn read16(&mut self,a:u32)->Result<u16,Fault>{self.read(a,2).map(|v|v as u16)}
 fn read32(&mut self,a:u32)->Result<u32,Fault>{self.read(a,4)}
 fn write8(&mut self,a:u32,v:u8)->Result<(),Fault>{self.write(a,u32::from(v),1)}
 fn write16(&mut self,a:u32,v:u16)->Result<(),Fault>{self.write(a,u32::from(v),2)}
 fn write32(&mut self,a:u32,v:u32)->Result<(),Fault>{self.write(a,v,4)}
 fn fetch(&mut self,pc:u32)->Result<[u8;4],Fault>{
  if pc&1!=0{return self.fail(pc,2,false,Fault::Misaligned);}let Some(lo)=self.ram_read(pc,2)else{return self.fail(pc,2,false,Fault::Prohibited);};
  let hi=if lo&3==3{let Some(v)=pc.checked_add(2).and_then(|p|self.ram_read(p,2))else{return self.fail(pc,4,false,Fault::Prohibited);};v}else{0};Ok((lo|hi<<16).to_le_bytes())
 }
}

//! K146 xiaozhi-card44aa2f2b config/InitializeDisplay and pinned EPDe7e44203:
//! source-compatible byte-X/gate-Y176x264,RAM24,22/20refresh. Die identity unknown.
//! SharedRST47 gatesFT6336U@38 SDA2/SCL1 IRQ16. No charger/gauge/audio/modem model.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,ft6336::{Ft6336g,Ft6336State}};
use esp_soc::{BoardModel,board::{SpiPins,BoardEdge,VirtualCycle},pins::PinRoutes,devices::{ssd16xx_mono::Ssd16xxMono,i2c_bitbang::I2cBitBang}};
const HZ:u64=240_000_000;
pub struct XiaozhiCard {panel:Ssd16xxMono,touch:Arc<Mutex<Ft6336State>>,i2c:I2cBitBang,attached:bool,reset:bool,dc:Option<bool>,levels:[bool;2],irq_high:bool,irq_until:u64,cycle:u64,edges:Vec<BoardEdge>}
impl Default for XiaozhiCard {fn default()->Self {Self::new()}}
impl XiaozhiCard {pub fn new()->Self {Self{panel:Ssd16xxMono::sourced_dimensions(176,264,48,HZ/1000).expect("source176x264"),touch:Default::default(),i2c:I2cBitBang::new(),attached:false,reset:false,dc:None,levels:[true;2],irq_high:true,irq_until:0,cycle:0,edges:Vec::new()}}fn irq(&mut self) {let high=!self.reset||self.cycle>=self.irq_until;if high!=self.irq_high {self.irq_high=high;self.edges.push(BoardEdge{cycle:self.cycle,pin:16,level:high});}}}
impl BoardModel for XiaozhiCard {
 fn name(&self)->&'static str {"xiaozhi-card"}
 fn uses_gpio_routes(&self)->bool {true}
 fn gpio_waveform_at(&mut self,c:VirtualCycle,r:PinRoutes<'_>) {self.advance_to(c);let reset=r.software_output(47)==Some(true);if self.reset&&!reset {self.panel.reset();self.touch.lock().expect("K146 touch").reset();}self.reset=reset;self.touch.lock().expect("K146 touch").powered=reset;self.dc=r.software_output(41);self.irq();let scl=r.software_output(1).unwrap_or(true);let sda=self.i2c.drive(scl,r.software_output(2).unwrap_or(true));for(i,(pin,level))in[(2,sda),(1,scl)].into_iter().enumerate(){if level!=self.levels[i]{self.levels[i]=level;self.edges.push(BoardEdge{cycle:c,pin,level});}}}
 fn uses_spi_pins(&self)->bool {true}
 fn spi_transfer_pins(&mut self,h:u8,p:SpiPins,tx:&[u8],rx:usize)->Vec<u8> {if matches!(h,2|3)&&self.reset&&!self.panel.busy()&&p.sclk&(1<<45)!=0&&p.mosi&(1<<46)!=0&&p.cs&(1<<42)!=0 {if let Some(dc)=self.dc{self.panel.write(dc,tx);}}vec![255;rx]}
 fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {if !self.attached {self.i2c.attach(0x38,Box::new(Ft6336g::unknown_identity(self.touch.clone())));self.attached=true;}(0..2).flat_map(|bus|self.i2c.controller_devices(Some((2,1))).into_iter().map(move|(a,d)|(bus,a,d))).collect()}
 fn advance_to(&mut self,c:VirtualCycle) {self.cycle=c;self.panel.advance_to(c);self.irq();}
 fn next_deadline(&self)->Option<VirtualCycle> {self.panel.next_deadline().into_iter().chain((self.cycle<self.irq_until).then_some(self.irq_until)).min()}
 fn take_edges(&mut self)->Vec<BoardEdge> {let mut edges=std::mem::take(&mut self.edges);edges.extend(self.panel.take_edges());edges.sort_by_key(|e|e.cycle);edges}
 fn input_levels(&self)->Vec<(u8,bool)> {vec![(21,true),(2,self.levels[0]),(1,self.levels[1]),(16,self.irq_high),(48,self.panel.busy())]}
 fn named_pin(&self,n:&str)->Option<u8> {(n=="wake").then_some(21)}
 fn touch(&mut self,x:u16,y:u16,down:bool) {if !self.reset{return;}self.touch.lock().expect("K146 touch").update(x.min(175),y.min(263),down);self.irq_until=self.cycle.saturating_add(HZ/1000);self.irq();}
 fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {Some((176,264,self.panel.frame.clone(),self.panel.version))}
 fn display_version(&self)->u64 {self.panel.version}
 fn display_frames(&self)->u64 {self.panel.frames}
 fn display_push_hz(&self)->u64 {5}
 fn report(&self)->String {format!("[xiaozhi-card]176x264 monoEPD RAMbytes={} opticalFrames={} BUSY48={} FT6336U SDA2/SCL1 RST47 IRQ16;20ms refresh/1ms IRQ digital policies, no calibrated timing/gray/ghosting; SD-NAND,AW32001/BQ27220 power,codec,audio,modem,network,dock unimplemented",self.panel.ram_bytes_written,self.panel.frames,self.panel.busy())}
}

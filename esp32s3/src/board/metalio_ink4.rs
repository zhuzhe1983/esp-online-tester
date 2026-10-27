//! Ordinary Metalio INK4, not Plus/S31. Physical routes and rail/reset writes:
//! CloudZao/Metalio-E-INK4 d12f481f, config.h, IOExpander.hpp and metalio_epd.cc.
//! SSD1677, TCA9555 and CST816S digital subset; see docs/platform/ink4-runtime.md.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,tca9555::{Tca9555,Tca9555State},cst816s::{Cst816s,Cst816State}};
use esp_soc::board::{BoardModel,BoardEdge,VirtualCycle,SpiPins};
use esp_soc::devices::{i2c_bitbang::I2cBitBang,ssd1677::Ssd1677};

struct Wiring {expander:Arc<Mutex<Tca9555State>>,touch:Arc<Mutex<Cst816State>>,panel:Mutex<Ssd1677>,active:Mutex<bool>}
impl Wiring {
    fn active(&self)->bool {self.expander.lock().expect("INK4 TCA9555 mutex poisoned").driven_high()&0x60==0x60}
    fn sync(&self) {
        let high=self.expander.lock().expect("INK4 TCA9555 mutex poisoned").driven_high();let active=high&0x60==0x60;
        let mut old=self.active.lock().expect("INK4 power mutex poisoned");if *old && !active {self.panel.lock().expect("INK4 panel mutex poisoned").reset();}*old=active;
        let mut touch=self.touch.lock().expect("INK4 touch mutex poisoned");touch.powered=active && high&0x200!=0;if !touch.powered {touch.reset();}
    }
}
struct Expander {inner:Tca9555,wiring:Arc<Wiring>}
impl I2cDevice for Expander {
    fn start(&mut self,read:bool)->bool {self.inner.start(read)}
    fn write(&mut self,b:u8)->bool {let ack=self.inner.write(b);self.wiring.sync();ack}
    fn read(&mut self)->u8 {self.inner.read()}
    fn stop(&mut self) {self.inner.stop();}
}
pub struct MetalioInk4 {wiring:Arc<Wiring>,i2c:I2cBitBang,i2c_levels:[bool;2],dc:bool,reset:bool,cycle:u64,irq:[bool;2],edges:Vec<BoardEdge>,gpio_events:u64}
impl MetalioInk4 {
    pub fn new()->Self {Self{wiring:Arc::new(Wiring{expander:Default::default(),touch:Default::default(),panel:Mutex::new(Ssd1677::new(800,480,9,crate::periph::CPU_HZ/1000)),active:Mutex::new(false)}),i2c:I2cBitBang::new(),i2c_levels:[true,true],dc:false,reset:true,cycle:0,irq:[true,true],edges:Vec::new(),gpio_events:0}}
    fn sync_irq(&mut self) {let levels=[self.wiring.touch.lock().expect("INK4 touch mutex poisoned").irq_high(),self.wiring.expander.lock().expect("INK4 TCA9555 mutex poisoned").irq_high()];for (i,&level) in levels.iter().enumerate(){if self.irq[i]!=level {self.irq[i]=level;self.edges.push(BoardEdge{cycle:self.cycle,pin:(i+1) as u8,level});}}}
}
impl Default for MetalioInk4 {fn default()->Self {Self::new()}}
impl BoardModel for MetalioInk4 {
    fn name(&self)->&'static str {"metalio-ink4"}
    fn gpio_changes(&mut self,changes:&[(u8,bool)]) {self.gpio_events+=changes.len() as u64;for &(pin,high) in changes {match pin {13=>self.dc=high,18=>{if self.reset && !high {self.wiring.panel.lock().expect("INK4 panel mutex poisoned").reset();}self.reset=high;},_=>{}}}}
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {
        self.cycle=cycle;self.gpio_changes(changes);let high=|p:u8|enabled&(1u64<<p)==0 || output&(1u64<<p)!=0;
        let scl=high(42);let sda=self.i2c.drive(scl,high(41));for (i,(pin,level)) in [(41,sda),(42,scl)].into_iter().enumerate(){if self.i2c_levels[i]!=level {self.i2c_levels[i]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
    }
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        self.wiring.sync();if self.reset && self.wiring.active() && pins.sclk&(1<<14)!=0 && pins.mosi&(1<<8)!=0 && pins.cs&(1<<45)!=0 {self.wiring.panel.lock().expect("INK4 panel mutex poisoned").write(self.dc,tx);}vec![0xff;rx_len]
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();self.i2c.attach(0x20,Box::new(Expander{inner:Tca9555::new(self.wiring.expander.clone()),wiring:self.wiring.clone()}));self.i2c.attach(0x15,Box::new(Cst816s::new(self.wiring.touch.clone())));
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some((41,42))).into_iter().map(move |(a,d)|(bus,a,d))).collect()
    }
    fn touch(&mut self,x:u16,y:u16,down:bool) {self.wiring.sync();self.wiring.touch.lock().expect("INK4 touch mutex poisoned").update(x.min(479),y.min(799),down);self.sync_irq();}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true),(3,true),(1,self.irq[0]),(2,self.irq[1]),(9,self.wiring.panel.lock().expect("INK4 panel mutex poisoned").busy()),(41,self.i2c_levels[0]),(42,self.i2c_levels[1]),(46,true)]}
    fn advance_to(&mut self,cycle:VirtualCycle) {self.cycle=cycle;self.wiring.panel.lock().expect("INK4 panel mutex poisoned").advance_to(cycle);self.sync_irq();}
    fn next_deadline(&self)->Option<VirtualCycle> {self.wiring.panel.lock().expect("INK4 panel mutex poisoned").next_deadline()}
    fn take_edges(&mut self)->Vec<BoardEdge> {self.sync_irq();let mut edges=std::mem::take(&mut self.edges);edges.extend(self.wiring.panel.lock().expect("INK4 panel mutex poisoned").take_edges());edges.sort_by_key(|e|e.cycle);edges}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {let p=self.wiring.panel.lock().expect("INK4 panel mutex poisoned");let mut frame=vec![0xffff;480*800];for y in 0..480 {for x in 0..800 {frame[x*480+(479-y)]=p.frame[y*800+x];}}Some((480,800,frame,p.version))}
    fn display_version(&self)->u64 {self.wiring.panel.lock().expect("INK4 panel mutex poisoned").version}
    fn display_frames(&self)->u64 {self.wiring.panel.lock().expect("INK4 panel mutex poisoned").frames}
    fn sd_card_slot(&self)->Option<([u8;6],bool)> {Some(([38,40,39,255,255,46],self.wiring.active()))}
    fn named_pin(&self,id:&str)->Option<u8> {match id {"power"=>Some(3),"boot"=>Some(0),_=>None}}
    fn button_at(&mut self,cycle:VirtualCycle,id:&str,pressed:bool)->bool {
        self.cycle=cycle;self.wiring.sync();match id {
            "volume-up"|"volume-down"=>self.wiring.expander.lock().expect("INK4 TCA9555 mutex poisoned").set_input(if id=="volume-up"{8}else{7},!pressed),
            "home"|"previous"|"next"=>self.wiring.touch.lock().expect("INK4 touch mutex poisoned").update(match id {"home"=>80,"previous"=>400,_=>240},900,pressed),
            _=>return false,
        }self.sync_irq();true
    }
    fn report(&self)->String {let p=self.wiring.panel.lock().expect("INK4 panel mutex poisoned");format!("[metalio-ink4] SSD1677 frames={} RAM_bytes={} rails={} touch=CST816S RTC/IMU/gauge/modems=absent",p.frames,p.ram_bytes_written,self.wiring.active())}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn expander(board:&MetalioInk4)->Expander {Expander{inner:Tca9555::new(board.wiring.expander.clone()),wiring:board.wiring.clone()}}
    fn write(d:&mut dyn I2cDevice,r:u8,bytes:&[u8]) {assert!(d.start(false));assert!(d.write(r));for &b in bytes {assert!(d.write(b));}}
    #[test]
    fn rails_reset_spi_routes_and_semantic_buttons_obey_devices() {
        let mut board=MetalioInk4::new();let mut exp=expander(&board);
        assert!(!board.wiring.active());write(&mut exp,2,&[0x60,2]);assert!(!board.wiring.active(),"input pullups do not enable rails");
        write(&mut exp,6,&[0x9f,0xfd]);assert!(board.wiring.active());
        let pins=SpiPins{sclk:1<<14,mosi:1<<8,cs:1<<45,..Default::default()};
        board.gpio_changes(&[(13,false)]);board.spi_transfer_pins(3,pins,&[0x24],0);board.gpio_changes(&[(13,true)]);
        board.spi_transfer_pins(3,SpiPins{mosi:1<<7,..pins},&[0],0);assert_eq!(board.wiring.panel.lock().unwrap().ram_bytes_written,0);
        board.spi_transfer_pins(3,pins,&[0],0);assert_eq!(board.wiring.panel.lock().unwrap().ram_bytes_written,1);
        assert!(board.button_at(1,"volume-up",true));assert!(!board.wiring.expander.lock().unwrap().irq_high());
        write(&mut exp,1,&[]);assert!(exp.start(true));assert_eq!(exp.read()&1,0);assert!(board.wiring.expander.lock().unwrap().irq_high());
        assert!(board.button_at(2,"volume-up",false));assert!(board.button_at(3,"home",true));
        {let t=board.wiring.touch.lock().unwrap();assert_eq!((t.x,t.y,t.down),(80,900,true));assert!(!t.irq_high());}
        assert!(board.button_at(4,"home",false));assert!(!board.button_at(5,"invented",true));
        write(&mut exp,3,&[0]);board.touch(1,1,true);assert!(!board.wiring.touch.lock().unwrap().down,"held reset rejects touch");
        write(&mut exp,2,&[0]);assert!(!board.wiring.active());assert_eq!(board.sd_card_slot(),Some(([38,40,39,255,255,46],false)));
    }
}

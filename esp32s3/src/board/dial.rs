//! M5Stack Dial K130 (not K130-V11). Official Sch_M5Dial.pdf and M5GFX
//! 65c816a3 / M5Unified 368ad608: SPI DC4/MOSI5/CLK6/CS7/RST8, I2C11/12,
//! touch IRQ14, press42, encoder A41/B40. Scope: docs/platform/dial-runtime.md.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,ft6336::{Ft6336g,Ft6336State}};
use esp_soc::board::{BoardModel,BoardEdge,VirtualCycle,SpiPins};
use esp_soc::devices::{DcsPanel,i2c_bitbang::I2cBitBang};

pub struct Dial {panel:DcsPanel,command:u8,read_at:usize,reset_high:bool,display_changes:u64,i2c:I2cBitBang,levels:[bool;2],edges:Vec<BoardEdge>,touch:Arc<Mutex<Ft6336State>>,touch_irq:bool,cycle:VirtualCycle,gpio_events:u64}
impl Dial {pub fn new()->Self {let mut touch=Ft6336State::default();touch.powered=true;Self{panel:DcsPanel::new(240,240),command:0,read_at:0,reset_high:true,display_changes:0,i2c:I2cBitBang::new(),levels:[true,true],edges:Vec::new(),touch:Arc::new(Mutex::new(touch)),touch_irq:true,cycle:0,gpio_events:0}}}
impl Default for Dial {fn default()->Self {Self::new()}}
impl BoardModel for Dial {
    fn name(&self)->&'static str {"dial"}
    fn gpio_changes(&mut self,changes:&[(u8,bool)]) {
        self.gpio_events+=changes.len() as u64;
        for &(pin,high) in changes {match pin {4=>self.panel.dc=high,8=>{if self.reset_high && !high {self.panel.reset();self.command=0;self.read_at=0;self.display_changes+=1;self.touch.lock().expect("Dial touch mutex poisoned").reset();if !self.touch_irq {self.touch_irq=true;self.edges.push(BoardEdge{cycle:self.cycle,pin:14,level:true});}}self.reset_high=high;self.touch.lock().expect("Dial touch mutex poisoned").powered=high;},_=>{}}}
    }
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {
        self.cycle=cycle;self.gpio_changes(changes);let released=|pin:u8|enabled&(1u64<<pin)==0 || output&(1u64<<pin)!=0;
        let scl=released(12);let sda=self.i2c.drive(scl,released(11));
        for (index,(pin,level)) in [(11,sda),(12,scl)].into_iter().enumerate() {if level!=self.levels[index] {self.levels[index]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
    }
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        if !self.reset_high || pins.sclk&(1<<6)==0 || pins.cs&(1<<7)==0 || (pins.mosi&(1<<5)==0 && pins.miso!=Some(5)) {return vec![0xff;rx_len];}
        for &b in tx {if !self.panel.dc {self.command=b;self.read_at=0;self.display_changes+=1;}self.panel.byte(b);}
        (0..rx_len).map(|_| {let value=match self.command {0x04=>[0x00,0x9a,0x01].get(self.read_at).copied().unwrap_or(0),0xda=>0,0xdb=>0x9a,0xdc=>1,_=>0xff};self.read_at=self.read_at.saturating_add(1);value}).collect()
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();self.i2c.attach(0x38,Box::new(Ft6336g::ft3267(self.touch.clone())));
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some((11,12))).into_iter().map(move |(address,device)|(bus,address,device))).collect()
    }
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true),(42,true),(40,true),(41,true),(14,self.touch_irq),(11,self.levels[0]),(12,self.levels[1])]}
    fn named_pin(&self,name:&str)->Option<u8> {match name {"btnA"|"button"|"sw"=>Some(42),"boot"=>Some(0),"clk"=>Some(41),"dt"=>Some(40),_=>None}}
    fn encoder(&self)->Option<(u8,u8)> {Some((41,40))}
    fn touch(&mut self,x:u16,y:u16,down:bool) {
        let mut touch=self.touch.lock().expect("Dial touch mutex poisoned");if !touch.powered {return;}touch.update(x.min(239),y.min(239),down);drop(touch);
        let level=!down;if level!=self.touch_irq {self.touch_irq=level;self.edges.push(BoardEdge{cycle:self.cycle,pin:14,level});}
    }
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {Some((240,240,if self.panel.on && !self.panel.sleeping {self.panel.gram.clone()} else {vec![0;240*240]},self.display_version()))}
    fn display_version(&self)->u64 {self.panel.pixels_written+self.panel.resets+self.display_changes}
    fn display_frames(&self)->u64 {self.panel.frames}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)> {Some((self.panel.gram.clone(),240,240))}
    fn report(&self)->String {format!("[dial] GC9A01 frames={} pixels={} FT3267=single-contact encoder=A41/B40",self.panel.frames,self.panel.pixels_written)}
}
#[cfg(test)]
mod tests {
    use super::*;
    fn pins()->SpiPins {SpiPins{sclk:1<<6,mosi:1<<5,cs:1<<7,..Default::default()}}
    fn cmd(b:&mut Dial,c:u8,data:&[u8]) {b.gpio_changes(&[(4,false)]);b.spi_transfer_pins(3,pins(),&[c],0);b.gpio_changes(&[(4,true)]);b.spi_transfer_pins(3,pins(),data,0);}
    #[test]
    fn gc9a01_route_id_pixels_and_reset() {
        let mut b=Dial::new();assert_eq!(b.spi_transfer_pins(2,pins(),&[4],3),[0,0x9a,1]);assert_eq!(b.spi_transfer_pins(3,SpiPins{sclk:1<<17,..pins()},&[4],3),[0xff;3]);
        cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x3a,&[0x55]);cmd(&mut b,0x2a,&[0,0,0,1]);cmd(&mut b,0x2b,&[0,0,0,0]);cmd(&mut b,0x2c,&[0xf8,0,0x07,0xe0]);assert_eq!(&b.display().unwrap().2[..2],&[0xf800,0x07e0]);
        cmd(&mut b,0x28,&[]);assert!(b.display().unwrap().2.iter().all(|p|*p==0));b.gpio_changes(&[(8,false)]);assert_eq!(b.spi_transfer_pins(3,pins(),&[4],3),[0xff;3]);
    }
    #[test]
    fn touch_report_and_gpio_are_source_mapped() {
        let mut b=Dial::new();assert_eq!(b.encoder(),Some((41,40)));assert_eq!(b.named_pin("sw"),Some(42));let mut devices=b.i2c_devices();assert!(devices.iter().all(|(_,a,d)|*a==0x38 && d.pins()==Some((11,12))));
        b.touch(100,200,true);assert!(b.input_levels().contains(&(14,false)));let d=devices[0].2.as_mut();assert!(d.start(false));d.write(2);d.start(true);assert_eq!((0..5).map(|_|d.read()).collect::<Vec<_>>(),[1,0x80,100,0,200]);b.touch(100,200,false);assert!(b.input_levels().contains(&(14,true)));
        b.touch(1,2,true);b.gpio_changes(&[(8,false)]);assert!(!d.start(false));assert!(b.input_levels().contains(&(14,true)));assert!(b.take_edges().iter().any(|e|e.pin==14 && !e.level));
    }
}

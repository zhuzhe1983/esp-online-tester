//! Arduino Nesso N1 DK001, TPX00227 schematic and M5GFX 65c816a3 /
//! M5Unified 368ad608. ST7789 + FT6336 + dual PI4IOE5V6408 digital subset.
//! Fuel gauge, charger, LoRa RF, buzzer and power-key controller are not modeled.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,ft6336::{Ft6336g,Ft6336State},bmi270::{Bmi270,Bmi270Sample},pi4ioe5v6408::{Pi4ioe5v6408,Pi4ioState}};
use esp_soc::board::{BoardModel,BoardEdge,VirtualCycle,SpiPins};
use esp_soc::devices::{DcsPanel,i2c_bitbang::I2cBitBang};

pub struct NessoN1 {panel:DcsPanel,command:u8,read_at:usize,reset_edges:u64,display_changes:u64,i2c:I2cBitBang,levels:[bool;2],edges:Vec<BoardEdge>,touch:Arc<Mutex<Ft6336State>>,imu:Arc<Mutex<Bmi270Sample>>,io:[Arc<Mutex<Pi4ioState>>;2],touch_irq:bool,cycle:VirtualCycle,gpio_events:u64}
impl NessoN1 {
    pub fn new()->Self {
        let mut touch=Ft6336State::default();touch.powered=true;
        let io=std::array::from_fn(|_|Arc::new(Mutex::new(Pi4ioState::default())));
        // KEY1/KEY2 have board pull-ups; VIN_DET is asserted in this USB-powered profile.
        io[0].lock().expect("Nesso IO mutex poisoned").set_input(0,Some(true));io[0].lock().expect("Nesso IO mutex poisoned").set_input(1,Some(true));io[1].lock().expect("Nesso IO mutex poisoned").set_input(5,Some(true));
        Self{panel:DcsPanel::st7789(),command:0,read_at:0,reset_edges:0,display_changes:0,i2c:I2cBitBang::new(),levels:[true,true],edges:Vec::new(),touch:Arc::new(Mutex::new(touch)),imu:Arc::new(Mutex::new(Bmi270Sample::default())),io,touch_irq:true,cycle:0,gpio_events:0}
    }
    fn sync_reset(&mut self) {let io=self.io[1].lock().expect("Nesso IO mutex poisoned");if self.reset_edges!=io.low_edges[1] {self.reset_edges=io.low_edges[1];self.panel.reset();self.command=0;self.read_at=0;self.display_changes+=1;}}
    fn lit(&self)->bool {let io=self.io[1].lock().expect("Nesso IO mutex poisoned");io.drive(1)==Some(true) && io.drive(6)==Some(true)}
}
impl Default for NessoN1 {fn default()->Self {Self::new()}}
impl BoardModel for NessoN1 {
    fn name(&self)->&'static str {"nesso-n1"}
    fn gpio_changes(&mut self,changes:&[(u8,bool)]) {self.gpio_events+=changes.len() as u64;for &(pin,high) in changes {if pin==16 {self.panel.dc=high;}}}
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {
        self.cycle=cycle;self.gpio_changes(changes);let released=|pin:u8|enabled&(1u64<<pin)==0 || output&(1u64<<pin)!=0;
        let scl=released(8);let sda=self.i2c.drive(scl,released(10));
        for (index,(pin,level)) in [(10,sda),(8,scl)].into_iter().enumerate() {if level!=self.levels[index] {self.levels[index]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
        self.sync_reset();
    }
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        self.sync_reset();
        if self.io[1].lock().expect("Nesso IO mutex poisoned").drive(1)!=Some(true) || pins.sclk&(1<<20)==0 || pins.cs&(1<<17)==0 || (pins.mosi&(1<<21)==0 && pins.miso!=Some(21)) {return vec![0xff;rx_len];}
        for &b in tx {if !self.panel.dc {self.command=b;self.read_at=0;self.display_changes+=1;}self.panel.byte(b);}
        let data:&[u8]=match self.command {0x04=>&[0x85,0x85,0x52],0xda|0xdb=>&[0x85],0xdc=>&[0x52],_=>&[]};
        (0..rx_len).map(|_|{let v=data.get(self.read_at).copied().unwrap_or(0);self.read_at=self.read_at.saturating_add(1);v}).collect()
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();self.i2c.attach(0x38,Box::new(Ft6336g::new(self.touch.clone())));self.i2c.attach(0x68,Box::new(Bmi270::new(self.imu.clone())));
        for (i,state) in self.io.iter().enumerate() {self.i2c.attach(0x43+i as u8,Box::new(Pi4ioe5v6408::new(state.clone())));}
        self.i2c.controller_devices(Some((10,8))).into_iter().map(|(a,d)|(0,a,d)).collect()
    }
    fn advance_to(&mut self,cycle:VirtualCycle) {self.cycle=cycle;}
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(18,false),(3,self.touch_irq),(10,self.levels[0]),(8,self.levels[1])]}
    fn button_at(&mut self,cycle:VirtualCycle,id:&str,pressed:bool)->bool {let pin=match id {"btnA"=>0,"btnB"=>1,_=>return false};self.cycle=cycle;self.io[0].lock().expect("Nesso IO mutex poisoned").set_input(pin,Some(!pressed));true}
    fn touch(&mut self,x:u16,y:u16,down:bool) {self.touch.lock().expect("Nesso touch mutex poisoned").update(x.min(134),y.min(239),down);let level=!down;if level!=self.touch_irq {self.touch_irq=level;self.edges.push(BoardEdge{cycle:self.cycle,pin:3,level});}}
    fn set_imu_sample(&mut self,accel_g:[f64;3],gyro_dps:[f64;3])->bool {if !accel_g.iter().chain(gyro_dps.iter()).all(|v|v.is_finite()) {return false;}let mut s=self.imu.lock().expect("Nesso IMU mutex poisoned");s.accel_g=accel_g;s.gyro_dps=gyro_dps;s.version=s.version.wrapping_add(1);true}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {
        let mut frame=vec![0;135*240];if self.lit() && self.panel.on && !self.panel.sleeping {for y in 0..240 {for x in 0..135 {frame[y*135+x]=self.panel.gram[(40+y)*240+52+x];}}}
        Some((135,240,frame,self.display_version()))
    }
    fn display_version(&self)->u64 {self.panel.pixels_written+self.panel.resets+self.display_changes+u64::from(self.lit())}
    fn display_frames(&self)->u64 {self.panel.frames}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)> {Some((self.panel.gram.clone(),240,320))}
    fn report(&self)->String {format!("[nesso-n1] ST7789 frames={} pixels={} madctl={:#x} lit={} gpio={} expreset={:?} FT6336=single-contact PI4IO=keys/backlight; charger/fuel/LoRa/audio unmodeled",self.panel.frames,self.panel.pixels_written,self.panel.madctl,self.lit(),self.gpio_events,self.io[1].lock().expect("Nesso IO mutex poisoned").drive(1))}
}
#[cfg(test)]
mod tests {
    use super::*;
    fn write(d:&mut dyn I2cDevice,r:u8,v:u8) {assert!(d.start(false));assert!(d.write(r));assert!(d.write(v));}
    fn read(d:&mut dyn I2cDevice,r:u8,n:usize)->Vec<u8> {assert!(d.start(false));assert!(d.write(r));assert!(d.start(true));(0..n).map(|_|d.read()).collect()}
    fn pins()->SpiPins {SpiPins{sclk:1<<20,mosi:1<<21,cs:1<<17,..Default::default()}}
    fn cmd(b:&mut NessoN1,c:u8,data:&[u8]) {b.gpio_changes(&[(16,false)]);b.spi_transfer_pins(2,pins(),&[c],0);b.gpio_changes(&[(16,true)]);b.spi_transfer_pins(2,pins(),data,0);}
    #[test]
    fn expander_keys_and_touch_use_actual_registers_and_routes() {
        let mut b=NessoN1::new();let mut ds=b.i2c_devices();assert!(ds.iter().all(|(_,_,d)|d.pins()==Some((10,8))));assert_eq!(ds.iter().map(|(_,a,_)|*a).collect::<Vec<_>>(),[0x38,0x68,0x43,0x44]);
        assert_eq!(read(ds[2].2.as_mut(),15,1)[0]&3,3);assert!(b.button_at(10,"btnA",true));assert_eq!(read(ds[2].2.as_mut(),15,1)[0]&3,2);assert!(b.button_at(11,"btnB",true));assert_eq!(read(ds[2].2.as_mut(),15,1)[0]&3,0);assert!(!b.button_at(12,"power",true));assert_eq!(b.named_pin("btnA"),None);
        b.touch(300,400,true);assert_eq!(read(ds[0].2.as_mut(),2,5),[1,0x80,134,0,239]);assert!(b.input_levels().contains(&(3,false)));b.touch(134,239,false);assert!(b.take_edges().iter().any(|e|e.pin==3 && e.level));
        assert!(b.input_levels().contains(&(18,false)),"QFN40 Nesso board ID resistor");assert_eq!(b.named_pin("boot"),None,"G9 is IR, not a button");
    }
    #[test]
    fn lcd_requires_expander_reset_backlight_and_source_spi_pins() {
        let mut b=NessoN1::new();let mut ds=b.i2c_devices();assert_eq!(b.spi_transfer_pins(2,pins(),&[4],3),[255;3]);
        let d=ds[3].2.as_mut();write(d,3,0xc7);write(d,7,0x18);write(d,5,0x82);
        assert_eq!(b.spi_transfer_pins(2,pins(),&[4],3),[0x85,0x85,0x52]);assert_eq!(b.spi_transfer_pins(2,SpiPins{cs:1<<23,..pins()},&[4],3),[255;3]);
        cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x3a,&[0x55]);cmd(&mut b,0x2a,&[0,52,0,52]);cmd(&mut b,0x2b,&[0,40,0,40]);cmd(&mut b,0x2c,&[0xf8,0]);
        assert_eq!(b.display().unwrap().2[0],0,"LCD backlight low");write(d,5,0xc2);assert_eq!(b.display().unwrap().2[0],0xf800);write(d,7,0x58);assert_eq!(b.display().unwrap().2[0],0,"high-Z is not power");
        write(d,7,0x18);write(d,5,0xc0);write(d,5,0xc2);cmd(&mut b,0x29,&[]);assert_eq!(b.panel.pixels_written,0,"reset pulse observed even between SPI transfers");assert!(b.display().unwrap().2.iter().all(|v|*v==0));
    }
}

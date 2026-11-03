//! K141 StamPLC v1.0 CPU/BASE schematics; M5StamPLC-UserDemo 7fc748c3.
//! ST7789V2 SPI2, PI4IO front keys/RGB/backlight, AW9523 relay GPIO subset.
//! Ideal fixed digital supplies. Sensors, RTC, SD, industrial buses and power key absent.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,pi4ioe5v6408::{Pi4ioe5v6408,Pi4ioState}};
use esp_soc::board::{BoardModel,BoardEdge,VirtualCycle,SpiPins};
use esp_soc::devices::{DcsPanel,Ws2812Chain,i2c_bitbang::I2cBitBang};
use super::stamplc_io::{Io,Device};
pub struct StamPlc {panel:DcsPanel,io_a:Arc<Mutex<Pi4ioState>>,io_b:Arc<Mutex<Io>>,i2c:I2cBitBang,levels:[bool;3],edges:Vec<BoardEdge>,reset_high:bool,backlight:bool,display_changes:u64,cycle:u64,stamp:Ws2812Chain,stamp_powered:bool,leds:[[u8;3];2],led_updates:u64}
impl StamPlc {
    pub fn new()->Self {Self{panel:DcsPanel::st7789(),io_a:Default::default(),io_b:Arc::new(Mutex::new(Io::new())),i2c:I2cBitBang::new(),levels:[true,true,false],edges:Vec::new(),reset_high:true,backlight:false,display_changes:1,cycle:0,stamp:Ws2812Chain::new(1),stamp_powered:false,leds:[[0;3];2],led_updates:1}}
    fn sync(&mut self,cycle:u64) {
        let a=self.io_a.lock().expect("StamPLC PI4IO poisoned");let lit=self.reset_high && a.drive(7)==Some(false);if lit!=self.backlight {self.backlight=lit;self.display_changes+=1;}
        let front=[6,5,4].map(|p|if a.drive(p)==Some(false) {255}else{0});let leds=[front,if self.stamp_powered {self.stamp.leds[0]}else{[0;3]}];if leds!=self.leds {self.leds=leds;self.led_updates+=1;}
        let irq=a.irq_high() && self.io_b.lock().expect("StamPLC IO poisoned").irq_high();if irq!=self.levels[2] {self.levels[2]=irq;self.edges.push(BoardEdge{cycle,pin:14,level:irq});}
    }
}
impl Default for StamPlc {fn default()->Self {Self::new()}}
impl BoardModel for StamPlc {
    fn name(&self)->&'static str {"stamplc"}
    fn gpio_output_at(&mut self,cycle:VirtualCycle,_changes:&[(u8,bool)],enabled:u64,output:u64) {
        self.cycle=cycle;let high=|pin:u8|enabled&(1u64<<pin)==0 || output&(1u64<<pin)!=0;self.panel.dc=output&(1<<6)!=0;
        let reset=high(3);if self.reset_high && !reset {self.panel.reset();self.io_a.lock().expect("StamPLC PI4IO poisoned").reset();self.io_b.lock().expect("StamPLC IO poisoned").reset();self.display_changes+=1;}
        self.reset_high=reset;self.io_a.lock().expect("StamPLC PI4IO poisoned").powered=reset;self.io_b.lock().expect("StamPLC IO poisoned").powered=reset;
        let powered=enabled&output&(1u64<<38)!=0;if self.stamp_powered && !powered {self.stamp.leds.fill([0;3]);}self.stamp_powered=powered;
        let scl=high(15);let sda=self.i2c.drive(scl,high(13));for(i,(pin,level))in[(13,sda),(15,scl)].into_iter().enumerate(){if self.levels[i]!=level {self.levels[i]=level;self.edges.push(BoardEdge{cycle,pin,level});}}self.sync(cycle);
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.io_a.lock().expect("StamPLC PI4IO poisoned").reset();self.io_b.lock().expect("StamPLC IO poisoned").reset();self.i2c.clear();
        self.i2c.attach(0x43,Box::new(Pi4ioe5v6408::new(self.io_a.clone())));self.i2c.attach(0x59,Box::new(Device::new(self.io_b.clone())));
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some((13,15))).into_iter().map(move|(a,d)|(bus,a,d))).collect()
    }
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        if host!=2 || !self.reset_high || pins.sclk&(1<<7)==0 || pins.cs&(1<<12)==0 || pins.mosi&(1<<8)==0 {return vec![255;rx_len];}
        for &v in tx {if !self.panel.dc {self.display_changes+=1;}self.panel.byte(v);}
        // Write-only subset. No panel identity or GRAM readback is invented.
        vec![255;rx_len]
    }
    fn button_at(&mut self,cycle:VirtualCycle,id:&str,pressed:bool)->bool {
        let pin=match id {"button-a"=>2,"button-b"=>1,"button-c"=>0,_=>return false};self.io_a.lock().expect("StamPLC PI4IO poisoned").set_input(pin,if pressed{Some(false)}else{None});self.sync(cycle);true
    }
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {if pin==21 && self.stamp_powered {self.stamp.from_bits(bits);self.sync(self.cycle);}}
    fn advance_to(&mut self,cycle:VirtualCycle) {self.cycle=cycle;self.sync(cycle);}
    fn take_edges(&mut self)->Vec<BoardEdge> {self.sync(self.cycle);std::mem::take(&mut self.edges)}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true),(46,false),(13,self.levels[0]),(15,self.levels[1]),(14,self.levels[2])]}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.leds,self.led_updates))}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {
        // M5GFX 65c816a38386062d2c3dc47d41237a9083acbc75 Panel_LCD rotation1:
        // offsets40/53, MADCTL64.
        // Fixed physical landscape glass; guest rotations still address the same GRAM.
        let mut frame=vec![0;240*135];if self.reset_high && self.backlight && self.panel.on && !self.panel.sleeping {for y in 0..135 {for x in 0..240 {frame[y*240+x]=self.panel.gram[(40+x)*240+186-y];}}}Some((240,135,frame,self.display_version()))
    }
    fn display_version(&self)->u64 {self.panel.pixels_written+self.panel.resets+self.display_changes}
    fn display_frames(&self)->u64 {self.panel.frames}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)> {Some((self.panel.gram.clone(),240,320))}
    fn report(&self)->String {format!("[stamplc] ST7789V2 frames={} pixels={} backlight={} RGB={:?} relay-drive-mask={:#x}; opto-inputs=disconnected sensors/RTC/SD/CAN/RS485/audio/power/network=unsupported",self.panel.frames,self.panel.pixels_written,self.backlight,self.leds,self.io_b.lock().expect("StamPLC IO poisoned").relay_mask())}
}
#[cfg(test)]
mod tests {
    use super::*;
    fn wr(d:&mut dyn I2cDevice,r:u8,v:u8){assert!(d.start(false));assert!(d.write(r));assert!(d.write(v));}
    fn rd(d:&mut dyn I2cDevice,r:u8)->u8{assert!(d.start(false));d.write(r);d.start(true);d.read()}
    fn pins()->SpiPins{SpiPins{sclk:1<<7,mosi:1<<8,miso:Some(9),cs:1<<12}}
    fn cmd(b:&mut StamPlc,c:u8,p:&[u8]){b.panel.dc=false;b.spi_transfer_pins(2,pins(),&[c],0);b.panel.dc=true;b.spi_transfer_pins(2,pins(),p,0);}
    #[test]
    fn factory_rotation_glass_corners_spi_routes_and_pi4_backlight() {
        let mut b=StamPlc::new();let mut devs=b.i2c_devices();let a=devs.iter_mut().find(|(bus,addr,_)|*bus==0&&*addr==0x43).unwrap().2.as_mut();assert_eq!(a.pins(),Some((13,15)));
        cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x36,&[0x64]);cmd(&mut b,0x3a,&[0x55]);
        for(x,y,c)in[(0u16,0u16,0xf800u16),(239,0,0x07e0),(0,134,0x001f),(239,134,0xffff)]{let x=(40+x).to_be_bytes();let y=(53+y).to_be_bytes();cmd(&mut b,0x2a,&[x[0],x[1],x[0],x[1]]);cmd(&mut b,0x2b,&[y[0],y[1],y[0],y[1]]);cmd(&mut b,0x2c,&c.to_be_bytes());}
        assert!(b.display().unwrap().2.iter().all(|v|*v==0));wr(a,3,0x80);wr(a,7,0x7f);wr(a,5,0);b.advance_to(1);let f=b.display().unwrap().2;assert_eq!([f[0],f[239],f[134*240],f[135*240-1]],[0xf800,0x07e0,0x001f,0xffff]);
        let n=b.panel.pixels_written;b.spi_transfer_pins(2,SpiPins{cs:1<<10,..pins()},&[0xff;6],0);assert_eq!(b.panel.pixels_written,n);wr(a,5,0x80);b.advance_to(2);assert!(b.display().unwrap().2.iter().all(|v|*v==0));
        b.gpio_output_at(3,&[],1<<3,0);assert!(!a.start(false));assert_eq!(b.spi_transfer_pins(2,pins(),&[4],3),[255;3]);assert_eq!(b.io_b.lock().unwrap().relay_mask(),0);
    }
    #[test]
    fn physical_keys_use_pulls_expander_irq_and_rgb_high_impedance() {
        let mut b=StamPlc::new();let mut devs=b.i2c_devices();let a=devs.iter_mut().find(|(bus,addr,_)|*bus==0&&*addr==0x43).unwrap().2.as_mut();rd(a,1);wr(a,3,0x70);wr(a,13,7);wr(a,9,7);rd(a,19);assert_eq!(rd(a,15)&7,7);
        assert!(b.button_at(1,"button-a",true));assert_eq!(rd(a,15)&7,3);assert!(!b.io_a.lock().unwrap().irq_high());assert_eq!(rd(a,19)&7,4);assert!(b.io_a.lock().unwrap().irq_high());b.button_at(2,"button-a",false);assert_eq!(rd(a,15)&7,7);
        wr(a,5,0);wr(a,7,0xbf);b.advance_to(3);assert_eq!(b.leds[0],[255,0,0]);wr(a,7,0xff);b.advance_to(4);assert_eq!(b.leds[0],[0;3]);b.rmt_frame(21,&[true;24]);assert_eq!(b.leds[1],[0;3]);assert!(!b.button_at(5,"power",true));assert_eq!(b.named_pin("button-a"),None);
    }
}

//! C151 / schematic V0.5, M5GFX 5268353 and M5Unified 46aa9ede.
//! ED2208 SPI, M5PM1, SHT40 samples, register-only RX8130, A/B/C and G21 RGB.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,Reg8Device,m5::{M5pm1,M5pm1State},sht4x::{Sht4x,Sht4xState}};
use esp_soc::board::{BoardModel,BoardEdge,VirtualCycle,SpiPins};
use esp_soc::devices::{Ws2812Chain,ed2208::Ed2208,i2c_bitbang::I2cBitBang};

struct Wiring {panel:Mutex<Ed2208>,pmic:Arc<Mutex<M5pm1State>>,active:Mutex<bool>}
impl Wiring {fn sync(&self)->bool {
    let p=self.pmic.lock().expect("PaperColor PMIC poisoned");let active=!p.power_off_requested && p.driven_high&1!=0;drop(p);
    let mut previous=self.active.lock().expect("PaperColor power poisoned");if *previous && !active {self.panel.lock().expect("PaperColor panel poisoned").reset();}*previous=active;active
}}
struct PowerDevice {device:M5pm1,wiring:Arc<Wiring>}
impl I2cDevice for PowerDevice {
    fn start(&mut self,r:bool)->bool {self.device.start(r)}
    fn write(&mut self,b:u8)->bool {let ack=self.device.write(b);self.wiring.sync();ack}
    fn read(&mut self)->u8 {self.device.read()}
    fn stop(&mut self) {self.device.stop();}
}
pub struct PaperColor {wiring:Arc<Wiring>,sensor:Arc<Mutex<Sht4xState>>,i2c:I2cBitBang,levels:[bool;2],edges:Vec<BoardEdge>,dc:bool,reset_high:bool,cycle:u64,gpio_events:u64,rgb:Ws2812Chain,leds:[[u8;3];2],led_updates:u64,led_power:bool}
impl PaperColor {
    pub fn new()->Self {Self{wiring:Arc::new(Wiring{panel:Mutex::new(Ed2208::new(11,crate::periph::CPU_HZ/1000)),pmic:Arc::new(Mutex::new(M5pm1State{input_mask:2,input_high:2,..Default::default()})),active:Mutex::new(false)}),sensor:Default::default(),i2c:I2cBitBang::new(),levels:[true;2],edges:Vec::new(),dc:false,reset_high:true,cycle:0,gpio_events:0,rgb:Ws2812Chain::new(2),leds:[[0;3];2],led_updates:0,led_power:true}}
    fn sync_leds(&mut self) {
        let on=self.wiring.pmic.lock().expect("PaperColor PMIC poisoned").ldo3v3_enabled;
        if self.led_power && !on {self.rgb=Ws2812Chain::new(2);}self.led_power=on;
        let colors=if on {[self.rgb.leds[0],self.rgb.leds[1]]}else{[[0;3];2]};if colors!=self.leds {self.leds=colors;self.led_updates+=1;}
    }
}
impl Default for PaperColor {fn default()->Self {Self::new()}}
impl BoardModel for PaperColor {
    fn name(&self)->&'static str {"papercolor"}
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {
        self.cycle=cycle;self.gpio_events+=changes.len() as u64;self.dc=output&(1<<43)!=0;
        let high=|pin:u8|enabled&(1u64<<pin)==0 || output&(1u64<<pin)!=0;
        let reset=high(12);if self.reset_high && !reset {self.wiring.panel.lock().expect("PaperColor panel poisoned").reset();}self.reset_high=reset;
        let scl=high(2);let sda=self.i2c.drive(scl,high(3));for (i,(pin,level)) in [(3,sda),(2,scl)].into_iter().enumerate(){if self.levels[i]!=level{self.levels[i]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
    }
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        if self.reset_high && self.wiring.sync() && pins.sclk&(1<<15)!=0 && pins.mosi&(1<<13)!=0 && pins.cs&(1<<44)!=0 {self.wiring.panel.lock().expect("PaperColor panel poisoned").write(self.dc,tx);}
        vec![0xff;rx_len]
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();self.i2c.attach(0x6e,Box::new(PowerDevice{device:M5pm1::new(self.wiring.pmic.clone()),wiring:self.wiring.clone()}));
        self.i2c.attach(0x44,Box::new(Sht4x::new(self.sensor.clone(),crate::periph::CPU_HZ/1_000_000)));
        // Required for identification and factory flag RAM. Calendar progression,
        // alarm/wake and battery-domain retention are not represented by register RAM.
        self.i2c.attach(0x32,Box::new(Reg8Device::new("rx8130-register-ram",&[])));
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some((3,2))).into_iter().map(move |(address,device)|(bus,address,device))).collect()
    }
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true),(1,true),(9,true),(10,true),(7,true),(11,self.wiring.panel.lock().expect("PaperColor panel poisoned").busy_high()),(3,self.levels[0]),(2,self.levels[1])]}
    fn advance_to(&mut self,cycle:VirtualCycle) {
        self.cycle=cycle;self.wiring.sync();self.wiring.panel.lock().expect("PaperColor panel poisoned").advance_to(cycle);self.sync_leds();
        let mut sensor=self.sensor.lock().expect("PaperColor SHT40 poisoned");sensor.cycle=cycle;sensor.powered= !self.wiring.pmic.lock().expect("PaperColor PMIC poisoned").power_off_requested;
    }
    fn next_deadline(&self)->Option<VirtualCycle> {self.wiring.panel.lock().expect("PaperColor panel poisoned").next_deadline()}
    fn take_edges(&mut self)->Vec<BoardEdge> {let mut edges=std::mem::take(&mut self.edges);edges.extend(self.wiring.panel.lock().expect("PaperColor panel poisoned").take_edges());edges.sort_by_key(|e|e.cycle);edges}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {let panel=self.wiring.panel.lock().expect("PaperColor panel poisoned");Some((400,600,panel.frame.clone(),panel.version))}
    fn display_version(&self)->u64 {self.wiring.panel.lock().expect("PaperColor panel poisoned").version}
    fn display_frames(&self)->u64 {self.wiring.panel.lock().expect("PaperColor panel poisoned").frames}
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {self.sync_leds();if pin==21 && self.led_power {self.rgb.from_bits(bits);self.sync_leds();}}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.leds,self.led_updates))}
    fn named_pin(&self,name:&str)->Option<u8> {match name {"button-a"|"btnA"=>Some(9),"button-b"|"btnB"=>Some(10),"button-c"|"btnC"=>Some(1),_=>None}}
    fn report(&self)->String {let p=self.wiring.panel.lock().expect("PaperColor panel poisoned");format!("[papercolor] ED2208 frames={} ram_bytes={} invalid_colors={} RGB={} RTC=register-ram TF=absent sensor=synthetic audio/network=unverified",p.frames,p.ram_bytes_written,p.invalid_colors,self.led_updates)}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wr(d:&mut dyn I2cDevice,r:u8,v:u8) {assert!(d.start(false));assert!(d.write(r));assert!(d.write(v));d.stop();}
    fn send(b:&mut PaperColor,c:u8,d:&[u8],pins:SpiPins) {b.dc=false;b.spi_transfer_pins(2,pins,&[c],0);b.dc=true;b.spi_transfer_pins(2,pins,d,0);}
    #[test]
    fn display_requires_physical_pins_power_reset_and_refresh() {
        let mut b=PaperColor::new();let pins=SpiPins{sclk:1<<15,mosi:1<<13,cs:1<<44,..Default::default()};
        send(&mut b,0x10,&[0],pins);assert_eq!(b.wiring.panel.lock().unwrap().ram_bytes_written,0);
        let mut devices=b.i2c_devices();assert!(devices.iter().all(|(_,_,d)|d.pins()==Some((3,2))));let pm=devices.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x6e).unwrap().2.as_mut();
        for (r,v) in [(0x10,1),(0x11,1),(0x13,0)] {wr(pm,r,v);}
        send(&mut b,0x10,&[0],SpiPins{sclk:1<<16,..pins});assert_eq!(b.wiring.panel.lock().unwrap().ram_bytes_written,0);
        send(&mut b,0x61,&[1,0x90,2,0x58],pins);send(&mut b,0x10,&[0x01],pins);send(&mut b,0x04,&[],pins);b.advance_to(crate::periph::CPU_HZ/1000);send(&mut b,0x12,&[0],pins);
        b.advance_to(crate::periph::CPU_HZ/10);assert_eq!(&b.display().unwrap().2[..2],&[0,0xffff]);
        wr(pm,0x11,0);assert_eq!(b.display().unwrap().2[0],0,"optical image survives rail off");
        assert_eq!(b.named_pin("button-a"),Some(9));assert_eq!(b.named_pin("button-b"),Some(10));assert_eq!(b.named_pin("button-c"),Some(1));assert_eq!(b.named_pin("power"),None);
    }
    #[test]
    fn rgb_requires_g21_and_ldo_and_power_loss_drops_latched_colors() {
        let mut b=PaperColor::new();let mut devices=b.i2c_devices();let pm=devices.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x6e).unwrap().2.as_mut();
        b.rmt_frame(20,&[true;48]);assert_eq!(b.leds,[[0;3];2]);b.rmt_frame(21,&[true;48]);assert_eq!(b.leds,[[255;3];2]);
        wr(pm,6,0x13);b.advance_to(1);assert_eq!(b.leds,[[0;3];2]);b.rmt_frame(21,&[true;48]);wr(pm,6,0x17);b.advance_to(2);assert_eq!(b.leds,[[0;3];2]);
    }
}

//! CloudZao (formerly Metalio) Claw 1.54 display/input subset.
//! CloudZao/Metalio-Claw-1.54 ba2fcbde5cd7c42e6f684aea64bed63951309684:
//! config.h, metalio-claw-1.54.cc and main/display/touch_feed.cc.
//! No modem, ES8311, battery ADC, power latch or factory-application claim.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,cst816s::{Cst816s,Cst816State}};
use esp_soc::board::{BoardModel,BoardEdge,SpiPins,VirtualCycle};
use esp_soc::devices::i2c_bitbang::I2cBitBang;
use esp_soc::pins::PinRoutes;
use super::lilygo_display::{LilygoDisplay,Profile};

// Sharing the proven DCS/SPI implementation does not share another board's
// wiring, firmware identity or validation. CST816S is polled: the official
// board initializes BOTH int_gpio_num and rst_gpio_num to GPIO_NUM_NC.
const DISPLAY:Profile=Profile{name:"cloudzao-claw-154",controller:"ST7789",gram:[240,320],glass:[0,0,240,240],spi:[9,10,14],dc:Some(8),reset:Some(18),light:Some((13,true)),light_pwm:true,enable:None,quad:false,buttons:&[("boot",0),("power",47)],encoder:None,touch:None,expander:false};

pub struct Claw154 {display:LilygoDisplay,touch:Arc<Mutex<Cst816State>>,i2c:I2cBitBang,levels:[bool;2],edges:Vec<BoardEdge>}
impl Claw154 {
    pub fn new()->Self {let mut touch=Cst816State::default();touch.powered=true;Self{display:LilygoDisplay::new(&DISPLAY),touch:Arc::new(Mutex::new(touch)),i2c:I2cBitBang::new(),levels:[true,true],edges:Vec::new()}}
}
impl Default for Claw154 {fn default()->Self {Self::new()}}
impl BoardModel for Claw154 {
    fn name(&self)->&'static str {"cloudzao-claw-154"}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,cycle:VirtualCycle,routes:PinRoutes<'_>) {
        self.display.observe_routes(cycle,&routes);
        let scl=routes.software_output(42)!=Some(false);let sda=self.i2c.drive(scl,routes.software_output(41)!=Some(false));
        for (i,(pin,level)) in [(41,sda),(42,scl)].into_iter().enumerate() {if self.levels[i]!=level {self.levels[i]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
    }
    fn uses_gpio_pwm(&self)->bool {true}
    fn gpio_pwm_at(&mut self,cycle:VirtualCycle,pwm:&dyn Fn(u8)->Option<(f64,u32)>) {self.display.gpio_pwm_at(cycle,pwm);}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,host:u8,pins:SpiPins,tx:&[u8],rx:usize)->Vec<u8> {self.display.spi_transfer_pins(host,pins,tx,rx)}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();self.i2c.attach(0x15,Box::new(Cst816s::new(self.touch.clone())));
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some((41,42))).into_iter().map(move |(address,device)|(bus,address,device))).collect()
    }
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true),(47,true),(41,self.levels[0]),(42,self.levels[1])]}
    fn named_pin(&self,name:&str)->Option<u8> {self.display.named_pin(name)}
    fn touch(&mut self,x:u16,y:u16,down:bool) {
        // esp_lcd_touch 1.2.1 mirror transforms use x_max - raw (not -1).
        // Host coordinates are visible pixels; register bytes remain physical
        // raw coordinates and pass through the guest driver's mirror flags.
        self.touch.lock().expect("Claw154 touch mutex poisoned").update(240-x.min(239),240-y.min(239),down);
    }
    fn button_at(&mut self,_cycle:VirtualCycle,id:&str,pressed:bool)->bool {
        // touch_feed.cc explicitly gives screen-space x=180/200/220 and
        // y about -360 for the cover strip. Inverse mirror gives raw y=600.
        // This is a source-derived logical contact, not measured calibration.
        let x=match id {"previous"=>60,"home"=>40,"next"=>20,_=>return false};
        self.touch.lock().expect("Claw154 touch mutex poisoned").update(x,600,pressed);true
    }
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {self.display.display()}
    fn display_version(&self)->u64 {self.display.display_version()}
    fn display_frames(&self)->u64 {self.display.display_frames()}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)> {self.display.gram()}
    fn report(&self)->String {format!("{}; CST816S@15h SDA41/SCL42 polled/no IRQ; mirrored contacts and cover navigation; power key GPIO47 only, no physical power-off",self.display.report())}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn report(d:&mut dyn I2cDevice)->Vec<u8> {assert!(d.start(false));assert!(d.write(2));assert!(d.start(true));(0..5).map(|_|d.read()).collect()}
    #[test]
    fn polled_touch_uses_exact_routes_mirrors_cover_coordinates_and_release() {
        let mut b=Claw154::new();let mut devices=b.i2c_devices();
        assert_eq!(devices.len(),2);assert!(devices.iter().all(|(_,a,d)|*a==0x15&&d.pins()==Some((41,42))));assert_eq!(b.touch_irq(),None);
        let d=devices[0].2.as_mut();b.touch(0,239,true);assert_eq!(report(d),[1,0,240,0,1]);
        b.touch(999,999,false);assert_eq!(report(d),[0,0,1,0,1]);
        for (id,x) in [("previous",60),("home",40),("next",20)] {assert!(b.button_at(10,id,true));assert_eq!(report(d),[1,0,x,2,88]);assert!(b.button_at(11,id,false));assert_eq!(report(d)[0],0);}
        assert!(!b.button_at(12,"volume-up",true));assert_eq!(b.named_pin("boot"),Some(0));assert_eq!(b.named_pin("power"),Some(47));
        assert!(b.sd_card_slot().is_none());assert!(b.leds().is_none());assert!(!b.set_imu_sample([0.0;3],[0.0;3]));
        assert!(b.take_edges().is_empty(),"polled contacts cannot invent a touch interrupt GPIO");
    }
    #[test]
    fn touch_sleep_does_not_wake_by_resetting_the_unrelated_display() {
        let mut b=Claw154::new();let mut devices=b.i2c_devices();let d=devices[0].2.as_mut();
        assert!(d.start(false));d.write(0xa5);d.write(3);assert!(!d.start(true));
        b.touch(120,120,true);assert!(!d.start(true));
        let gpio=esp_periph::Gpio::new();let mux=esp_periph::RegRam::new();b.gpio_waveform_at(100,esp_soc::pins::ChipPins::S3.routes(&gpio,&mux));
        assert!(!d.start(false),"LCD GPIO18 is not a documented touch reset");
    }
}

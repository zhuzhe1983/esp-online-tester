//! ESP32-C3-LCD-1.47 at manufacturer commit a62da5a5c5dfdc1f2b2c8fe37283aa784f47cbab.
//! CH32V003 EX0/EX2 select LCD/TF; there is no ESP GPIO chip-select substitute.
//! Storefront says172x320/ST7798; both current BSPs use ST7789T180x320 at X30.
//! We expose the advertised172-column glass centred in the240-column GRAM
//! (X34), documenting the four-column BSP/visible-area discrepancy each side.
use esp_periph::{i2c::I2cDevice,sdmmc::{SdCardConfig,SdStore,card::spi::SpiSdCard}};
use esp_soc::{BoardModel,board::SpiQspiPins,pins::PinRoutes,devices::{DcsPanel,
    imu_motion::ImuMotion,qmi8658::Qmi8658,waveshare_touch_parts::{Ch32Port,Ch32V003}}};
use std::{cell::RefCell,rc::Rc,sync::{Arc,Mutex,atomic::Ordering}};
type PersistentDevice=Rc<RefCell<Box<dyn I2cDevice>>>;
struct RoutedDevice(PersistentDevice);
impl I2cDevice for RoutedDevice {
    fn pins(&self)->Option<(u8,u8)>{Some((4,3))}
    fn start_address_with_clock(&mut self,c:u8,a:u8,r:bool,h:Option<u32>)->bool{self.0.borrow_mut().start_address_with_clock(c,a,r,h)}
    fn write(&mut self,b:u8)->bool{self.0.borrow_mut().write(b)}
    fn read(&mut self)->u8{self.0.borrow_mut().read()}
    fn stop(&mut self){self.0.borrow_mut().stop();}
}
pub struct Lcd147 {
    panel:DcsPanel,port:Arc<Mutex<Ch32Port>>,dc:Option<bool>,reset_high:bool,reset_falling:u64,
    backlight:u8,command:u8,generation:u64,rejected:u64,devices:Vec<(u8,PersistentDevice)>,
    motion:Arc<ImuMotion>,sd:Option<SpiSdCard>,lcd_selected:bool,card_selected:bool,
}
impl Default for Lcd147{fn default()->Self{Self::new()}}
impl Lcd147 {
    pub fn new()->Self{
        let port=Arc::new(Mutex::new(Ch32Port::new()));let motion=Arc::new(ImuMotion::default());
        let parts:Vec<(u8,Box<dyn I2cDevice>)>=vec![(0x24,Box::new(Ch32V003::new(port.clone()))),
            (0x6b,Box::new(Qmi8658::new(motion.clone(),crate::periph::CPU_HZ)))];
        Self{panel:DcsPanel::st7789(),port,dc:None,reset_high:false,reset_falling:0,backlight:0,command:0,generation:1,rejected:0,
            devices:parts.into_iter().map(|(a,d)|(a,Rc::new(RefCell::new(d)))).collect(),motion,sd:None,lcd_selected:false,card_selected:false}
    }
    fn sync_ports(&mut self){
        let p=self.port.lock().expect("C3 CH32V003 state");let high=p.high(1);let falling=p.falling_edges(1);let brightness=p.pwm.unwrap_or(0);
        let lcd=p.direction&1!=0&&!p.high(0);let card=p.direction&4!=0&&!p.high(2);drop(p);
        if falling!=self.reset_falling||self.reset_high&&!high{self.panel.reset();self.command=0;self.generation=self.generation.wrapping_add(1);}
        if self.reset_high!=high||self.backlight!=brightness{self.generation=self.generation.wrapping_add(1);}
        if self.card_selected&&!card{if let Some(sd)=&mut self.sd{sd.deselect();}}
        self.reset_high=high;self.reset_falling=falling;self.backlight=brightness;self.lcd_selected=lcd;self.card_selected=card;
    }
}
impl BoardModel for Lcd147 {
    fn name(&self)->&'static str{"waveshare-c3-lcd147"}
    fn named_pin(&self,name:&str)->Option<u8>{matches!(name,"boot"|"button").then_some(9)}
    fn input_levels(&self)->Vec<(u8,bool)>{vec![(9,true)]}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{self.devices.iter().map(|(a,d)|(0,*a,Box::new(RoutedDevice(d.clone()))as Box<dyn I2cDevice>)).collect()}
    fn advance_to(&mut self,cycle:u64){self.motion.cycle.store(cycle,Ordering::Relaxed);self.sync_ports();}
    fn set_imu_motion(&mut self,mode:u32)->bool{self.motion.mode.store(mode,Ordering::Relaxed);true}
    fn imu_motion(&self)->u32{self.motion.mode.load(Ordering::Relaxed)}
    fn uses_gpio_routes(&self)->bool{true}
    fn gpio_waveform_at(&mut self,cycle:u64,r:PinRoutes<'_>){self.advance_to(cycle);self.dc=r.software_output(8);}
    fn uses_qspi_pins(&self)->bool{true}
    fn spi_transfer_qspi_pins(&mut self,host:u8,p:SpiQspiPins,tx:&[u8],rx_len:usize)->Vec<u8>{
        self.sync_ports();let floating=vec![0xff;rx_len];
        if host!=2||p.unsupported_mode||p.sio||p.clock_idle_high||p.clock_output_edge
            ||[p.command_width,p.address_width,p.data_width,p.read_width].iter().any(|&w|w>1)
            ||p.sclk&(1<<7)==0||p.data[0]&(1<<5)==0||!p.command_bits.is_multiple_of(8)||!p.address_bits.is_multiple_of(8)
            ||!p.data_bits.is_multiple_of(8)||!p.read_bits.is_multiple_of(8)||p.dummy_cycles!=0{self.rejected+=1;return floating;}
        if self.lcd_selected&&self.card_selected{self.rejected+=1;if let Some(sd)=&mut self.sd{sd.deselect();}return floating;}
        if let Some(sd)=&mut self.sd{
            let rx=if p.full_duplex||tx.is_empty()||rx_len==0{sd.transfer(self.card_selected,tx,rx_len)}else{sd.transfer(self.card_selected,tx,0);sd.transfer(self.card_selected,&[],rx_len)};
            if self.card_selected{return if p.input[1]==Some(6){rx}else{floating};}
        }
        if self.lcd_selected&&self.reset_high&&self.dc.is_some(){
            if rx_len!=0||p.full_duplex{self.rejected+=1;return floating;}
            self.panel.dc=self.dc==Some(true);
            if self.panel.dc&&self.command==0x2c&&self.panel.colmod!=0x55{self.rejected+=1;return floating;}
            for &b in tx{if !self.panel.dc{self.command=b;}self.panel.byte(b);}
            if !tx.is_empty(){self.generation=self.generation.wrapping_add(1);}
        }floating
    }
    fn attach_spi_sd_card(&mut self,image:Box<dyn SdStore>,config:SdCardConfig)->Result<String,String>{let sd=SpiSdCard::new(image,config)?;let result=sd.describe();self.sd=Some(sd);Ok(result)}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{
        let p=&self.panel;let visible=self.reset_high&&self.backlight!=0&&p.on&&!p.sleeping;
        // Pinned ST7789.c:140 uses mirror(true,false): native front view
        // reverses the glass column scan. Guest MADCTL still transforms GRAM.
        let pixels=(0..320).flat_map(|y|(0..172).map(move|x|{
            if !visible{return 0;}let mut v=p.gram[y*240+(239-x-34)];if p.madctl&8!=0{v=(v&0x7e0)|((v&0xf800)>>11)|((v&31)<<11);}
            let d=u32::from(self.backlight);(((u32::from(v>>11)*d/255)<<11)|((u32::from((v>>5)&63)*d/255)<<5)|(u32::from(v&31)*d/255))as u16
        })).collect();Some((172,320,pixels,self.generation))
    }
    fn display_version(&self)->u64{self.generation}
    fn display_frames(&self)->u64{self.panel.frames}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)>{Some((self.panel.gram.clone(),240,320))}
    fn report(&self)->String{format!("[waveshare-c3-lcd147] ST7789T-compatible RGB565 172x320 visible/X34/source-MX-reference; BSP180/X30 conflict retained; pixels={} RAMWR={} duty={}/255 rejected={}; CH32V00324 EX0=LCDCS EX1=RST EX2=SDCS; QMI8658-common6b BOOT9; SD {}; ADC/analog color/IMU IRQ/RF unverified",self.panel.pixels_written,self.panel.frames,self.backlight,self.rejected,self.sd.as_ref().map_or_else(||"absent".into(),SpiSdCard::report))}
}

#[cfg(test)]mod tests{
    use super::*;
    use esp_periph::{Gpio,RegRam};
    use esp_soc::pins::ChipPins;
    fn out(b:&mut Lcd147,reg:u8,v:u8){let mut d=b.i2c_devices().remove(0).2;assert_eq!(d.pins(),Some((4,3)));assert!(d.start_address_with_clock(0x24,0x24,false,None));assert!(d.write(reg));assert!(d.write(v));d.stop();b.advance_to(0);}
    fn dc(b:&mut Lcd147,data:bool){let mut g=Gpio::new();let mut m=RegRam::new();g.func_out_sel[8]=128;g.enable=1<<8;if data{g.out=1<<8;}m.write(4+8*4,1<<12|1<<9);b.gpio_waveform_at(0,ChipPins::C3.routes(&g,&m));}
    fn pins()->SpiQspiPins{SpiQspiPins{sclk:1<<7,data:[1<<5,0,0,0],data_width:1,data_bits:8,..Default::default()}}
    fn send(b:&mut Lcd147,c:u8,bytes:&[u8]){dc(b,false);b.spi_transfer_qspi_pins(2,pins(),&[c],0);dc(b,true);b.spi_transfer_qspi_pins(2,SpiQspiPins{data_bits:bytes.len()as u32*8,..pins()},bytes,0);}
    #[test]fn waveshare_c3_expander_selects_display_and_reset_without_fake_gpio_cs(){
        let mut b=Lcd147::new();assert_eq!(b.named_pin("boot"),Some(9));assert_eq!(b.i2c_devices().iter().map(|(_,a,_)|*a).collect::<Vec<_>>(),[0x24,0x6b]);
        out(&mut b,3,6);out(&mut b,5,255);send(&mut b,0x11,&[]);send(&mut b,0x29,&[]);send(&mut b,0x3a,&[0x55]);send(&mut b,0x36,&[0x40]);send(&mut b,0x2a,&[0,34,0,34]);send(&mut b,0x2b,&[0,0,0,0]);send(&mut b,0x2c,&[0xf8,0]);assert_eq!(b.display().unwrap().2[0],0xf800);
        out(&mut b,3,7);send(&mut b,0x2c,&[0,31]);assert_eq!(b.panel.pixels_written,1,"GPIO CS masks cannot select the expander-wired panel");
        out(&mut b,3,2);send(&mut b,0x2c,&[0,31]);assert_eq!(b.panel.pixels_written,1,"two active physical chip selects reject the bus collision");
        out(&mut b,3,6);dc(&mut b,true);b.spi_transfer_qspi_pins(2,SpiQspiPins{sclk:1<<5,..pins()},&[0,31],0);assert_eq!(b.panel.pixels_written,1);
        out(&mut b,3,4);assert_eq!(b.panel.resets,1);assert!(b.display().unwrap().2.iter().all(|v|*v==0));
    }
    #[test]fn waveshare_c3_tf_card_uses_ex2_and_physical_miso6(){
        use esp_periph::sdmmc::card::MemStore;
        let mut b=Lcd147::new();b.attach_spi_sd_card(Box::new(MemStore(vec![0;1024*1024])),SdCardConfig::default()).unwrap();
        out(&mut b,3,7);let p=SpiQspiPins{full_duplex:true,input:[None,Some(6),None,None],data_bits:80,read_bits:80,..pins()};b.spi_transfer_qspi_pins(2,p,&[255;10],10);
        out(&mut b,3,3);let rx=b.spi_transfer_qspi_pins(2,SpiQspiPins{data_bits:48,read_bits:48,..p},&[0x40,0,0,0,0,0x95],6);assert!(rx.iter().all(|b|*b==255));
        let rx=b.spi_transfer_qspi_pins(2,SpiQspiPins{data_bits:64,read_bits:64,..p},&[255;8],8);assert!(rx.contains(&1));
        let wrong=b.spi_transfer_qspi_pins(2,SpiQspiPins{input:[None,Some(5),None,None],data_bits:64,read_bits:64,..p},&[255;8],8);assert!(wrong.iter().all(|b|*b==255));
        out(&mut b,3,7);assert!(!b.card_selected);
    }
}

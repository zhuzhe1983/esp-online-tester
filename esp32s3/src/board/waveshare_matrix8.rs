//! ESP32-S3-Matrix SKU27119, not the HUB75 ESP32-S3-RGB-Matrix board.
//! Original schematic: S3FH4R2, RGB data14, QMI8658C SDA11/SCL12.
//! Original WS_Matrix.cpp explicitly selects NEO_RGB + NEO_KHZ800.
//! Chain index order is exposed separately from uncalibrated physical LED pose.
use std::{cell::RefCell,rc::Rc,sync::{Arc,atomic::Ordering}};
use esp_periph::i2c::I2cDevice;
use esp_soc::{board::{BoardModel,BoardEdge},pins::PinRoutes,devices::{Ws2812Chain,i2c_bitbang::I2cBitBang,
    imu_motion::ImuMotion,qmi8658::{Qmi8658,Qmi8658Port}}};
const HZ:u64=240_000_000;
pub struct WaveshareMatrix8{chain:Ws2812Chain,leds:[[u8;3];64],motion:Arc<ImuMotion>,qmi:Rc<RefCell<Qmi8658>>,i2c:I2cBitBang,attached:bool,levels:[bool;2],edges:Vec<BoardEdge>}
impl Default for WaveshareMatrix8{fn default()->Self{Self::new()}}
impl WaveshareMatrix8{
 pub fn new()->Self{let motion=Arc::new(ImuMotion::default());let qmi=Rc::new(RefCell::new(Qmi8658::new_c(motion.clone(),HZ)));Self{chain:Ws2812Chain::new(64).with_gpio_clock(HZ as u32),leds:[[0;3];64],motion,qmi,i2c:I2cBitBang::new(),attached:false,levels:[true;2],edges:Vec::new()}}
 fn sync(&mut self){for(d,s)in self.leds.iter_mut().zip(&self.chain.leds){*d=[s[1],s[0],s[2]];}}
}
impl BoardModel for WaveshareMatrix8{
 fn name(&self)->&'static str{"waveshare-s3-matrix8"}
 fn uses_gpio_routes(&self)->bool{true}
 fn uses_gpio_waveform(&self)->bool{true}
 fn gpio_waveform_at(&mut self,c:u64,p:PinRoutes<'_>){let level=p.software_output(14);self.chain.gpio_drive(c,level.is_some(),level.unwrap_or(false));self.sync();let clock=p.software_output(12).unwrap_or(true);let data=self.i2c.drive(clock,p.software_output(11).unwrap_or(true));for(i,(pin,level))in[(11,data),(12,clock)].into_iter().enumerate(){if self.levels[i]!=level{self.levels[i]=level;self.edges.push(BoardEdge{cycle:c,pin,level});}}}
 fn next_deadline(&self)->Option<u64>{self.chain.gpio_deadline()}
 fn advance_to(&mut self,c:u64){self.chain.advance_gpio(c);self.sync();self.motion.cycle.store(c,Ordering::Relaxed);}
 fn rmt_frame(&mut self,pin:u8,bits:&[bool]){if pin==14{self.chain.from_bits(bits);self.sync();}}
 fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{if !self.attached{self.i2c.attach(0x6b,Box::new(Qmi8658Port::new(self.qmi.clone())));self.attached=true;}(0..2).flat_map(|b|self.i2c.controller_devices(Some((11,12))).into_iter().map(move|(a,d)|(b,a,d))).collect()}
 fn input_levels(&self)->Vec<(u8,bool)>{vec![(0,true),(11,self.levels[0]),(12,self.levels[1])]}
 fn named_pin(&self,n:&str)->Option<u8>{(n=="boot").then_some(0)}
 fn take_edges(&mut self)->Vec<BoardEdge>{std::mem::take(&mut self.edges)}
 fn set_imu_motion(&mut self,m:u32)->bool{self.qmi.borrow_mut().set_motion_mode(m)}
 fn set_imu_sample(&mut self,a:[f64;3],g:[f64;3])->bool{self.qmi.borrow_mut().set_host_sample(a,g)}
 fn imu_motion(&self)->u32{self.motion.mode.load(Ordering::Relaxed)}
 fn leds(&self)->Option<(&[[u8;3]],u64)>{Some((&self.leds,self.chain.updates))}
 fn report(&self)->String{format!("ESP32-S3-Matrix SKU27119 FH4R2 4F/2P: 64 LEDs GPIO14 NEO_RGB wire order; BOOT0; QMI8658C@6B SDA11/SCL12 digital sample subset. chainUpdates={}. No HUB75/display glass/TF, unmodeled RF, sensor interrupts, external Dout extension and analog current/brightness/thermal response. LED chain-to-physical-position pose is not calibrated; no original factory binary found in the retained source archive.",self.chain.updates)}
}
#[cfg(test)]mod tests{use super::*;
 fn bits(bytes:&[u8])->Vec<bool>{bytes.iter().flat_map(|b|(0..8).map(move|i|b&(0x80>>i)!=0)).collect()}
 #[test]fn original_rgb_order_all_64_chain_positions_and_wrong_route(){let mut b=WaveshareMatrix8::new();let bytes=(0..64).flat_map(|i|[i,255-i,17]).collect::<Vec<_>>();b.rmt_frame(21,&bits(&bytes));assert_eq!(b.chain.updates,0);b.rmt_frame(14,&bits(&bytes));for(i,c)in b.leds.iter().enumerate(){assert_eq!(*c,[i as u8,255-i as u8,17]);}assert!(b.display().is_none());assert!(b.sd_card_slot().is_none());let ds=b.i2c_devices();assert_eq!(ds.len(),2);assert!(ds.iter().all(|(_,a,d)|*a==0x6b&&d.pins()==Some((11,12))));assert_eq!(b.named_pin("boot"),Some(0));}
 #[test]fn matrix_gpio_pulses_require_mux_and_latch(){let mut b=WaveshareMatrix8::new();let mut g=esp_periph::Gpio::new();let mut m=esp_periph::RegRam::new();m.write(4+4*14,1<<12);g.enable=1<<14;let mut c=0;for bit in bits(&[0x12,0x34,0x56]){g.out=1<<14;b.gpio_waveform_at(c,esp_soc::pins::ChipPins::S3.routes(&g,&m));c+=if bit{192}else{96};g.out=0;b.gpio_waveform_at(c,esp_soc::pins::ChipPins::S3.routes(&g,&m));c+=if bit{108}else{204};}let end=b.next_deadline().unwrap();b.advance_to(end-1);assert_eq!(b.chain.updates,0);b.advance_to(end);assert_eq!(b.leds[0],[0x12,0x34,0x56]);g.enable=0;b.gpio_waveform_at(end+1,esp_soc::pins::ChipPins::S3.routes(&g,&m));assert!(b.next_deadline().is_none());}
}

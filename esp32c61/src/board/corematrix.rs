//! M5Stack C158 CoreMatrix, schematic v0.7 (2026-06-09), official M5GFX
//! 28d47949 Panel_TM1680 and M5Unified pin definitions. All displayed pixels
//! come from actual TM1680 I2C writes; no host-selected image/animation path.
use std::{rc::Rc,sync::{Arc,Mutex}};
use esp_periph::i2c::{I2cDevice,bmi270::{Bmi270,Bmi270Sample},m5::{M5pm1,M5pm1State,M5ioe1,M5ioe1State},tm1680::{Tm1680,Tm1680State}};
use esp_periph::sdmmc::{SdStore,SdCardConfig,card::spi::SpiSdCard};
use esp_soc::{BoardModel,board::{BoardEdge,SpiPins,BoardPowerState,BoardPowerEvent},pins::PinRoutes,devices::i2c_bitbang::I2cBitBang};
const CPU_HZ:u64=160_000_000;
const AUDIO_HZ:u32=48_000;

struct Wiring {
    pmic:Arc<Mutex<M5pm1State>>,ioe:Arc<Mutex<M5ioe1State>>,ioe_device:Arc<Mutex<M5ioe1>>,matrix:Arc<Mutex<Tm1680State>>,
    imu:Arc<Mutex<Bmi270>>,sample:Arc<Mutex<Bmi270Sample>>,sd:Mutex<Option<Box<SpiSdCard>>>,rails:Mutex<(bool,bool,bool)>,
}
impl Wiring {
    fn main_powered(&self)->bool {let pm=self.pmic.lock().expect("CoreMatrix PM1 poisoned");pm.dcdc_enabled&&!pm.power_off_requested}
    fn imu_powered(&self)->bool {self.pmic.lock().expect("CoreMatrix PM1 poisoned").ldo3v3_enabled}
    fn reset_peripherals(&self) {
        *self.ioe_device.lock().expect("CoreMatrix IOE device poisoned")=M5ioe1::new(self.ioe.clone(),0);
        self.matrix.lock().expect("CoreMatrix matrix poisoned").set_power(false);
        if let Some(card)=&mut *self.sd.lock().expect("CoreMatrix SD poisoned"){card.power_cycle();}
    }
    fn sync(&self) {
        let main=self.main_powered();let imu=self.imu_powered();let old=*self.rails.lock().expect("CoreMatrix rails poisoned");
        if old.0&&!main {self.reset_peripherals();}
        if old.2&&!imu {*self.imu.lock().expect("CoreMatrix IMU poisoned")=Bmi270::new(self.sample.clone());}
        let high=self.ioe.lock().expect("CoreMatrix IOE poisoned").driven_high;
        self.matrix.lock().expect("CoreMatrix matrix poisoned").set_power(main&&high&8!=0);
        let sd=main&&high&1!=0;
        if old.1&&!sd {if let Some(card)=&mut *self.sd.lock().expect("CoreMatrix SD poisoned"){card.power_cycle();}}
        *self.rails.lock().expect("CoreMatrix rails poisoned")=(main,sd,imu);
    }
}
struct RailDevice {inner:Box<dyn I2cDevice>,wiring:Rc<Wiring>,always:bool}
impl I2cDevice for RailDevice {
    fn start(&mut self,read:bool)->bool {self.wiring.sync();(self.always||self.wiring.main_powered())&&self.inner.start(read)}
    fn start_address_with_clock(&mut self,c:u8,a:u8,read:bool,hz:Option<u32>)->bool {self.wiring.sync();(self.always||self.wiring.main_powered())&&self.inner.start_address_with_clock(c,a,read,hz)}
    fn write(&mut self,b:u8)->bool {if !self.always&&!self.wiring.main_powered(){return false;}let ack=self.inner.write(b);self.wiring.sync();ack}
    fn read(&mut self)->u8 {if self.always||self.wiring.main_powered(){self.inner.read()}else{0xff}}
    fn stop(&mut self) {self.inner.stop();self.wiring.sync();}
}
struct SharedDevice<T>(Arc<Mutex<T>>);
impl<T:I2cDevice> I2cDevice for SharedDevice<T> {
    fn start(&mut self,read:bool)->bool {self.0.lock().expect("CoreMatrix device poisoned").start(read)}
    fn start_address_with_clock(&mut self,c:u8,a:u8,read:bool,hz:Option<u32>)->bool {self.0.lock().expect("CoreMatrix device poisoned").start_address_with_clock(c,a,read,hz)}
    fn write(&mut self,b:u8)->bool {self.0.lock().expect("CoreMatrix device poisoned").write(b)}
    fn read(&mut self)->u8 {self.0.lock().expect("CoreMatrix device poisoned").read()}
    fn stop(&mut self) {self.0.lock().expect("CoreMatrix device poisoned").stop();}
}
struct ImuDevice(Rc<Wiring>);
impl I2cDevice for ImuDevice {
    fn start(&mut self,read:bool)->bool {self.0.imu_powered()&&self.0.imu.lock().expect("CoreMatrix IMU poisoned").start(read)}
    fn write(&mut self,b:u8)->bool {self.0.imu_powered()&&self.0.imu.lock().expect("CoreMatrix IMU poisoned").write(b)}
    fn read(&mut self)->u8 {if self.0.imu_powered(){self.0.imu.lock().expect("CoreMatrix IMU poisoned").read()}else{0xff}}
}

pub struct CoreMatrix {
    pmic_device:Arc<Mutex<M5pm1>>,ioe_device:Arc<Mutex<M5ioe1>>,wiring:Rc<Wiring>,i2c:I2cBitBang,levels:[bool;2],edges:Vec<BoardEdge>,irq:bool,
    cycle:u64,leds:[[u8;3];256],led_version:u64,pcm:Vec<i16>,audio_remainder:u64,
    buzzer_phase:u64,gpio_events:u64,sd_present:bool,
}
impl CoreMatrix {
    pub fn new()->Self {
        let sample=Arc::new(Mutex::new(Bmi270Sample{accel_g:[0.0,0.0,1.0],..Default::default()}));
        let pmic=Arc::new(Mutex::new(M5pm1State{input_mask:7,input_high:7,vin_mv:5000,..Default::default()}));
        let ioe=Arc::new(Mutex::new(M5ioe1State{input_mask:0x280,input_high:0x280,..Default::default()}));
        let ioe_device=Arc::new(Mutex::new(M5ioe1::new(ioe.clone(),0)));
        Self{pmic_device:Arc::new(Mutex::new(M5pm1::new(pmic.clone()))),ioe_device:ioe_device.clone(),wiring:Rc::new(Wiring{pmic,ioe,ioe_device,matrix:Arc::new(Mutex::new(Tm1680State::new(CPU_HZ))),imu:Arc::new(Mutex::new(Bmi270::new(sample.clone()))),sample,sd:Mutex::new(None),rails:Mutex::new((true,false,true))}),i2c:I2cBitBang::new(),levels:[true;2],edges:Vec::new(),irq:true,cycle:0,leds:[[0;3];256],led_version:0,pcm:Vec::new(),audio_remainder:0,buzzer_phase:0,gpio_events:0,sd_present:false}
    }
    fn synchronize(&mut self) {
        self.wiring.sync();
        let level=if self.wiring.imu_powered() {let mut imu=self.wiring.imu.lock().expect("CoreMatrix IMU poisoned");imu.advance_to_us(self.cycle/(CPU_HZ/1_000_000));imu.interrupt_pins()[0]}else{None};
        {let mut pm=self.wiring.pmic.lock().expect("CoreMatrix PM1 poisoned");if let Some(high)=level {pm.set_input(4,high);}else{pm.release_input(4);}}
        // An IMU edge may wake the PMIC and restore its DCDC output.
        self.wiring.sync();let mut m=self.wiring.matrix.lock().expect("CoreMatrix matrix poisoned");m.advance_to(self.cycle);
        if self.led_version!=m.version {for (i,led) in self.leds.iter_mut().enumerate(){*led=[m.pixel(i/16,i%16);3];}self.led_version=m.version;}
        drop(m);let pm=self.wiring.pmic.lock().expect("CoreMatrix PM1 poisoned");let irq=!(pm.irq_low&&pm.irq_pins&8!=0);drop(pm);
        if irq!=self.irq {self.irq=irq;self.edges.push(BoardEdge{cycle:self.cycle,pin:2,level:irq});}
    }
    fn synthesize(&mut self,delta:u64) {
        let main=self.wiring.main_powered();let ioe=*self.wiring.ioe.lock().expect("CoreMatrix IOE poisoned");
        let frequency=u64::from(ioe.pwm_frequency_hz);let duty=u64::from(ioe.pwm[2]);
        let active=main&&ioe.driven_high&2!=0&&frequency!=0&&duty>0&&duty<4095;
        // Use exact guest-cycle accumulation. Sample count/phase survive capture
        // draining; no host-time stretching, resampling or synthesized UI notes.
        let total=u128::from(self.audio_remainder)+u128::from(delta)*u128::from(AUDIO_HZ);
        let count=(total/u128::from(CPU_HZ)) as usize;self.audio_remainder=(total%u128::from(CPU_HZ)) as u64;
        let modulus=u64::from(AUDIO_HZ)*4095;
        for _ in 0..count {
            let phase=(self.buzzer_phase+frequency*4095/2)%modulus;
            self.pcm.push(if active {if phase<duty*u64::from(AUDIO_HZ){8192}else{-8192}}else{0});
            self.buzzer_phase=(self.buzzer_phase+frequency*4095)%modulus;
        }
    }
}
impl Default for CoreMatrix {fn default()->Self {Self::new()}}
impl BoardModel for CoreMatrix {
    fn name(&self)->&'static str {"corematrix"}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();
        for (address,inner,always) in [
            (0x6e,Box::new(SharedDevice(self.pmic_device.clone())) as Box<dyn I2cDevice>,true),
            (0x4f,Box::new(SharedDevice(self.ioe_device.clone())),false),
        ] {self.i2c.attach(address,Box::new(RailDevice{inner,wiring:self.wiring.clone(),always}));}
        self.i2c.attach(0x68,Box::new(ImuDevice(self.wiring.clone())));
        self.i2c.attach(0x72,Box::new(Tm1680::new(self.wiring.matrix.clone())));
        // A CPU-only reset does not cycle independent PM1/IOE/LED/SD supplies.
        self.pcm.clear();self.synchronize();
        self.i2c.controller_devices(Some((0,1))).into_iter().map(|(address,device)|(0,address,device)).collect()
    }
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,cycle:u64,routes:PinRoutes<'_>) {
        self.advance_to(cycle);let scl=routes.software_output(1)!=Some(false);let sda=self.i2c.drive(scl,routes.software_output(0)!=Some(false));
        for (i,(pin,level)) in [(0,sda),(1,scl)].into_iter().enumerate(){if self.levels[i]!=level {self.levels[i]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
        // Socket CS release ends a partial SPI command/data phase.
        if routes.software_output(28)!=Some(false){if let Some(sd)=&mut *self.wiring.sd.lock().expect("CoreMatrix SD poisoned"){sd.deselect();}}
        self.synchronize();
    }
    fn gpio_output_at(&mut self,_cycle:u64,changes:&[(u8,bool)],_enabled:u64,_output:u64) {self.gpio_events=self.gpio_events.wrapping_add(changes.len() as u64);}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,self.levels[0]),(1,self.levels[1]),(2,self.irq),(9,true)]}
    fn take_edges(&mut self)->Vec<BoardEdge> {self.synchronize();std::mem::take(&mut self.edges)}
    fn advance_to(&mut self,cycle:u64) {
        let cycle=self.cycle.max(cycle);
        loop {
            let at=self.next_deadline().filter(|next|*next>self.cycle&&*next<cycle).unwrap_or(cycle);
            self.synthesize(at-self.cycle);self.cycle=at;self.wiring.pmic.lock().expect("CoreMatrix PM1 poisoned").advance_to_ms(at/(CPU_HZ/1000));self.synchronize();
            if at==cycle {break;}
        }
    }
    fn next_deadline(&self)->Option<u64> {
        let matrix=self.wiring.matrix.lock().expect("CoreMatrix matrix poisoned").next_deadline();
        let power=self.wiring.pmic.lock().expect("CoreMatrix PM1 poisoned").next_deadline_ms().and_then(|ms|ms.checked_mul(CPU_HZ/1000));
        let imu=if self.wiring.imu_powered(){self.wiring.imu.lock().expect("CoreMatrix IMU poisoned").next_deadline_us().and_then(|us|us.checked_mul(CPU_HZ/1_000_000))}else{None};
        matrix.into_iter().chain(power).chain(imu).filter(|at|*at>self.cycle).min()
    }
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {let m=self.wiring.matrix.lock().expect("CoreMatrix matrix poisoned");Some((16,16,(0..256).map(|i|{let v=u16::from(m.pixel(i/16,i%16));((v>>3)<<11)|((v>>2)<<5)|(v>>3)}).collect(),m.version))}
    fn display_frames(&self)->u64 {self.wiring.matrix.lock().expect("CoreMatrix matrix poisoned").frames}
    fn display_version(&self)->u64 {self.wiring.matrix.lock().expect("CoreMatrix matrix poisoned").version}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.leds,self.led_version))}
    fn button_at(&mut self,cycle:u64,id:&str,pressed:bool)->bool {
        if id=="power"||id=="button-power" {self.advance_to(cycle);self.wiring.pmic.lock().expect("CoreMatrix PM1 poisoned").power_button(pressed,cycle/(CPU_HZ/1000));self.synchronize();return true;}
        let pin=match id {"button-a"|"btnA"=>0,"button-b"|"btnB"=>1,"button-c"|"btnC"=>2,_=>return false};
        self.advance_to(cycle);self.wiring.pmic.lock().expect("CoreMatrix PM1 poisoned").set_input(pin,!pressed);self.synchronize();true
    }
    fn set_imu_sample(&mut self,accel_g:[f64;3],gyro_dps:[f64;3])->bool {
        if !accel_g.iter().chain(gyro_dps.iter()).all(|v|v.is_finite()){return false;}
        self.synchronize(); // sample the old stimulus through the current guest horizon
        let mut sample=self.wiring.sample.lock().expect("CoreMatrix sample poisoned");sample.accel_g=accel_g;sample.gyro_dps=gyro_dps;sample.version=sample.version.wrapping_add(1);true
    }
    fn uses_spi_pins(&self)->bool {true}
    fn attach_spi_sd_card(&mut self,image:Box<dyn SdStore>,config:SdCardConfig)->Result<String,String> {let card=SpiSdCard::new(image,config)?;let description=card.describe();*self.wiring.sd.lock().expect("CoreMatrix SD poisoned")=Some(Box::new(card));self.set_sd_card_present(true);Ok(description)}
    fn set_sd_card_present(&mut self,present:bool) {self.sd_present=present;let mut ioe=self.wiring.ioe.lock().expect("CoreMatrix IOE poisoned");ioe.input_high=(ioe.input_high&!0x200)|if present {0}else{0x200};drop(ioe);if !present {if let Some(sd)=&mut *self.wiring.sd.lock().expect("CoreMatrix SD poisoned"){sd.power_cycle();}}}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        self.wiring.sync();if !self.sd_present||!self.wiring.rails.lock().expect("CoreMatrix rails poisoned").1||pins.sclk&(1<<25)==0||pins.mosi&(1<<27)==0{return vec![255;rx_len];}
        let mut sd=self.wiring.sd.lock().expect("CoreMatrix SD poisoned");let Some(sd)=sd.as_mut()else{return vec![255;rx_len]};
        let rx=sd.transfer(pins.cs&(1<<28)!=0,tx,rx_len);if pins.miso==Some(26){rx}else{vec![255;rx_len]}
    }
    fn power_state(&self)->Option<BoardPowerState> {Some(BoardPowerState{controller_powered:self.wiring.main_powered()})}
    fn take_power_event(&mut self)->Option<BoardPowerEvent> {
        let mut pm=self.wiring.pmic.lock().expect("CoreMatrix PM1 poisoned");
        if !pm.reset_requested&&!pm.wake_requested&&!pm.download_requested{return None;}
        let download=pm.download_requested;pm.reset_requested=false;pm.wake_requested=false;pm.download_requested=false;drop(pm);
        self.wiring.reset_peripherals();self.synchronize();Some(BoardPowerEvent::Reset{download})
    }
    fn audio(&self)->(&[i16],u32) {(&self.pcm,AUDIO_HZ)}
    fn clear_audio_capture(&mut self) {self.pcm.clear();}
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn hardware_diagnostics(&self)->Option<String> {
        let m=self.wiring.matrix.lock().expect("CoreMatrix matrix poisoned");let ioe=*self.wiring.ioe.lock().expect("CoreMatrix IOE poisoned");
        Some(format!("{{\"schemaVersion\":1,\"board\":\"corematrix\",\"state\":{{\"controllerPowered\":{},\"matrix\":{{\"powered\":{},\"enabled\":{},\"pwmSteps\":{},\"frames\":{},\"bytesWritten\":{}}},\"buzzer\":{{\"frequencyHz\":{},\"duty4095\":{},\"supplyEnabled\":{}}},\"grove5vEnabled\":{},\"sdPresent\":{}}}}}",self.wiring.main_powered(),m.powered(),m.enabled(),m.pwm_steps(),m.frames,m.bytes_written,ioe.pwm_frequency_hz,ioe.pwm[2],self.wiring.main_powered()&&ioe.driven_high&2!=0,self.wiring.main_powered()&&ioe.driven_high&16!=0,self.sd_present))
    }
    fn report(&self)->String {format!("[corematrix] TM1680={} frames; SD={}; PWM buzzer=48kHz digital; IMU=clocked data-ready/motion IRQ",self.display_frames(),self.wiring.sd.lock().expect("CoreMatrix SD poisoned").as_ref().map_or_else(||"absent".into(),|sd|sd.report()))}
}

#[cfg(test)]
mod tests {
    use super::*;
    type Devices=Vec<(u8,u8,Box<dyn I2cDevice>)>;
    fn device(ds:&mut Devices,address:u8)->&mut dyn I2cDevice {ds.iter_mut().find(|(_,a,_)|*a==address).unwrap().2.as_mut()}
    fn write(ds:&mut Devices,a:u8,bytes:&[u8]) {let d=device(ds,a);assert!(d.start(false));for &b in bytes {assert!(d.write(b));}d.stop();}
    fn read(ds:&mut Devices,a:u8,reg:u8,n:usize)->Vec<u8> {let d=device(ds,a);assert!(d.start(false));assert!(d.write(reg));assert!(d.start(true));let out=(0..n).map(|_|d.read()).collect();d.stop();out}
    fn enable(ds:&mut Devices,mask:u16) {for (reg,value) in [(3,mask as u8),(4,(mask>>8) as u8),(0x13,0),(0x14,0),(5,mask as u8),(6,(mask>>8) as u8)] {write(ds,0x4f,&[reg,value]);}}
    #[test]
    fn matrix_uses_real_i2c_power_nibbles_and_shared_cpu_reset_state() {
        let mut b=CoreMatrix::new();let mut ds=b.i2c_devices();assert_eq!(ds.len(),4);assert!(ds.iter().all(|(bus,_,d)|*bus==0&&d.pins()==Some((0,1))));
        assert!(!device(&mut ds,0x72).start(false));write(&mut ds,0x4f,&[5,8]);assert!(!device(&mut ds,0x72).start(false),"an output latch alone doesn't power the LEDs");
        enable(&mut ds,8);assert!(!device(&mut ds,0x72).start(true));write(&mut ds,0x72,&[0xa4,0x98,0x81,0xbf,0x83,0,0x10]);b.advance_to(1);
        assert_eq!(b.leds().unwrap().0[0],[255;3]);assert_eq!(b.leds().unwrap().0.iter().filter(|x|**x!=[0;3]).count(),1);assert_eq!(b.display().unwrap().2[0],0xffff);
        write(&mut ds,0x72,&[0xb7]);b.advance_to(2);assert_eq!(b.leds().unwrap().0[0],[128;3]);
        ds=b.i2c_devices();assert_eq!(b.leds().unwrap().0[0],[128;3],"CPU-only reset doesn't cut independent LED supply");
        write(&mut ds,0x4f,&[5,0]);b.advance_to(3);assert_eq!(b.leds().unwrap().0[0],[0;3]);enable(&mut ds,8);write(&mut ds,0x72,&[0xa4,0x81,0xbf,0x83]);b.advance_to(4);assert_eq!(b.leds().unwrap().0[0],[0;3],"rail recovery needs a fresh frame");
    }
    #[test]
    fn controller_register_commands_nack_wrong_route_and_feed_actual_matrix_ram() {
        use esp_periph::i2c::{I2c,INT_NACK};
        fn send(master:&mut I2c,address:u8,bytes:&[u8])->bool {
            master.write(0x24,u32::MAX);master.write(0x18,3<<12);master.write(0x1c,u32::from(address));
            for &v in bytes {master.write(0x1c,u32::from(v));}
            master.write(0x58,6<<11);master.write(0x5c,(1<<11)|(1<<8)|(bytes.len() as u32+1));master.write(0x60,2<<11);
            master.write(4,1<<5);esp_periph::device::Device::tick(master,u64::MAX);master.int_raw&INT_NACK==0
        }
        let mut b=CoreMatrix::new();let mut master=I2c::new();for (_,a,d) in b.i2c_devices(){master.attach(a,d);}
        assert!(!send(&mut master,0x9e,&[3,8]));master.set_pins(Some((1,0)));assert!(!send(&mut master,0x9e,&[3,8]));
        master.set_pins(Some((0,1)));for bytes in [&[3,8][..],&[0x13,0],&[5,8]] {assert!(send(&mut master,0x9e,bytes));}
        master.set_pins(Some((1,0)));assert!(!send(&mut master,0xe4,&[0xa4,0x81,0xbf,0x83,0,0x10]));assert!(b.display().unwrap().2.iter().all(|v|*v==0));
        master.set_pins(Some((0,1)));assert!(!send(&mut master,0xe2,&[0xa4,0x81,0xbf,0x83,0,0x10]));
        assert!(send(&mut master,0xe4,&[0xa4,0x81,0xbf,0x83,0,0x10]));b.advance_to(1);assert_eq!(b.leds().unwrap().0[0],[255;3]);
        assert!(!send(&mut master,0xe5,&[]),"TM1680 read address NACKs even on the correct powered bus");
    }
    #[test]
    fn pmic_button_input_irq_and_power_wake_have_physical_routes() {
        let mut b=CoreMatrix::new();let mut ds=b.i2c_devices();assert_eq!(read(&mut ds,0x6e,0x12,1)[0]&7,7);
        assert!(b.button_at(1,"button-c",true));assert_eq!(read(&mut ds,0x6e,0x12,1)[0]&7,3);assert!(!b.button_at(2,"button-d",true));
        b.button_at(3,"button-c",false);write(&mut ds,0x6e,&[0x16,0x40]);write(&mut ds,0x6e,&[0x43,0x1b]);write(&mut ds,0x6e,&[0x40,0,0,0]);
        b.button_at(4,"button-c",true);assert!(b.take_edges().iter().any(|e|e.pin==2&&!e.level));assert_eq!(read(&mut ds,0x6e,0x40,1)[0]&4,4);
        write(&mut ds,0x6e,&[0x40,0]);assert!(b.take_edges().iter().any(|e|e.pin==2&&e.level));b.button_at(5,"button-c",false);
        write(&mut ds,0x6e,&[0x18,4,0]);write(&mut ds,0x6e,&[0x0c,0xa1]);assert!(!b.power_state().unwrap().controller_powered);
        assert!(!device(&mut ds,0x4f).start(false));assert!(device(&mut ds,0x6e).start(false));
        b.button_at(6,"button-c",true);assert!(b.power_state().unwrap().controller_powered);assert_eq!(b.take_power_event(),Some(BoardPowerEvent::Reset{download:false}));assert_eq!(b.take_power_event(),None);
    }
    #[test]
    fn power_key_double_click_stops_supply_and_release_then_wake_does_not_retrigger() {
        let mut b=CoreMatrix::new();let _ds=b.i2c_devices();let ms=CPU_HZ/1000;
        b.button_at(10*ms,"power",true);b.button_at(30*ms,"power",false);
        b.button_at(100*ms,"power",true);b.button_at(120*ms,"power",false);
        assert!(!b.power_state().unwrap().controller_powered);assert_eq!(b.take_power_event(),None);
        b.button_at(300*ms,"power",true);assert!(b.power_state().unwrap().controller_powered);
        assert_eq!(b.take_power_event(),Some(BoardPowerEvent::Reset{download:false}));
        b.button_at(320*ms,"power",false);b.advance_to(1000*ms);assert_eq!(b.take_power_event(),None,"releasing a wake press must not enqueue a second reset");
        b.button_at(1100*ms,"power",true);b.button_at(1120*ms,"power",false);
        let deadline=b.next_deadline().unwrap();assert!(deadline>1120*ms);b.advance_to(deadline);
        assert_eq!(b.take_power_event(),Some(BoardPowerEvent::Reset{download:false}));
    }
    #[test]
    fn sd_detect_power_routing_and_protocol_reset_preserve_storage() {
        let mut b=CoreMatrix::new();let mut ds=b.i2c_devices();let pins=SpiPins{sclk:1<<25,mosi:1<<27,miso:Some(26),cs:1<<28};let cmd=[0x40,0,0,0,0,0x95];
        assert_eq!(read(&mut ds,0x4f,8,1)[0]&2,2);b.attach_spi_sd_card(Box::new(esp_periph::sdmmc::MemStore(vec![0;1<<20])),Default::default()).unwrap();assert_eq!(read(&mut ds,0x4f,8,1)[0]&2,0);
        b.spi_transfer_pins(2,pins,&cmd,0);assert_eq!(b.spi_transfer_pins(2,pins,&[],2),[255;2]);enable(&mut ds,1);
        b.spi_transfer_pins(2,SpiPins{cs:0,..pins},&[255;10],0);
        for bad in [SpiPins{sclk:1<<24,..pins},SpiPins{mosi:1<<26,..pins},SpiPins{cs:1<<29,..pins}] {b.spi_transfer_pins(2,bad,&cmd,0);assert_eq!(b.spi_transfer_pins(2,pins,&[],2),[255;2]);}
        b.spi_transfer_pins(2,pins,&cmd,0);assert_eq!(b.spi_transfer_pins(2,pins,&[],2),[255,1]);
        write(&mut ds,0x4f,&[5,0]);enable(&mut ds,1);b.spi_transfer_pins(2,pins,&cmd,0);assert_eq!(b.spi_transfer_pins(2,pins,&[],2),[255;2],"power-cycled card requires startup clocks again");
        b.set_sd_card_present(false);assert_eq!(read(&mut ds,0x4f,8,1)[0]&2,2);assert_eq!(b.spi_transfer_pins(2,pins,&[],3),[255;3]);assert!(b.sd_card_slot().is_none());
    }
    #[test]
    fn real_expander_pwm_drives_audio_and_capture_drain_keeps_phase() {
        let mut b=CoreMatrix::new();let mut ds=b.i2c_devices();enable(&mut ds,0x402);
        write(&mut ds,0x4f,&[0x25,0xe8,3]);write(&mut ds,0x4f,&[0x1f,0,0x88]);
        b.advance_to(CPU_HZ/100);let (pcm,hz)=b.audio();assert_eq!(hz,48_000);assert_eq!(pcm.len(),480);assert!(pcm.contains(&8192)&&pcm.contains(&-8192));
        let positive=pcm.iter().filter(|x|**x>0).count();assert_eq!(positive,240);assert_eq!(pcm.windows(2).filter(|p|p[0]!=p[1]).count(),19);let phase=b.buzzer_phase;b.clear_audio_capture();assert_eq!(b.buzzer_phase,phase);assert!(b.audio().0.is_empty());
        write(&mut ds,0x4f,&[5,0]);b.advance_to(CPU_HZ/50);assert_eq!(b.audio().0,vec![0;480]);
    }
    #[test]
    fn imu_int1_wakes_only_with_l1_hold_and_actual_feature_duration() {
        for hold in [false,true] {
            let mut b=CoreMatrix::new();let mut ds=b.i2c_devices();b.set_imu_sample([0.;3],[0.;3]);
            write(&mut ds,0x68,&[0x59,0]);let mut config=vec![0;8193];config[0]=0x5e;write(&mut ds,0x68,&config);write(&mut ds,0x68,&[0x59,1]);write(&mut ds,0x68,&[0x7d,4]);
            // Bosch standard any-motion:100ms,XYZ,83mg,out_conf7 -> statusbit6.
            for bytes in [&[0x2f,1][..],&[0x3c,5,0xe0,0xaa,0xb8],&[0x53,8],&[0x55,1],&[0x56,0x40]] {write(&mut ds,0x68,bytes);}
            write(&mut ds,0x6e,&[0x18,0x10,0]);write(&mut ds,0x6e,&[7,if hold {0x20}else{0}]);b.advance_to(0);
            write(&mut ds,0x6e,&[0x0c,0xa1]);assert!(!b.power_state().unwrap().controller_powered);
            b.set_imu_sample([0.5,0.,0.],[0.;3]);b.advance_to(CPU_HZ*8/100);assert!(!b.power_state().unwrap().controller_powered,"the sensor's100ms duration must elapse");
            b.advance_to(CPU_HZ/10);assert_eq!(b.power_state().unwrap().controller_powered,hold);
            assert_eq!(b.take_power_event(),if hold {Some(BoardPowerEvent::Reset{download:false})}else{None});
            if hold {assert_eq!(read(&mut ds,0x68,0x21,1),[1],"L1 retains initialized BMI over MCU power-off");assert_ne!(read(&mut ds,0x6e,5,1)[0]&0x20,0);}
        }
    }
    #[test]
    fn nominal_usb_voltage_is_a_read_only_sensor_input() {
        let mut b=CoreMatrix::new();let mut ds=b.i2c_devices();assert_eq!(read(&mut ds,0x6e,0x24,2),5000u16.to_le_bytes());
        write(&mut ds,0x6e,&[0x24,0,0]);assert_eq!(read(&mut ds,0x6e,0x24,2),5000u16.to_le_bytes());assert_eq!(read(&mut ds,0x6e,0x22,2),3900u16.to_le_bytes());
    }
    #[test]
    fn imu_sample_flows_through_real_init_range_and_power_registers() {
        let mut b=CoreMatrix::new();let mut ds=b.i2c_devices();assert_eq!(read(&mut ds,0x68,0,1),[0x24]);
        write(&mut ds,0x68,&[0x59,0]);let mut config=vec![0;8193];config[0]=0x5e;write(&mut ds,0x68,&config);write(&mut ds,0x68,&[0x59,1]);assert_eq!(read(&mut ds,0x68,0x21,1),[1]);
        write(&mut ds,0x68,&[0x7d,6]);assert!(b.set_imu_sample([1.,0.,-1.],[0.,125.,0.]));b.advance_to(CPU_HZ/100);assert_eq!(read(&mut ds,0x68,0x0c,6),[0,16,0,0,0,240]);
        assert!(!b.set_imu_sample([f64::NAN,0.,0.],[0.;3]));write(&mut ds,0x6e,&[6,0x15]);assert_eq!(read(&mut ds,0x68,0x21,1),[1],"IMU has independent L1 LDO supply while core DCDC is off");write(&mut ds,0x6e,&[6,0x13]);assert!(!device(&mut ds,0x68).start(true));write(&mut ds,0x6e,&[6,0x17]);assert_eq!(read(&mut ds,0x68,0x21,1),[0],"lost L1 supply resets sensor protocol");
    }
}

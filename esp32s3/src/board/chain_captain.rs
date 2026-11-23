//! M5Stack C160 Chain Captain. Wiring comes from the official product page
//! (Chain_Captain, English revision 95ce1ef4), schematic
//! Chain_Captain_V0.6_SCH_20261104 (7823cc2e) page 1 power tree / net list, and
//! the factory M5Chain-Captain-UserDemo V0.2 `hal_*` sources (ESP-IDF 5.5.5).
//! The panel is a JD9853A on GPSPI2 host 2 — SCK15/MOSI16/CS45/D-C46 with no
//! MISO — whose reset (IOE1 IO1), supply (IO12) and backlight (IO11, PWM3) are
//! expander outputs, not ESP32 GPIOs. I2C0 (SDA3/SCL2) carries M5PM1 0x6e,
//! M5IOE1 0x4f, BMI270 0x68 and the ES8311 control registers at 0x18. The three
//! user keys are active-low inputs on G1/G4/G5 with internal pull-ups; the power
//! key is a PM1 input (`pmic->btnGetState`), not a GPIO. The RGB LED is three
//! M5IOE1 PWM outputs (IO8=G, IO9=B, IO10=R on one shared frequency), not a
//! WS2812/RMT chain.
use std::{rc::Rc,sync::{Arc,Mutex}};
use esp_periph::i2c::{I2cDevice,Reg8Device,bmi270::{Bmi270,Bmi270Sample},m5::{M5ioe1,M5ioe1State,M5pm1,M5pm1State}};
use esp_soc::{BoardModel,board::{BoardPowerEvent,BoardPowerState,SpiPins,VirtualCycle},devices::{DcsPanel,i2c_bitbang::I2cBitBang}};

const HZ:u64=crate::periph::CPU_HZ;
/// JD9853A internal RAM is 240 columns × 320 rows; the C160 glass shows 240×240.
const COLS:usize=240;const ROWS:usize=240;const GRAM_ROWS:usize=320;
// Schematic net names → ESP32-S3 GPIO.
const TFT_SCK:u8=15;const TFT_MOSI:u8=16;const TFT_CS:u8=45;const TFT_DC:u8=46;
const I2C_SDA:u8=3;const I2C_SCL:u8=2;const KEYS:[u8;3]=[1,4,5];
// M5IOE1 IO1..IO14; `driven_high`/`pwm` bit i names IO(i+1).
const IO_TFT_RST:u16=1<<0;const IO_IR:u16=1<<1;const IO_AUDIO:u16=1<<4;const IO_EXT5V:u16=1<<5;
const IO_RGB_G:u16=1<<7;const IO_RGB_B:u16=1<<8;const IO_RGB_R:u16=1<<9;const IO_BACKLIGHT:u16=1<<10;const IO_TFT_PWR:u16=1<<11;
/// The panel supply and reset are both required before any DCS byte is accepted.
const TFT_ON:u16=IO_TFT_PWR|IO_TFT_RST;

#[derive(Clone,Copy,PartialEq)]enum Rail{Always,L1,Audio}

/// `version` counts every DCS byte the board has accepted plus every panel
/// re-arm, so it only ever grows. `DcsPanel::pixels_written` cannot be part of
/// it: a reset zeroes that counter, and a blanked panel must still look changed.
struct Panel {panel:DcsPanel,active:bool,version:u64}
impl Panel {fn new()->Self {Self{panel:DcsPanel::new(COLS,GRAM_ROWS),active:false,version:0}}}

struct Wiring {
    pmic:Arc<Mutex<M5pm1State>>,ioe:Arc<Mutex<M5ioe1State>>,ioe_device:Arc<Mutex<M5ioe1>>,
    imu:Arc<Mutex<Bmi270>>,sample:Arc<Mutex<Bmi270Sample>>,panel:Mutex<Panel>,rails:Mutex<(bool,bool,bool)>,
}
impl Wiring {
    /// 3V3_L2/L3A: PM1 DCDC3V3_EN drives the core rail and the RGB/key/TFT upstream.
    fn main_powered(&self)->bool {let pm=self.pmic.lock().expect("Chain Captain PM1 poisoned");pm.dcdc_enabled&&!pm.power_off_requested}
    /// 3V3_L1: PM1 LDO3V3_EN feeds M5IOE1 and BMI270 and turns on with power-up.
    fn l1_powered(&self)->bool {self.pmic.lock().expect("Chain Captain PM1 poisoned").ldo3v3_enabled}
    fn driven(&self)->u16 {self.ioe.lock().expect("Chain Captain IOE poisoned").driven_high}
    fn panel_active(&self)->bool {self.main_powered()&&self.driven()&TFT_ON==TFT_ON}
    /// L3B audio sub-switch: IOE1 IO5 gates the ES8311/AW8737A branch.
    fn audio_active(&self)->bool {self.main_powered()&&self.driven()&IO_AUDIO!=0}
    fn live(&self,rail:Rail)->bool {match rail{Rail::Always=>true,Rail::L1=>self.l1_powered(),Rail::Audio=>self.audio_active()}}
    fn reset_peripherals(&self) {
        *self.ioe_device.lock().expect("Chain Captain IOE device poisoned")=M5ioe1::new(self.ioe.clone(),0);
        *self.imu.lock().expect("Chain Captain IMU poisoned")=Bmi270::new(self.sample.clone());
    }
    /// Apply the rail model, resetting any device whose supply or reset just dropped.
    fn sync(&self) {
        let (main,l1,panel)=(self.main_powered(),self.l1_powered(),self.panel_active());
        let old=*self.rails.lock().expect("Chain Captain rails poisoned");
        if (old.0&&!main)||(old.1&&!l1) {self.reset_peripherals();}
        {let mut p=self.panel.lock().expect("Chain Captain panel poisoned");
            if p.active&&!panel {p.panel.reset();}
            if p.active!=panel {p.version=p.version.wrapping_add(1);}
            p.active=panel;}
        *self.rails.lock().expect("Chain Captain rails poisoned")=(main,l1,panel);
    }
}

struct RailDevice {inner:Box<dyn I2cDevice>,wiring:Rc<Wiring>,rail:Rail}
impl I2cDevice for RailDevice {
    fn start(&mut self,read:bool)->bool {self.wiring.sync();self.wiring.live(self.rail)&&self.inner.start(read)}
    fn start_address_with_clock(&mut self,c:u8,a:u8,read:bool,hz:Option<u32>)->bool {self.wiring.sync();self.wiring.live(self.rail)&&self.inner.start_address_with_clock(c,a,read,hz)}
    fn write(&mut self,b:u8)->bool {if !self.wiring.live(self.rail){return false;}let ack=self.inner.write(b);self.wiring.sync();ack}
    fn read(&mut self)->u8 {if self.wiring.live(self.rail){self.inner.read()}else{0xff}}
    fn stop(&mut self) {self.inner.stop();self.wiring.sync();}
}
struct SharedDevice<T>(Arc<Mutex<T>>);
impl<T:I2cDevice> I2cDevice for SharedDevice<T> {
    fn start(&mut self,read:bool)->bool {self.0.lock().expect("Chain Captain device poisoned").start(read)}
    fn start_address_with_clock(&mut self,c:u8,a:u8,read:bool,hz:Option<u32>)->bool {self.0.lock().expect("Chain Captain device poisoned").start_address_with_clock(c,a,read,hz)}
    fn write(&mut self,b:u8)->bool {self.0.lock().expect("Chain Captain device poisoned").write(b)}
    fn read(&mut self)->u8 {self.0.lock().expect("Chain Captain device poisoned").read()}
    fn stop(&mut self) {self.0.lock().expect("Chain Captain device poisoned").stop();}
}
struct ImuDevice(Rc<Wiring>);
impl I2cDevice for ImuDevice {
    fn start(&mut self,read:bool)->bool {self.0.l1_powered()&&self.0.imu.lock().expect("Chain Captain IMU poisoned").start(read)}
    fn write(&mut self,b:u8)->bool {self.0.l1_powered()&&self.0.imu.lock().expect("Chain Captain IMU poisoned").write(b)}
    fn read(&mut self)->u8 {if self.0.l1_powered(){self.0.imu.lock().expect("Chain Captain IMU poisoned").read()}else{0xff}}
}

pub struct ChainCaptain {
    pmic_device:Arc<Mutex<M5pm1>>,ioe_device:Arc<Mutex<M5ioe1>>,wiring:Rc<Wiring>,i2c:I2cBitBang,
    cycle:VirtualCycle,leds:[[u8;3];1],led_version:u64,gpio_events:u64,
}
impl ChainCaptain {
    pub fn new()->Self {
        let sample=Arc::new(Mutex::new(Bmi270Sample::default()));
        let pmic=Arc::new(Mutex::new(M5pm1State::default()));
        let ioe=Arc::new(Mutex::new(M5ioe1State::default()));
        let ioe_device=Arc::new(Mutex::new(M5ioe1::new(ioe.clone(),0)));
        Self{pmic_device:Arc::new(Mutex::new(M5pm1::new(pmic.clone()))),ioe_device:ioe_device.clone(),
            wiring:Rc::new(Wiring{pmic,ioe,ioe_device,imu:Arc::new(Mutex::new(Bmi270::new(sample.clone()))),sample,panel:Mutex::new(Panel::new()),rails:Mutex::new((true,true,false))}),
            i2c:I2cBitBang::new(),cycle:0,leds:[[0;3];1],led_version:0,gpio_events:0}
    }
    /// Re-derive the RGB colour and the IMU clock from real expander/sensor state.
    fn synchronize(&mut self) {
        self.wiring.sync();
        let ioe=*self.wiring.ioe.lock().expect("Chain Captain IOE poisoned");
        let lit=|duty:u16|->u8{if self.wiring.main_powered()&&duty>0{(u32::from(duty)*255/4095) as u8}else{0}};
        // PWM4=IO10 red, PWM2=IO8 green, PWM1=IO9 blue (M5IOE1 1.0.9 channel map).
        let rgb=[lit(ioe.pwm[3]),lit(ioe.pwm[1]),lit(ioe.pwm[0])];
        if rgb!=self.leds[0] {self.leds[0]=rgb;self.led_version=self.led_version.wrapping_add(1);}
        if self.wiring.l1_powered() {self.wiring.imu.lock().expect("Chain Captain IMU poisoned").advance_to_us(self.cycle/(HZ/1_000_000));}
    }
}
impl Default for ChainCaptain {fn default()->Self {Self::new()}}
impl BoardModel for ChainCaptain {
    fn name(&self)->&'static str {"chain-captain"}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();
        // ES8311 answers as control-register RAM only: `Hal::init()` wraps
        // `audioInit()` in ESP_ERROR_CHECK, so 0x18 must ACK or the factory image
        // aborts. Same subset already modelled for StopWatch (ES8311 rev10.0
        // Jan2021 p29 revision registers). There is no I2S, PCM or microphone model.
        for (address,inner,rail) in [
            (0x6e,Box::new(SharedDevice(self.pmic_device.clone())) as Box<dyn I2cDevice>,Rail::Always),
            (0x4f,Box::new(SharedDevice(self.ioe_device.clone())),Rail::L1),
            (0x18,Box::new(Reg8Device::new("es8311-control-registers",&[(0xfd,0x83),(0xfe,0x11)])),Rail::Audio),
        ] {self.i2c.attach(address,Box::new(RailDevice{inner,wiring:self.wiring.clone(),rail}));}
        self.i2c.attach(Bmi270::ADDRESS,Box::new(ImuDevice(self.wiring.clone())));
        // RX8130CE 0x32, IR 38 kHz, the Chain UART daisy-chain and Ext.Port stay
        // unattached, so firmware sees a NACK instead of a pretend device.
        self.synchronize();
        self.i2c.controller_devices(Some((I2C_SDA,I2C_SCL))).into_iter().map(|(address,device)|(0,address,device)).collect()
    }
    fn gpio_changes(&mut self,changes:&[(u8,bool)]) {
        self.gpio_events=self.gpio_events.wrapping_add(changes.len() as u64);
        for &(pin,high) in changes {if pin==TFT_DC {let mut p=self.wiring.panel.lock().expect("Chain Captain panel poisoned");p.panel.dc=high;}}
    }
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        self.wiring.sync();
        // The panel is wired to GPSPI2 with fixed pads and no MISO; anything else
        // is not this display, and reads return the unattached-line level.
        if host!=2 || pins.sclk&(1<<TFT_SCK)==0 || pins.mosi&(1<<TFT_MOSI)==0 || pins.cs&(1<<TFT_CS)==0 || !self.wiring.panel_active() {return vec![0xff;rx_len];}
        let mut p=self.wiring.panel.lock().expect("Chain Captain panel poisoned");
        for &b in tx {p.version=p.version.wrapping_add(1);p.panel.byte(b);}
        vec![0xff;rx_len]
    }
    fn input_levels(&self)->Vec<(u8,bool)> {let high=self.wiring.main_powered();KEYS.iter().map(|&pin|(pin,high)).collect()}
    fn named_pin(&self,name:&str)->Option<u8> {match name {"keyA"|"btnA"|"button-a"|"ok"=>Some(KEYS[0]),"keyB"|"btnB"|"button-b"|"prev"=>Some(KEYS[1]),"keyC"|"btnC"|"button-c"|"next"=>Some(KEYS[2]),"boot"=>Some(0),"sda"=>Some(I2C_SDA),"scl"=>Some(I2C_SCL),"ir-rx"=>Some(47),"ir-tx"=>Some(48),"chain-tx"=>Some(18),"chain-rx"=>Some(17),"grove-tx"=>Some(6),"grove-rx"=>Some(7),_=>None}}
    fn button_at(&mut self,cycle:VirtualCycle,id:&str,pressed:bool)->bool {
        if id=="power"||id=="button-power" {self.advance_to(cycle);self.wiring.pmic.lock().expect("Chain Captain PM1 poisoned").power_button(pressed,cycle/(HZ/1000));self.synchronize();return true;}
        // G1/G4/G5 are plain chip inputs: the SoC GPIO path drives them, so a
        // semantic button id is not needed and must not fake an electrical level.
        false
    }
    fn set_imu_sample(&mut self,accel_g:[f64;3],gyro_dps:[f64;3])->bool {
        if !accel_g.iter().chain(gyro_dps.iter()).all(|v|v.is_finite()){return false;}
        self.synchronize();
        let mut sample=self.wiring.sample.lock().expect("Chain Captain sample poisoned");sample.accel_g=accel_g;sample.gyro_dps=gyro_dps;sample.version=sample.version.wrapping_add(1);true
    }
    fn advance_to(&mut self,cycle:VirtualCycle) {
        let cycle=self.cycle.max(cycle);
        loop {
            let at=self.next_deadline().filter(|next|*next>self.cycle&&*next<cycle).unwrap_or(cycle);
            self.cycle=at;self.wiring.pmic.lock().expect("Chain Captain PM1 poisoned").advance_to_ms(at/(HZ/1000));self.synchronize();
            if at==cycle {break;}
        }
    }
    fn next_deadline(&self)->Option<VirtualCycle> {
        let power=self.wiring.pmic.lock().expect("Chain Captain PM1 poisoned").next_deadline_ms().and_then(|ms|ms.checked_mul(HZ/1000));
        let imu=if self.wiring.l1_powered(){self.wiring.imu.lock().expect("Chain Captain IMU poisoned").next_deadline_us().and_then(|us|us.checked_mul(HZ/1_000_000))}else{None};
        power.into_iter().chain(imu).filter(|at|*at>self.cycle).min()
    }
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {
        // One lock only: `std::sync::Mutex` is not reentrant, so the version is
        // read from the same guard instead of calling `display_version`.
        let p=self.wiring.panel.lock().expect("Chain Captain panel poisoned");
        let mut frame=vec![0;COLS*ROWS];
        if p.active&&p.panel.on&&!p.panel.sleeping {for row in 0..ROWS {for col in 0..COLS {frame[row*COLS+col]=p.panel.gram[(ROWS-1-row)*COLS+(COLS-1-col)];}}}
        Some((COLS as u32,ROWS as u32,frame,p.version))
    }
    fn display_version(&self)->u64 {self.wiring.panel.lock().expect("Chain Captain panel poisoned").version}
    fn display_frames(&self)->u64 {self.wiring.panel.lock().expect("Chain Captain panel poisoned").panel.frames}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)> {Some((self.wiring.panel.lock().expect("Chain Captain panel poisoned").panel.gram.clone(),COLS,GRAM_ROWS))}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.leds,self.led_version))}
    fn power_state(&self)->Option<BoardPowerState> {Some(BoardPowerState{controller_powered:self.wiring.main_powered()})}
    fn take_power_event(&mut self)->Option<BoardPowerEvent> {
        let mut pm=self.wiring.pmic.lock().expect("Chain Captain PM1 poisoned");
        if !pm.reset_requested&&!pm.wake_requested&&!pm.download_requested{return None;}
        let download=pm.download_requested;pm.reset_requested=false;pm.wake_requested=false;pm.download_requested=false;drop(pm);
        self.wiring.reset_peripherals();self.synchronize();Some(BoardPowerEvent::Reset{download})
    }
    fn hardware_diagnostics(&self)->Option<String> {
        // Snapshot every shared rail before formatting so no guard is held while
        // another accessor locks the same mutex.
        let (main,l1)= { let pm=self.wiring.pmic.lock().expect("Chain Captain PM1 poisoned"); (pm.dcdc_enabled&&!pm.power_off_requested,pm.ldo3v3_enabled) };
        let ioe=*self.wiring.ioe.lock().expect("Chain Captain IOE poisoned");
        let p=self.wiring.panel.lock().expect("Chain Captain panel poisoned");
        let tft=main&&ioe.driven_high&TFT_ON==TFT_ON;
        let audio=main&&ioe.driven_high&IO_AUDIO!=0;
        let (frames,pixels,madctl,colmod,on,sleeping)=(p.panel.frames,p.panel.pixels_written,p.panel.madctl,p.panel.colmod,p.panel.on,p.panel.sleeping);
        drop(p);
        let rgb=[(u32::from(ioe.pwm[3])*255/4095) as u8,(u32::from(ioe.pwm[1])*255/4095) as u8,(u32::from(ioe.pwm[0])*255/4095) as u8];
        let rgb=[if main{rgb[0]}else{0},if main{rgb[1]}else{0},if main{rgb[2]}else{0}];
        Some(format!("{{\"schemaVersion\":1,\"board\":\"chain-captain\",\"state\":{{\"controllerPowered\":{},\"railL1\":{},\"tftPower\":{},\"tftReset\":{},\"backlightDuty4095\":{},\"audioSwitch\":{},\"irSwitch\":{},\"ext5vSwitch\":{},\"pwmFrequencyHz\":{},\"rgb\":{:?},\"rgbDrive\":{},\"backlightDrive\":{},\"panel\":{{\"active\":{},\"frames\":{},\"pixels\":{},\"madctl\":{:#x},\"colmod\":{:#x},\"on\":{},\"sleeping\":{}}}}}}}",
            main,l1,ioe.driven_high&IO_TFT_PWR!=0,ioe.driven_high&IO_TFT_RST!=0,ioe.pwm[2],audio,ioe.driven_high&IO_IR!=0,ioe.driven_high&IO_EXT5V!=0,ioe.pwm_frequency_hz,rgb,ioe.driven_high&(IO_RGB_G|IO_RGB_B|IO_RGB_R),ioe.driven_high&IO_BACKLIGHT!=0,tft,frames,pixels,madctl,colmod,on,sleeping))
    }
    fn report(&self)->String {
        let p=self.wiring.panel.lock().expect("Chain Captain panel poisoned");
        format!("[chain-captain] JD9853A 240x240 (240x320 GRAM) frames={} pixels={} madctl={:#x}; keys=G1/G4/G5 power=PM1; RGB=M5IOE1 PWM8G/9B/10R; ES8311 control RAM only; RTC/IR/Chain UART/audio path unmodeled",p.panel.frames,p.panel.pixels_written,p.panel.madctl)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    type Devices=Vec<(u8,u8,Box<dyn I2cDevice>)>;
    fn device(ds:&mut Devices,address:u8)->&mut dyn I2cDevice {ds.iter_mut().find(|(_,a,_)|*a==address).unwrap().2.as_mut()}
    fn write(ds:&mut Devices,address:u8,bytes:&[u8]) {let d=device(ds,address);assert!(d.start(false),"address {address:#04x} must ACK on a powered rail");for &b in bytes {assert!(d.write(b));}d.stop();}
    fn read(ds:&mut Devices,address:u8,reg:u8,len:usize)->Vec<u8> {let d=device(ds,address);assert!(d.start(false));assert!(d.write(reg));assert!(d.start(true));let out=(0..len).map(|_|d.read()).collect();d.stop();out}
    fn addresses(ds:&Devices)->Vec<u8> {let mut v:Vec<u8>=ds.iter().map(|(_,a,_)|*a).collect();v.sort_unstable();v}
    fn attached(ds:&Devices,address:u8)->bool {ds.iter().any(|(_,a,_)|*a==address)}
    /// OUT_L/OUT_H with direction and output-enable, as hal_power.cpp brings the expander up.
    fn enable(ds:&mut Devices,mask:u16) {for (reg,value) in [(3,mask as u8),(4,(mask>>8) as u8),(0x13,0),(0x14,0),(5,mask as u8),(6,(mask>>8) as u8)] {write(ds,0x4f,&[reg,value]);}}
    fn power(ds:&mut Devices,pwr_cfg:u8) {write(ds,0x6e,&[6,pwr_cfg]);}
    /// PWMn_DUTY_L/H (0x1B..0x22): enable bit7, inverted-output bit6, 12-bit duty.
    fn pwm(ds:&mut Devices,ch:usize,duty:u16,inverted:bool) {let lo=0x1b+(ch as u8)*2;write(ds,0x4f,&[lo,(duty&0xff) as u8,0x80|(u8::from(inverted)<<6)|((duty>>8) as u8&0xf)]);}
    fn spi()->SpiPins {SpiPins{sclk:1<<TFT_SCK,mosi:1<<TFT_MOSI,miso:None,cs:1<<TFT_CS,keep_cs:false}}
    /// D/C46 low for the command byte then high for parameters, exactly as GPSPI2 drives it.
    fn cmd(b:&mut ChainCaptain,c:u8,params:&[u8]) {b.gpio_changes(&[(TFT_DC,false)]);b.spi_transfer_pins(2,spi(),&[c],0);if !params.is_empty() {b.gpio_changes(&[(TFT_DC,true)]);b.spi_transfer_pins(2,spi(),params,0);}}
    /// LV_COLOR_FORMAT_RGB565_SWAPPED: the high byte leaves first, so pixels are big-endian.
    fn pixels(b:&mut ChainCaptain,data:&[u16]) {let mut bytes=Vec::with_capacity(data.len()*2);for &v in data {bytes.extend(v.to_be_bytes());}b.gpio_changes(&[(TFT_DC,true)]);b.spi_transfer_pins(2,spi(),&bytes,0);}
    fn glass(b:&ChainCaptain,col:usize,row:usize)->u16 {b.display().unwrap().2[row*COLS+col]}
    fn lit(b:&ChainCaptain)->usize {b.display().unwrap().2.iter().filter(|v|**v!=0).count()}
    fn ms()->u64 {HZ/1000}

    #[test]
    fn catalogued_pins_attach_the_modelled_subset_and_leave_the_rest_unattached() {
        let mut b=ChainCaptain::new();assert_eq!(b.name(),"chain-captain");
        let mut ds=b.i2c_devices();
        assert!(ds.iter().all(|(bus,_,d)|*bus==0&&d.pins()==Some((I2C_SDA,I2C_SCL))),"I2C0 is SDA=G3, SCL=G2");
        assert_eq!(addresses(&ds),[0x18,0x4f,0x68,0x6e],"ES8311 control registers, M5IOE1, M5PM1 and BMI270 only");
        for address in [0x32u8,0x51,0x6d,0x72] {assert!(!attached(&ds,address),"unmodelled {address:#04x} must not answer");}
        assert_eq!(b.input_levels(),vec![(1u8,true),(4,true),(5,true)],"the three user keys idle high on their pull-ups");
        assert_eq!((b.named_pin("keyA"),b.named_pin("prev"),b.named_pin("next")),(Some(1u8),Some(4),Some(5)));
        assert_eq!((b.named_pin("chain-tx"),b.named_pin("chain-rx"),b.named_pin("grove-tx"),b.named_pin("grove-rx")),(Some(18u8),Some(17),Some(6),Some(7)));
        assert_eq!(b.named_pin("power"),None,"the power key is a PM1 input, not a GPIO");
        assert!(!device(&mut ds,0x18).start(false),"IOE1 IO5 is low at power-on, so the audio branch is dead");
        assert!(device(&mut ds,0x6e).start(false)&&device(&mut ds,0x4f).start(false)&&device(&mut ds,0x68).start(false));
        assert_eq!(lit(&b),0,"no panel supply or reset yet");
    }

    #[test]
    fn hardware_i2c_reaches_the_subset_only_on_the_captured_pins() {
        use esp_periph::i2c::I2c;
        fn probe(master:&mut I2c,address:u8)->bool {
            master.write(0x24,u32::MAX);master.write(0x18,3<<12);master.write(0x1c,u32::from(address)<<1);
            master.write(0x58,6<<11);master.write(0x5c,(1<<11)|(1<<8)|1);master.write(0x60,2<<11);
            master.write(4,1<<5);esp_periph::device::Device::tick(master,u64::MAX);master.int_raw&(1<<10)==0
        }
        let mut b=ChainCaptain::new();let mut ds=b.i2c_devices();
        enable(&mut ds,TFT_ON|IO_AUDIO);power(&mut ds,0x17);
        let mut master=I2c::new();for (_,address,d) in b.i2c_devices(){master.attach(address,d);}
        master.set_pins(Some((I2C_SDA,I2C_SCL)));
        for address in [0x18u8,0x4f,0x68,0x6e] {assert!(probe(&mut master,address),"{address:#04x} answers on G3/G2");}
        for address in [0x32u8,0x51,0x6d] {assert!(!probe(&mut master,address),"RX8130CE and any other address stay unattached");}
        master.set_pins(Some((I2C_SCL,I2C_SDA)));
        for address in [0x18u8,0x4f,0x68,0x6e] {assert!(!probe(&mut master,address),"a swapped SDA/SCL route is not this board's bus");}
    }

    #[test]
    fn gpspi_pads_and_the_expander_gate_every_panel_byte() {
        let mut b=ChainCaptain::new();let mut ds=b.i2c_devices();
        assert_eq!(b.spi_transfer_pins(2,spi(),&[0x11],2),[0xff,0xff],"no MISO is wired: reads see the idle line");
        cmd(&mut b,0x2c,&[0xff,0xff]);
        assert_eq!(b.gram().unwrap().0.iter().filter(|v|**v!=0).count(),0,"reset (IO1) and supply (IO12) are IOE1 outputs, not ESP GPIOs");
        enable(&mut ds,IO_TFT_PWR);cmd(&mut b,0x2c,&[0xff,0xff]);
        assert_eq!(lit(&b),0,"the panel stays in reset until IO1 goes high too");
        enable(&mut ds,IO_TFT_RST);cmd(&mut b,0x2c,&[0xff,0xff]);
        assert_eq!(lit(&b),0,"and the reverse half of the pair is just as dead");
        enable(&mut ds,TFT_ON);
        for bad in [SpiPins{sclk:1<<(TFT_SCK+1),..spi()},SpiPins{mosi:1<<(TFT_MOSI+1),..spi()},SpiPins{cs:1<<(TFT_CS+1),..spi()}] {
            assert_eq!(b.spi_transfer_pins(2,bad,&[0x2c,0xff,0xff],1),[0xff],"G15/G16/G45 are the fixed panel pads");
        }
        assert_eq!(b.spi_transfer_pins(3,spi(),&[0x2c,0xff,0xff],2),[0xff,0xff],"SPI3 is not the display bus");
        assert_eq!(b.gram().unwrap().0.iter().filter(|v|**v!=0).count(),0);
        cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x2c,&[0xff,0xff]);
        assert_eq!(lit(&b),1);assert_eq!(b.display_frames(),1);assert_eq!(b.display().unwrap().3,b.display_version());
    }

    #[test]
    fn verified_jd9853a_window_maps_onto_the_240x240_glass_crop() {
        let mut b=ChainCaptain::new();let mut ds=b.i2c_devices();enable(&mut ds,TFT_ON|IO_BACKLIGHT);
        cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);
        assert_eq!((b.display().unwrap().0,b.display().unwrap().1),(240,240));
        assert_eq!((b.gram().unwrap().1,b.gram().unwrap().2),(COLS,GRAM_ROWS),"240x320 of controller RAM behind 240x240 of glass");
        cmd(&mut b,0x36,&[0xd4]);cmd(&mut b,0x3a,&[0x05]);
        cmd(&mut b,0x2a,&[0,0,0,1]);cmd(&mut b,0x2b,&[0,0x50,0,0x51]);cmd(&mut b,0x2c,&[]);
        pixels(&mut b,&[0xf800,0x07e0,0x001f,0xffff]);
        assert_eq!(glass(&b,0,0),0xf800);assert_eq!(glass(&b,1,0),0x07e0);
        assert_eq!(glass(&b,0,1),0x001f);assert_eq!(glass(&b,1,1),0xffff);
        let first=b.display_version();
        cmd(&mut b,0x2a,&[0,238,0,239]);cmd(&mut b,0x2b,&[0x01,0x3e,0x01,0x3f]);cmd(&mut b,0x2c,&[]);
        pixels(&mut b,&[0xf800,0x07e0,0x001f,0xffff]);
        assert_eq!(glass(&b,238,238),0xf800);assert_eq!(glass(&b,239,238),0x07e0);
        assert_eq!(glass(&b,238,239),0x001f);assert_eq!(glass(&b,239,239),0xffff);
        // MADCTL 0xD4 mirrors into the far end of the window, so the factory's rows
        // 80..319 are the visible 240; the other 80 GRAM rows are dropped.
        cmd(&mut b,0x2a,&[0,0,0,0]);cmd(&mut b,0x2b,&[0,0,0,0]);cmd(&mut b,0x2c,&[0x12,0x34]);
        assert_eq!(b.gram().unwrap().0[319*COLS+239],0x1234);
        assert_eq!(b.display().unwrap().2.iter().filter(|v|**v==0x1234).count(),0,"the unused GRAM band never reaches the glass");
        assert!(b.display_version()>first);
        cmd(&mut b,0x28,&[]);assert_eq!(lit(&b),0,"DISPOFF blanks the panel and keeps the RAM");
        assert_eq!(b.gram().unwrap().0[239*COLS+239],0xf800);
        cmd(&mut b,0x10,&[]);cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);assert_eq!(lit(&b),8,"both windows are back on the glass");
        cmd(&mut b,0x01,&[]);assert_eq!(lit(&b),0,"SLPOUT/DisplayON are back to power-on after a soft reset");
        assert_eq!(b.gram().unwrap().0[239*COLS+239],0xf800,"the controller RAM survives its own reset");
    }

    #[test]
    fn expander_and_pmic_rails_blank_the_panel_without_erasing_its_ram() {
        let mut b=ChainCaptain::new();let mut ds=b.i2c_devices();enable(&mut ds,TFT_ON);
        cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x36,&[0xd4]);
        cmd(&mut b,0x2a,&[0,0,0,0]);cmd(&mut b,0x2b,&[0,80,0,80]);cmd(&mut b,0x2c,&[0xf8,0x00]);
        assert_eq!(glass(&b,0,0),0xf800);let version=b.display_version();
        write(&mut ds,0x4f,&[5,0]);b.advance_to(ms());
        assert_eq!(lit(&b),0,"pulling OUT_EN drops IO12/IO1 and the glass goes dark");
        assert_eq!(b.gram().unwrap().0[239*COLS+239],0xf800,"the controller keeps its RAM while unpowered");
        assert!(b.display_version()>version,"re-arming the panel resets its DCS registers");
        enable(&mut ds,TFT_ON);assert_eq!(lit(&b),0,"the panel is back in power-on state: a real re-init is required");
        cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x36,&[0xd4]);
        cmd(&mut b,0x2a,&[0,0,0,0]);cmd(&mut b,0x2b,&[0,80,0,80]);cmd(&mut b,0x2c,&[0xf8,0x00]);
        assert_eq!(glass(&b,0,0),0xf800);
        power(&mut ds,0x11);b.advance_to(2*ms());
        assert!(!b.power_state().unwrap().controller_powered,"DCDC3V3_EN is 3V3_L2/L3A");
        for address in [0x4fu8,0x68,0x18] {assert!(!device(&mut ds,address).start(false),"L1 went with the LDO, so IOE1, BMI270 and ES8311 are unpowered");}
        assert!(device(&mut ds,0x6e).start(false),"M5PM1 itself stays on 3V3_L0");
        assert_eq!(lit(&b),0);
        power(&mut ds,0x17);b.advance_to(3*ms());
        assert_eq!(read(&mut ds,0x4f,0x13,1),[0xff],"the hot-removed expander cold-starts with every pin released");
        assert_eq!(lit(&b),0,"and every drive is off, so the glass is still dark");
        enable(&mut ds,TFT_ON);
        cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x36,&[0xd4]);
        cmd(&mut b,0x2a,&[0,0,0,0]);cmd(&mut b,0x2b,&[0,80,0,80]);cmd(&mut b,0x2c,&[0x07,0xe0]);
        assert_eq!(glass(&b,0,0),0x07e0);
        let mut again=b.i2c_devices();assert_eq!(addresses(&again),[0x18,0x4f,0x68,0x6e],"re-attaching must not duplicate a slot");
        enable(&mut again,TFT_ON|IO_BACKLIGHT);assert_eq!(glass(&b,0,0),0x07e0,"the shared rails keep their state across a re-attach");
    }

    #[test]
    fn audio_sub_switch_gates_the_es8311_control_registers() {
        let mut b=ChainCaptain::new();let mut ds=b.i2c_devices();
        assert!(!device(&mut ds,0x18).start(false),"IOE1 IO5 (L3B) is off, so the codec cannot be addressed");
        enable(&mut ds,TFT_ON|IO_AUDIO);
        assert!(device(&mut ds,0x18).start(false));
        assert_eq!(read(&mut ds,0x18,0xfd,1),[0x83],"reg253 = 0x83, the ES8311 rev10.0 chip id the factory image reads");
        assert_eq!(read(&mut ds,0x18,0xfe,1),[0x11],"reg254 = 0x11");
        power(&mut ds,0x15);b.advance_to(ms());
        assert!(!device(&mut ds,0x18).start(false),"the codec branch is upstream of 3V3_L2 as well as of IO5");
        power(&mut ds,0x17);enable(&mut ds,IO_AUDIO);assert!(device(&mut ds,0x18).start(false));
    }

    #[test]
    fn rgb_leds_follow_the_real_ioe1_pwm_channels() {
        let mut b=ChainCaptain::new();let mut ds=b.i2c_devices();
        write(&mut ds,0x4f,&[0x25,0x20]);write(&mut ds,0x4f,&[0x26,0x4e]);assert!(b.hardware_diagnostics().unwrap().contains("\"pwmFrequencyHz\":20000"),"hal_power.cpp setPwmFrequency(20000)");
        enable(&mut ds,IO_RGB_R);pwm(&mut ds,3,4095,false);b.advance_to(ms());
        assert_eq!(b.leds().unwrap().0[0],[255,0,0],"PWM4/IO10 is red (hal_rgb.cpp)");
        enable(&mut ds,IO_RGB_R|IO_RGB_G|IO_RGB_B);pwm(&mut ds,1,2048,false);b.advance_to(2*ms());
        assert_eq!(b.leds().unwrap().0[0],[255,127,0],"PWM2/IO8 is green at half duty");
        pwm(&mut ds,0,4095,false);b.advance_to(3*ms());assert_eq!(b.leds().unwrap().0[0],[255,127,255],"PWM1/IO9 is blue");
        enable(&mut ds,IO_RGB_G|IO_RGB_B);b.advance_to(4*ms());
        assert_eq!(b.leds().unwrap().0[0],[0,127,255],"clearing IO10 drive kills the red channel alone");
        pwm(&mut ds,1,4095,true);b.advance_to(5*ms());
        assert_eq!(b.leds().unwrap().0[0],[0,0,255],"inverted polarity bit6 complements the duty");
        let version=b.leds().unwrap().1;
        write(&mut ds,0x4f,&[0x25,0]);write(&mut ds,0x4f,&[0x26,0]);b.advance_to(6*ms());
        assert_eq!(b.leds().unwrap().0[0],[0,0,0],"a zero PWM frequency drives nothing");
        assert_eq!(b.leds().unwrap().1,version.wrapping_add(1));
        write(&mut ds,0x4f,&[0x25,0x20]);write(&mut ds,0x4f,&[0x26,0x4e]);
        enable(&mut ds,IO_RGB_R|IO_RGB_G|IO_RGB_B);pwm(&mut ds,3,4095,false);b.advance_to(7*ms());
        assert_eq!(b.leds().unwrap().0[0],[255,0,255],"re-arming the drives restores red; the armed blue duty was never erased");
        power(&mut ds,0x15);b.advance_to(8*ms());
        assert_eq!(b.leds().unwrap().0[0],[0,0,0],"the RGB LED hangs off 3V3_L2, so DCDC off kills it even with the drives high");
        power(&mut ds,0x17);
        let mut again=b.i2c_devices();
        enable(&mut again,IO_RGB_R|IO_RGB_G|IO_RGB_B);
        write(&mut again,0x4f,&[0x25,0x20]);write(&mut again,0x4f,&[0x26,0x4e]);
        pwm(&mut again,3,4095,false);b.advance_to(9*ms());
        assert_eq!(b.leds().unwrap().0[0],[255,0,0],"the re-armed expander needs its PWM clock back before any duty counts");
    }

    #[test]
    fn power_key_single_double_and_long_press_follow_pm1_timing() {
        let mut b=ChainCaptain::new();let mut ds=b.i2c_devices();enable(&mut ds,TFT_ON);
        assert!(!b.button_at(0,"button-c",true),"G1/G4/G5 are chip GPIOs, not board buttons");
        assert!(b.button_at(10*ms(),"power",true));assert!(b.button_at(30*ms(),"power",false));
        let deadline=b.next_deadline().unwrap();assert_eq!(deadline,280*ms(),"single-click qualification is 125ms<<1 after the release");
        b.advance_to(deadline);
        assert_eq!(b.take_power_event(),Some(BoardPowerEvent::Reset{download:false}),"a single click restarts the core");
        assert_eq!(b.take_power_event(),None);
        assert_eq!(read(&mut ds,0x4f,0x13,1),[0xff],"the core restart re-arms the expander");
        assert!(b.button_at(1000*ms(),"power",true));assert!(b.button_at(1020*ms(),"power",false));
        assert!(b.button_at(1040*ms(),"power",true));assert!(b.button_at(1060*ms(),"power",false));
        assert!(!b.power_state().unwrap().controller_powered,"a double click requests power off");
        assert_eq!(b.take_power_event(),None,"power off is not a reset");
        assert!(b.button_at(1200*ms(),"power",true));
        assert!(b.power_state().unwrap().controller_powered);
        assert_eq!(b.take_power_event(),Some(BoardPowerEvent::Reset{download:false}),"pressing again wakes the PMIC");
        assert!(b.button_at(1220*ms(),"power",false));b.advance_to(1600*ms());assert_eq!(b.take_power_event(),None,"the release after a wake must not queue a second reset");
        assert!(b.button_at(2000*ms(),"power",true));
        let long_press=b.next_deadline().unwrap();assert_eq!(long_press,4000*ms(),"PWR_BTN_CFG long press is 1000ms*(2+1) by default");
        b.advance_to(long_press);assert_eq!(b.take_power_event(),Some(BoardPowerEvent::Reset{download:true}),"a long press enters download mode");
        assert!(b.button_at(4100*ms(),"power",false));
        let mut ds=b.i2c_devices();write(&mut ds,0x6e,&[0x0c,0xa2]);
        assert_eq!(b.take_power_event(),Some(BoardPowerEvent::Reset{download:false}),"the register-level reset command is the second reboot path");
    }

    #[test]
    fn imu_subset_reaches_data_registers_and_resets_when_l1_drops() {
        let mut b=ChainCaptain::new();let mut ds=b.i2c_devices();
        assert_eq!(read(&mut ds,0x68,0,1),[0x24],"hal_imu.cpp expects BMI270_CHIP_ID 0x24");
        write(&mut ds,0x68,&[0x59,0]);let mut config=vec![0u8;8193];config[0]=0x5e;write(&mut ds,0x68,&config);
        write(&mut ds,0x68,&[0x59,1]);assert_eq!(read(&mut ds,0x68,0x21,1),[1],"the real config-upload handshake");
        write(&mut ds,0x68,&[0x7d,6]);
        // hal_imu.cpp asks for AccelScale 8192 LSB/g and GyroScale 32.768 LSB/dps,
        // i.e. ACC_RANGE=1 (4 g) and GYR_RANGE=1 (1000 dps).
        write(&mut ds,0x68,&[0x41,1]);write(&mut ds,0x68,&[0x43,1]);
        assert!(b.set_imu_sample([1.,0.,-1.],[0.,125.,0.]));
        b.advance_to(HZ/100);assert_eq!(read(&mut ds,0x68,0x0c,6),[0,32,0,0,0,224],"+1 g = 0x2000, -1 g = 0xe000 at 8192 LSB/g");
        assert_eq!(read(&mut ds,0x68,0x12,6),[0,0,0,16,0,0],"GYR_X..Z live at 0x12..0x17: 125 dps = 4096");
        assert!(!b.set_imu_sample([f64::NAN,0.,0.],[0.,0.,0.]),"a non-finite injection is rejected");
        power(&mut ds,0x11);assert!(!device(&mut ds,0x68).start(true),"the sensor loses its 3V3_L1 rail");
        assert_eq!(read(&mut ds,0x6e,0x22,2),3900u16.to_le_bytes(),"the PMIC itself keeps answering on 3V3_L0");
        power(&mut ds,0x17);assert_eq!(read(&mut ds,0x68,0x21,1),[0],"an unpowered sensor restarts its configuration protocol");
        write(&mut ds,0x68,&[0x59,0]);let mut config=vec![0u8;8193];config[0]=0x5e;write(&mut ds,0x68,&config);
        write(&mut ds,0x68,&[0x59,1]);write(&mut ds,0x68,&[0x7d,6]);
        assert!(b.set_imu_sample([0.,0.,1.],[0.,0.,0.]));b.advance_to(HZ/10);assert_eq!(read(&mut ds,0x68,0x10,2),[0,16]);
    }

    #[test]
    fn replay_is_deterministic_across_two_boards() {
        #[derive(Debug,PartialEq)]
        struct Snapshot {glass:Vec<u16>,gram:Vec<u16>,rgb:[u8;3],version:u64,edges:u64,frames:u64,stamp:u64,diagnostics:String,imu:Vec<u8>,power:Option<BoardPowerEvent>}
        fn script(b:&mut ChainCaptain)->Snapshot {
            let mut ds=b.i2c_devices();
            enable(&mut ds,TFT_ON|IO_BACKLIGHT|IO_RGB_R|IO_RGB_G|IO_RGB_B|IO_AUDIO);
            write(&mut ds,0x4f,&[0x25,0x20]);write(&mut ds,0x4f,&[0x26,0x4e]);
            pwm(&mut ds,3,4095,false);pwm(&mut ds,1,2048,false);
            cmd(b,0x11,&[]);cmd(b,0x29,&[]);cmd(b,0x36,&[0xd4]);cmd(b,0x3a,&[0x05]);
            cmd(b,0x2a,&[0,0,0,239]);cmd(b,0x2b,&[0,80,0x01,0x3f]);cmd(b,0x2c,&[]);
            for value in 0..8u16 {pixels(b,&[value,value+0x100,value+0x200,value+0x300]);}
            write(&mut ds,0x68,&[0x59,0]);let mut config=vec![0u8;8193];config[0]=0x5e;write(&mut ds,0x68,&config);
            write(&mut ds,0x68,&[0x59,1]);write(&mut ds,0x68,&[0x7d,6]);
            assert!(b.set_imu_sample([0.,1.,-1.],[90.,0.,-45.]));
            let before=b.gpio_events();
            b.gpio_changes(&[(1,false),(4,false),(5,false)]);b.gpio_changes(&[(1,true),(4,true),(5,true)]);
            b.button_at(10*ms(),"power",true);b.button_at(30*ms(),"power",false);
            for step in 1..=6u64 {b.advance_to(step*40*ms());}
            let imu=read(&mut ds,0x68,0x0c,6);
            let (w,h,frame,version)=b.display().unwrap();assert_eq!((w,h,frame.len()),(240,240,COLS*ROWS));
            let gram=b.gram().unwrap();
            let leds=b.leds().unwrap().0[0];
            let power=b.take_power_event();
            Snapshot {glass:frame,gram:gram.0,rgb:leds,version,edges:b.gpio_events()-before,frames:b.display_frames(),
                stamp:b.display_version().wrapping_add(gram.1 as u64+gram.2 as u64),diagnostics:b.hardware_diagnostics().unwrap(),imu,power}
        }
        let mut left=ChainCaptain::new();let mut right=ChainCaptain::new();
        assert_eq!(script(&mut left),script(&mut right),"the same input script must reproduce every observable");
        let mut b=ChainCaptain::new();let snapshot=script(&mut b);
        assert_eq!(snapshot.rgb,[255,127,0]);assert_eq!(snapshot.edges,6,"the three keys pressed and released; the power key is a PM1 event, not a GPIO edge");
        assert!(snapshot.gram.iter().any(|v|*v!=0));assert_eq!(snapshot.imu.len(),6);
        assert_eq!(snapshot.power,None,"the single click has not reached its qualification deadline yet");
        b.advance_to(600*ms());assert_eq!(b.take_power_event(),Some(BoardPowerEvent::Reset{download:false}));
        let after=script(&mut b);assert!(after.version>snapshot.version,"a second script run on the same board continues, it does not restart");
        assert_eq!(after.glass,snapshot.glass,"the panel keeps its state, so the rewritten window reads back the same");
    }
}

//! General Driver for Robots SKU23730, official2023 schematic and UGV01_BASE.
//! QMI8658C SA0 is grounded: address6B per QST table, despite demo _L=6A name.
//! I2C32/33, SPI SD14CLK/13MOSI/12MISO/15CS; motor inputs25/17/21/22/23/26.
//! There is no onboard OLED. No attached motor, encoder, bus servo or lidar is
//! invented. AK09918C, BMP280, INA21942, level shifters and analog power unmodeled.
use std::{cell::RefCell,rc::Rc,sync::{Arc,atomic::Ordering::Relaxed}};
use esp_periph::{i2c::I2cDevice,sdmmc::{SdStore,SdCardConfig,card::spi::SpiSdCard}};
use esp_soc::{BoardModel,board::{BoardEdge,SpiPins,VirtualCycle},pins::PinRoutes};
use esp_soc::devices::{i2c_bitbang::I2cBitBang,imu_motion::ImuMotion,qmi8658::{Qmi8658,Qmi8658Port}};
pub const NAME:&str="waveshare-general-driver-robots";
const MOTOR_PINS:[u8;6]=[25,17,21,22,23,26];
pub struct RobotDriver {
    motion:Arc<ImuMotion>,imu:Rc<RefCell<Qmi8658>>,i2c:I2cBitBang,levels:[bool;2],edges:Vec<BoardEdge>,
    motor_levels:[Option<bool>;6],pwm:[Option<u32>;2],card:Option<SpiSdCard>,horizon:u64,elapsed:u64,
}
impl Default for RobotDriver {fn default()->Self {Self::new()}}
impl RobotDriver {pub fn new()->Self {
    let motion=Arc::new(ImuMotion::default());let imu=Rc::new(RefCell::new(Qmi8658::new_c(motion.clone(),crate::periph::CPU_HZ)));
    Self{motion,imu,i2c:I2cBitBang::new(),levels:[true;2],edges:Vec::new(),motor_levels:[None;6],pwm:[None;2],card:None,horizon:0,elapsed:0}
}}
impl BoardModel for RobotDriver {
    fn name(&self)->&'static str {NAME}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,cycle:VirtualCycle,routes:PinRoutes<'_>) {
        self.motor_levels=MOTOR_PINS.map(|pin|routes.software_output(pin));
        let scl=routes.software_output(33)!=Some(false);let sda=self.i2c.drive(scl,routes.software_output(32)!=Some(false));
        for(i,(pin,level))in[(32,sda),(33,scl)].into_iter().enumerate() {if self.levels[i]!=level {self.levels[i]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
        if routes.software_output(15)==Some(true) {if let Some(card)=&mut self.card {card.deselect();}}
    }
    fn uses_gpio_pwm(&self)->bool {true}
    fn gpio_pwm_at(&mut self,_cycle:u64,pwm:&dyn Fn(u8)->Option<(f64,u32)>) {self.pwm=[25,26].map(|pin|pwm(pin).map(|(_,d)|d));}
    fn advance_to(&mut self,cycle:VirtualCycle) {
        self.elapsed=self.elapsed.saturating_add(cycle.saturating_sub(self.horizon));self.horizon=cycle;self.motion.cycle.store(self.elapsed,Relaxed);
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();self.i2c.attach(0x6b,Box::new(Qmi8658Port::new(self.imu.clone())));
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some((32,33))).into_iter().map(move|(address,device)|(bus,address,device))).collect()
    }
    fn set_imu_sample(&mut self,a:[f64;3],g:[f64;3])->bool {self.imu.borrow_mut().set_host_sample(a,g)}
    fn set_imu_motion(&mut self,mode:u32)->bool {self.imu.borrow_mut().set_motion_mode(mode)}
    fn imu_motion(&self)->u32 {self.motion.mode.load(Relaxed)}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true),(32,self.levels[0]),(33,self.levels[1])]}
    fn named_pin(&self,id:&str)->Option<u8> {(id=="boot").then_some(0)}
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        if pins.sclk&(1<<14)==0||(!tx.is_empty()&&pins.mosi&(1<<13)==0) {return vec![0xff;rx_len];}
        if let Some(card)=&mut self.card {let rx=card.transfer(pins.cs&(1<<15)!=0,tx,rx_len);if pins.miso==Some(12) {return rx;}}
        vec![0xff;rx_len]
    }
    fn attach_spi_sd_card(&mut self,store:Box<dyn SdStore>,config:SdCardConfig)->Result<String,String> {let card=SpiSdCard::new(store,config)?;let description=card.describe();self.card=Some(card);Ok(description)}
    fn hardware_diagnostics(&self)->Option<String> {
        let level=|x:Option<bool>|x.map_or("null",|v|if v{"true"}else{"false"});
        let duty=|x:Option<u32>|x.map_or_else(||"null".into(),|v|v.to_string());
        Some(format!("{{\"motorControlPins\":[25,17,21,22,23,26],\"gpioLevels\":[{}],\"pwmDuty65535\":[{},{}],\"mechanicalMotorDynamics\":false,\"encodersAttached\":false}}",self.motor_levels.map(level).join(","),duty(self.pwm[0]),duty(self.pwm[1])))
    }
    fn report(&self)->String {format!("[{NAME}] QMI8658C@6B SDA32/SCL33 with guest-time sampling; SPI SD14/13/12/15 attached={}; motor GPIO/PWM observation only; no onboard OLED; magnetometer/barometer/current/servo/lidar/encoder/analog/factory-boot unmodeled",self.card.is_some())}
}

//! Waveshare Servo Driver with ESP32, SKU21593. Official schematic and
//! SERVO_DRIVER_WITH_ESP32/ServoDriver/{BOARD_DEV.h,RGB_CTRL.h,ServoDriver.ino}.
//! SSD1306 128x32 on SDA21/SCL22, two onboard WS2812B on GPIO23, BOOT0.
//! The demo configures ten chained pixels; only the two schematic devices are
//! fitted here. External servo bus/UART1 RX18/TX19, radio and power are unmodeled.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::I2cDevice;
use esp_soc::{BoardModel,board::{BoardEdge,VirtualCycle},pins::PinRoutes};
use esp_soc::devices::{Ws2812Chain,i2c_bitbang::I2cBitBang,oled_mono::{OledMono,OledKind,OledI2c}};

pub const NAME:&str="waveshare-servo-driver-esp32";
pub struct ServoDriver {
    oled:Arc<Mutex<OledMono>>,i2c:I2cBitBang,levels:[bool;2],edges:Vec<BoardEdge>,rgb:Ws2812Chain,
}
impl Default for ServoDriver {fn default()->Self {Self::new()}}
impl ServoDriver {pub fn new()->Self {Self{oled:Arc::new(Mutex::new(OledMono::new(OledKind::Ssd1306,128,32,0))),i2c:I2cBitBang::new(),levels:[true;2],edges:Vec::new(),rgb:Ws2812Chain::new(2)}}}
impl BoardModel for ServoDriver {
    fn name(&self)->&'static str {NAME}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,cycle:VirtualCycle,routes:PinRoutes<'_>) {
        let scl=routes.software_output(22)!=Some(false);let sda=self.i2c.drive(scl,routes.software_output(21)!=Some(false));
        for (i,(pin,level))in[(21,sda),(22,scl)].into_iter().enumerate() {
            if self.levels[i]!=level {self.levels[i]=level;self.edges.push(BoardEdge{cycle,pin,level});}
        }
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();self.i2c.attach(0x3c,Box::new(OledI2c::new(self.oled.clone(),(21,22))));
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some((21,22))).into_iter().map(move|(address,device)|(bus,address,device))).collect()
    }
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true),(21,self.levels[0]),(22,self.levels[1])]}
    fn named_pin(&self,name:&str)->Option<u8> {(name=="boot").then_some(0)}
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {if pin==23 {self.rgb.from_bits(bits);}}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.rgb.leds,self.rgb.updates))}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {let d=self.oled.lock().unwrap();Some((128,32,d.frame(),d.version()))}
    fn display_version(&self)->u64 {self.oled.lock().unwrap().version()}
    fn display_frames(&self)->u64 {self.oled.lock().unwrap().frames()}
    fn report(&self)->String {format!("[{NAME}] {}; two onboard WS2812B23 updates={}; BOOT0; external servos/UART transceiver/voltage/ESP-NOW/factory boot unverified",self.oled.lock().unwrap().report(),self.rgb.updates)}
}

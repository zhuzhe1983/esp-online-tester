//! M5Stack Cardputer-Adv: ST7789V2 and TCA8418 matrix keyboard, official K132-Adv pinmap.
//! Digital ES8311 PCM/control, a host-overridable battery divider and BMI270 initialization subsets.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::I2cDevice;
use esp_soc::devices::es8311::{Es8311,Es8311State};
use esp_periph::i2c::bmi270::{Bmi270,Bmi270Sample};
use esp_periph::i2c::tca8418::{Tca8418,Tca8418State};
use esp_soc::board::{BoardModel,BoardEdge,VirtualCycle,SpiPins};
use esp_soc::devices::i2c_bitbang::I2cBitBang;
use super::cardputer_lcd::CardputerLcd;

pub struct CardputerAdv {codec:Arc<Mutex<Es8311State>>,lcd:CardputerLcd,i2c:I2cBitBang,levels:[bool;2],keyboard:Arc<Mutex<Tca8418State>>,imu:Arc<Mutex<Bmi270Sample>>,cycle:u64,irq_high:bool,edges:Vec<BoardEdge>,gpio_events:u64}
impl CardputerAdv {
    pub fn new()->Self {Self{codec:Default::default(),lcd:CardputerLcd::new(),i2c:I2cBitBang::new(),levels:[true,true],keyboard:Arc::new(Mutex::new(Tca8418State::new(crate::periph::CPU_HZ))),imu:Default::default(),cycle:0,irq_high:true,edges:Vec::new(),gpio_events:0}}
    fn sync_irq(&mut self) {let high=self.keyboard.lock().expect("Cardputer keyboard mutex poisoned").irq_high();if high!=self.irq_high {self.irq_high=high;self.edges.push(BoardEdge{cycle:self.cycle,pin:11,level:high});}}

}
impl Default for CardputerAdv {fn default()->Self {Self::new()}}
impl BoardModel for CardputerAdv {
    fn name(&self)->&'static str {"cardputer-adv"}
    fn default_analog_inputs(&self)->Vec<(u8,esp_periph::AnalogSource)> {
        // M5Unified 96576a14 Power_Class.inl: ADC1 GPIO10, battery/pad ratio2.
        // Explicit virtual 3.7V battery, not a fuel gauge or charger model. Host
        // pad volts/raw/streams (including zero) win via AnalogInputs::set_default.
        vec![(10,esp_periph::AnalogSource::Const(1.85))]
    }
    fn gpio_changes(&mut self,changes:&[(u8,bool)]) {self.gpio_events+=changes.len() as u64;self.lcd.gpio(changes); }
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {
        self.cycle=cycle;self.gpio_changes(changes);let released=|pin:u8|enabled&(1u64<<pin)==0 || output&(1u64<<pin)!=0;
        let scl=released(9);let sda=self.i2c.drive(scl,released(8));
        for (index,(pin,level)) in [(8,sda),(9,scl)].into_iter().enumerate() {if level!=self.levels[index] {self.levels[index]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
    }
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {self.lcd.spi(pins,tx,rx_len)}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();self.i2c.attach(0x34,Box::new(Tca8418::new(self.keyboard.clone())));self.i2c.attach(0x68,Box::new(Bmi270::new(self.imu.clone())));
        self.i2c.attach(0x18,Box::new(Es8311::with_state(self.codec.clone())));
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some((8,9))).into_iter().map(move |(address,device)|(bus,address,device))).collect()
    }
    fn i2s_rx_pcm(&self,port:usize,routes:&esp_soc::pins::PinRoutes<'_>,bits:u32,sample_bytes:usize,bytes:&mut [u8]) {
        // M5Stack K132-Adv Audio table: BCLK41/LRCK43/ASDOUT46. IDF v5.4.2
        // soc/esp32s3/include/soc/gpio_sig_map.h: I2S0 RX25/26/27, I2S1 RX30/31/32.
        let (data,bck,ws)=if port==0 {(25,26,27)}else{(30,31,32)};
        let wired=routes.matrix_input(data)==Some(46)&&routes.matrix_output(41,bck)&&routes.matrix_output(43,ws);
        if !wired || !(2..=4).contains(&sample_bytes) {bytes.fill(0);return;}
        let gain=self.codec.lock().expect("ES8311 state poisoned").adc_gain(bits);
        for sample in bytes.chunks_exact_mut(sample_bytes) {
            let n=sample_bytes;let value=i16::from_le_bytes([sample[n-2],sample[n-1]]);
            let scaled=(f64::from(value)*gain).round() as i16;
            sample.fill(0);sample[n-2..].copy_from_slice(&scaled.to_le_bytes());
        }
    }
    fn i2s_tx_pcm(&self,port:usize,routes:&esp_soc::pins::PinRoutes<'_>,bits:u32,samples:&mut [i16]) {
        // DSDIN42. A full-duplex master may drive the shared wires from RX clocks.
        let (data,clocks)=if port==0 {(25,[[22,24],[26,27]])}else{(30,[[28,29],[31,32]])};
        let wired=routes.matrix_output(42,data)&&clocks.iter().any(|&[b,w]|routes.matrix_output(41,b)&&routes.matrix_output(43,w));
        let gain=if wired{self.codec.lock().expect("ES8311 state poisoned").dac_gain(bits)}else{0.0};
        for sample in samples {*sample=(f64::from(*sample)*gain).round() as i16;}
    }
    fn matrix_key_at(&mut self,cycle:VirtualCycle,row:u8,column:u8,pressed:bool)->bool {
        if row>=4 || column>=14 {return false;}
        // Inverse of factory keyboard.cpp remap: logical 4x14 -> wired 7x8.
        self.keyboard.lock().expect("Cardputer keyboard mutex poisoned").key(cycle,column/2,row+4*(column&1),pressed)
    }
    fn matrix_key_debounced_at(&mut self,cycle:VirtualCycle,row:u8,column:u8,pressed:bool)->esp_soc::board::KeyEventAcceptance {
        use esp_soc::board::KeyEventAcceptance::*;
        if row>=4 || column>=14 {return Invalid;}
        // Preserve a guest ACK's deassertion before admission can assert IRQ again.
        // A short host run may return before advance_to/take_edges publishes it.
        self.sync_irq();
        let result=self.keyboard.lock().expect("Cardputer keyboard mutex poisoned").try_key_debounced(cycle,column/2,row+4*(column&1),pressed);
        if matches!(result,Accepted|Duplicate) {self.cycle=self.cycle.max(cycle);self.sync_irq();}result
    }
    fn matrix_key_diagnostics(&self)->Option<esp_soc::board::KeyEventDiagnostics> {Some(self.keyboard.lock().expect("Cardputer keyboard mutex poisoned").key_diagnostics())}
    fn advance_to(&mut self,cycle:VirtualCycle) {
        let edges=self.keyboard.lock().expect("Cardputer keyboard mutex poisoned").advance_to(cycle);
        for (at,level) in edges {self.irq_high=level;self.edges.push(BoardEdge{cycle:at,pin:11,level});}
        self.cycle=cycle;self.sync_irq();
    }
    fn next_deadline(&self)->Option<VirtualCycle> {self.keyboard.lock().expect("Cardputer keyboard mutex poisoned").next_deadline()}
    fn take_edges(&mut self)->Vec<BoardEdge> {self.sync_irq();std::mem::take(&mut self.edges)}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true),(8,self.levels[0]),(9,self.levels[1]),(11,self.irq_high)]}
    fn named_pin(&self,name:&str)->Option<u8> {match name {"boot"|"g0"=>Some(0),_=>None}}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {Some(self.lcd.display())}
    fn display_version(&self)->u64 {self.lcd.version()}
    fn display_frames(&self)->u64 {self.lcd.panel.frames}
    fn report(&self)->String {format!("[cardputer-adv] {} battery=nominal-3.7V-overridable",self.lcd.report())}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lcd_only_answers_its_physical_bus_and_default_id() {
        let mut b=CardputerAdv::new();let pins=SpiPins{sclk:1<<36,mosi:1<<35,cs:1<<37,..Default::default()};
        assert_eq!(b.spi_transfer_pins(2,pins,&[4],3),[0x85,0x85,0x52]);assert_eq!(b.spi_transfer_pins(3,SpiPins{sclk:1<<40,..pins},&[4],3),[0xff;3]);
        b.gpio_changes(&[(33,false)]);assert_eq!(b.spi_transfer_pins(3,pins,&[4],3),[0xff;3]);b.gpio_changes(&[(33,true)]);assert_eq!(b.spi_transfer_pins(3,pins,&[0xda],1),[0x85]);
    }
    #[test]
    fn physical_key_positions_remap_bijectively_and_require_scanning() {
        let mut b=CardputerAdv::new();let mut devices=b.i2c_devices();let d=devices.iter_mut().find(|(bus,a,_)|*bus==0 && *a==0x34).unwrap().2.as_mut();
        for (r,v) in [(0x1d,0x7f),(0x1e,0xff),(1,1)] {d.start(false);d.write(r);d.write(v);}
        assert!(!b.matrix_key_at(0,4,0,true));assert!(!b.matrix_key_at(0,0,14,true));assert!(b.matrix_key_at(0,2,13,true));
        b.advance_to(crate::periph::CPU_HZ/20);assert!(b.take_edges().contains(&BoardEdge{cycle:crate::periph::CPU_HZ/20,pin:11,level:false}));
        d.start(false);d.write(4);d.start(true);assert_eq!(d.read(),0xc3,"Enter is wired row6,col6, TCA key67 press");
        let mut seen=std::collections::HashSet::new();for row in 0..4 {for column in 0..14 {assert!(seen.insert((column/2,row+4*(column&1))));}}assert_eq!(seen.len(),56);
    }
}

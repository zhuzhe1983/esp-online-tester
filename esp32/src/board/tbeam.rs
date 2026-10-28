//! Original LILYGO T-Beam V1.2 SX1276 (868/915 MHz), no optional OLED fitted.
//! Wiring: LilyGo-LoRa-Series 1991b1ed docs/en/t_beam/t_beam_hw.md,
//! examples/Factory/utilities.h and schematic/LilyGo_TBeam_V1.2.pdf.
mod axp2101;
mod sx1276;
use std::sync::{Arc,Mutex};
use esp_periph::i2c::I2cDevice;
use esp_soc::board::{BoardModel,BoardEdge,SpiPins,VirtualCycle};
use axp2101::{Axp2101,AxpDevice};
use sx1276::Sx1276;
pub struct TBeam {pmu:Arc<Mutex<Axp2101>>,radio:Sx1276,cycle:u64,levels:[bool;4],edges:Vec<BoardEdge>,leds:[[u8;3];1],led_updates:u64}
impl TBeam {
    pub fn new()->Self {Self{pmu:Arc::new(Mutex::new(Axp2101::new())),radio:Sx1276::new(),cycle:0,levels:[true,false,false,false],edges:Vec::new(),leds:[[0;3]],led_updates:0}}
    fn sync(&mut self) {let pmu=self.pmu.lock().unwrap();self.radio.controls(!pmu.power_off_requested&&pmu.regs[0x90]&2!=0,self.radio.reset_high);let rgb=if pmu.regs[0x69]&0x35==0x35 {[255,0,0]}else{[0;3]};if rgb!=self.leds[0]{self.leds[0]=rgb;self.led_updates+=1;}}
}
impl Default for TBeam {fn default()->Self {Self::new()}}
impl BoardModel for TBeam {
    fn name(&self)->&'static str {"lilygo-t-beam-v1-2"}
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {self.advance_to(cycle);for &(pin,level) in changes {if pin==23 {self.radio.controls(self.radio.powered,level);}}self.radio.select(enabled&(1<<18)!=0&&output&(1<<18)==0);}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {self.sync();if pins.sclk&(1<<5)==0||pins.mosi&(1<<27)==0||pins.cs&(1<<18)==0 {return vec![0xff;rx_len];}let rx=self.radio.transfer(tx,rx_len);if pins.miso==Some(19){rx}else{vec![0xff;rx_len]}}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {(0..2).map(|bus|(bus,0x34,Box::new(AxpDevice::new(self.pmu.clone())) as Box<dyn I2cDevice>)).collect()}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(38,true),(35,self.levels[0]),(26,self.levels[1]),(33,self.levels[2]),(32,self.levels[3]),(21,true),(22,true)]}
    fn named_pin(&self,id:&str)->Option<u8> {match id {"user"|"button"=>Some(38),_=>None}}
    fn button_at(&mut self,cycle:VirtualCycle,id:&str,pressed:bool)->bool {if id!="power" {return false;}self.advance_to(cycle);self.pmu.lock().unwrap().button(cycle,pressed);true}
    fn advance_to(&mut self,cycle:VirtualCycle) {self.cycle=cycle;self.pmu.lock().unwrap().advance_to(cycle);self.sync();self.radio.advance_to(cycle);}
    fn next_deadline(&self)->Option<VirtualCycle> {self.pmu.lock().unwrap().next_deadline().into_iter().chain(self.radio.next_deadline()).min()}
    fn take_edges(&mut self)->Vec<BoardEdge> {self.sync();let irq=self.radio.irq();let levels=[!self.pmu.lock().unwrap().irq(),irq[0],irq[1],irq[2]];for (i,pin) in [35,26,33,32].into_iter().enumerate(){if levels[i]!=self.levels[i] {self.levels[i]=levels[i];self.edges.push(BoardEdge{cycle:self.cycle,pin,level:levels[i]});}}std::mem::take(&mut self.edges)}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.leds,self.led_updates))}
    fn lora_receive_at(&mut self,cycle:VirtualCycle,p:&esp_soc::devices::sx1262::LoRaInput)->bool {self.advance_to(cycle);self.radio.receive(p)}
    fn lora_snapshot(&self)->Option<String> {Some(self.radio.snapshot())}
    fn report(&self)->String {let pmu=self.pmu.lock().unwrap();format!("[t-beam-v1-2] AXP2101 irq={} shutdown_requested={} virtual USB=5000mV battery=absent; SX1276 tx={} rx={} rejected={}; OLED=absent GNSS/PSRAM/analog-power/FSK=unmodeled",pmu.irq(),pmu.power_off_requested,self.radio.tx_count,self.radio.rx_count,self.radio.rejected)}
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn pmu_is_shared_and_pin_scoped_without_optional_display() {let mut b=TBeam::new();let mut devices=b.i2c_devices();assert_eq!(devices.len(),2);for (_,a,d) in &devices {assert_eq!(*a,0x34);assert_eq!(d.pins(),Some((21,22)));}let d=&mut devices[0].2;d.start(false);d.write(0x41);d.write(8);b.button_at(0,"power",true);b.button_at(crate::periph::CPU_HZ/5,"power",false);assert!(b.take_edges().iter().any(|e|e.pin==35&&!e.level));let d=&mut devices[1].2;d.start(false);d.write(0x49);d.start(true);assert_eq!(d.read()&8,8);d.start(false);d.write(0x49);d.write(8);assert!(b.take_edges().iter().any(|e|e.pin==35&&e.level));assert!(b.display().is_none());assert_eq!(b.named_pin("user"),Some(38));assert_eq!(b.named_pin("power"),None);}
}

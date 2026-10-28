//! Atom VoiceS3R C126-ECHO: G41 and ES8311 control-bus initialization subset.
//! M5Stack Atom_VoiceS3R pin map / Sch_M5_AtomEchoS3R_Audio_v1.0_20250716:
//! SDA45/SCL0, CE tied low -> 0x18. No RGB, screen, microphone or USB audio model.
//! ES8311 rev10.0 Jan2021 p29: FD/FE identification reset values 83/11.
//! https://files.waveshare.com/wiki/common/ES8311.DS.pdf ; full codec reset/RO/audio semantics are not modeled.
use esp_periph::i2c::{I2cDevice,Reg8Device};
use esp_soc::board::{BoardModel,BoardEdge,VirtualCycle};
use esp_soc::devices::i2c_bitbang::I2cBitBang;

pub struct AtomVoiceS3R {i2c:I2cBitBang,levels:[bool;2],edges:Vec<BoardEdge>,gpio_events:u64}
impl AtomVoiceS3R {
    pub fn new()->Self {Self{i2c:I2cBitBang::new(),levels:[true,true],edges:Vec::new(),gpio_events:0}}
}
impl Default for AtomVoiceS3R {fn default()->Self {Self::new()}}
impl BoardModel for AtomVoiceS3R {
    fn name(&self)->&'static str {"atom-voices3r"}
    fn gpio_changes(&mut self,changes:&[(u8,bool)]) {self.gpio_events+=changes.len() as u64;}
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {
        self.gpio_changes(changes);let released=|pin:u8|enabled&(1u64<<pin)==0 || output&(1u64<<pin)!=0;
        let scl=released(0);let sda=self.i2c.drive(scl,released(45));
        for (index,(pin,level)) in [(45,sda),(0,scl)].into_iter().enumerate() {if level!=self.levels[index] {self.levels[index]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
    }
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();self.i2c.attach(0x18,Box::new(Reg8Device::new("es8311-initialization-subset",&[(0xfd,0x83),(0xfe,0x11)])));
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some((45,0))).into_iter().map(move |(address,device)|(bus,address,device))).collect()
    }
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(41,true),(45,self.levels[0]),(0,self.levels[1])]}
    fn named_pin(&self,name:&str)->Option<u8> {match name {"button-a"|"btnA"|"g41"=>Some(41),_=>None}}
    fn report(&self)->String {"[atom-voices3r] G41 / ES8311 control initialization only; audio, microphone, USB UAC, IR and reset supervisor not modeled".into()}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn codec_controllers_share_register_state_and_require_the_audio_pins() {
        let mut b=AtomVoiceS3R::new();let mut devices=b.i2c_devices();
        assert_eq!(devices.len(),2);
        for (bus,address,d) in &devices {assert!(*bus<2);assert_eq!(*address,0x18);assert_eq!(d.pins(),Some((45,0)));}
        let d=devices[0].2.as_mut();assert!(!d.start_address(0x18,0x19,false));
        assert!(d.start(false));assert!(d.write(0xfd));assert!(d.start(true));assert_eq!(d.read(),0x83);assert_eq!(d.read(),0x11);
        d.start(false);d.write(0x32);d.write(0x77);d.stop();
        let d=devices[1].2.as_mut();d.start(false);d.write(0x32);d.start(true);assert_eq!(d.read(),0x77);
        assert_eq!(b.named_pin("button-a"),Some(41));assert_eq!(b.named_pin("reset"),None);
        assert!(b.input_levels().contains(&(41,true)));assert!(b.leds().is_none());assert!(b.display().is_none());
    }
}

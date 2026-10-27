//! NanoC6 C125: M5Stack Sch_M5NanoC6_v0.0.1 and M5NanoC6 7cf50969.
//! G9 active-low key (R2 10k pull-up); G20 WS2812 data; G19 enables
//! AW35122 RGB power; G7 sources the blue LED through R9 to ground.
//! Digital polarity/drive only: no IR, optical current or RF simulation here.
use esp_soc::{BoardModel,devices::Ws2812Chain,pins::PinRoutes};
pub struct NanoC6 {rgb:Ws2812Chain,powered:bool,leds:[[u8;3];2],updates:u64}
impl Default for NanoC6 {fn default()->Self {Self::new()}}
impl NanoC6 {
    pub fn new()->Self {Self{rgb:Ws2812Chain::new(1),powered:false,leds:[[0;3];2],updates:0}}
    fn publish(&mut self,index:usize,color:[u8;3]) {if self.leds[index]!=color {self.leds[index]=color;self.updates=self.updates.wrapping_add(1);}}
}
impl BoardModel for NanoC6 {
    fn name(&self)->&'static str {"nanoc6"}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(9,true),(8,true)]}
    fn named_pin(&self,name:&str)->Option<u8> {match name {"button"|"boot"|"btnA"=>Some(9),_=>None}}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,_cycle:u64,routes:PinRoutes<'_>) {
        let powered=routes.software_output(19)==Some(true);
        if self.powered && !powered {self.rgb=Ws2812Chain::new(1);self.publish(0,[0;3]);}
        self.powered=powered;
        self.publish(1,if routes.software_output(7)==Some(true) {[0,0,255]} else {[0;3]});
    }
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {if pin==20 && self.powered {self.rgb.from_bits(bits);self.publish(0,self.rgb.leds[0]);}}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.leds,self.updates))}
    fn report(&self)->String {format!("[nanoc6] rgb_power={} rgb={:?} blue={:?} changes={}; IR/RF/analog unverified",self.powered,self.leds[0],self.leds[1],self.updates)}
}
#[cfg(test)]
mod tests {
    use super::*;
    use emu_core::Bus;
    use esp_soc::SocBus as _;
    // ESP-IDF v5.5.1 components/soc/esp32c6/register/soc/{io_mux,gpio}_reg.h:
    // IO_MUX function 1; GPIO selector 128; OUT and ENABLE W1TS/W1TC offsets.
    // Official factory uses IDF e0944287f3 (v5.2 development); the pinned headers
    // agree with these v5.5.1 fields. Sources/hash receipts: nanoc6/sources.json.
    fn drive(b:&mut crate::bus::SocBus,pin:u8,on:bool) {
        b.write32(0x6009_0004+4*pin as u32,1<<12).unwrap();
        b.write32(0x6009_1554+4*pin as u32,128).unwrap();
        b.write32(0x6009_1024,1<<pin).unwrap();
        b.write32(if on {0x6009_1008} else {0x6009_100c},1<<pin).unwrap();
    }
    #[test]
    fn gpio_power_route_enable_and_reset_gate_real_outputs() {
        let mut m=crate::machine([0;6],4<<20);let b=&mut m.bus;b.board=Box::new(NanoC6::new());b.attach_board_devices();
        assert_eq!(b.gpio_input()&(1<<9),1<<9);
        let bits:Vec<bool>=[0x20u8,0x40,0x80].iter().flat_map(|v|(0..8).map(move |i|v&(0x80>>i)!=0)).collect();
        b.board.rmt_frame(20,&bits);assert_eq!(b.board.leds().unwrap().0,[[0;3];2]);
        drive(b,19,true);b.board.rmt_frame(21,&bits);assert_eq!(b.board.leds().unwrap().0[0],[0;3]);
        b.board.rmt_frame(20,&bits);drive(b,7,true);assert_eq!(b.board.leds().unwrap().0,[[0x40,0x20,0x80],[0,0,255]]);
        b.write32(0x6009_1028,1<<19|1<<7).unwrap();assert_eq!(b.board.leds().unwrap().0,[[0;3];2]);
        drive(b,19,true);assert_eq!(b.board.leds().unwrap().0[0],[0;3],"power-up needs a fresh RGB frame");
        b.board.rmt_frame(20,&bits);b.write32(0x6009_0004+4*19,0).unwrap();assert_eq!(b.board.leds().unwrap().0[0],[0;3],"wrong IO_MUX disconnects power drive");
        drive(b,19,true);drive(b,7,true);b.board.rmt_frame(20,&bits);b.reboot([0;6]);assert_eq!(b.board.leds().unwrap().0,[[0;3];2]);
    }
}

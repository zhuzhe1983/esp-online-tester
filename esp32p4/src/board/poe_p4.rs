//! M5Stack U213 Unit PoE-P4. Manufacturer UserDemo 2840dceb6d3c6d9e51b82fff3df4bb1bf22d463c
//! sdkconfig selects IP101 at address 1, MDC31/MDIO52/reset51. The product pin
//! table supplies the RMII pads. The RGB LED is common-anode (led_ctrl.cpp),
//! and the active-low user key is GPIO45. Unattached FPCs do not create displays.
use esp_soc::{BoardModel,board::{BoardEdge,EmacConfig,EthernetPhy},pins::PinRoutes};
pub const EMAC:EmacConfig=EmacConfig{phy:EthernetPhy::Ip101g,phy_addr:1,mdc:31,mdio:52,rmii:[49,34,35,29,30,28],ref_clk:Some(50),reset_pin:Some(51)};
pub struct PoeP4 { key:bool,edges:Vec<BoardEdge>,raw:[Option<bool>;3],led:[[u8;3];1],version:u64 }
impl Default for PoeP4 {fn default()->Self{Self::new()}}
impl PoeP4 {
    pub fn new()->Self{Self{key:true,edges:Vec::new(),raw:[None;3],led:[[0;3]],version:0}}
    fn set_led(&mut self,colors:[u8;3]){if self.led[0]!=colors{self.led[0]=colors;self.version=self.version.wrapping_add(1);}}
}
impl BoardModel for PoeP4 {
    fn name(&self)->&'static str{"m5stack-poe-p4"}
    fn emac_config(&self)->Option<EmacConfig>{Some(EMAC)}
    fn named_pin(&self,name:&str)->Option<u8>{matches!(name,"user"|"btn"|"button").then_some(45)}
    fn input_levels(&self)->Vec<(u8,bool)>{vec![(45,self.key)]}
    fn button_at(&mut self,cycle:u64,id:&str,pressed:bool)->bool{if self.named_pin(id)!=Some(45){return false;}let high=!pressed;if self.key!=high{self.key=high;self.edges.push(BoardEdge{cycle,pin:45,level:high});}true}
    fn take_edges(&mut self)->Vec<BoardEdge>{std::mem::take(&mut self.edges)}
    fn uses_gpio_routes(&self)->bool{true}
    fn gpio_waveform_at(&mut self,_cycle:u64,routes:PinRoutes<'_>){self.raw=[17,15,16].map(|pin|routes.software_output(pin));self.set_led(self.raw.map(|v|if v==Some(false){255}else{0}));}
    fn advance_inputs_with_clock(&mut self,_cycle:u64,clock:&dyn Fn(u8)->Option<(f64,u32)>){let colors=std::array::from_fn(|i|clock([17,15,16][i]).filter(|(hz,_)|*hz>0.0).map(|(_,d)|255-((d.min(65535)*255+32767)/65535)as u8).unwrap_or(if self.raw[i]==Some(false){255}else{0}));self.set_led(colors);}
    fn leds(&self)->Option<(&[[u8;3]],u64)>{Some((&self.led,self.version))}
    fn report(&self)->String{format!("[poe-p4] IP101GRI addr1, MDC31 MDIO52 reset51 RMII49/34/35/29/30/28 REF50; user45 {}; common-anode RGB17/15/16 {:?}; analog PoE negotiation/current, external FPC/CSI/display, IR and USB application acceptance unverified",if self.key{"released"}else{"pressed"},self.led[0])}
}

//! K016-B black M5Stick: ESP32-WROOM-32, no gray-version MPU9250.
//! M5Stack 0.2.9 examples/Stick/FactoryTest uses U8X8_SH1107_64X128_4W_HW_SPI
//! with CS14/DC27/RST33 and default VSPI18/23; button35, blue LED19, buzzer26.
//! SH1107 page-addressed monochrome subset; no DCS/ILI934x interpretation.
use esp_soc::{BoardModel,board::SpiPins};
pub struct M5StickOled {ram:Vec<u8>,page:u8,column:u8,pending:Option<u8>,on:bool,inverted:bool,all_on:bool,seg_reverse:bool,com_reverse:bool,start:u8,offset:u8,dc:bool,reset:bool,version:u64,bytes:u64,led:[[u8;3];1],led_version:u64}
impl Default for M5StickOled{fn default()->Self{Self::new()}}
impl M5StickOled {
    pub fn new()->Self{Self{ram:vec![0;128*16],page:0,column:0,pending:None,on:false,inverted:false,all_on:false,seg_reverse:true,com_reverse:true,start:0,offset:96,dc:false,reset:false,version:0,bytes:0,led:[[0;3]],led_version:0}}
    fn reset(&mut self){self.page=0;self.column=0;self.pending=None;self.on=false;self.inverted=false;self.all_on=false;self.start=0;self.version=self.version.wrapping_add(1);}
    fn byte(&mut self,b:u8){if self.dc {let at=usize::from(self.page&15)*128+usize::from(self.column&127);self.ram[at]=b;self.column=self.column.wrapping_add(1)&127;self.bytes=self.bytes.wrapping_add(1);}else if let Some(c)=self.pending.take(){match c{0xdc=>self.start=b&127,0xd3=>self.offset=b&127,_=>{}}}else{match b{0x00..=0x0f=>self.column=(self.column&0x70)|(b&15),0x10..=0x17=>self.column=(self.column&15)|((b&7)<<4),0xb0..=0xbf=>self.page=b&15,0xae=>self.on=false,0xaf=>self.on=true,0xa0|0xa1=>self.seg_reverse=b&1!=0,0xc0|0xc8=>self.com_reverse=b&8!=0,0xa4|0xa5=>self.all_on=b&1!=0,0xa6|0xa7=>self.inverted=b&1!=0,0x81|0xa8|0xad|0xd3|0xd5|0xd9|0xda|0xdb|0xdc=>self.pending=Some(b),_=>{}}}self.version=self.version.wrapping_add(1);}
}
impl BoardModel for M5StickOled {
    fn name(&self)->&'static str{"m5stick-oled"}
    fn gpio_changes(&mut self,changes:&[(u8,bool)]){for &(pin,high) in changes{match pin{27=>self.dc=high,33=>{if !high&&!self.reset{self.reset();}self.reset= !high;},19=>{self.led=[if high{[0,0,255]}else{[0;3]}];self.led_version=self.led_version.wrapping_add(1);},_=>{}}}}
    fn uses_spi_pins(&self)->bool{true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8>{if !self.reset&&pins.sclk&(1<<18)!=0&&pins.mosi&(1<<23)!=0&&pins.cs&(1<<14)!=0{for &b in tx{self.byte(b);}}vec![255;rx_len]}
    fn input_levels(&self)->Vec<(u8,bool)>{vec![(35,true)]}
    fn named_pin(&self,id:&str)->Option<u8>{(id=="button").then_some(35)}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{let mut pixels=vec![0;64*128];if self.on&&!self.reset{for y in 0..128{for x in 0..64{let column=if self.seg_reverse{x}else{63-x};let row=(if self.com_reverse{y}else{127-y}+usize::from(self.start)+usize::from(self.offset)+32)&127;let bit=self.ram[(row/8)*128+column]&(1<<(row&7))!=0;pixels[y*64+x]=if (self.all_on||bit)^self.inverted{0xffff}else{0};}}}Some((64,128,pixels,self.version))}
    fn display_version(&self)->u64{self.version}
    fn display_frames(&self)->u64{self.bytes/1024}
    fn leds(&self)->Option<(&[[u8;3]],u64)>{Some((&self.led,self.led_version))}
    fn report(&self)->String{format!("[m5stick-oled] SH1107 page RAM SPI bytes={} on={} inverted={}; black SKU has no IMU; IP5306 power/buzzer/IR waveform and physical scan orientation unvalidated",self.bytes,self.on,self.inverted)}
}
#[cfg(test)]mod tests{
    use super::*;
    const P:SpiPins=SpiPins{sclk:1<<18,mosi:1<<23,miso:None,cs:1<<14,keep_cs:false};
    fn cmd(b:&mut M5StickOled,c:&[u8]){b.gpio_changes(&[(27,false)]);b.spi_transfer_pins(3,P,c,0);}
    #[test]fn sh1107_page_address_and_control_commands_use_the_real_wires(){let mut b=M5StickOled::new();cmd(&mut b,&[0xaf,0xa1,0xc8,0xd3,96,0xdc,0,0xb0,0x10,0]);b.gpio_changes(&[(27,true)]);b.spi_transfer_pins(3,P,&[1],0);assert_eq!(b.display().unwrap().2[0],0xffff);assert_eq!(b.display().unwrap().2[64],0);b.spi_transfer_pins(3,SpiPins{mosi:1<<19,..P},&[255],0);assert_eq!(b.bytes,1);cmd(&mut b,&[0xa7]);assert_eq!(b.display().unwrap().2[0],0);assert_eq!(b.display().unwrap().2[64],0xffff);cmd(&mut b,&[0xae]);assert!(b.display().unwrap().2.iter().all(|&p|p==0));b.gpio_changes(&[(33,false)]);cmd(&mut b,&[0xaf]);assert!(b.display().unwrap().2.iter().all(|&p|p==0));assert_eq!(b.named_pin("button"),Some(35));assert_eq!(b.named_pin("power"),None);assert!(b.i2c_devices().is_empty());}
}

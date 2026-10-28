//! Original LILYGO T-Deck SX1262, T-Deck 2027dee7 utilities.h and schematic.pdf.
//! ST7789 / GT911 / ESP32-C3 keyboard protocol / trackball GPIO; no Plus GNSS.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,gt911::{Gt911,Gt911State}};
use esp_soc::board::{BoardModel,BoardEdge,VirtualCycle,SpiPins};
use esp_soc::devices::{DcsPanel,i2c_bitbang::I2cBitBang,sx1262::Sx1262};
use super::tdeck_keyboard::{Keyboard,KeyboardState};
// Physical QWERTY rows, from the official photo, -> C3 (column,row). The last
// row has five keys; holes are not switches. No translation through UART.
const MATRIX:[[Option<(u8,u8)>;10];4]=[
    [Some((0,0)),Some((0,1)),Some((1,0)),Some((2,0)),Some((2,2)),Some((3,2)),Some((3,0)),Some((4,2)),Some((4,0)),Some((1,3))],
    [Some((0,3)),Some((1,1)),Some((1,2)),Some((2,6)),Some((2,1)),Some((3,1)),Some((3,6)),Some((4,6)),Some((4,1)),Some((4,3))],
    [Some((0,4)),Some((1,5)),Some((1,4)),Some((2,5)),Some((2,4)),Some((3,4)),Some((3,5)),Some((4,5)),Some((4,4)),Some((3,3))],
    [None,Some((1,6)),Some((0,6)),None,Some((0,5)),None,None,Some((0,2)),Some((2,3)),None],
];
pub struct TDeck {panel:DcsPanel,i2c:I2cBitBang,levels:[bool;2],touch:Arc<Mutex<Gt911State>>,keyboard:Arc<Mutex<KeyboardState>>,radio:Sx1262,cycle:u64,enabled:u64,output:u64,edges:Vec<BoardEdge>,irq:bool,radio_levels:[bool;2],gpio_events:u64,spi_seen:u64,spi_matched:u64,display_epoch:u64}
impl TDeck {
    pub fn new()->Self {let touch=Arc::new(Mutex::new(Gt911State::new(240,320,crate::periph::CPU_HZ/100)));touch.lock().expect("T-Deck touch poisoned").powered=false;Self{panel:DcsPanel::st7789(),i2c:I2cBitBang::new(),levels:[true;2],touch,keyboard:Arc::new(Mutex::new(KeyboardState::new(crate::periph::CPU_HZ))),radio:Sx1262::new(crate::periph::CPU_HZ),cycle:0,enabled:0,output:0,edges:Vec::new(),irq:true,radio_levels:[false;2],gpio_events:0,spi_seen:0,spi_matched:0,display_epoch:0}}
    fn powered(&self)->bool {self.enabled&self.output&(1<<10)!=0}
    fn sync(&mut self) {
        let mut t=self.touch.lock().expect("T-Deck touch poisoned");t.advance_to(self.cycle);let high=!t.powered||!t.ready;if high!=self.irq {self.irq=high;self.edges.push(BoardEdge{cycle:self.cycle,pin:16,level:high});}
        for (i,(pin,level)) in [(13,self.radio.busy()),(45,self.radio.irq())].into_iter().enumerate(){if self.radio_levels[i]!=level {self.radio_levels[i]=level;self.edges.push(BoardEdge{cycle:self.cycle,pin,level});}}
    }
}
impl Default for TDeck {fn default()->Self {Self::new()}}
impl BoardModel for TDeck {
    fn name(&self)->&'static str {"tdeck"}
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {
        let was_powered=self.powered();let backlight=self.output&(1<<42)!=0;self.cycle=cycle;self.enabled=enabled;self.output=output;self.gpio_events+=changes.len() as u64;self.panel.dc=output&(1<<11)!=0;
        let powered=self.powered();if was_powered && !powered {self.panel.reset();}
        if powered!=was_powered || backlight!=(output&(1<<42)!=0) {self.display_epoch+=1;}
        {let mut t=self.touch.lock().expect("T-Deck touch poisoned");t.powered=powered;if was_powered && !powered {t.ready=false;t.down=false;}}self.keyboard.lock().expect("T-Deck keyboard poisoned").power(cycle,powered);
        self.radio.advance_to(cycle);self.radio.controls(powered,enabled&output&(1<<17)!=0,true);self.radio.select(enabled&(1<<9)!=0 && output&(1<<9)==0);
        let high=|pin:u8|enabled&(1u64<<pin)==0 || output&(1u64<<pin)!=0;let scl=high(8);let sda=self.i2c.drive(scl,high(18));
        for (i,(pin,level)) in [(18,sda),(8,scl)].into_iter().enumerate(){if self.levels[i]!=level {self.levels[i]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
        self.sync();
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();self.i2c.attach(0x5d,Box::new(Gt911::new(self.touch.clone())));self.i2c.attach(0x55,Box::new(Keyboard::new(self.keyboard.clone())));
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some((18,8))).into_iter().map(move |(address,device)|(bus,address,device))).collect()
    }
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        self.spi_seen+=1;if !self.powered() || pins.sclk&(1<<40)==0 || pins.mosi&(1<<41)==0 {return vec![0xff;rx_len];}
        let cs=pins.cs&((1<<9)|(1<<12)|(1<<39));
        if cs==1<<12 {self.spi_matched+=1;for &b in tx {self.panel.byte(b);}return vec![0;rx_len];}
        if cs==1<<9 {let rx=self.radio.transfer(tx,rx_len);return if rx_len==0 || pins.miso==Some(38) {rx}else{vec![0xff;rx_len]};}
        vec![0xff;rx_len]
    }
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true),(1,true),(2,true),(3,true),(15,true),(8,self.levels[1]),(18,self.levels[0]),(16,self.irq),(13,self.radio_levels[0]),(45,self.radio_levels[1]),(46,true)]}
    fn named_pin(&self,name:&str)->Option<u8> {match name {"center"|"boot"=>Some(0),"left"=>Some(1),"right"=>Some(2),"up"=>Some(3),"down"=>Some(15),_=>None}}
    fn advance_to(&mut self,cycle:VirtualCycle) {self.cycle=cycle;self.keyboard.lock().expect("T-Deck keyboard poisoned").advance(cycle);self.radio.advance_to(cycle);self.sync();}
    fn next_deadline(&self)->Option<VirtualCycle> {self.keyboard.lock().expect("T-Deck keyboard poisoned").next_deadline().into_iter().chain(self.radio.next_deadline()).min()}
    fn take_edges(&mut self)->Vec<BoardEdge> {self.sync();std::mem::take(&mut self.edges)}
    fn matrix_key_at(&mut self,cycle:VirtualCycle,row:u8,column:u8,pressed:bool)->bool {let Some(Some((c,r)))=MATRIX.get(row as usize).and_then(|line|line.get(column as usize)) else {return false};self.keyboard.lock().expect("T-Deck keyboard poisoned").key(cycle,*c,*r,pressed)}
    fn touch(&mut self,x:u16,y:u16,down:bool) {self.touch.lock().expect("T-Deck touch poisoned").update(239-y.min(239),x.min(319),down);self.sync();}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {let visible=self.powered() && self.output&(1<<42)!=0 && self.panel.on && !self.panel.sleeping;let mut frame=Vec::with_capacity(320*240);for y in 0..240 {for x in 0..320 {frame.push(if visible {self.panel.gram[x*240+239-y]}else{0});}}Some((320,240,frame,self.display_version()))}
    fn display_version(&self)->u64 {self.panel.pixels_written+self.panel.resets+self.display_epoch}
    fn display_frames(&self)->u64 {self.panel.frames}
    fn lora_receive_at(&mut self,cycle:VirtualCycle,p:&esp_soc::devices::sx1262::LoRaInput)->bool {self.radio.advance_to(cycle);self.radio.receive(&p.config,&p.payload,p.rssi_dbm,p.snr_quarter_db,p.crc_error)}
    fn lora_snapshot(&self)->Option<String> {Some(self.radio.snapshot())}
    fn report(&self)->String {let k=self.keyboard.lock().expect("T-Deck keyboard poisoned");format!("[tdeck] ST7789 frames={} pixels={} spi={}/{} madctl={:#x} powered={} GT911=single-contact keyboard=I2C-C3 raw={} scans={} reads={} SX1262 commands={} audio/SD/GNSS=absent RF=unmodeled",self.panel.frames,self.panel.pixels_written,self.spi_matched,self.spi_seen,self.panel.madctl,self.powered(),k.raw,k.scans,k.reads,self.radio.stats.commands)}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn physical_keyboard_layout_is_a_bijection_and_holes_are_rejected() {let mut b=TDeck::new();let mut seen=std::collections::HashSet::new();for (row,line) in MATRIX.iter().enumerate(){for (col,key) in line.iter().enumerate(){assert_eq!(b.matrix_key_at(0,row as u8,col as u8,true),key.is_some());if let Some(k)=key {assert!(seen.insert(k));}}}assert_eq!(seen.len(),35);assert!(!b.matrix_key_at(0,4,0,true));assert!(!b.matrix_key_at(0,0,10,true));}
    #[test]
    fn lcd_requires_spi_route_exclusive_select_and_power_then_maps_native_gram() {
        let mut b=TDeck::new();let pins=SpiPins{sclk:1<<40,mosi:1<<41,miso:Some(38),cs:1<<12};let mask=(1<<10)|(1<<11)|(1<<42);
        let command=|b:&mut TDeck,c:u8,data:&[u8],p:SpiPins|{b.gpio_output_at(1,&[],mask,(1<<10)|(1<<42));b.spi_transfer_pins(2,p,&[c],0);b.gpio_output_at(2,&[],mask,mask);b.spi_transfer_pins(2,p,data,0);};
        command(&mut b,0x11,&[],pins);command(&mut b,0x29,&[],pins);command(&mut b,0x36,&[0x60],pins);command(&mut b,0x3a,&[0x55],pins);
        command(&mut b,0x2a,&[0,0,1,63],pins);command(&mut b,0x2b,&[0,0,0,239],pins);
        command(&mut b,0x2c,&[0xf8,0],SpiPins{sclk:1<<39,..pins});assert_eq!(b.panel.pixels_written,0);
        command(&mut b,0x2c,&[0xf8,0],SpiPins{cs:(1<<12)|(1<<9),..pins});assert_eq!(b.panel.pixels_written,0);
        command(&mut b,0x2c,&[0xf8,0,0x07,0xe0],pins);let frame=b.display().unwrap();assert_eq!((frame.0,frame.1),(320,240));assert_eq!(&frame.2[..3],&[0xf800,0x07e0,0]);
        b.gpio_output_at(3,&[],mask,0);assert!(b.display().unwrap().2.iter().all(|p|*p==0));assert!(!b.touch.lock().unwrap().powered);
    }
    #[test]
    fn devices_use_original_pins_and_require_peripheral_power() {let mut b=TDeck::new();let mut d=b.i2c_devices();assert!(d.iter().all(|(_,_,d)|d.pins()==Some((18,8))));assert!(d.iter_mut().all(|(_,_,d)|!d.start(true)));b.gpio_output_at(1,&[],1<<10,1<<10);assert!(d.iter_mut().all(|(_,_,d)|d.start(true)));b.touch(20,30,true);let t=b.touch.lock().unwrap();assert_eq!((t.x,t.y),(209,20));}
}

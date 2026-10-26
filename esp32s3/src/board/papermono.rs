//! M5Stack PaperMono: digital EPD/touch/power wiring, no RF/NFC/SD/audio model.
//! Pin and reset sources: M5 PaperMono schematic; M5GFX 02107b82 M5GFX.cpp.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,Reg8Device};
use esp_periph::i2c::m5::{M5pm1,M5pm1State,M5ioe1,M5ioe1State};
use esp_periph::i2c::bmi270::{Bmi270,Bmi270Sample};
use esp_periph::i2c::ft6336::{Ft6336g,Ft6336State};
use esp_soc::board::{BoardModel,BoardEdge,VirtualCycle};
use esp_soc::devices::ssd1677::Ssd1677;
use esp_soc::devices::i2c_bitbang::I2cBitBang;

struct Wiring {
    panel:Mutex<Ssd1677>,pmic:Arc<Mutex<M5pm1State>>,expander:Arc<Mutex<M5ioe1State>>,touch:Arc<Mutex<Ft6336State>>,active:Mutex<bool>,
}
impl Wiring {
    fn sync(&self) {
        let off=self.pmic.lock().expect("PaperMono PMIC mutex poisoned").power_off_requested;
        let high=self.expander.lock().expect("PaperMono IOE mutex poisoned").driven_high;
        let active=!off && high & 0x14 == 0x14;
        let mut old=self.active.lock().expect("PaperMono power mutex poisoned");
        if *old && !active {self.panel.lock().expect("PaperMono panel mutex poisoned").reset();}*old=active;
        let mut touch=self.touch.lock().expect("PaperMono touch mutex poisoned");touch.powered=!off && high & 0x1020 == 0x1020;
        if !touch.powered {touch.reset();}
    }
}
struct PowerDevice {device:Box<dyn I2cDevice>,wiring:Arc<Wiring>}
impl I2cDevice for PowerDevice {
    fn start(&mut self,read:bool)->bool {self.device.start(read)}
    fn write(&mut self,b:u8)->bool {let ack=self.device.write(b);self.wiring.sync();ack}
    fn read(&mut self)->u8 {self.device.read()}
    fn stop(&mut self) {self.device.stop();}
}
pub struct PaperMono {i2c:I2cBitBang, i2c_levels:[bool;2], wiring:Arc<Wiring>,dc:bool,cs:Option<bool>,cycle:u64,touch_irq_high:bool,edges:Vec<BoardEdge>,gpio_events:u64,imu:Arc<Mutex<Bmi270Sample>>}
impl PaperMono {
    pub const WIDTH:usize=480;pub const HEIGHT:usize=800;
    pub fn new()->Self {Self {i2c:I2cBitBang::new(),i2c_levels:[true,true],wiring:Arc::new(Wiring{panel:Mutex::new(Ssd1677::new(800,480,18,crate::periph::CPU_HZ/1000)),pmic:Default::default(),expander:Default::default(),touch:Default::default(),active:Mutex::new(false)}),dc:false,cs:None,cycle:0,touch_irq_high:true,edges:Vec::new(),gpio_events:0,imu:Default::default()}}
    fn sync_touch_irq(&mut self) {let state=self.wiring.touch.lock().expect("PaperMono touch mutex poisoned");let high=!state.powered || !state.down;if high!=self.touch_irq_high {self.touch_irq_high=high;self.edges.push(BoardEdge{cycle:self.cycle,pin:4,level:high});}}
}
impl Default for PaperMono {fn default()->Self {Self::new()}}
impl BoardModel for PaperMono {
    fn name(&self)->&'static str {"papermono"}
    fn gpio_changes(&mut self,changes:&[(u8,bool)]) {self.gpio_events+=changes.len() as u64;for &(pin,level) in changes {match pin {16=>self.cs=Some(level),17=>self.dc=level,_=>{}}}}
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {
        self.cycle=cycle;self.gpio_changes(changes);
        let released_high=|pin:u8| enabled & (1u64<<pin)==0 || output & (1u64<<pin)!=0;
        let scl=released_high(48);let sda=self.i2c.drive(scl,released_high(47));
        for (index,(pin,level)) in [(47,sda),(48,scl)].into_iter().enumerate() {if self.i2c_levels[index]!=level {self.i2c_levels[index]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
    }
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn spi_transfer(&mut self,host:u8,tx:&[u8],rx_len:usize)->Vec<u8> {
        if host==2 {self.wiring.sync();if *self.wiring.active.lock().expect("PaperMono power mutex poisoned") && self.cs!=Some(true) {self.wiring.panel.lock().expect("PaperMono panel mutex poisoned").write(self.dc,tx);}}
        vec![0xff;rx_len]
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {self.i2c.clear();let devices:Vec<(u8,u8,Box<dyn I2cDevice>)>=vec![
        (0,0x6e,Box::new(PowerDevice{device:Box::new(M5pm1::new(self.wiring.pmic.clone())),wiring:self.wiring.clone()})),
        (0,0x4f,Box::new(PowerDevice{device:Box::new(M5ioe1::new(self.wiring.expander.clone(),0)),wiring:self.wiring.clone()})),
        (0,0x38,Box::new(Ft6336g::new(self.wiring.touch.clone()))),
        (0,0x68,Box::new(Bmi270::new(self.imu.clone()))),
        // RX8130 RAM is needed for board identification and app settings. Calendar,
        // alarms, battery-domain retention and wake are not implemented by this subset.
        (0,0x32,Box::new(Reg8Device::new("rx8130-register-ram",&[]))),
    ];for (_,address,device) in devices {self.i2c.attach(address,device);}
        // Both ESP controllers can route to the physical 47/48 bus. M5GFX uses
        // I2C1 while other firmware may use I2C0; state belongs to the devices.
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some((47,48))).into_iter().map(move |(address,device)|(bus,address,device))).collect()}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {
        let panel=self.wiring.panel.lock().expect("PaperMono panel mutex poisoned");let mut frame=vec![0xffff;Self::WIDTH*Self::HEIGHT];
        // Native landscape is configured with M5GFX offset_rotation=3.
        for y in 0..480 {for x in 0..800 {frame[x*480+(479-y)]=panel.frame[y*800+x];}}
        Some((480,800,frame,panel.version))
    }
    fn display_version(&self)->u64 {self.wiring.panel.lock().expect("PaperMono panel mutex poisoned").version}
    fn display_frames(&self)->u64 {self.wiring.panel.lock().expect("PaperMono panel mutex poisoned").frames}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(18,self.wiring.panel.lock().expect("PaperMono panel mutex poisoned").busy()),(4,self.touch_irq_high),(2,true),(3,true),(47,self.i2c_levels[0]),(48,self.i2c_levels[1])]}
    fn touch(&mut self,x:u16,y:u16,down:bool) {self.wiring.sync();let mut state=self.wiring.touch.lock().expect("PaperMono touch mutex poisoned");if state.powered {state.update(x.min(479),y.min(799),down);}drop(state);self.sync_touch_irq();}
    fn advance_to(&mut self,cycle:VirtualCycle) {self.cycle=cycle;self.wiring.panel.lock().expect("PaperMono panel mutex poisoned").advance_to(cycle);self.sync_touch_irq();}
    fn next_deadline(&self)->Option<VirtualCycle> {self.wiring.panel.lock().expect("PaperMono panel mutex poisoned").next_deadline()}
    fn take_edges(&mut self)->Vec<BoardEdge> {self.sync_touch_irq();let mut edges=std::mem::take(&mut self.edges);edges.extend(self.wiring.panel.lock().expect("PaperMono panel mutex poisoned").take_edges());edges.sort_by_key(|e|e.cycle);edges}
    fn named_pin(&self,name:&str)->Option<u8> {match name {"a"|"btn_a"=>Some(2),"b"|"btn_b"=>Some(3),_=>None}}
    fn report(&self)->String {let panel=self.wiring.panel.lock().expect("PaperMono panel mutex poisoned");format!("[papermono] frames={} ram_bytes={} busy={} rtc=register-ram nfc=unimplemented",panel.frames,panel.ram_bytes_written,panel.busy())}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(device:&mut dyn I2cDevice,reg:u8,value:u8) {assert!(device.start(false));assert!(device.write(reg));assert!(device.write(value));device.stop();}
    fn read(device:&mut dyn I2cDevice,reg:u8,count:usize)->Vec<u8> {assert!(device.start(false));assert!(device.write(reg));assert!(device.start(true));let bytes=(0..count).map(|_|device.read()).collect();device.stop();bytes}
    fn send(board:&mut PaperMono,command:u8,data:&[u8]) {board.gpio_changes(&[(16,false),(17,false)]);board.spi_transfer(2,&[command],0);board.gpio_changes(&[(17,true)]);board.spi_transfer(2,data,0);}
    #[test]
    fn shared_physical_i2c_and_expander_gated_touch() {
        let mut board=PaperMono::new();let mut devices=board.i2c_devices();assert_eq!(devices.len(),10);
        assert!(devices.iter().all(|(_,_,d)|d.pins()==Some((47,48))));
        let find=|bus,address|devices.iter().position(|(b,a,_)|*b==bus && *a==address).unwrap();
        let rtc0=find(0,0x32);let rtc1=find(1,0x32);let ioe=find(1,0x4f);let touch=find(1,0x38);
        write(devices[rtc0].2.as_mut(),0x20,0x45);assert_eq!(read(devices[rtc1].2.as_mut(),0x20,1),[0x45]);
        assert!(!devices[touch].2.start(true));board.touch(99,200,true);assert!(board.input_levels().contains(&(4,true)));
        for (reg,value) in [(3,0x34),(4,0x10),(5,0x34),(6,0x10),(0x13,0),(0x14,0)] {write(devices[ioe].2.as_mut(),reg,value);}
        board.touch(479,799,true);assert!(board.input_levels().contains(&(4,false)));
        assert_eq!(read(devices[touch].2.as_mut(),2,5),[1,0x81,0xdf,3,0x1f]);
        write(devices[ioe].2.as_mut(),6,0);board.advance_to(1);assert!(!devices[touch].2.start(true));assert!(board.input_levels().contains(&(4,true)));
    }
    #[test]
    fn panel_requires_power_and_refresh_and_rotates_native_pixels() {
        let mut board=PaperMono::new();let mut devices=board.i2c_devices();
        send(&mut board,0x24,&[0]);assert_eq!(board.wiring.panel.lock().unwrap().ram_bytes_written,0);
        let ioe=devices.iter_mut().find(|(bus,address,_)|*bus==1 && *address==0x4f).unwrap();
        for (reg,value) in [(3,0x14),(5,0x14),(0x13,0)] {write(ioe.2.as_mut(),reg,value);}
        send(&mut board,0x11,&[1]);send(&mut board,0x44,&[0,0,7,0]);send(&mut board,0x45,&[0xdf,1,0xdf,1]);
        send(&mut board,0x4f,&[0xdf,1]);send(&mut board,0x24,&[0x7f]);
        assert_eq!(board.display_frames(),0);send(&mut board,0x22,&[0xc4]);send(&mut board,0x20,&[]);
        assert!(board.input_levels().contains(&(18,true)));board.advance_to(crate::periph::CPU_HZ/50);
        let (width,height,frame,version)=board.display().unwrap();assert_eq!((width,height,version),(480,800,1));
        assert_eq!(frame[479],0);assert_eq!(frame[478],0xffff);assert_eq!(frame[480+479],0xffff);
        write(ioe.2.as_mut(),5,0);assert_eq!(board.display().unwrap().2[479],0,"power loss retains the bistable image");
    }
}

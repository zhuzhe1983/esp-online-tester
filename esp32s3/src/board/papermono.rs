//! M5Stack PaperMono: digital EPD/touch/power and ST25R3916 no-peer reader subset.
//! Pin and reset sources: M5 PaperMono schematic; M5GFX 02107b82 M5GFX.cpp.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,Reg8Device};
use esp_periph::i2c::m5::{M5pm1,M5pm1State,M5ioe1,M5ioe1State};
use esp_periph::i2c::bmi270::{Bmi270,Bmi270Sample};
use esp_periph::i2c::ft6336::{Ft6336g,Ft6336State};
use esp_periph::i2c::st25r3916::{St25r3916,St25r3916State};
use esp_soc::board::{BoardModel,BoardEdge,VirtualCycle,SpiPins};
use esp_soc::devices::sx1262::Sx1262;
use esp_soc::devices::ssd1677::Ssd1677;
use esp_soc::devices::i2c_bitbang::I2cBitBang;

struct Wiring {
    panel:Mutex<Ssd1677>,pmic:Arc<Mutex<M5pm1State>>,expander:Arc<Mutex<M5ioe1State>>,touch:Arc<Mutex<Ft6336State>>,active:Mutex<bool>,nfc:Option<Arc<Mutex<St25r3916State>>>,lora:Option<Mutex<Sx1262>>,
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
        // Manufacturer schematic GET 2026-10-27, NFC sheet V0.2: R1=0R bypasses
        // the NC load switch. IOE4 (PYB_NFC_EN) is spare, not a power gate.
        if let Some(nfc)=&self.nfc {nfc.lock().expect("PaperMono NFC mutex poisoned").set_powered(!off);}
        // C153 schematic U9 EN=PM1 GPIO2; U14 NRST=IOE10, antenna switch=IOE2.
        if let Some(lora)=&self.lora {let pm=self.pmic.lock().expect("PaperMono PMIC mutex poisoned");lora.lock().expect("PaperMono LoRa mutex poisoned").controls(!off && pm.driven_high&4!=0,high&0x200!=0,high&2!=0);}
    }
}
struct PowerDevice {device:Box<dyn I2cDevice>,wiring:Arc<Wiring>}
impl I2cDevice for PowerDevice {
    fn start(&mut self,read:bool)->bool {self.device.start(read)}
    fn write(&mut self,b:u8)->bool {let ack=self.device.write(b);self.wiring.sync();ack}
    fn read(&mut self)->u8 {self.device.read()}
    fn stop(&mut self) {self.device.stop();}
}
pub struct PaperMono {i2c:I2cBitBang, i2c_levels:[bool;2], wiring:Arc<Wiring>,dc:bool,cs:Option<bool>,cycle:u64,touch_irq_high:bool,nfc_irq_high:bool,lora_levels:[bool;2],edges:Vec<BoardEdge>,gpio_events:u64,leds:[[u8;3];1],led_updates:u64,imu:Arc<Mutex<Bmi270Sample>>}
impl PaperMono {
    pub const WIDTH:usize=480;pub const HEIGHT:usize=800;
    pub fn new()->Self {Self::with_nfc(true)}
    pub fn lite()->Self {Self::with_nfc(false)}
    fn with_nfc(nfc:bool)->Self {Self {i2c:I2cBitBang::new(),i2c_levels:[true,true],wiring:Arc::new(Wiring{panel:Mutex::new(Ssd1677::new(800,480,18,crate::periph::CPU_HZ/1000)),pmic:Default::default(),expander:Default::default(),touch:Default::default(),active:Mutex::new(false),nfc:nfc.then(||Arc::new(Mutex::new(St25r3916State::new(crate::periph::CPU_HZ)))),lora:nfc.then(||Mutex::new(Sx1262::new(crate::periph::CPU_HZ)))}),dc:false,cs:None,cycle:0,touch_irq_high:true,nfc_irq_high:false,lora_levels:[false,false],edges:Vec::new(),gpio_events:0,leds:[[0;3]],led_updates:0,imu:Default::default()}}
    fn sync_leds(&mut self) {let pm=self.wiring.pmic.lock().expect("PaperMono PMIC mutex poisoned");let ioe=self.wiring.expander.lock().expect("PaperMono IOE mutex poisoned");
        let rgb=if pm.power_off_requested {[0;3]} else {[if pm.led_enabled {255}else{0},((u32::from(ioe.pwm[1])*255+2047)/4095) as u8,((u32::from(ioe.pwm[0])*255+2047)/4095) as u8]};
        if self.leds[0]!=rgb {self.leds[0]=rgb;self.led_updates+=1;}}
    fn sync_touch_irq(&mut self) {let state=self.wiring.touch.lock().expect("PaperMono touch mutex poisoned");let high=!state.powered || !state.down;if high!=self.touch_irq_high {self.touch_irq_high=high;self.edges.push(BoardEdge{cycle:self.cycle,pin:4,level:high});}}
}
impl Default for PaperMono {fn default()->Self {Self::new()}}
impl BoardModel for PaperMono {
    fn name(&self)->&'static str {if self.wiring.nfc.is_some(){"papermono"}else{"papermono-lite"}}
    fn gpio_changes(&mut self,changes:&[(u8,bool)]) {self.gpio_events+=changes.len() as u64;for &(pin,level) in changes {match pin {16=>self.cs=Some(level),17=>self.dc=level,_=>{}}}}
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {
        self.cycle=cycle;self.gpio_changes(changes);
        let released_high=|pin:u8| enabled & (1u64<<pin)==0 || output & (1u64<<pin)!=0;
        if let Some(lora)=&self.wiring.lora {let mut l=lora.lock().expect("PaperMono LoRa mutex poisoned");l.advance_to(cycle);l.select(enabled&(1<<41)!=0 && output&(1<<41)==0);}
        let scl=released_high(48);let sda=self.i2c.drive(scl,released_high(47));
        for (index,(pin,level)) in [(47,sda),(48,scl)].into_iter().enumerate() {if self.i2c_levels[index]!=level {self.i2c_levels[index]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
    }
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn spi_transfer(&mut self,host:u8,tx:&[u8],rx_len:usize)->Vec<u8> {
        if host==2 {self.wiring.sync();if *self.wiring.active.lock().expect("PaperMono power mutex poisoned") && self.cs!=Some(true) {self.wiring.panel.lock().expect("PaperMono panel mutex poisoned").write(self.dc,tx);}}
        vec![0xff;rx_len]
    }
    fn uses_spi_pins(&self)->bool {self.wiring.lora.is_some()}
    fn spi_transfer_pins(&mut self,host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        self.wiring.sync();
        if pins.sclk&(1<<39)!=0 && pins.mosi&(1<<38)!=0 && pins.cs&(1<<41)!=0 {if let Some(lora)=&self.wiring.lora {
            let rx=lora.lock().expect("PaperMono LoRa mutex poisoned").transfer(tx,rx_len);
            if pins.miso==Some(40) {return rx;}
            return vec![0xff;rx_len];
        }}self.spi_transfer(host,tx,rx_len)
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {if self.wiring.expander.lock().unwrap().input_mask&1==0 {self.set_sd_card_present(false);}self.wiring.sync();self.i2c.clear();let devices:Vec<(u8,u8,Box<dyn I2cDevice>)>=vec![
        (0,0x6e,Box::new(PowerDevice{device:Box::new(M5pm1::new(self.wiring.pmic.clone())),wiring:self.wiring.clone()})),
        (0,0x4f,Box::new(PowerDevice{device:Box::new(M5ioe1::new(self.wiring.expander.clone(),0)),wiring:self.wiring.clone()})),
        (0,0x38,Box::new(Ft6336g::new(self.wiring.touch.clone()))),
        (0,0x68,Box::new(Bmi270::new(self.imu.clone()))),
        // RX8130 RAM is needed for board identification and app settings. Calendar,
        // alarms, battery-domain retention and wake are not implemented by this subset.
        (0,0x32,Box::new(Reg8Device::new("rx8130-register-ram",&[]))),
    ];for (_,address,device) in devices {self.i2c.attach(address,device);}
        if let Some(nfc)=&self.wiring.nfc {self.i2c.attach(0x50,Box::new(St25r3916::new(nfc.clone())));}
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
    fn input_levels(&self)->Vec<(u8,bool)> {let mut levels=vec![(18,self.wiring.panel.lock().expect("PaperMono panel mutex poisoned").busy()),(4,self.touch_irq_high),(6,self.nfc_irq_high),(2,true),(3,true),(47,self.i2c_levels[0]),(48,self.i2c_levels[1])];if self.wiring.lora.is_some() {levels.extend([(21,self.lora_levels[0]),(5,self.lora_levels[1])]);}levels}
    fn touch(&mut self,x:u16,y:u16,down:bool) {self.wiring.sync();let mut state=self.wiring.touch.lock().expect("PaperMono touch mutex poisoned");if state.powered {state.update(x.min(479),y.min(799),down);}drop(state);self.sync_touch_irq();}
    fn advance_to(&mut self,cycle:VirtualCycle) {self.cycle=cycle;self.wiring.panel.lock().expect("PaperMono panel mutex poisoned").advance_to(cycle);self.sync_touch_irq();self.sync_leds();if let Some(nfc)=&self.wiring.nfc {nfc.lock().expect("PaperMono NFC mutex poisoned").advance_to(cycle);}
        if let Some(lora)=&self.wiring.lora {lora.lock().expect("PaperMono LoRa mutex poisoned").advance_to(cycle);}}
    fn next_deadline(&self)->Option<VirtualCycle> {self.wiring.panel.lock().expect("PaperMono panel mutex poisoned").next_deadline().into_iter().chain(self.wiring.nfc.as_ref().and_then(|n|n.lock().expect("PaperMono NFC mutex poisoned").next_deadline())).chain(self.wiring.lora.as_ref().and_then(|l|l.lock().expect("PaperMono LoRa mutex poisoned").next_deadline())).min()}
    fn take_edges(&mut self)->Vec<BoardEdge> {self.sync_touch_irq();if let Some(nfc)=&self.wiring.nfc {let high=nfc.lock().expect("PaperMono NFC mutex poisoned").irq();if high!=self.nfc_irq_high {self.nfc_irq_high=high;self.edges.push(BoardEdge{cycle:self.cycle,pin:6,level:high});}}
        if let Some(lora)=&self.wiring.lora {let l=lora.lock().expect("PaperMono LoRa mutex poisoned");for (i,(pin,level)) in [(21,l.busy()),(5,l.irq())].into_iter().enumerate(){if self.lora_levels[i]!=level {self.lora_levels[i]=level;self.edges.push(BoardEdge{cycle:self.cycle,pin,level});}}}let mut edges=std::mem::take(&mut self.edges);edges.extend(self.wiring.panel.lock().expect("PaperMono panel mutex poisoned").take_edges());edges.sort_by_key(|e|e.cycle);edges}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.leds,self.led_updates))}
    fn lora_receive_at(&mut self,cycle:VirtualCycle,p:&esp_soc::devices::sx1262::LoRaInput)->bool {self.wiring.sync();let Some(l)=&self.wiring.lora else{return false};let mut l=l.lock().expect("PaperMono LoRa mutex poisoned");l.advance_to(cycle);l.receive(&p.config,&p.payload,p.rssi_dbm,p.snr_quarter_db,p.crc_error)}
    fn lora_snapshot(&self)->Option<String> {self.wiring.lora.as_ref().map(|l|l.lock().expect("PaperMono LoRa mutex poisoned").snapshot())}
    fn sd_card_slot(&self)->Option<([u8;6],bool)> {let pm=self.wiring.pmic.lock().expect("PaperMono PMIC mutex poisoned");let ioe=self.wiring.expander.lock().expect("PaperMono IOE mutex poisoned");Some(([13,12,11,10,9,8],!pm.power_off_requested && ioe.driven_high & (1<<13)!=0))}
    fn set_sd_card_present(&mut self,present:bool) {let mut ioe=self.wiring.expander.lock().expect("PaperMono IOE mutex poisoned");ioe.input_mask|=1;ioe.input_high=(ioe.input_high & !1)|u16::from(!present);}
    fn named_pin(&self,name:&str)->Option<u8> {match name {"a"|"btn_a"=>Some(2),"b"|"btn_b"=>Some(3),_=>None}}
    fn report(&self)->String {let panel=self.wiring.panel.lock().expect("PaperMono panel mutex poisoned");let nfc=self.wiring.nfc.as_ref().map(|n|{let s=n.lock().expect("PaperMono NFC mutex poisoned");format!("no-peer commands={} tx={} no_response={} modes={:x} unsupported={} unsupported_commands={:016x}",s.commands,s.transmissions,s.no_response,s.modes_seen,s.unsupported,s.unsupported_commands)}).unwrap_or_else(||"absent".into());let lora=self.wiring.lora.as_ref().map(|l|{let l=l.lock().expect("PaperMono LoRa mutex poisoned");format!("digital commands={} tx={} rx={} timeouts={} rejected={} unsupported={:016x?}",l.stats.commands,l.stats.tx,l.stats.rx,l.stats.timeouts,l.stats.rejected,l.stats.unsupported)}).unwrap_or_else(||"absent".into());format!("[{}] frames={} ram_bytes={} busy={} rtc=register-ram nfc={nfc} lora={lora}",self.name(),panel.frames,panel.ram_bytes_written,panel.busy())}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(device:&mut dyn I2cDevice,reg:u8,value:u8) {assert!(device.start(false));assert!(device.write(reg));assert!(device.write(value));device.stop();}
    fn read(device:&mut dyn I2cDevice,reg:u8,count:usize)->Vec<u8> {assert!(device.start(false));assert!(device.write(reg));assert!(device.start(true));let bytes=(0..count).map(|_|device.read()).collect();device.stop();bytes}
    fn send(board:&mut PaperMono,command:u8,data:&[u8]) {board.gpio_changes(&[(16,false),(17,false)]);board.spi_transfer(2,&[command],0);board.gpio_changes(&[(17,true)]);board.spi_transfer(2,data,0);}
    #[test]
    fn full_nfc_is_physical_shared_bus_lite_has_none_and_spare_ioe_is_not_a_gate() {
        let mut full=super::super::make_board("papermono").unwrap();let mut lite=super::super::make_board("papermono-lite").unwrap();
        assert_eq!(lite.name(),"papermono-lite");assert_eq!(lite.i2c_devices().len(),10);assert!(!lite.i2c_devices().iter().any(|(_,addr,_)|*addr==0x50));
        let mut devices=full.i2c_devices();let nfc=devices.iter().position(|(bus,addr,_)|*bus==0&&*addr==0x50).unwrap();let nfc1=devices.iter().position(|(bus,addr,_)|*bus==1&&*addr==0x50).unwrap();
        assert_eq!(devices[nfc].2.pins(),Some((47,48)));assert_eq!(read(devices[nfc].2.as_mut(),0x7f,1),[0x2a]);
        write(devices[nfc].2.as_mut(),2,0x80);full.advance_to(168_000);assert!(full.take_edges().iter().any(|e|e.pin==6&&e.level));
        assert_eq!(read(devices[nfc1].2.as_mut(),0x5a,1),[0x80]);assert!(full.take_edges().iter().any(|e|e.pin==6&&!e.level));
        let ioe=devices.iter().position(|(bus,addr,_)|*bus==0&&*addr==0x4f).unwrap();write(devices[ioe].2.as_mut(),5,0);assert!(devices[nfc].2.start(true));
        let pm=devices.iter().position(|(bus,addr,_)|*bus==0&&*addr==0x6e).unwrap();write(devices[pm].2.as_mut(),0x0c,0xa1);assert!(!devices[nfc].2.start(true));
    }
    #[test]
    fn lora_requires_pmic_push_pull_power_reset_and_physical_spi_pins() {
        let mut board=PaperMono::new();let mut devices=board.i2c_devices();
        let pm=devices.iter().position(|(bus,addr,_)|*bus==0&&*addr==0x6e).unwrap();let ioe=devices.iter().position(|(bus,addr,_)|*bus==0&&*addr==0x4f).unwrap();
        for (r,v) in [(0x16,0),(0x10,4),(0x11,4)] {write(devices[pm].2.as_mut(),r,v);}
        for (r,v) in [(3,2),(4,2),(5,2),(6,2),(0x13,0),(0x14,0)] {write(devices[ioe].2.as_mut(),r,v);}
        assert!(!board.wiring.lora.as_ref().unwrap().lock().unwrap().snapshot().contains("\"powered\":true"),"open-drain release does not drive the enable high");
        write(devices[pm].2.as_mut(),0x13,0x1b);board.advance_to(1_000_000);
        let pins=SpiPins{sclk:1<<39,mosi:1<<38,miso:Some(40),cs:1<<41};let tx=[0x1d,3,0x20,0,0];
        for (host,route,expected) in [(3,pins,b'S'),(2,pins,b'S'),(3,SpiPins{sclk:1<<15,..pins},255),(3,SpiPins{miso:Some(20),..pins},255),(3,SpiPins{cs:0,..pins},255)] {
            board.gpio_output_at(2_000_000,&[],1<<41,0);assert_eq!(board.spi_transfer_pins(host,route,&tx,5)[4],expected);board.gpio_output_at(2_000_001,&[],1<<41,1<<41);
        }
        write(devices[pm].2.as_mut(),0x16,0x10);assert!(board.wiring.lora.as_ref().unwrap().lock().unwrap().snapshot().contains("\"powered\":false"),"alternate function does not drive standard GPIO2");
        assert!(PaperMono::lite().lora_snapshot().is_none());assert!(PaperMono::lite().input_levels().iter().all(|(pin,_)|*pin!=5 && *pin!=21));
    }
    #[test]
    fn shared_physical_i2c_and_expander_gated_touch() {
        let mut board=PaperMono::new();let mut devices=board.i2c_devices();assert_eq!(devices.len(),12);
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
    #[test]
    fn rgb_uses_pmic_and_expander_pwm_and_sd_detect_is_not_output_latch() {
        let mut board=PaperMono::new();let mut devices=board.i2c_devices();
        let ioe=devices.iter().position(|(bus,address,_)|*bus==1&&*address==0x4f).unwrap();
        let pm=devices.iter().position(|(bus,address,_)|*bus==1&&*address==0x6e).unwrap();
        assert_eq!(read(devices[ioe].2.as_mut(),7,1)[0]&1,1);board.set_sd_card_present(true);assert_eq!(read(devices[ioe].2.as_mut(),7,1)[0]&1,0);
        assert!(!board.sd_card_slot().unwrap().1);
        for (reg,value) in [(3,0x80),(4,0x21),(5,0x80),(6,0x20),(0x13,0),(0x14,0),(0x25,0x88),(0x26,0x13),(0x1b,0xff),(0x1c,0x8f),(0x1d,0),(0x1e,0x88)] {write(devices[ioe].2.as_mut(),reg,value);}
        board.advance_to(1);assert_eq!(board.leds().unwrap().0,[[255,128,255]]);assert!(board.sd_card_slot().unwrap().1);
        write(devices[pm].2.as_mut(),6,7);write(devices[ioe].2.as_mut(),0x1c,0xcf);board.advance_to(2);assert_eq!(board.leds().unwrap().0,[[0,128,0]]);
        write(devices[ioe].2.as_mut(),0x1e,8);board.advance_to(3);assert_eq!(board.leds().unwrap().0,[[0,0,0]]);
        write(devices[pm].2.as_mut(),0x0c,0xa1);board.advance_to(4);assert!(!board.sd_card_slot().unwrap().1);
    }

}

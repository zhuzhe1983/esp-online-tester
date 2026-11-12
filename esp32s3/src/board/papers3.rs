//! C139 V1.0 schematic, M5GFX c6f92dc / M5Unified 5580ff6.
//! ET047TC1 physical i80 routes; GT911 one-contact scans; BMI270 explicit samples.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,gt911::{Gt911,Gt911State},bmi270::{Bmi270,Bmi270Sample}};
use esp_soc::board::{BoardModel,BoardEdge,VirtualCycle,LcdI80Pins,SpiPins};
use esp_soc::devices::{i2c_bitbang::I2cBitBang,et047tc1::Et047tc1,bm8563::{Bm8563,Bm8563State}};
pub struct PaperS3 {rtc:Arc<Mutex<Bm8563State>>,sd:Option<Box<esp_periph::sdmmc::card::spi::SpiSdCard>>,i2c:I2cBitBang,levels:[bool;2],edges:Vec<BoardEdge>,output:u64,enabled:u64,panel:Et047tc1,touch:Arc<Mutex<Gt911State>>,touch_irq:bool,imu:Arc<Mutex<Bmi270Sample>>,gpio_events:u64,cycle:u64,rgb_routes:bool,rgb_frames:u64,rgb_scan:bool}
impl PaperS3 {
    pub fn new()->Self {Self{rtc:Arc::new(Mutex::new(Bm8563State::new(crate::periph::CPU_HZ))),sd:None,i2c:I2cBitBang::new(),levels:[true;2],edges:Vec::new(),output:0,enabled:0,panel:Et047tc1::new(),touch:Arc::new(Mutex::new(Gt911State::new(540,960,crate::periph::CPU_HZ/200))),touch_irq:true,imu:Default::default(),gpio_events:0,cycle:0,rgb_routes:false,rgb_frames:0,rgb_scan:false}}
    fn powered(&self)->bool {let mask=(1<<45)|(1<<46);self.enabled&self.output&mask==mask}
    fn sync_touch(&mut self) {let mut t=self.touch.lock().expect("PaperS3 touch poisoned");t.advance_to(self.cycle);let level=!t.ready;if level!=self.touch_irq {self.touch_irq=level;self.edges.push(BoardEdge{cycle:self.cycle,pin:48,level});}}
}
impl Default for PaperS3 {fn default()->Self {Self::new()}}
impl BoardModel for PaperS3 {
    fn name(&self)->&'static str {"papers3"}
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {
        if output&(1<<47)!=0 {if let Some(sd)=&mut self.sd {sd.deselect();}}
        self.rtc.lock().expect("PaperS3 RTC poisoned").advance_to(cycle);let previous=self.output;self.cycle=cycle;self.output=output;self.enabled=enabled;self.gpio_events+=changes.len() as u64;
        let high=|pin:u8|enabled&(1u64<<pin)==0 || output&(1u64<<pin)!=0;
        let scl=high(42);let sda=self.i2c.drive(scl,high(41));for (i,(pin,level)) in [(41,sda),(42,scl)].into_iter().enumerate(){if self.levels[i]!=level {self.levels[i]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
        if !self.powered() {self.panel.stop_scan();self.rgb_scan=false;return;}
        if previous&(1<<17)!=0 && output&(1<<17)==0 {self.panel.begin_scan();self.rgb_scan=false;}
        if previous&(1<<15)==0 && output&(1<<15)!=0 {self.panel.latch();}
    }
    fn uses_spi_pins(&self)->bool {true}
    fn attach_spi_sd_card(&mut self,image:Box<dyn esp_periph::sdmmc::SdStore>,config:esp_periph::sdmmc::SdCardConfig)->Result<String,String> {let sd=esp_periph::sdmmc::card::spi::SpiSdCard::new(image,config)?;let description=sd.describe();self.sd=Some(Box::new(sd));Ok(description)}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        // C139 V1.0 schematic J3: SCK39, DI38, DO40, CS47; 3V3 has no GPIO gate.
        if pins.sclk&(1<<39)==0 || pins.mosi&(1<<38)==0 {return vec![255;rx_len];}
        let Some(sd)=&mut self.sd else {return vec![255;rx_len]};let result=sd.transfer(pins.cs&(1<<47)!=0,tx,rx_len);if pins.miso==Some(40){result}else{vec![255;rx_len]}
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();self.i2c.attach(0x51,Box::new(Bm8563::new(self.rtc.clone())));self.i2c.attach(0x5d,Box::new(Gt911::new(self.touch.clone())));self.i2c.attach(0x68,Box::new(Bmi270::new(self.imu.clone())));
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some((41,42))).into_iter().map(move |(address,device)|(bus,address,device))).collect()
    }
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true),(4,true),(5,true),(41,self.levels[0]),(42,self.levels[1]),(48,self.touch_irq)]}
    fn take_edges(&mut self)->Vec<BoardEdge> {self.sync_touch();std::mem::take(&mut self.edges)}
    fn advance_to(&mut self,cycle:VirtualCycle) {self.rtc.lock().expect("PaperS3 RTC poisoned").advance_to(cycle);self.cycle=cycle;self.sync_touch();}
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn lcd_i80(&mut self,pins:LcdI80Pins,data_phase:bool,words:&[u16]) {
        let data=[6,14,7,12,9,11,8,10];
        if data_phase && pins.width==8 && pins.clock&(1<<16)!=0 && pins.select&(1<<13)!=0 && data.iter().enumerate().all(|(i,p)|pins.data[i]&(1<<p)!=0) && self.powered() && self.panel.scanning && self.output&(1<<15)==0 && self.output&(1<<18)!=0 {self.panel.line(words);}
    }
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,_cycle:VirtualCycle,routes:esp_soc::pins::PinRoutes<'_>) {
        // IDF v5.5.1 gpio_sig_map.h (unchanged in 5.5.2): PCLK154, HSYNC151,
        // DE150, DATA8..15=141..148. Matrix decoder rejects inverted/disabled pads.
        self.rgb_routes=routes.matrix_output(16,154) && routes.matrix_output(15,151) && routes.matrix_output(13,150)
            && [8,10,9,11,7,12,6,14].iter().enumerate().all(|(i,&p)|routes.matrix_output(p,141+i as u32));
    }
    fn lcd_rgb_start(&mut self,_cycle:VirtualCycle,w:u32,h:u32,bpp:u32,clock:&dyn Fn(u8)->Option<(u64,u32)>) {
        self.rgb_scan=self.rgb_routes && self.powered() && self.panel.scanning && self.panel.row==0
            && w==241 && h==544 && bpp==1 && clock(18).is_some_and(|(period,pulses)|period!=0 && pulses>=h);
    }
    fn lcd_frame(&mut self,w:u32,h:u32,bytes:&[u8]) {
        let scan=std::mem::take(&mut self.rgb_scan);
        if !scan || !self.rgb_routes || !self.powered() || !self.panel.scanning || w!=241 || h!=544 || bytes.len()!=241*544 {return;}
        // EPDiy d84d26eb output_lcd/lcd_driver.c: 8-bit RGB has one FIFO dummy
        // byte per row, and rounds 540 gate rows to 544. D8..15 are wired to
        // panel D6,D7,D4,D5,D2,D3,D0,D1, so reverse the four two-bit pairs.
        let mut words=[0u16;240];
        for line in bytes.chunks_exact(241).take(540) {
            for (word,&v) in words.iter_mut().zip(&line[1..]) {*word=u16::from((v>>6)|((v>>2)&12)|((v<<2)&48)|(v<<6));}
            self.panel.line(&words);self.panel.latch();
        }
        self.rgb_frames+=1;
    }
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {Some((540,960,self.panel.portrait(),self.panel.version))}
    fn display_version(&self)->u64 {self.panel.version}
    fn display_frames(&self)->u64 {self.panel.frames}
    fn display_quiet_push(&self)->bool {true}
    fn touch(&mut self,x:u16,y:u16,down:bool) {self.touch.lock().expect("PaperS3 touch poisoned").update(x,y,down);self.sync_touch();}
    fn set_imu_sample(&mut self,accel_g:[f64;3],gyro_dps:[f64;3])->bool {
        if !accel_g.iter().chain(gyro_dps.iter()).all(|v|v.is_finite()){return false;}
        let mut imu=self.imu.lock().expect("PaperS3 IMU poisoned");imu.accel_g=accel_g;imu.gyro_dps=gyro_dps;imu.version=imu.version.wrapping_add(1);true
    }
    fn report(&self)->String {format!("[papers3] ET047TC1 bw frames={} row={} powered={} GT911=single-contact RTC=calendar-only audio/power-key=absent gray/network=unverified RGB-scans={} SPI-SD={}",self.panel.frames,self.panel.row,self.powered(),self.rgb_frames,self.sd.as_ref().map_or_else(||"absent".into(),|sd|sd.report()))}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spi_sd_needs_correct_clock_mosi_miso_select_and_survives_board_reinitialization() {
        let mut b=PaperS3::new();let pins=SpiPins{sclk:1<<39,mosi:1<<38,miso:Some(40),cs:1<<47};
        assert_eq!(b.spi_transfer_pins(2,pins,&[0x40,0,0,0,0,0x95],0),Vec::<u8>::new());assert_eq!(b.spi_transfer_pins(2,pins,&[],2),[255,255]);
        b.attach_spi_sd_card(Box::new(esp_periph::sdmmc::MemStore(vec![0;1<<20])),Default::default()).unwrap();
        b.spi_transfer_pins(2,SpiPins{cs:0,..pins},&[255;10],0);
        for bad in [SpiPins{sclk:1<<38,..pins},SpiPins{mosi:1<<39,..pins},SpiPins{cs:1<<46,..pins}] {b.spi_transfer_pins(2,bad,&[0x40,0,0,0,0,0x95],0);assert_eq!(b.spi_transfer_pins(2,pins,&[],2),[255,255]);}
        b.spi_transfer_pins(3,pins,&[0x40,0,0,0,0,0x95],0);assert_eq!(b.spi_transfer_pins(3,SpiPins{miso:Some(39),..pins},&[],2),[255,255]);
        b.spi_transfer_pins(3,pins,&[0x40,0,0,0,0,0x95],0);assert_eq!(b.spi_transfer_pins(3,pins,&[],2),[255,1]);
        b.i2c_devices();assert!(b.sd.is_some());assert!(b.sd_card_slot().is_none(),"SPI must not be routed to native SDMMC");
    }
    #[test]
    fn routes_power_and_touch_are_distinct_from_paperdiy() {
        let mut b=PaperS3::new();assert_eq!(b.name(),"papers3");assert_eq!(b.named_pin("power"),None);let mask=(1<<45)|(1<<46)|(1<<18)|(1<<17)|(1<<15);
        b.gpio_output_at(1,&[],mask,1<<46);assert!(!b.powered());b.gpio_output_at(2,&[],mask,(1<<45)|(1<<46));assert!(b.powered());
        let devices=b.i2c_devices();assert!(devices.iter().all(|(_,_,d)|d.pins()==Some((41,42))));assert!(!devices.iter().any(|(_,a,_)|*a==0x6e));assert_eq!(devices.iter().filter(|(_,a,_)|*a==0x51).count(),2);
        b.touch(100,200,true);assert!(b.take_edges().iter().any(|e|e.pin==48&&!e.level));let t=b.touch.lock().unwrap();assert_eq!((t.x,t.y),(100,200));
    }
    #[test]
    fn rtc_calendar_shares_both_controllers_and_board_guest_time() {
        let mut b=PaperS3::new();let mut devices=b.i2c_devices();
        let writer=&mut devices.iter_mut().find(|(bus,address,_)|*bus==0&&*address==0x51).unwrap().2;
        writer.start(false);for byte in [2,0x59,0x59,0x23,0x31,6,0x12,0x99] {assert!(writer.write(byte));}writer.stop();
        b.advance_to(crate::periph::CPU_HZ);
        let reader=&mut devices.iter_mut().find(|(bus,address,_)|*bus==1&&*address==0x51).unwrap().2;
        reader.start(false);reader.write(2);reader.start(true);assert_eq!((0..7).map(|_|reader.read()).collect::<Vec<_>>(),[0,0,0,1,0,0x81,0]);reader.stop();
        // Reattaching adapters must not reset the battery-domain calendar state.
        let mut devices=b.i2c_devices();let reader=&mut devices.iter_mut().find(|(bus,address,_)|*bus==0&&*address==0x51).unwrap().2;
        reader.start(false);reader.write(5);reader.start(true);assert_eq!((0..4).map(|_|reader.read()).collect::<Vec<_>>(),[1,0,0x81,0]);reader.stop();
    }

    fn rgb_route(b:&mut PaperS3,bad:Option<usize>) {
        let mut gpio=esp_periph::Gpio::new();let mut mux=esp_periph::RegRam::new();
        for (p,signal) in [(16,154),(15,151),(13,150),(8,141),(10,142),(9,143),(11,144),(7,145),(12,146),(6,147),(14,148)] {
            gpio.enable|=1<<p;mux.write(4+4*p as u32,1<<12);gpio.func_out_sel[p]=signal|1024;
        }
        if let Some(pin)=bad {gpio.func_out_sel[pin]|=512;}
        b.gpio_waveform_at(0,esp_soc::pins::ChipPins::S3.routes(&gpio,&mux));
    }
    fn rgb_begin(b:&mut PaperS3) {
        let mask=(1<<45)|(1<<46)|(1<<17);b.gpio_output_at(1,&[],mask,mask);b.gpio_output_at(2,&[],mask,mask&!(1<<17));
    }
    #[test]
    fn rgb_epdiy_decodes_dummy_pair_order_padding_and_retained_optical_state() {
        let mut b=PaperS3::new();rgb_route(&mut b,None);rgb_begin(&mut b);
        let mut bytes=vec![0;241*544];bytes[0]=0x55;bytes[1]=0xe4; // dummy ignored; physical x1 black.
        bytes[539*241+240]=0x40; // last source pixel in the last physical row.
        bytes[540*241..].fill(0x55); // rounded DMA rows have no gate electrodes.
        b.lcd_rgb_start(3,241,544,1,&|pin|if pin==18 {Some((5280,546))}else{None});
        assert_eq!(b.panel.frames,0,"START has no invented pixels");b.lcd_frame(241,544,&bytes);
        assert_eq!(b.panel.frames,1);assert_eq!(b.rgb_frames,1);assert_eq!(b.panel.row,540);
        assert_eq!(&b.panel.frame[..4],&[0xffff,0,0xffff,0xffff]);assert_eq!(*b.panel.frame.last().unwrap(),0);
        assert_eq!(b.panel.frame.iter().filter(|&&p|p==0).count(),2);
        let pixels=b.panel.frame.clone();let version=b.panel.version;
        b.lcd_frame(241,544,&vec![0x55;241*544]);assert_eq!(b.panel.version,version,"no new gate scan");
        b.gpio_output_at(4,&[],0,0);assert_eq!(b.panel.frame,pixels,"power loss retains e-paper pixels");
    }
    #[test]
    fn rgb_requires_all_routes_power_scan_and_a_running_gate_clock() {
        let bytes=vec![0x55;241*544];
        for bad in [16,15,13,8,10,9,11,7,12,6,14] {
            let mut b=PaperS3::new();rgb_route(&mut b,Some(bad));rgb_begin(&mut b);b.lcd_rgb_start(3,241,544,1,&|_|Some((5280,546)));b.lcd_frame(241,544,&bytes);assert_eq!(b.panel.frames,0,"inverted GPIO {bad}");
        }
        for bad in 0..8 {
            let mut b=PaperS3::new();rgb_route(&mut b,None);if bad!=0 {rgb_begin(&mut b);}
            let (w,h,bpp)=match bad {1=>(240,544,1),2=>(241,540,1),3=>(241,544,2),_=>(241,544,1)};
            b.lcd_rgb_start(3,w,h,bpp,&|_|match bad {4=>None,5=>Some((5280,543)),_=>Some((5280,546))});
            if bad==6 {b.gpio_output_at(4,&[],0,0);}if bad==7 {rgb_route(&mut b,Some(16));}
            b.lcd_frame(241,544,&bytes);assert_eq!(b.panel.frames,0,"invalid scan {bad}");
        }
    }

}

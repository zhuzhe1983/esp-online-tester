//! AXS5106L one-contact report subset used by Waveshare C6 Touch LCD 1.47.
//! Pinned source: ESP32-C6-Touch-LCD-1.47@baea85be7ac288b42664670a456f9c86df7b5a63,
//! examples/factory/components/esp_lcd_touch_axs5106/esp_lcd_touch_axs5106.c.
//! The driver selects 01h, reads 14 bytes, and decodes count at byte1 and XY at
//! bytes2..5. Unspecified header/pressure/second-contact fields are zero. No
//! identity, calibration, gesture or physical scanning timing is fabricated.
use esp_periph::i2c::I2cDevice;
use std::sync::{Arc,Mutex};

#[derive(Default)]
pub struct Axs5106State {
    powered:bool,down:bool,x:u16,y:u16,pending:bool,event:u64,reset_epoch:u64,
}
impl Axs5106State {
    /// Digital reset line only; the board supplies the actual route/polarity.
    pub fn set_reset(&mut self,high:bool) {
        if !high && self.powered {
            self.down=false;self.pending=false;self.reset_epoch=self.reset_epoch.wrapping_add(1);
            self.event=self.event.wrapping_add(1);
        }
        self.powered=high;
    }
    pub fn update(&mut self,x:u16,y:u16,down:bool) {
        if self.powered {self.x=x.min(4095);self.y=y.min(4095);self.down=down;self.pending=true;self.event=self.event.wrapping_add(1);}
    }
    pub fn irq_high(&self)->bool {!self.powered || !self.pending}
}

pub struct Axs5106 {
    state:Arc<Mutex<Axs5106State>>,selected:bool,first:bool,index:usize,
    report:[u8;14],report_event:u64,reset_epoch:u64,
}
impl Axs5106 {
    pub fn new(state:Arc<Mutex<Axs5106State>>)->Self {Self{state,selected:false,first:true,index:14,report:[0;14],report_event:0,reset_epoch:0}}
    fn sync_reset(&mut self)->bool {
        let s=self.state.lock().expect("AXS5106 state");
        if self.reset_epoch!=s.reset_epoch {self.selected=false;self.index=14;self.reset_epoch=s.reset_epoch;}
        s.powered
    }
}
impl I2cDevice for Axs5106 {
    fn start(&mut self,read:bool)->bool {
        if !self.sync_reset(){return false;}
        if !read {self.first=true;return true;}
        if !self.selected{return false;}
        let s=self.state.lock().expect("AXS5106 state");
        self.report=[0;14];self.report[1]=u8::from(s.down);
        self.report[2]=(s.x>>8)as u8&15;self.report[3]=s.x as u8;
        self.report[4]=(s.y>>8)as u8&15;self.report[5]=s.y as u8;
        self.report_event=s.event;self.index=0;true
    }
    fn write(&mut self,value:u8)->bool {
        if !self.sync_reset()||!self.first{return false;}
        self.first=false;self.selected=value==1;self.selected
    }
    fn read(&mut self)->u8 {
        if !self.sync_reset()||self.index>=14{return 0xff;}
        let value=self.report[self.index];self.index+=1;
        if self.index==14 {
            let mut s=self.state.lock().expect("AXS5106 state");
            if s.event==self.report_event {s.pending=false;}
        }
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn select(d:&mut Axs5106) {assert!(d.start(false));assert!(d.write(1));d.stop();assert!(d.start(true));}
    #[test]
    fn manufacturer_report_offsets_and_release_are_real_contacts() {
        let s=Arc::new(Mutex::new(Axs5106State::default()));let mut d=Axs5106::new(s.clone());
        assert!(!d.start(false));s.lock().unwrap().set_reset(true);assert!(!d.start(true));
        s.lock().unwrap().update(171,319,true);select(&mut d);
        let b:Vec<_>=(0..14).map(|_|d.read()).collect();
        // Same independent decoder as the pinned vendor driver.
        assert_eq!(b[1]&15,1);assert_eq!((u16::from(b[2]&15)<<8)|u16::from(b[3]),171);
        assert_eq!((u16::from(b[4]&15)<<8)|u16::from(b[5]),319);assert!(s.lock().unwrap().irq_high());
        s.lock().unwrap().update(80,110,false);select(&mut d);let b:Vec<_>=(0..14).map(|_|d.read()).collect();assert_eq!(b[1],0);
        assert_eq!(d.read(),0xff);assert!(d.start(false));assert!(!d.write(0xa7));assert!(!d.start(true));
    }
    #[test]
    fn old_report_does_not_consume_new_event_and_reset_invalidates_selection() {
        let s=Arc::new(Mutex::new(Axs5106State::default()));s.lock().unwrap().set_reset(true);let mut d=Axs5106::new(s.clone());
        s.lock().unwrap().update(10,20,true);select(&mut d);assert_eq!(d.read(),0);
        s.lock().unwrap().update(30,40,false);let rest:Vec<_>=(1..14).map(|_|d.read()).collect();assert_eq!(rest[0],1);
        assert!(!s.lock().unwrap().irq_high());select(&mut d);let b:Vec<_>=(0..14).map(|_|d.read()).collect();assert_eq!(&b[1..6],&[0,0,30,0,40]);assert!(s.lock().unwrap().irq_high());
        s.lock().unwrap().set_reset(false);assert!(!d.start(false));s.lock().unwrap().set_reset(true);assert!(!d.start(true));
        select(&mut d);assert!(!d.write(7));
    }
}

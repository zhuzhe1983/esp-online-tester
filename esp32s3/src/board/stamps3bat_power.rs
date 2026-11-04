//! S015's opt-in M5PM1 NeoPixel and button-polling endpoint.
//! M5PM1 315d2899c863ae34b59197392b9f4c59490751ef src/M5PM1.h/.cpp:
//! BTN_STATUS 0x48, GPIO_MODE/DRV/FUNC, NEO_CFG 0x50, RGB565 LE 0x60..9f.
//! The official S3Bat RGB example additionally enables PY_LED_EN and GPIO0.
//! No autonomous click timing, reset/download, analog charging or CPU power-off.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,m5::{M5pm1,M5pm1State}};

#[derive(Debug)]
pub(super) struct Controls {
    pub leds:[[u8;3];1],pub updates:u64,pub pressed:bool,pub flag:bool,
    pub cfg1:u8,pub cfg2:u8,pub presses:u64,pub reads:u64,
}
impl Default for Controls {fn default()->Self {Self{leds:[[0;3];1],updates:1,pressed:false,flag:false,cfg1:0x2a,cfg2:0,presses:0,reads:0}}}
impl Controls {
    pub fn button(&mut self,pressed:bool)->bool {
        // Only polling firmware that disables both autonomous actions is supported.
        // Releasing a previously accepted hold remains safe after a config change.
        if pressed && (self.cfg1&1==0 || self.cfg2&1==0) {return false;}
        if pressed && !self.pressed {self.flag=true;self.presses+=1;}self.pressed=pressed;true
    }
}
pub(super) struct Power {
    base:M5pm1,pmic:Arc<Mutex<M5pm1State>>,pub controls:Arc<Mutex<Controls>>,
    regs:[u8;256],ptr:u8,first:bool,
}
impl Power {
    pub fn new(pmic:Arc<Mutex<M5pm1State>>,controls:Arc<Mutex<Controls>>)->Self {
        *controls.lock().expect("S3Bat controls poisoned")=Controls::default();
        let mut regs=[0;256];regs[6]=7;regs[0x13]=0x1f;regs[0x49]=0x2a;
        Self{base:M5pm1::with_power_config(pmic.clone(),7),pmic,controls,regs,ptr:0,first:true}
    }
    fn sync(&mut self,refresh:bool) {
        let p=self.pmic.lock().expect("S3Bat power poisoned");let powered=p.led_enabled && !p.power_off_requested;
        let mut c=self.controls.lock().expect("S3Bat controls poisoned");c.cfg1=self.regs[0x49];c.cfg2=self.regs[0x4a];
        if !powered {if c.leds!=[[0;3];1] {c.leds=[[0;3];1];c.updates+=1;}return;}
        let count=self.regs[0x50]&0x3f;
        if refresh && (1..=32).contains(&count) && self.regs[0x16]&3==3 && self.regs[0x10]&1!=0 && self.regs[0x13]&1==0 {
            let v=u16::from_le_bytes([self.regs[0x60],self.regs[0x61]]);
            // Full-range RGB565 presentation, not measured optical brightness.
            c.leds[0]=[((v>>11) as u32*255/31) as u8,(((v>>5)&63) as u32*255/63) as u8,((v&31) as u32*255/31) as u8];c.updates+=1;
        }
    }
}
impl I2cDevice for Power {
    fn start(&mut self,read:bool)->bool {self.first= !read;self.base.start(read)}
    fn start_address_with_clock(&mut self,configured:u8,address:u8,read:bool,hz:Option<u32>)->bool {
        if configured!=address {return false;}self.first= !read;self.base.start_address_with_clock(configured,address,read,hz)
    }
    fn write(&mut self,byte:u8)->bool {
        if self.first {self.ptr=byte;self.first=false;return self.base.write(byte);}
        let reg=self.ptr;self.ptr=self.ptr.wrapping_add(1);let ack=self.base.write(byte);
        if reg!=0x48 {self.regs[reg as usize]=byte;}
        let refresh=reg==0x50 && byte&0x40!=0;self.sync(refresh);
        // Digital command completion is immediate; waveform/transfer latency is absent.
        if refresh {self.regs[0x50]&=!0x40;}ack
    }
    fn read(&mut self)->u8 {
        let reg=self.ptr;self.ptr=self.ptr.wrapping_add(1);let base=self.base.read();
        match reg {
            0x48=>{let mut c=self.controls.lock().expect("S3Bat controls poisoned");let v=u8::from(c.pressed)|(u8::from(c.flag)<<7);c.flag=false;c.reads+=1;v},
            0x50=>self.regs[0x50],_=>base,
        }
    }
    fn stop(&mut self) {self.base.stop();}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wr(p:&mut Power,r:u8,values:&[u8]) {assert!(p.start(false));assert!(p.write(r));for v in values {assert!(p.write(*v));}p.stop();}
    fn rd(p:&mut Power,r:u8)->u8 {wr(p,r,&[]);assert!(p.start(true));let v=p.read();p.stop();v}
    #[test]
    fn rgb_requires_supply_gpio_mode_refresh_and_valid_count() {
        let c=Arc::new(Mutex::new(Controls::default()));let mut p=Power::new(Default::default(),c.clone());
        wr(&mut p,0x60,&[0,0xf8]);wr(&mut p,0x50,&[0x41]);assert_eq!(c.lock().unwrap().updates,1);
        wr(&mut p,6,&[0x17]);wr(&mut p,0x50,&[0x41]);assert_eq!(c.lock().unwrap().updates,1);
        for (r,v) in [(0x16,3),(0x10,1),(0x13,0x1e)] {wr(&mut p,r,&[v]);}
        assert_eq!(c.lock().unwrap().leds,[[0;3]]);wr(&mut p,0x50,&[0x41]);assert_eq!(rd(&mut p,0x50),1);assert_eq!(c.lock().unwrap().leds,[[255,0,0]]);
        wr(&mut p,0x60,&[0xe0,7]);assert_eq!(c.lock().unwrap().leds,[[255,0,0]]);
        wr(&mut p,0x50,&[0x7f]);assert_eq!(c.lock().unwrap().leds,[[255,0,0]],"invalid count cannot index beyond the 32-entry buffer");
        wr(&mut p,6,&[7]);assert_eq!(c.lock().unwrap().leds,[[0;3]]);wr(&mut p,6,&[0x17]);assert_eq!(c.lock().unwrap().leds,[[0;3]],"restored rail needs a new refresh");
        wr(&mut p,0x50,&[0x41]);assert_eq!(c.lock().unwrap().leds,[[0,255,0]]);wr(&mut p,0x0c,&[0xa1]);assert_eq!(c.lock().unwrap().leds,[[0;3]],"modeled LED supply stops; CPU shutdown is outside this endpoint");
    }
    #[test]
    fn physical_button_polling_is_read_clear_and_never_an_esp_gpio() {
        let c=Arc::new(Mutex::new(Controls::default()));let mut p=Power::new(Default::default(),c.clone());
        assert!(!c.lock().unwrap().button(true),"autonomous reset/off configuration unsupported");
        wr(&mut p,0x49,&[0x2b,1]);assert!(c.lock().unwrap().button(true));assert_eq!(rd(&mut p,0x48),0x81);assert_eq!(rd(&mut p,0x48),1);
        wr(&mut p,0x48,&[0]);assert_eq!(rd(&mut p,0x48),1,"status is read-only");
        assert!(c.lock().unwrap().button(false));assert_eq!(rd(&mut p,0x48),0);assert!(c.lock().unwrap().button(true));c.lock().unwrap().button(false);assert_eq!(rd(&mut p,0x48),0x80);assert_eq!(rd(&mut p,0x48),0);
    }
}

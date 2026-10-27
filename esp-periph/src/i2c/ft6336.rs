//! FT6336G single-contact register subset. Report layout and vendor register follow
//! FocalTech FT6x36 Application Note (registers 02h–08h/A4h/A5h/A8h), and M5GFX
//! 02107b828acb3eb782fccd747863638d64c876d7 Touch_FT5x06.cpp. Firmware/chip revision
//! IDs are deliberately zero (unknown), not asserted as measured on this panel.
use super::{I2cDevice, Reg8Device};
use std::sync::{Arc,Mutex};

#[derive(Default)]
pub struct Ft6336State { pub powered: bool, pub down: bool, pub x: u16, pub y: u16, pending_release: bool, reads: u8 }
impl Ft6336State {
    pub fn update(&mut self,x:u16,y:u16,down:bool) {
        self.x=x.min(4095);self.y=y.min(4095);
        if down {self.down=true;self.pending_release=false;self.reads=0;}
        else if self.reads>=2 {self.down=false;} else {self.pending_release=true;}
    }
    pub fn reset(&mut self) {self.down=false;self.pending_release=false;self.reads=0;}
}
pub struct Ft6336g {registers:Reg8Device,state:Arc<Mutex<Ft6336State>>}
impl Ft6336g {
    pub fn new(state:Arc<Mutex<Ft6336State>>) -> Self {Self{registers:Reg8Device::new("ft6336g",&[(0xa8,0x11)]),state}}
    /// FT3267 uses the same FT5x06 single-contact register report on M5Dial.
    /// Only this common subset is modeled; undocumented chip/revision IDs remain zero.
    pub fn ft3267(state:Arc<Mutex<Ft6336State>>) -> Self {Self{registers:Reg8Device::new("ft3267",&[(0xa8,0x11)]),state}}
}
impl I2cDevice for Ft6336g {
    fn start(&mut self,read:bool) -> bool {self.state.lock().expect("FT6336 state mutex poisoned").powered && self.registers.start(read)}
    fn write(&mut self,byte:u8) -> bool {
        if !self.registers.first && matches!(self.registers.ptr,0x01..=0x08|0xa3|0xa6|0xa8) {self.registers.ptr=self.registers.ptr.wrapping_add(1);return true;}
        self.registers.write(byte)
    }
    fn read(&mut self) -> u8 {
        let mut state=self.state.lock().expect("FT6336 state mutex poisoned");
        if self.registers.ptr==2 {
            if state.pending_release && state.reads>=2 {state.reset();}
            if state.down {state.reads=state.reads.saturating_add(1);}
        }
        let visible=state.powered && state.down && self.registers.regs[0xa5]!=3;
        self.registers.regs[2]=u8::from(visible);
        self.registers.regs[3]=0x80|((state.x>>8) as u8 & 15);self.registers.regs[4]=state.x as u8;
        self.registers.regs[5]=(state.y>>8) as u8 & 15;self.registers.regs[6]=state.y as u8;
        self.registers.regs[7]=u8::from(visible);self.registers.regs[8]=0;
        self.registers.read()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn read(device:&mut Ft6336g,reg:u8,n:usize)->Vec<u8> {assert!(device.start(false));device.write(reg);assert!(device.start(true));(0..n).map(|_|device.read()).collect()}
    #[test]
    fn touch_report_is_stable_for_driver_double_read_and_release() {
        let state=Arc::new(Mutex::new(Ft6336State{powered:true,..Default::default()}));let mut device=Ft6336g::new(state.clone());
        assert_eq!(read(&mut device,0xa3,6),[0,0,0,0,0,0x11]);
        state.lock().unwrap().update(479,799,true);state.lock().unwrap().update(479,799,false);
        assert_eq!(read(&mut device,2,5),[1,0x81,0xdf,3,0x1f]);assert_eq!(read(&mut device,2,5),[1,0x81,0xdf,3,0x1f]);assert_eq!(read(&mut device,2,1),[0]);
        state.lock().unwrap().powered=false;assert!(!device.start(true));
    }
}

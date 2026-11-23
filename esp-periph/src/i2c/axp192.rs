//! AXP192 power-IC digital subset for the M5Stack Core2 family.
//! Register map, bit assignments and PEKEY interrupt semantics: lewisxhe/XPowersLib master
//! 17132a58605034281a47c4015f0d4fe08de5ac57 — src/AXP192Constants.h and src/XPowersAXP192.hpp.
//! The cached M5Stack AXP192 datasheet PDF (4 pages) carries no register table, so the library
//! is the register authority for this lane and that is recorded in the evidence sources.
//! Not modelled: ADC conversions, battery/charger analog loops, coulomb counting and the PMU IRQ
//! pin. Voltage-measurement registers (0x26, 0x56..0x79, 0xB0..0xB8) read 0 so a guest can never
//! infer a battery percentage from the simulation.
use super::I2cDevice;
use std::sync::{Arc,Mutex};

pub const AXP192_ADDRESS: u8 = 0x34;
/// 0x12 LDO23_DC123_EXT_CTL (AXP192Constants.h L21): bit0 DC1, bit1 DC3, bit2 LDO2,
/// bit3 LDO3, bit4 DC2, bit6 EXTEN — confirmed against isEnableDC1/DC3/LDO2/LDO3/DC2 and
/// enableExternalPin in XPowersAXP192.hpp (L712/852/611/655/795/895).
pub const REG_CTL: u8 = 0x12;
/// 0x32 OFF_CTL bit7 = shutdown: "turn off all power channels, only VRTC" (XPowersAXP192.hpp L364).
pub const REG_OFF: u8 = 0x32;
/// 0x36 POK_SET: bits7:6 press-on time, bits5:4 long-press time, bit3 long-press power-off enable,
/// bits1:0 press-off time (XPowersAXP192.hpp L1812/1854/1835).
pub const REG_POK: u8 = 0x36;
pub const REG_INTEN3: u8 = 0x42;
/// 0x46 INTSTS3: bit1 PEKEY short press, bit0 PEKEY long press (XPowersAXP192.hpp L1063/1068).
pub const REG_INTSTS3: u8 = 0x46;
pub const PEKEY_SHORT: u8 = 2;
pub const PEKEY_LONG: u8 = 1;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct Axp192Rails { pub dc1:bool, pub dc2:bool, pub dc3:bool, pub ldo2:bool, pub ldo3:bool, pub exten:bool }

pub struct Axp192State {
    pub regs:[u8;256],
    pub power_off_requested:bool,
    panel_reset_count:u64,
    clock_hz:u64,
    pressed:Option<u64>,
    long_sent:bool,
}

impl Axp192State {
    /// Cold reset profile. Channel enables are left off: the Core2 guest firmware has to write
    /// 0x12 (and the panel/audio rails) before the display or touch answers. 0x03 IC_TYPE is
    /// read-only and reports 0x03, the AXP192 code XPowersLib compares against.
    pub fn new(clock_hz:u64)->Self {let mut s=Self{regs:[0;256],power_off_requested:false,panel_reset_count:0,clock_hz:clock_hz.max(1),pressed:None,long_sent:false};s.regs[3]=3;s}
    pub fn rails(&self)->Axp192Rails {let c=self.regs[usize::from(REG_CTL)];Axp192Rails{dc1:c&1!=0,dc3:c&2!=0,ldo2:c&4!=0,ldo3:c&8!=0,dc2:c&16!=0,exten:c&64!=0}}
    /// Panel 3V3 (LDO2) and backlight (DC3) are only up while the PMIC is not shut down.
    pub fn panel_powered(&self)->bool {!self.power_off_requested && self.rails().ldo2}
    pub fn backlight_on(&self)->bool {!self.power_off_requested && self.rails().ldo2 && self.rails().dc3}
    pub fn vibration_on(&self)->bool {!self.power_off_requested && self.rails().ldo3}
    pub fn external_5v_on(&self)->bool {!self.power_off_requested && self.rails().exten}
    pub fn ldo2_mv(&self)->u16 {1800+u16::from((self.regs[0x28]>>4)&0xf)*100}
    pub fn ldo3_mv(&self)->u16 {1800+u16::from(self.regs[0x28]&0xf)*100}
    pub fn dc3_mv(&self)->u16 {700+u16::from(self.regs[0x27])*25}
    /// M5Core2 67099371 AXP192.cpp::SetLCDRSet and M5Unified AXP192_Class::_set_GPIO3_4:
    /// these outputs use 96h[1:0]; 95h selects open-drain mode.
    pub fn gpio34_high(&self,io:u8)->bool {
        let Some(index)=io.checked_sub(3).filter(|&index|index<2) else {return false;};
        (self.regs[0x95]>>(index*2))&3==1 && self.regs[0x96]&(1<<index)!=0
    }
    pub fn panel_ready(&self)->bool {self.panel_powered()&&self.gpio34_high(4)}
    /// Count reset assertions even if the next LCD transaction comes after release.
    pub fn panel_reset_count(&self)->u64 {self.panel_reset_count}
    /// AXP192 GPIO output level through XPowersLib's `digitalWrite`:
    /// IO0 90h bit0, IO1 92h bit0, IO2 93h bit0, IO5 9Eh bit5. IO3/4 use the board-proven mapping above.
    pub fn io_output(&self,io:u8)->Option<bool> {
        let regs=&self.regs;
        Some(match io {
            0=>regs[0x90]&1!=0,
            1=>regs[0x92]&1!=0,
            2=>regs[0x93]&1!=0,
            3|4=>self.gpio34_high(io),
            5=>regs[0x9e]&32!=0,
            _=>return None,
        })
    }
    /// PWM1 drives AXP_IO1, PWM2 drives AXP_IO2 (0x98..0x9a / 0x9b..0x9d): duty = hi<<4 | lo/16.
    pub fn io_pwm_duty(&self,io:u8)->Option<u16> {
        let (freq,d1,d2): (u8,u8,u8) = match io {1=>(0x98,0x99,0x9a),2=>(0x9b,0x9c,0x9d),_=>return None};
        if self.regs[usize::from(freq)]&3==0 {return Some(0);}
        Some((u16::from(self.regs[usize::from(d1)])<<4)|(u16::from(self.regs[usize::from(d2)])>>4))
    }
    /// Any unmasked interrupt status: INTEN1..5 (0x40,0x41,0x42,0x43,0x4A) gate INTSTS1..5
    /// (0x44,0x45,0x46,0x47,0x4D). Reading does not clear; `clearIrqStatus` writes 0xFF.
    pub fn irq(&self)->bool {
        [(0x40u8,0x44u8),(0x41,0x45),(0x42,0x46),(0x43,0x47),(0x4a,0x4d)]
            .iter().any(|&(en,st)|self.regs[usize::from(en)]&self.regs[usize::from(st)]!=0)
    }
    fn press_on_ticks(&self)->u64 {
        // bits7:6 select 128 ms / 512 ms / 1 s / 2 s (xpowers_press_on_time_t).
        let ms=[128u64,512,1000,2000][usize::from((self.regs[usize::from(REG_POK)]>>6)&3)];
        (u128::from(ms)*u128::from(self.clock_hz)).div_ceil(1000) as u64
    }
    pub fn next_deadline(&self)->Option<u64> {self.pressed.filter(|_|!self.long_sent).map(|t|t.saturating_add(self.press_on_ticks()))}
    pub fn advance_to(&mut self,cycle:u64) {
        if self.next_deadline().is_some_and(|t|t<=cycle) {self.regs[usize::from(REG_INTSTS3)]|=PEKEY_LONG;self.long_sent=true;}
    }
    /// PEKEY (PWRON/HPWR) press and release. A release before the press-on time reports a short
    /// press; after it, only the long-press status bit is set.
    pub fn button(&mut self,cycle:u64,pressed:bool) {
        self.advance_to(cycle);
        if pressed && self.pressed.is_none() {self.pressed=Some(cycle);self.long_sent=false;}
        if !pressed && self.pressed.take().is_some() && !self.long_sent {self.regs[usize::from(REG_INTSTS3)]|=PEKEY_SHORT;}
    }
    fn read_register(&mut self,reg:u8)->u8 {
        match reg {
            3=>3,
            0x26|0x56..=0x5d|0x62..=0x6b|0x78|0x79|0xb0..=0xb8=>0,
            _=>self.regs[usize::from(reg)],
        }
    }
    fn write_register(&mut self,reg:u8,value:u8) {
        let ready=self.panel_ready();
        match reg {
            3|0x26|0x56..=0x5d|0x62..=0x6b|0x78|0x79|0xb0..=0xb8=>{},
            0x44..=0x47|0x4d=>self.regs[usize::from(reg)]&=!value,
            REG_OFF=>{self.regs[usize::from(REG_OFF)]=value;self.power_off_requested|=value&128!=0;}
            0..=0x02|0x06..=0x0a|0x12|0x23..=0x25|0x27..=0x29|0x30..=0x37|0x40..=0x43|0x4a
            |0x80..=0x9e=>self.regs[usize::from(reg)]=value,
            _=>{},
        }
        if ready && !self.panel_ready() {self.panel_reset_count=self.panel_reset_count.wrapping_add(1);}
    }
}

pub struct Axp192 {pub state:Arc<Mutex<Axp192State>>,pins:(u8,u8),pointer:u8,address:bool}
impl Axp192 {
    pub fn new(state:Arc<Mutex<Axp192State>>)->Self {Self::with_pins(state,(21,22))}
    pub fn with_pins(state:Arc<Mutex<Axp192State>>,pins:(u8,u8))->Self {Self{state,pins,pointer:0,address:true}}
}
impl I2cDevice for Axp192 {
    fn pins(&self)->Option<(u8,u8)> {Some(self.pins)}
    /// A shut-down PMIC answers no transaction, matching the Core2 rails going dead.
    fn start(&mut self,read:bool)->bool {if !read {self.address=true;} !self.state.lock().unwrap().power_off_requested}
    fn write(&mut self,value:u8)->bool {
        if self.address {self.pointer=value;self.address=false;}else {self.state.lock().unwrap().write_register(self.pointer,value);self.pointer=self.pointer.wrapping_add(1);}
        true
    }
    fn read(&mut self)->u8 {let value=self.state.lock().unwrap().read_register(self.pointer);self.pointer=self.pointer.wrapping_add(1);value}
}

#[cfg(test)] mod tests {
    use super::*;
    const HZ:u64=240_000_000;
    #[test] fn rails_follow_the_0x12_bit_map() {
        let mut p=Axp192State::new(HZ);
        assert!(!p.panel_powered()&&!p.backlight_on()&&!p.vibration_on()&&!p.external_5v_on());
        p.write_register(0x12,0x5f);
        let r=p.rails();
        assert!(r.dc1&&r.dc2&&r.dc3&&r.ldo2&&r.ldo3&&r.exten);
        assert!(p.panel_powered()&&p.backlight_on()&&p.vibration_on()&&p.external_5v_on());
        p.write_register(0x12,0x47);assert!(p.panel_powered()&&p.backlight_on()&&!p.vibration_on(),"bit3 is LDO3, not LDO2");
        p.write_register(0x12,0x4b);assert!(!p.panel_powered()&&!p.backlight_on(),"LDO2 gates both the panel rail and the backlight");
    }
    #[test] fn voltage_codes_match_xpowers_ranges() {
        let mut p=Axp192State::new(HZ);
        p.write_register(0x28,0xff);p.write_register(0x27,0x50);
        assert_eq!((p.ldo2_mv(),p.ldo3_mv(),p.dc3_mv()),(3300,3300,2_700));
        p.write_register(0x28,0);assert_eq!(p.ldo2_mv(),1800);
    }
    #[test] fn shutdown_clears_rails_and_nacks_the_bus() {
        let state=Arc::new(Mutex::new(Axp192State::new(HZ)));
        state.lock().unwrap().write_register(0x12,0x07);
        assert!(state.lock().unwrap().backlight_on());
        let mut dev=Axp192::new(state.clone());
        assert!(dev.start(false));
        assert!(dev.write(0x32)&&dev.write(0x80));
        assert!(!dev.start(false)&&!dev.start(true));
        assert!(state.lock().unwrap().power_off_requested&&!state.lock().unwrap().backlight_on());
    }
    #[test] fn pekey_short_and_long_press_are_masked_then_write_one_cleared() {
        let mut p=Axp192State::new(HZ);
        p.button(0,true);let at=p.next_deadline().unwrap();
        assert_eq!(at,HZ*128/1000,"default press-on time is 128 ms");
        p.button(at-1,false);
        assert_eq!(p.regs[0x46]&3,PEKEY_SHORT);
        assert!(!p.irq(), "no INTEN3 bits yet");
        p.write_register(0x42,3);assert!(p.irq());
        p.write_register(0x46,0xff);assert!(!p.irq()&&p.regs[0x46]==0);
        p.button(HZ,true);p.advance_to(HZ+at);
        assert_eq!(p.regs[0x46]&3,PEKEY_LONG);
        p.button(HZ+at,false);assert_eq!(p.regs[0x46]&3,PEKEY_LONG,"a long press never reports short");
    }
    #[test] fn press_on_time_follows_pok_set() {
        let mut p=Axp192State::new(HZ);
        for (code,ms) in [(0u8,128u64),(1,512),(2,1000),(3,2000)] {
            p.write_register(0x36,code<<6);p.pressed=None;p.long_sent=false;
            p.button(0,true);
            assert_eq!(p.next_deadline().unwrap(),(u128::from(ms)*u128::from(HZ)).div_ceil(1000) as u64);
        }
    }
    #[test] fn adc_and_coulomb_registers_read_zero_and_are_not_writable() {
        let mut p=Axp192State::new(HZ);
        for reg in [0x26u8,0x56,0x64,0x78,0xb0,0xb8] {p.write_register(reg,0xff);assert_eq!(p.read_register(reg),0,"register {reg:#04x} must not report a fake measurement");}
    }
    #[test] fn gpio_outputs_follow_the_board_driver_mode_and_signal_bits() {
        let mut p=Axp192State::new(HZ);
        p.write_register(0x90,1);p.write_register(0x93,1);p.write_register(0x96,0x18);
        assert!(!p.gpio34_high(4));
        p.write_register(0x95,0x85);p.write_register(0x96,3);
        assert_eq!(p.io_output(0),Some(true));assert_eq!(p.io_output(2),Some(true));
        assert_eq!((p.io_output(3),p.io_output(4)),(Some(true),Some(true)));
        assert_eq!((p.io_output(1),p.io_output(6)),(Some(false),None));
        p.write_register(0x12,4);assert!(p.panel_ready());
        p.write_register(0x96,1);p.write_register(0x96,3);assert_eq!(p.panel_reset_count(),1);
        p.write_register(0x12,0);assert_eq!(p.panel_reset_count(),2);
    }
    #[test] fn pwm1_duty_is_io1_and_pwm2_duty_is_io2() {
        let mut p=Axp192State::new(HZ);
        p.write_register(0x98,1);p.write_register(0x99,0x80);p.write_register(0x9a,0xf0);
        assert_eq!(p.io_pwm_duty(1),Some(0x80f));
        assert_eq!(p.io_pwm_duty(2),Some(0));
        p.write_register(0x9b,2);p.write_register(0x9c,0x10);p.write_register(0x9d,0x80);
        assert_eq!(p.io_pwm_duty(2),Some(0x108));
    }
}

//! AXP192 digital subsets for the M5Stack Core2/Tough and Stick boards.
//! Core2/Tough register and PEKEY semantics follow XPowersLib 17132a586:
//! src/AXP192Constants.h and src/XPowersAXP192.hpp. Their ADC and coulomb counters read zero.
//! StickC/Plus add virtual voltage and chip-temperature ADC channels (X-Powers AXP192 v1.1,
//! sections 9.7, 9.11.4–6, 9.11.19, 9.11.31–33 and M5StickC-Plus AXP192.cpp).
//! StickT2 models polled controls and short PEK events. All Stick boards start with a powered
//! test fixture. Battery/charger analog dynamics and the physical IRQ pin are unmodeled.
use super::I2cDevice;
use std::sync::{Arc,Mutex};

pub const AXP192_ADDRESS: u8 = 0x34;
/// 0x12 LDO23_DC123_EXT_CTL (AXP192Constants.h L21): bit0 DC1, bit1 DC3, bit2 LDO2,
/// bit3 LDO3, bit4 DC2, bit6 EXTEN — confirmed against isEnableDC1/DC3/LDO2/LDO3/DC2 and
/// enableExternalPin in XPowersAXP192.hpp (L712/852/611/655/795/895).
pub const REG_CTL: u8 = 0x12;
/// 0x32 OFF_CTL bit7 = shutdown: "turn off all power channels, only VRTC" (XPowersAXP192.hpp L364).
pub const REG_OFF: u8 = 0x32;
/// 0x36 POK_SET: bits7:6 power-on time, bits5:4 long-press time, bit3 long-press power-off enable,
/// bits1:0 press-off time (XPowersAXP192.hpp L1812/1854/1835).
pub const REG_POK: u8 = 0x36;
pub const REG_INTEN3: u8 = 0x42;
/// 0x46 INTSTS3: bit1 PEKEY short press, bit0 PEKEY long press (XPowersAXP192.hpp L1063/1068).
pub const REG_INTSTS3: u8 = 0x46;
pub const PEKEY_SHORT: u8 = 2;
pub const PEKEY_LONG: u8 = 1;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct Axp192Rails { pub dc1:bool, pub dc2:bool, pub dc3:bool, pub ldo2:bool, pub ldo3:bool, pub exten:bool }

/// Simulator room-temperature stimulus, NOT a sensor reading or silicon reset value.
pub const DEFAULT_CHIP_TEMPERATURE_MC: i32 = 25_000;

/// External virtual stimuli, independent of guest-written ADC/control registers.
/// Voltages are unsigned microvolts; the 12-bit ADC saturates, without wrapping.
/// Presence/valid/charging flags are explicit stimuli, not inferred analog thresholds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Axp192PowerInput {
    pub battery_voltage_uv: u32,
    pub battery_present: bool,
    pub vbus_voltage_uv: u32,
    pub vbus_present: bool,
    pub vbus_valid: bool,
    /// Charge-in-progress stimulus; additionally gated by battery/VBUS presence,
    /// VBUS validity and guest charger-enable bit 0x33[7]. No charge-current model.
    pub charging: bool,
}
impl Axp192PowerInput {
    /// Sufficient-charge simulator fixture. Not a battery estimate or chip reset value.
    pub const BATTERY_4V1: Self = Self {
        battery_voltage_uv: 4_100_000,
        battery_present: true,
        vbus_voltage_uv: 0,
        vbus_present: false,
        vbus_valid: false,
        charging: false,
    };
    /// Reproduces the factory's real <3.2 V WARNING22 branch.
    pub const LOW_BATTERY_3V0: Self = Self {
        battery_voltage_uv: 3_000_000,
        ..Self::BATTERY_4V1
    };
}
impl Default for Axp192PowerInput {
    fn default() -> Self {
        Self::BATTERY_4V1
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Profile {
    Core2,
    StickC,
    StickT2,
}

pub struct Axp192State {
    profile: Profile,
    pub version: u64,
    // Stick boards use a polled PEK and report a short release without hold timing.
    key_down: bool,
    power_input: Axp192PowerInput,
    chip_temperature_mc: i32,
    now_us: u64,
    next_adc_us: u64,
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
    pub fn new(clock_hz: u64) -> Self {
        let mut regs = [0; 256];
        regs[3] = 3;
        Self {
            profile: Profile::Core2,
            regs,
            power_off_requested: false,
            panel_reset_count: 0,
            clock_hz: clock_hz.max(1),
            pressed: None,
            long_sent: false,
            version: 0,
            key_down: false,
            power_input: Axp192PowerInput::default(),
            chip_temperature_mc: DEFAULT_CHIP_TEMPERATURE_MC,
            now_us: 0,
            next_adc_us: 0,
        }
    }

    /// StickC and StickC Plus share the powered fixture with deterministic ADC sampling.
    pub fn stickc() -> Self {
        Self::with_power_input(Axp192PowerInput::default())
    }

    pub fn with_power_input(power_input: Axp192PowerInput) -> Self {
        let mut state = Self::new(1);
        state.profile = Profile::StickC;
        state.power_input = power_input;
        state.reset_powered_fixture();
        state
    }

    /// StickT2 starts with the display rails on and polled PEK status.
    pub fn stickt2() -> Self {
        let mut state = Self::new(1);
        state.profile = Profile::StickT2;
        state.regs[3] = 0;
        state.regs[0x12] = 0x4d;
        state.regs[0x28] = 0xcc;
        state.version = 1;
        state
    }


    /// Reset emulated registers/ADC latches/PEK to the powered-board fixture,
    /// preserving the external virtual power/temperature inputs and clock. This models a board after startup.
    pub fn reset_powered_fixture(&mut self) {
        if self.profile != Profile::StickC {
            return;
        }
        self.regs = [0; 256];
        self.regs[0x12] = 0x4d;
        self.regs[0x28] = 0xcc;
        // Documented defaults for the modeled charger/ADC controls, not all registers.
        self.regs[0x33] = 0xc8;
        self.regs[0x82] = 0x83;
        self.regs[0x83] = 0x80;
        self.regs[0x84] = 0x32;
        self.key_down = false;
        self.update_power_status();
        self.next_adc_us = self.now_us.saturating_add(self.adc_period_us());
        self.version = self.version.wrapping_add(1);
    }
    /// The configured virtual power stimulus for the StickC/Plus fixture.
    pub fn power_input(&self) -> Axp192PowerInput {
        self.power_input
    }
    /// Advance to the stimulus time BEFORE replacing the input. ADC values change
    /// only at the next virtual conversion; disabled channels retain their latch.
    pub fn set_power_input(&mut self, input: Axp192PowerInput) {
        if self.profile == Profile::StickC && self.power_input != input {
            self.power_input = input;
            self.update_power_status();
            self.version = self.version.wrapping_add(1);
        }
    }
    /// External virtual PMIC die temperature, in signed milli-Celsius, Applies to the die ADC.
    pub fn chip_temperature_mc(&self) -> i32 { self.chip_temperature_mc }
    /// Advance to the stimulus timestamp before calling, as for set_power_input.
    /// Keeps the requested i32 stimulus; the ADC alone floors/saturates to 12 bits.
    /// Does not touch guest registers or draw a screen; next enabled sample latches it.
    pub fn set_chip_temperature_mc(&mut self, mc: i32) {
        if self.profile == Profile::StickC && self.chip_temperature_mc != mc {
            self.chip_temperature_mc = mc;
            self.version = self.version.wrapping_add(1);
        }
    }
    pub fn adc_sample_hz(&self) -> u16 {
        25 << (self.regs[0x84] >> 6)
    }
    fn adc_period_us(&self) -> u64 {
        1_000_000 / u64::from(self.adc_sample_hz())
    }
    /// Board integration must call this using guest time, never wall time.
    /// A deterministic common conversion phase models 25/50/100/200 Hz. Disabling
    /// freezes the last value (explicit model policy; datasheet gives no latch rule).
    /// Backward time is ignored: reconstruct/reset a board to restart the clock.
    pub fn advance_to_us(&mut self, us: u64) {
        if self.profile != Profile::StickC || us < self.now_us {
            return;
        }
        self.now_us = us;
        if us < self.next_adc_us {
            return;
        }
        let old_battery = [self.regs[0x78], self.regs[0x79]];
        let old_vbus = [self.regs[0x5a], self.regs[0x5b]];
        let old_temperature = [self.regs[0x5e], self.regs[0x5f]];
        if self.regs[0x82] & 0x80 != 0 {
            let uv = if self.power_input.battery_present {
                self.power_input.battery_voltage_uv
            } else {
                0
            };
            self.sample_voltage(0x78, uv, 1100);
        }
        if self.regs[0x82] & 0x08 != 0 {
            let uv = if self.power_input.vbus_present {
                self.power_input.vbus_voltage_uv
            } else {
                0
            };
            self.sample_voltage(0x5a, uv, 1700);
        }
        if self.regs[0x83] & 0x80 != 0 {
            // AXP192 v1.1 ADC table: -144.7 C + raw*0.1 C, 000..FFF.
            // Exact signed integer arithmetic avoids f32 boundary/overflow artifacts.
            let raw = ((i64::from(self.chip_temperature_mc) + 144_700) / 100)
                .clamp(0, 0xfff) as u16;
            self.regs[0x5e] = (raw >> 4) as u8;
            self.regs[0x5f] = (raw & 15) as u8;
        }
        if old_battery != [self.regs[0x78], self.regs[0x79]]
            || old_vbus != [self.regs[0x5a], self.regs[0x5b]]
            || old_temperature != [self.regs[0x5e], self.regs[0x5f]]
        {
            self.version = self.version.wrapping_add(1);
        }
        // Skip redundant conversions at constant input; preserve the sample phase.
        let period = self.adc_period_us();
        let remainder = (us - self.next_adc_us) % period;
        self.next_adc_us = us.saturating_add(period - remainder);
    }
    fn sample_voltage(&mut self, high: usize, uv: u32, lsb_uv: u32) {
        // Floor quantization is an explicit ideal ADC policy, no noise/offset model.
        let raw = (uv / lsb_uv).min(0xfff) as u16;
        self.regs[high] = (raw >> 4) as u8;
        self.regs[high + 1] = (raw & 0xf) as u8;
    }
    fn update_power_status(&mut self) {
        let i = self.power_input;
        let valid = i.vbus_present && i.vbus_valid;
        let charging = i.charging && i.battery_present && valid && self.regs[0x33] & 0x80 != 0;
        self.regs[0] =
            (u8::from(i.vbus_present) << 5) | (u8::from(valid) << 4) | (u8::from(charging) << 2);
        self.regs[1] = (u8::from(i.battery_present) << 5) | (u8::from(charging) << 6);
    }

    pub fn rails(&self)->Axp192Rails {let c=self.regs[usize::from(REG_CTL)];Axp192Rails{dc1:c&1!=0,dc3:c&2!=0,ldo2:c&4!=0,ldo3:c&8!=0,dc2:c&16!=0,exten:c&64!=0}}
    /// Panel 3V3 (LDO2) and backlight (DC3) are only up while the PMIC is not shut down.
    pub fn panel_powered(&self)->bool {!self.power_off_requested && self.rails().ldo2}
    pub fn backlight_on(&self)->bool {!self.power_off_requested && self.rails().ldo2 && self.rails().dc3}
    pub fn vibration_on(&self)->bool {!self.power_off_requested && self.rails().ldo3}
    pub fn external_5v_on(&self)->bool {!self.power_off_requested && self.rails().exten}
    pub fn ldo_enabled(&self, ldo: u8) -> bool {
        !self.power_off_requested && match ldo {
            2 => self.rails().ldo2,
            3 => self.rails().ldo3,
            _ => false,
        }
    }
    pub fn ldo_mv(&self, ldo: u8) -> u16 {
        match ldo {
            2 if self.ldo_enabled(ldo) => self.ldo2_mv(),
            3 if self.ldo_enabled(ldo) => self.ldo3_mv(),
            _ => 0,
        }
    }
    pub fn ldo2_mv(&self)->u16 {1800+u16::from((self.regs[0x28]>>4)&0xf)*100}
    pub fn ldo3_mv(&self)->u16 {1800+u16::from(self.regs[0x28]&0xf)*100}
    pub fn dc3_mv(&self)->u16 {700+u16::from(self.regs[0x27]&0x7f)*25}
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
    fn long_press_ticks(&self)->u64 {
        // XPowersAXP192.hpp xpowers_axp192_pekey_long_press_t and
        // setPowerKeyLongPressOnTime: POK_SET [5:4], separate from power-on [7:6].
        let ms=[1000u64,1500,2000,2500][usize::from((self.regs[usize::from(REG_POK)]>>4)&3)];
        (u128::from(ms)*u128::from(self.clock_hz)).div_ceil(1000) as u64
    }
    pub fn next_deadline(&self)->Option<u64> {self.pressed.filter(|_|!self.long_sent).map(|t|t.saturating_add(self.long_press_ticks()))}
    pub fn advance_to(&mut self,cycle:u64) {
        if self.next_deadline().is_some_and(|t|t<=cycle) {self.regs[usize::from(REG_INTSTS3)]|=PEKEY_LONG;self.long_sent=true;}
    }
    /// PEKEY (PWRON/HPWR) press and release. A release before the long-press threshold reports a short
    /// press; after it, only the long-press status bit is set.
    pub fn button(&mut self,cycle:u64,pressed:bool) {
        if self.profile != Profile::Core2 {return;}
        self.advance_to(cycle);
        if pressed && self.pressed.is_none() {self.pressed=Some(cycle);self.long_sent=false;}
        if !pressed && self.pressed.take().is_some() && !self.long_sent {self.regs[usize::from(REG_INTSTS3)]|=PEKEY_SHORT;}
    }
    /// Stick PEK reports a short release via IRQ3[1]; the board polls this status over I2C.
    pub fn set_key(&mut self, pressed: bool) {
        if self.profile == Profile::Core2 {
            return;
        }
        if self.key_down && !pressed {
            self.regs[usize::from(REG_INTSTS3)] |= PEKEY_SHORT;
            self.version = self.version.wrapping_add(1);
        }
        self.key_down = pressed;
    }

    pub fn register(&self, reg: u8) -> u8 {
        match (self.profile, reg) {
            (Profile::Core2, 3) => 3,
            (Profile::Core2, 0x56..=0x7f | 0xb0..=0xb8) => 0,
            _ => self.regs[usize::from(reg)],
        }
    }
    fn write_register(&mut self,reg:u8,value:u8) {
        if self.profile != Profile::Core2 {
            self.write_stick_register(reg, value);
            return;
        }
        let ready=self.panel_ready();
        match reg {
            3|0x56..=0x7f|0xb0..=0xb8=>{},
            0x44..=0x47|0x4d=>self.regs[usize::from(reg)]&=!value,
            REG_OFF=>{self.regs[usize::from(REG_OFF)]=value;self.power_off_requested|=value&128!=0;}
            0..=0x02|0x06..=0x0a|0x12|0x23..=0x29|0x30..=0x37|0x40..=0x43|0x4a
            |0x80..=0x9e=>self.regs[usize::from(reg)]=value,
            _=>{},
        }
        if ready && !self.panel_ready() {self.panel_reset_count=self.panel_reset_count.wrapping_add(1);}
    }

    fn write_stick_register(&mut self, reg: u8, value: u8) {
        let index = usize::from(reg);
        match reg {
            0x44..=0x47 | 0x4d => self.regs[index] &= !value,
            0x83 if self.profile == Profile::StickC => self.regs[index] = value & 0x8f,
            0x84 if self.profile == Profile::StickC => {
                self.regs[index] = value & 0xf7;
                self.next_adc_us = self.now_us.saturating_add(self.adc_period_us());
            }
            0x10 | 0x12 | 0x23 | 0x25..=0x28 | 0x30..=0x3a | 0x40..=0x43 | 0x4a
            | 0x80..=0x84 | 0x8a | 0x90..=0x96 | 0x9d | 0xb8 => self.regs[index] = value,
            _ => return,
        }
        if reg == 0x33 && self.profile == Profile::StickC {
            self.update_power_status();
        }
        self.version = self.version.wrapping_add(1);
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
    fn read(&mut self)->u8 {let value=self.state.lock().unwrap().register(self.pointer);self.pointer=self.pointer.wrapping_add(1);value}
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
        p.write_register(0x28,0xff);p.write_register(0x27,0xd0);
        p.write_register(0x26,0x6a);assert_eq!(p.register(0x26),0x6a);
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
        assert_eq!(at,HZ,"default held-key threshold is 1 second");
        p.button(at-1,false);
        assert_eq!(p.regs[0x46]&3,PEKEY_SHORT);
        assert!(!p.irq(), "no INTEN3 bits yet");
        p.write_register(0x42,3);assert!(p.irq());
        p.write_register(0x46,0xff);assert!(!p.irq()&&p.regs[0x46]==0);
        p.button(HZ,true);p.advance_to(HZ+at);
        assert_eq!(p.regs[0x46]&3,PEKEY_LONG);
        p.button(HZ+at,false);assert_eq!(p.regs[0x46]&3,PEKEY_LONG,"a long press never reports short");
    }
    #[test] fn long_key_threshold_uses_only_pok_set_bits_five_and_four() {
        let mut p=Axp192State::new(HZ);
        for (code,ms) in [(0u8,1000u64),(1,1500),(2,2000),(3,2500)] {
            p.write_register(0x36,(code<<4)|0xc0);p.pressed=None;p.long_sent=false;
            p.button(0,true);
            assert_eq!(p.next_deadline().unwrap(),(u128::from(ms)*u128::from(HZ)).div_ceil(1000) as u64);
        }
    }
    #[test] fn adc_and_coulomb_registers_read_zero_and_are_not_writable() {
        let mut p=Axp192State::new(HZ);
        for reg in [0x56u8,0x64,0x78,0x7f,0xb0,0xb8] {p.write_register(reg,0xff);assert_eq!(p.register(reg),0,"register {reg:#04x} must not report a fake measurement");}
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

#[cfg(test)]
mod stick_tests {
    use super::*;
    fn write(d: &mut Axp192, r: u8, v: u8) {
        d.start(false);
        d.write(r);
        d.write(v);
        d.stop();
    }
    fn read(d: &mut Axp192, r: u8) -> u8 {
        d.start(false);
        d.write(r);
        d.start(true);
        let v = d.read();
        d.stop();
        v
    }
    #[test]
    fn shared_guest_registers_gate_ldos_and_preserve_voltage_nibbles() {
        let s = Arc::new(Mutex::new(Axp192State::stickc()));
        let mut a = Axp192::new(s.clone());
        let mut b = Axp192::new(s.clone());
        write(&mut a, 0x28, 0x9c);
        assert_eq!(read(&mut b, 0x28), 0x9c);
        assert_eq!(s.lock().unwrap().ldo_mv(2), 2700);
        assert_eq!(s.lock().unwrap().ldo_mv(3), 3000);
        write(&mut b, 0x12, 1);
        assert_eq!(s.lock().unwrap().ldo_mv(2), 0);
        assert!(!s.lock().unwrap().ldo_enabled(3));
        write(&mut a, 0x12, 0x4d);
        assert_eq!(s.lock().unwrap().ldo_mv(3), 3000);
    }
    #[test]
    fn power_key_latches_until_guest_writes_one_to_clear_not_gpio() {
        let s = Arc::new(Mutex::new(Axp192State::stickc()));
        let mut d = Axp192::new(s.clone());
        s.lock().unwrap().set_key(true);
        assert_eq!(read(&mut d, 0x46), 0);
        s.lock().unwrap().set_key(false);
        assert_eq!(read(&mut d, 0x46) & 3, 2);
        assert_eq!(read(&mut d, 0x46) & 3, 2);
        write(&mut d, 0x46, 0);
        assert_eq!(read(&mut d, 0x46), 2);
        write(&mut d, 0x46, 3);
        assert_eq!(read(&mut d, 0x46), 0);
    }
    #[test]
    fn pointer_wraps_and_readonly_adc_ignores_guest_writes_before_conversion() {
        let s = Arc::new(Mutex::new(Axp192State::stickc()));
        let mut d = Axp192::new(s);
        write(&mut d, 0x78, 0xff);
        assert_eq!(read(&mut d, 0x78), 0);
        d.start(false);
        d.write(0xff);
        d.start(true);
        assert_eq!(d.read(), 0);
        assert_eq!(d.read(), 0);
        assert_eq!(read(&mut d, 0x12), 0x4d);
    }
    fn raw(s: &Axp192State, high: u8) -> u16 {
        (u16::from(s.register(high)) << 4) | u16::from(s.register(high + 1) & 15)
    }
    #[test]
    fn virtual_battery_quantization_saturates_and_factory_threshold_is_not_bypassed() {
        for (uv, expected) in [
            (0, 0),
            (1099, 0),
            (1100, 1),
            (1101, 1),
            (3_000_000, 2727),
            (3_200_000, 2909),
            (3_201_000, 2910),
            (4_100_000, 3727),
            (4_504_499, 4094),
            (4_504_500, 4095),
            (u32::MAX, 4095),
        ] {
            let mut s = Axp192State::with_power_input(Axp192PowerInput {
                battery_voltage_uv: uv,
                ..Axp192PowerInput::BATTERY_4V1
            });
            assert_eq!(raw(&s, 0x78), 0); // no conversion at startup
            s.advance_to_us(39_999);
            assert_eq!(raw(&s, 0x78), 0);
            s.advance_to_us(40_000);
            assert_eq!(raw(&s, 0x78), expected, "{uv}");
            assert_eq!(s.register(0x79) & 0xf0, 0);
        }
        assert!((2727.0_f32 * 0.0011) < 3.2); // exact product driver, low fixture
        assert!((3727.0_f32 * 0.0011) > 3.2); // high fixture, not a forced result
    }
    #[test]
    fn disabled_adc_holds_latch_reenable_waits_for_virtual_sample_and_reset_clears_it() {
        let mut s = Axp192State::stickc();
        s.advance_to_us(40_000);
        assert_eq!(raw(&s, 0x78), 3727);
        s.write_register(0x82, 0);
        s.set_power_input(Axp192PowerInput::LOW_BATTERY_3V0);
        s.advance_to_us(120_000);
        assert_eq!(raw(&s, 0x78), 3727);
        s.write_register(0x82, 0x80);
        s.advance_to_us(159_999);
        assert_eq!(raw(&s, 0x78), 3727);
        s.advance_to_us(160_000);
        assert_eq!(raw(&s, 0x78), 2727);
        s.set_key(true);
        s.set_key(false);
        s.write_register(0x12, 0);
        s.write_register(0x84, 0xff);
        s.reset_powered_fixture();
        assert_eq!(s.power_input(), Axp192PowerInput::LOW_BATTERY_3V0);
        assert_eq!(raw(&s, 0x78), 0);
        assert_eq!(s.register(0x82), 0x83);
        assert_eq!(s.register(0x83), 0x80);
        assert_eq!(s.register(0x84), 0x32);
        assert_eq!(s.register(0x46), 0);
        assert!(s.ldo_enabled(3));
        s.advance_to_us(199_999);
        assert_eq!(raw(&s, 0x78), 0);
        s.advance_to_us(200_000);
        assert_eq!(raw(&s, 0x78), 2727);
    }
    #[test]
    fn sample_rate_reserved_bits_and_backward_time_are_deterministic() {
        for (control, period) in [
            (0x32, 40_000),
            (0x72, 20_000),
            (0xb2, 10_000),
            (0xf2, 5_000),
        ] {
            let mut s = Axp192State::stickc();
            s.write_register(0x84, control);
            s.advance_to_us(period - 1);
            assert_eq!(raw(&s, 0x78), 0);
            s.advance_to_us(period);
            assert_eq!(raw(&s, 0x78), 3727);
            s.advance_to_us(0);
            s.set_power_input(Axp192PowerInput::LOW_BATTERY_3V0);
            s.advance_to_us(period * 2 - 1);
            assert_eq!(raw(&s, 0x78), 3727);
            s.advance_to_us(period * 2);
            assert_eq!(raw(&s, 0x78), 2727);
        }
        let mut s = Axp192State::stickc();
        s.write_register(0x83, 0xff);
        s.write_register(0x84, 0xff);
        assert_eq!(s.register(0x83), 0x8f);
        assert_eq!(s.register(0x84), 0xf7);
    }
    #[test]
    fn vbus_adc_status_charger_gating_and_absent_battery_are_explicit_stimuli() {
        let input = Axp192PowerInput {
            vbus_voltage_uv: 5_000_000,
            vbus_present: true,
            vbus_valid: true,
            charging: true,
            ..Axp192PowerInput::BATTERY_4V1
        };
        let mut s = Axp192State::with_power_input(input);
        assert_eq!(s.register(0), 0x34);
        assert_eq!(s.register(1), 0x60);
        s.advance_to_us(40_000);
        assert_eq!(raw(&s, 0x5a), 0); // disabled by reset 0x83
        s.write_register(0x82, 0x88);
        s.advance_to_us(80_000);
        assert_eq!(raw(&s, 0x5a), 2941);
        s.write_register(0x33, 0x48);
        assert_eq!(s.register(0), 0x30);
        assert_eq!(s.register(1), 0x20);
        s.write_register(0x33, 0xc8);
        assert_eq!(s.register(1), 0x60);
        s.set_power_input(Axp192PowerInput {
            battery_present: false,
            vbus_valid: false,
            ..input
        });
        assert_eq!(s.register(0), 0x20);
        assert_eq!(s.register(1), 0);
        s.advance_to_us(120_000);
        assert_eq!(raw(&s, 0x78), 0);
        assert_eq!(raw(&s, 0x5a), 2941);
        s.set_power_input(Axp192PowerInput {
            vbus_present: false,
            ..input
        });
        assert_eq!(s.register(0), 0);
        assert_eq!(s.register(1), 0x20);
        s.advance_to_us(160_000);
        assert_eq!(raw(&s, 0x5a), 0);
    }
    #[test]
    fn bus_reads_shared_adc_and_cannot_write_status_or_measurement_registers() {
        let s = Arc::new(Mutex::new(Axp192State::stickc()));
        let mut d = Axp192::new(s.clone());
        s.lock().unwrap().advance_to_us(40_000);
        for (address, expected) in [
            (0x00, 0),
            (0x01, 0x20),
            (0x78, 0xe8),
            (0x79, 0x0f),
            (0x5a, 0),
            (0x5b, 0),
        ] {
            write(&mut d, address, 0xff);
            assert_eq!(read(&mut d, address), expected);
        }
        d.start(false);
        d.write(0x78);
        d.start(true);
        assert_eq!(d.read(), 0xe8);
        assert_eq!(d.read(), 0x0f);
    }
    #[test]
    fn temperature_integer_quantization_negative_zero_boundaries_and_saturation() {
        for (mc, expected) in [(i32::MIN,0),(-144_701,0),(-144_700,0),(-144_601,0),
            (-144_600,1),(-10_000,1347),(0,1447),(5_000,1497),(25_000,1697),
            (25_099,1697),(25_100,1698),(90_000,2347),(110_000,2547),
            (264_799,4094),(264_800,4095),(i32::MAX,4095)] {
            let mut s=Axp192State::stickc();s.set_chip_temperature_mc(mc);
            assert_eq!(s.chip_temperature_mc(),mc);
            assert_eq!(raw(&s,0x5e),0);s.advance_to_us(39_999);
            assert_eq!(raw(&s,0x5e),0);s.advance_to_us(40_000);
            assert_eq!(raw(&s,0x5e),expected,"{mc}");assert_eq!(s.register(0x5f)&0xf0,0);
        }
    }
    #[test]
    fn temperature_enable_is_83_bit7_not_battery_channel_or_read_side_effect() {
        let mut s=Axp192State::stickc();s.write_register(0x82,0);s.advance_to_us(40_000);
        assert_eq!(raw(&s,0x5e),1697);assert_eq!(raw(&s,0x78),0);
        s.write_register(0x83,0x0f);s.set_chip_temperature_mc(110_000);s.advance_to_us(120_000);
        assert_eq!(raw(&s,0x5e),1697);s.write_register(0x83,0x80);
        s.advance_to_us(159_999);assert_eq!(raw(&s,0x5e),1697);
        s.advance_to_us(160_000);assert_eq!(raw(&s,0x5e),2547);
    }
    #[test]
    fn temperature_sample_rate_change_live_input_backward_time_and_long_jump() {
        for (control,period) in [(0x32,40_000),(0x72,20_000),(0xb2,10_000),(0xf2,5_000)] {
            let mut s=Axp192State::stickc();s.write_register(0x84,control);
            s.advance_to_us(period-1);assert_eq!(raw(&s,0x5e),0);
            s.advance_to_us(period);assert_eq!(raw(&s,0x5e),1697);
            s.set_chip_temperature_mc(-10_000);assert_eq!(raw(&s,0x5e),1697);
            s.advance_to_us(0);s.advance_to_us(period*2-1);assert_eq!(raw(&s,0x5e),1697);
            s.advance_to_us(period*2);assert_eq!(raw(&s,0x5e),1347);
            s.advance_to_us(period*10_000);assert_eq!(raw(&s,0x5e),1347);
        }
        let mut s=Axp192State::stickc();s.advance_to_us(40_000);s.advance_to_us(41_000);
        s.set_chip_temperature_mc(110_000);s.write_register(0x84,0xf2);
        s.advance_to_us(45_999);assert_eq!(raw(&s,0x5e),1697);
        s.advance_to_us(46_000);assert_eq!(raw(&s,0x5e),2547);
    }
    #[test]
    fn temperature_reset_clears_latch_preserves_stimulus_and_restarts_default_phase() {
        let mut s=Axp192State::stickc();s.set_chip_temperature_mc(110_000);
        s.advance_to_us(40_000);s.write_register(0x83,0);s.write_register(0x84,0xf2);s.reset_powered_fixture();
        assert_eq!(s.chip_temperature_mc(),110_000);assert_eq!(raw(&s,0x5e),0);
        assert_eq!(s.register(0x83),0x80);assert_eq!(s.adc_sample_hz(),25);
        s.advance_to_us(79_999);assert_eq!(raw(&s,0x5e),0);
        s.advance_to_us(80_000);assert_eq!(raw(&s,0x5e),2547);
    }
    #[test]
    fn temperature_shared_bus_burst_is_readonly_and_default_is_explicit_virtual_25c() {
        let s=Arc::new(Mutex::new(Axp192State::stickc()));let mut a=Axp192::new(s.clone());
        let mut b=Axp192::new(s.clone());assert_eq!(s.lock().unwrap().chip_temperature_mc(),25_000);
        s.lock().unwrap().advance_to_us(40_000);
        for r in [0x5e,0x5f] {write(&mut a,r,0xff);}
        b.start(false);b.write(0x5e);b.start(true);assert_eq!(b.read(),0x6a);assert_eq!(b.read(),1);
        assert_eq!(s.lock().unwrap().power_input(),Axp192PowerInput::BATTERY_4V1);
    }
    #[test]
    fn temperature_input_and_disabled_samples_version_only_when_observable_state_changes() {
        let mut s=Axp192State::stickc();s.advance_to_us(40_000);let v=s.version;
        s.set_chip_temperature_mc(25_000);s.advance_to_us(80_000);assert_eq!(s.version,v);
        s.write_register(0x83,0);s.set_chip_temperature_mc(0);let v=s.version;
        s.advance_to_us(120_000);assert_eq!(s.version,v);assert_eq!(raw(&s,0x5e),1697);
    }
}

#[cfg(test)]
#[test]
fn stickt2_powered_profile_keeps_polled_rails_and_no_adc() {
    let mut pmic = Axp192State::stickt2();
    assert_eq!(pmic.register(3), 0);
    assert_eq!((pmic.ldo_mv(2), pmic.ldo_mv(3)), (3000, 3000));
    pmic.set_power_input(Axp192PowerInput::LOW_BATTERY_3V0);
    pmic.set_chip_temperature_mc(110_000);
    for reg in [0x82, 0x83, 0x84, 0x5e, 0x78] {
        pmic.write_register(reg, 0xff);
    }
    pmic.advance_to_us(1_000_000);
    assert_eq!((pmic.register(0x83), pmic.register(0x84)), (0xff, 0xff));
    assert_eq!((pmic.register(0x5e), pmic.register(0x78)), (0, 0));
    pmic.write_register(REG_OFF, 0x80);
    assert!(pmic.ldo_enabled(3));
    pmic.set_key(true);
    pmic.set_key(false);
    assert_eq!(pmic.register(REG_INTSTS3), PEKEY_SHORT);
    pmic.write_register(REG_INTSTS3, PEKEY_SHORT);
    assert_eq!(pmic.register(REG_INTSTS3), 0);
}

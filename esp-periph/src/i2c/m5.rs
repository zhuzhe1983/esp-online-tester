//! M5Stack PM1/IOE1 digital register behavior; analog power and bus wake delays are not simulated.
//! Sources: M5PM1 manual v1.9 and official driver 315d2899; see docs/m5-i2c.md.
use super::{I2cDevice, Reg8Device};
use std::sync::{Arc, Mutex};

/// Register/event storage for the shared PMIC. Board inputs must use `set_input`
/// when an edge must be visible before the next addressed I2C transfer.
#[derive(Debug)]
pub struct M5pm1Control {
    regs: [u8;256],
    initial_power_config:u8,
    last_inputs:u8,
    last_known:u8,
    power_pressed:bool,
    power_event:bool,
    press_started:Option<u64>,
    click_due:Option<u64>,
    first_release:Option<u64>,
    second_press:bool,
    long_fired:bool,
}
impl Default for M5pm1Control {
    fn default()->Self {
        let mut regs=[0;256];
        for (r,v) in [(0,0x50),(1,0x20),(2,5),(3,6),(6,0x17),(8,0x40),(0x13,0x1f),(0x49,0x2a)] {regs[r]=v;}
        Self {regs,initial_power_config:0x17,last_inputs:0,last_known:0,power_pressed:false,power_event:false,press_started:None,click_due:None,first_release:None,second_press:false,long_fired:false}
    }
}

/// Host inputs and digital outputs shared with the board after I2C attachment.
#[derive(Debug)]
pub struct M5pm1State {
    pub battery_mv:u16,
    /// Explicit nominal host voltages, not measured ADC/charger output.
    pub vref_mv:u16,
    pub vin_mv:u16,
    pub ext5v_mv:u16,
    pub charging:bool,
    pub power_off_requested:bool,
    pub led_enabled:bool,
    /// PWR_CFG[1], the CoreMatrix 3V3_L3A rail; always off during shutdown.
    pub dcdc_enabled:bool,
    /// PWR_CFG[3], retaining output after shutdown only with HOLD_CFG[6].
    pub grove_5v_enabled:bool,
    /// PWR_CFG[2], retaining output after shutdown only with HOLD_CFG[5].
    pub ldo3v3_enabled:bool,
    pub input_mask:u8,
    pub input_high:u8,
    /// Ordinary GPIO0..4 driven high in push-pull mode, excluding IRQ functions.
    pub driven_high:u8,
    /// GPIOs assigned the IRQ output function; `irq_low` is their active level.
    pub irq_pins:u8,
    pub irq_low:bool,
    /// Board consumes these requests after applying a PMIC wake/reset/download.
    pub wake_requested:bool,
    pub reset_requested:bool,
    pub download_requested:bool,
    pub control:M5pm1Control,
    pub i2c_first_hz:Option<u32>,
    pub i2c_last_hz:Option<u32>,
    pub i2c_max_hz:u32,
    /// I2C_CFG[4]: nominal slave setting, not an electrical ACK threshold.
    pub i2c_speed_hz:u32,
    pub i2c_address_phases:u64,
    pub i2c_speed_mismatches:u64,
    pub i2c_last_mismatch_hz:Option<u32>,
    pub i2c_last_mismatch_limit_hz:Option<u32>,
    pub download_locked:bool,
    pub download_lock_writes:u64,
    pub download_unlock_attempts:u64,
}
impl Default for M5pm1State {
    fn default()->Self {
        Self {battery_mv:3900,vref_mv:0,vin_mv:0,ext5v_mv:0,charging:false,power_off_requested:false,led_enabled:true,dcdc_enabled:true,grove_5v_enabled:false,ldo3v3_enabled:true,input_mask:0,input_high:0,driven_high:0,
            irq_pins:0,irq_low:false,wake_requested:false,reset_requested:false,download_requested:false,control:Default::default(),
            i2c_first_hz:None,i2c_last_hz:None,i2c_max_hz:0,i2c_speed_hz:100_000,i2c_address_phases:0,i2c_speed_mismatches:0,i2c_last_mismatch_hz:None,i2c_last_mismatch_limit_hz:None,
            download_locked:false,download_lock_writes:0,download_unlock_attempts:0}
    }
}
impl M5pm1State {
    /// Physical, board-wired input. Invalid pin numbers have no effect.
    pub fn set_input(&mut self,pin:u8,high:bool) {
        if pin>=5 {return;}
        self.input_mask|=1<<pin;
        if high {self.input_high|=1<<pin;} else {self.input_high&=!(1<<pin);}
        self.refresh(true);
    }
    /// Release an external driver. A configured pull resolves the pin; an
    /// unconnected, unpulled input is unknown and cannot create a wake edge.
    pub fn release_input(&mut self,pin:u8) {
        if pin>=5 {return;}self.input_mask&=!(1<<pin);self.refresh(true);
    }
    fn input_levels(&self)->(u8,u8) {
        let mut known=self.input_mask&0x1f;let mut high=self.input_high&known;
        let pulls=u16::from_le_bytes([self.control.regs[0x14],self.control.regs[0x15]]);
        for pin in 0..5 {
            if known&(1<<pin)!=0 {continue;}
            match (pulls>>(pin*2))&3 {
                1=>{known|=1<<pin;high|=1<<pin;},2=>known|=1<<pin,
                _ if pin==2=>{known|=4;if !self.charging {high|=4;}}, // legacy StopWatch CHG_STAT wiring
                _=>{}
            }
        }
        (known,high)
    }
    /// Current digital WAKE_SRC flags. Guest clearing uses write-zero-to-clear.
    pub fn wake_source(&self)->u8 {self.control.regs[5]&0x7f}
    /// PWR_BTN state and guest-millisecond clock. Click qualification is an ideal
    /// digital model, not the PMIC's analog debounce or 100 ms internal scan.
    pub fn power_button(&mut self,pressed:bool,now_ms:u64) {
        self.advance_to_ms(now_ms);
        if self.control.power_pressed==pressed {return;}
        self.control.power_pressed=pressed;
        if pressed {
            self.control.power_event=true;
            self.control.press_started=Some(now_ms);
            self.control.long_fired=false;
            let gap=125u64<<((self.control.regs[0x49]>>5)&3);
            self.control.second_press=self.control.first_release.is_some_and(|t| now_ms.saturating_sub(t)<=gap);
            if self.control.second_press {self.control.click_due=None;}
            if self.power_off_requested || !self.dcdc_enabled {
                self.restart(4);self.wake_requested=true;self.control.long_fired=true;
            }
        } else if !self.control.long_fired {
            if self.control.second_press {
                self.control.first_release=None;self.control.click_due=None;
                if self.control.regs[0x4a]&1==0 {self.power_off_requested=true;} else {self.control.regs[0x42]|=4;}
            } else {
                let single=125u64<<((self.control.regs[0x49]>>1)&3);
                let double=125u64<<((self.control.regs[0x49]>>5)&3);
                self.control.first_release=Some(now_ms);self.control.click_due=Some(now_ms.saturating_add(single.max(double)));
            }
        }
        self.refresh(false);
    }
    /// Next power-key qualification deadline in the same guest millisecond domain.
    pub fn next_deadline_ms(&self)->Option<u64> {
        let long=if self.control.power_pressed && !self.control.long_fired {
            self.control.press_started.map(|t| t.saturating_add(1000*(u64::from((self.control.regs[0x49]>>3)&3)+1)))
        } else {None};
        self.control.click_due.into_iter().chain(long).min()
    }
    /// The board advances this only with guest time; no wall-clock timer is used.
    pub fn advance_to_ms(&mut self,now_ms:u64) {
        if self.control.power_pressed && !self.control.long_fired && self.control.press_started.is_some_and(|t| now_ms.saturating_sub(t)>=1000*(u64::from((self.control.regs[0x49]>>3)&3)+1)) {
            self.control.long_fired=true;self.control.click_due=None;self.control.first_release=None;
            if !self.download_locked {self.restart(0);self.download_requested=true;}
        }
        if self.control.click_due.is_some_and(|t| now_ms>=t) {
            self.control.click_due=None;self.control.first_release=None;
            if self.control.regs[0x49]&1==0 {self.restart(8);self.reset_requested=true;} else {self.control.regs[0x42]|=1;}
        }
        self.refresh(true);
    }
    fn restart(&mut self,cause:u8) {
        // Manual register map: reset clears GPIO/functions and holds; WAKE_EN,
        // WAKE_CFG and RTC RAM survive. Physical host levels are not reset.
        let old=self.control.regs;
        let defaults=M5pm1Control::default();self.control.regs=defaults.regs;
        self.control.regs[5]=(old[5]|cause)&0x7f;
        self.control.regs[6]=(self.control.initial_power_config&!1)|(old[6]&1);
        self.control.regs[0x18]=old[0x18];self.control.regs[0x19]=old[0x19];
        self.control.regs[0xa0..0xc0].copy_from_slice(&old[0xa0..0xc0]);
        self.power_off_requested=false;
    }
    fn refresh(&mut self,edges:bool) {
        let funcs=u16::from_le_bytes([self.control.regs[0x16],self.control.regs[0x17]]);
        self.irq_pins=0;let mut ordinary=0;
        for pin in 0..5 {match (funcs>>(pin*2))&3 {0=>ordinary|=1<<pin,1=>self.irq_pins|=1<<pin,_=>{}}}
        let (known,inputs)=self.input_levels();
        let changed=(inputs^self.control.last_inputs)&known&self.control.last_known&!self.control.regs[0x10]&0x1f;
        if edges {
            if self.irq_pins!=0 {self.control.regs[0x40]|=changed&ordinary;}
            // Manual v1.9: GPIO1 cannot wake. A simultaneous pair 0/2 or 3/4
            // shares one hardware interrupt; reject an ambiguous pair.
            let mut enabled=self.control.regs[0x18]&0x1d;
            if enabled&5==5 {enabled&=!5;}
            if enabled&0x18==0x18 {enabled&=!0x18;}
            let matched=changed&enabled&!(inputs^self.control.regs[0x19]);
            if (self.power_off_requested || !self.dcdc_enabled) && matched!=0 {
                self.restart(0x20);self.wake_requested=true;
                self.irq_pins=0;ordinary=0x1f;
            }
            self.control.last_inputs=inputs;self.control.last_known=known;
        }
        let r=&mut self.control.regs;
        if self.irq_pins==0 {r[0x40..=0x42].fill(0);} else if r[5]&0x7f!=0 {r[0x42]|=2;}
        self.irq_low=self.irq_pins!=0 && ((r[0x40]&!r[0x43]&0x1f)|(r[0x41]&!r[0x44]&0x3f)|(r[0x42]&!r[0x45]&7))!=0;
        self.led_enabled=r[6]&0x10!=0 && !self.power_off_requested;
        self.dcdc_enabled=r[6]&2!=0 && !self.power_off_requested;
        self.grove_5v_enabled=r[6]&8!=0 && (!self.power_off_requested||r[7]&0x40!=0);
        self.ldo3v3_enabled=r[6]&4!=0 && (!self.power_off_requested||r[7]&0x20!=0);
        self.driven_high=r[0x10]&r[0x11]&!r[0x13]&ordinary&0x1f;
        if self.power_off_requested {self.driven_high&=r[7]&0x1f;}
        self.i2c_speed_hz=if r[9]&0x10!=0 {400_000}else{100_000};
        self.download_locked=r[0x49]&0x80!=0;
    }
    fn gpio_input(&self)->u8 {
        let r=&self.control.regs;
        let (_,high)=self.input_levels();
        let mut value=(high&!r[0x10])|self.driven_high;
        value&=!self.irq_pins;if !self.irq_low {value|=self.irq_pins;}
        value
    }
}

pub struct M5pm1 {ptr:u8,first:bool,state:Arc<Mutex<M5pm1State>>}
impl M5pm1 {
    pub const ADDRESS:u8=0x6e;
    pub fn new(state:Arc<Mutex<M5pm1State>>)->Self {Self::with_power_config(state,0x17)}
    /// Explicit PMIC cold start. A CPU-only reset should retain this device.
    pub fn with_power_config(state:Arc<Mutex<M5pm1State>>,power_config:u8)->Self {
        {let mut s=state.lock().expect("M5PM1 state mutex poisoned");
            *s=M5pm1State {battery_mv:s.battery_mv,vref_mv:s.vref_mv,vin_mv:s.vin_mv,ext5v_mv:s.ext5v_mv,charging:s.charging,input_mask:s.input_mask,input_high:s.input_high,..Default::default()};
            s.control.initial_power_config=power_config;s.control.regs[6]=power_config;(s.control.last_known,s.control.last_inputs)=s.input_levels();s.refresh(false);}
        Self {ptr:0,first:true,state}
    }
    fn observe_clock(&mut self,hz:Option<u32>) {
        let mut s=self.state.lock().expect("M5PM1 state mutex poisoned");s.refresh(true);
        s.i2c_address_phases=s.i2c_address_phases.saturating_add(1);s.i2c_last_hz=hz;
        if let Some(hz)=hz {
            s.i2c_first_hz.get_or_insert(hz);s.i2c_max_hz=s.i2c_max_hz.max(hz);
            if hz>s.i2c_speed_hz+s.i2c_speed_hz/100 {
                s.i2c_speed_mismatches=s.i2c_speed_mismatches.saturating_add(1);s.i2c_last_mismatch_hz=Some(hz);s.i2c_last_mismatch_limit_hz=Some(s.i2c_speed_hz);
            }
        }
    }
}
impl I2cDevice for M5pm1 {
    fn start(&mut self,read:bool)->bool {self.observe_clock(None);if !read {self.first=true;}true}
    fn start_address_with_clock(&mut self,configured:u8,address:u8,read:bool,hz:Option<u32>)->bool {
        if self.address(configured)!=address {return false;}self.observe_clock(hz);if !read {self.first=true;}true
    }
    fn write(&mut self,byte:u8)->bool {
        if self.first {self.ptr=byte;self.first=false;return true;}
        let reg=self.ptr;self.ptr=self.ptr.wrapping_add(1);
        let mut s=self.state.lock().expect("M5PM1 state mutex poisoned");s.refresh(true);
        if matches!(reg,0x00..=0x04|0x12|0x20..=0x27|0x48) {return true;}
        if reg==0x49 {
            if byte&0x80!=0 {s.download_lock_writes=s.download_lock_writes.saturating_add(1);}
            else if s.control.regs[0x49]&0x80!=0 {s.download_unlock_attempts=s.download_unlock_attempts.saturating_add(1);}
        }
        // Driver 315d2899 irqGet*Status(CLEAN_ONCE) writes ~status. This is
        // write-zero-to-clear; writing one cannot manufacture an interrupt.
        if matches!(reg,0x05|0x40..=0x42) {s.control.regs[reg as usize]&=byte;} else {s.control.regs[reg as usize]=byte;}
        if reg==0x0c {match byte {
            0xa1=>s.power_off_requested=true,
            0xa2=>{s.restart(0x10);s.reset_requested=true;},
            0xa3 if !s.download_locked=>{s.restart(0);s.download_requested=true;},
            _=>{}
        }}
        s.refresh(false);true
    }
    fn read(&mut self)->u8 {
        let mut s=self.state.lock().expect("M5PM1 state mutex poisoned");s.refresh(true);
        let value=match self.ptr {
            0x12=>s.gpio_input(),
            0x20..=0x27=>{let v=[s.vref_mv,s.battery_mv,s.vin_mv,s.ext5v_mv][((self.ptr-0x20)/2) as usize];v.to_le_bytes()[(self.ptr&1) as usize]},
            0x48=>{let v=u8::from(s.control.power_pressed)|(u8::from(s.control.power_event)<<7);s.control.power_event=false;v},
            _=>s.control.regs[self.ptr as usize]
        };
        self.ptr=self.ptr.wrapping_add(1);value
    }
}

/// Driven-high StopWatch signals. Input or released open-drain pins are treated as low;
/// external pulls and electrical behavior are not modeled.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct M5ioe1State {
    /// IO1..IO14 driven-high mask; lets each board apply its own wiring.
    pub driven_high: u16,
    /// Board-provided input pins; other inputs retain the existing conservative low behavior.
    pub input_mask: u16,
    pub input_high: u16,
    /// PWM1..4 effective high duty, 0..4095, after direction/drive/enable/polarity.
    pub pwm: [u16; 4],
    /// Shared PWM1..4 frequency in Hz (PWM_FREQ_L/H, 0x25/26).
    pub pwm_frequency_hz:u16,
    pub oled_rail: bool,
    pub oled_reset_high: bool,
    pub touch_reset_high: bool,
}

pub struct M5ioe1 {
    registers: Reg8Device,
    state: Arc<Mutex<M5ioe1State>>,
}

impl M5ioe1 {
    pub const ADDRESS: u8 = 0x4f;

    /// UID is a board-supplied synthetic identity, not a fixed M5IOE1 chip ID.
    pub fn new(state: Arc<Mutex<M5ioe1State>>, uid: u16) -> Self {
        let [lo, hi] = uid.to_le_bytes();
        {let mut state=state.lock().expect("M5IOE1 state mutex poisoned");let (input_mask,input_high)=(state.input_mask,state.input_high);*state=M5ioe1State{input_mask,input_high,..Default::default()};}
        Self {
            registers: Reg8Device::new("m5ioe1", &[
                (0x00, lo), (0x01, hi), (0x02, b'A'), (0x13, 0xff), (0x14, 0x3f),
            ]),
            state,
        }
    }
}

impl I2cDevice for M5ioe1 {
    fn start(&mut self, read: bool) -> bool { self.registers.start(read) }

    fn write(&mut self, byte: u8) -> bool {
        if !self.registers.first && matches!(self.registers.ptr, 0x00..=0x02 | 0x07..=0x08) {
            self.registers.ptr += 1;
            return true;
        }
        let ack = self.registers.write(byte);
        let regs = &self.registers.regs;
        let high = regs[0x03] & regs[0x05] & !regs[0x13];
        let mut state=self.state.lock().expect("M5IOE1 state mutex poisoned");
        let output=u16::from_le_bytes([regs[3],regs[4]]) & !u16::from_le_bytes([regs[0x13],regs[0x14]]);
        // M5IOE1 1.0.8 M5IOE1.h PWM duty 0x1B..0x22, enable bit7, polarity bit6; PWM1=IO9, PWM2=IO8.
        let pwm=std::array::from_fn(|ch| {let lo=0x1b+ch*2;let duty=u16::from_le_bytes([regs[lo],regs[lo+1]&15]);
            if regs[lo+1]&0x80==0 || output & (1 << [8,7,10,9][ch])==0 || regs[0x25]==0 && regs[0x26]==0 {0} else if regs[lo+1]&0x40!=0 {4095-duty} else {duty}});
        *state = M5ioe1State {
            input_mask:state.input_mask,input_high:state.input_high,pwm,pwm_frequency_hz:u16::from_le_bytes([regs[0x25],regs[0x26]]),
            driven_high: u16::from(high) | (u16::from(regs[0x04] & regs[0x06] & !regs[0x14] & 0x3f) << 8),
            oled_rail: high & 0x80 != 0,
            oled_reset_high: high & 0x10 != 0,
            touch_reset_high: high & 0x08 != 0,
        };
        ack
    }

    fn read(&mut self) -> u8 {
        // M5IOE1 1.0.8 (37db0486), M5IOE1.h GPIO_IN_L/H and digitalRead:
        // GPIO_IN reflects the driven pin, not the output latch alone. Model
        // push-pull outputs in both banks; external/input/open-drain pulls remain
        // unmodeled. Bank H has only IO9..IO14 (bits 0..5).
        let regs = &mut self.registers.regs;
        regs[0x07] = regs[0x03] & regs[0x05] & !regs[0x13];
        regs[0x08] = regs[0x04] & regs[0x06] & !regs[0x14] & 0x3f;
        let state=self.state.lock().expect("M5IOE1 state mutex poisoned");
        let inputs=state.input_mask & !u16::from_le_bytes([regs[3],regs[4]]);
        let value=(u16::from_le_bytes([regs[7],regs[8]]) & !inputs) | (state.input_high & inputs);
        [regs[7],regs[8]]=value.to_le_bytes();
        self.registers.read()
    }
}

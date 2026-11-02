//! M5Stack StopWatch's I2C register subset, not analog power or sleep timing simulation.
//! Register sources: M5PM1 Chip User Manual v1.9, pp. 4–10, 12–18, 23;
//! M5IOE1 Chip User Manual, sections III and IV (links and wiring in docs/m5-i2c.md).
use super::{I2cDevice, Reg8Device};
use std::sync::{Arc, Mutex};

/// Host inputs and a latched shutdown request, shared with the board after I2C attachment.
#[derive(Debug)]
pub struct M5pm1State {
    pub battery_mv: u16,
    pub charging: bool,
    pub power_off_requested: bool,
    pub led_enabled: bool,
    /// PWR_CFG bit3, retaining output after shutdown only with HOLD_CFG bit6.
    pub grove_5v_enabled: bool,
    /// PWR_CFG bit2; HOLD_CFG bit5 retains this output after shutdown.
    pub ldo3v3_enabled: bool,
    /// Board-wired GPIO inputs. Zero mask preserves legacy board behavior.
    pub input_mask: u8,
    pub input_high: u8,
    /// Standard GPIO0..4 driven high in push-pull mode (not open-drain release).
    pub driven_high: u8,
    /// Nominal master rates at addressed transfers; unknown software-I2C does not
    /// supply a rate. The first/max values refer only to known observations.
    pub i2c_first_hz: Option<u32>,
    pub i2c_last_hz: Option<u32>,
    pub i2c_max_hz: u32,
    /// I2C_CFG[4]: 100/400 kHz, not a claim that slower transfers must fail.
    pub i2c_speed_hz: u32,
    pub i2c_address_phases: u64,
    pub i2c_speed_mismatches: u64,
    pub i2c_last_mismatch_hz: Option<u32>,
    pub i2c_last_mismatch_limit_hz: Option<u32>,
    pub download_locked: bool,
    /// Guest writes requesting DL_LOCK=1, including repeated writes of that value.
    pub download_lock_writes: u64,
    /// Attempts to clear a previously set lock; silicon latch clearing is not modeled.
    pub download_unlock_attempts: u64,
}

impl Default for M5pm1State {
    fn default() -> Self {
        Self { battery_mv: 3900, charging: false, power_off_requested: false, led_enabled: true, grove_5v_enabled: false, ldo3v3_enabled:true, input_mask:0, input_high:0, driven_high: 0,
            i2c_first_hz:None, i2c_last_hz:None, i2c_max_hz:0, i2c_speed_hz:100_000, i2c_address_phases:0,
            i2c_speed_mismatches:0, i2c_last_mismatch_hz:None, i2c_last_mismatch_limit_hz:None,
            download_locked:false, download_lock_writes:0, download_unlock_attempts:0 }
    }
}

pub struct M5pm1 {
    registers: Reg8Device,
    state: Arc<Mutex<M5pm1State>>,
}

impl M5pm1 {
    pub const ADDRESS: u8 = 0x6e;

    pub fn new(state: Arc<Mutex<M5pm1State>>) -> Self {
        {let mut state=state.lock().expect("M5PM1 state mutex poisoned");*state=M5pm1State {battery_mv:state.battery_mv,charging:state.charging,input_mask:state.input_mask,input_high:state.input_high,..Default::default()};}
        Self {
            registers: Reg8Device::new("m5pm1", &[
                (0x00, 0x50), (0x01, 0x20), (0x02, 0x05), (0x03, 0x06),
                (0x06, 0x17), (0x08, 0x40), (0x13, 0x1f), (0x49, 0x2a),
            ]),
            state,
        }
    }

    fn observe_clock(&mut self, hz: Option<u32>) {
        let mut state=self.state.lock().expect("M5PM1 state mutex poisoned");
        state.i2c_address_phases=state.i2c_address_phases.saturating_add(1);
        state.i2c_last_hz=hz;
        if let Some(hz)=hz {
            state.i2c_first_hz.get_or_insert(hz);
            state.i2c_max_hz=state.i2c_max_hz.max(hz);
            // A diagnostic, never an ACK rule. Allow 1% nominal-divider rounding;
            // this is not an electrical tolerance or a measured failure threshold.
            if hz > state.i2c_speed_hz + state.i2c_speed_hz/100 {
                state.i2c_speed_mismatches=state.i2c_speed_mismatches.saturating_add(1);
                state.i2c_last_mismatch_hz=Some(hz);
                state.i2c_last_mismatch_limit_hz=Some(state.i2c_speed_hz);
            }
        }
    }
}

impl I2cDevice for M5pm1 {
    fn start(&mut self, read: bool) -> bool { self.observe_clock(None); self.registers.start(read) }

    fn start_address_with_clock(&mut self, configured: u8, address: u8, read: bool, hz: Option<u32>) -> bool {
        if self.address(configured)!=address {return false;}
        self.observe_clock(hz);
        self.registers.start(read)
    }

    fn write(&mut self, byte: u8) -> bool {
        if !self.registers.first {
            let reg = self.registers.ptr;
            if matches!(reg, 0x00..=0x03 | 0x12 | 0x22..=0x23) {
                self.registers.ptr = reg.wrapping_add(1);
                return true;
            }
            if reg == 0x0c && byte == 0xa1 {
                self.state.lock().expect("M5PM1 state mutex poisoned").power_off_requested = true;
            }
            // M5PM1 315d2899 src/M5PM1.h: BTN_CFG_1[7] is DL_LOCK. The
            // manufacturer's library refuses clearing a set lock; no silicon
            // power-cycle/latch enforcement is inferred from that library check.
            if reg == 0x49 {
                let mut state=self.state.lock().expect("M5PM1 state mutex poisoned");
                if byte & 0x80 != 0 {state.download_lock_writes=state.download_lock_writes.saturating_add(1);}
                else if self.registers.regs[0x49] & 0x80 != 0 {state.download_unlock_attempts=state.download_unlock_attempts.saturating_add(1);}
            }
        }
        let ack = self.registers.write(byte);
        let mut state=self.state.lock().expect("M5PM1 state mutex poisoned");
        state.led_enabled = self.registers.regs[0x06] & 0x10 != 0;
        // M5PM1 315d2899 src/M5PM1.h I2C_CFG (0x09), SPD bit4; manual v1.9 §System.
        state.i2c_speed_hz = if self.registers.regs[0x09] & 0x10 != 0 {400_000} else {100_000};
        state.download_locked = self.registers.regs[0x49] & 0x80 != 0;
        // M5PM1 v1.9 pp.13-14: PWR_CFG[3] boost, HOLD_CFG[6] keeps it at shutdown.
        state.grove_5v_enabled = self.registers.regs[0x06] & 8 != 0 && (!state.power_off_requested || self.registers.regs[0x07] & 0x40 != 0);
        state.ldo3v3_enabled = self.registers.regs[0x06] & 4 != 0 && (!state.power_off_requested || self.registers.regs[0x07] & 0x20 != 0);
        // M5PM1 manual pp.15–17: MODE/OUT/DRV and two-bit FUNC fields.
        let r=&self.registers.regs;let funcs=u16::from_le_bytes([r[0x16],r[0x17]]);
        let mut high=r[0x10]&r[0x11]&!r[0x13]&0x1f;
        for pin in 0..5 {if funcs & (3<<(pin*2))!=0 {high&=!(1<<pin);}}
        if state.power_off_requested {high&=r[0x07]&0x1f;}
        state.driven_high=high;
        ack
    }

    fn read(&mut self) -> u8 {
        let state = self.state.lock().expect("M5PM1 state mutex poisoned");
        self.registers.regs[0x12] = (self.registers.regs[0x12] & !4) | (u8::from(!state.charging) << 2);
        // M5PM1 v1.9 pp.15–16 GPIO_IN: board inputs override only input-mode GPIOs.
        // A configured push-pull output overrides the external input observation.
        let pins=state.input_mask&0x1f;let input=pins&!self.registers.regs[0x10];
        self.registers.regs[0x12]=(self.registers.regs[0x12]&!pins)|(state.input_high&input)|(state.driven_high&pins);
        [self.registers.regs[0x22], self.registers.regs[0x23]] = state.battery_mv.to_le_bytes();
        self.registers.read()
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
            input_mask:state.input_mask,input_high:state.input_high,pwm,
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

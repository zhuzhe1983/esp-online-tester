//! BMI270 samples, optional guest-clock data-ready and any/no-motion interrupt subset.
// Bosch BST-BMI270-DS000-08, sections 4.4, 4.6, 5.2 and 8.2; SensorAPI
// 41129fcfe39c583ee5462d79195741945d51c1fe/bmi2_defs.h. See docs/bmi270.md.
use super::I2cDevice;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug)]
pub struct Bmi270Sample { pub accel_g: [f64; 3], pub gyro_dps: [f64; 3], pub version: u64 }
impl Default for Bmi270Sample {
    fn default() -> Self { Self { accel_g: [0.0, 1.0, 0.0], gyro_dps: [0.0; 3], version: 1 } }
}

#[derive(Default)]
struct Motion {reference:[f64;3],count:u16}
struct Clock {
    // Quarter microseconds represent both 3200 Hz and the 6400 Hz IRQ pulse exactly.
    now:u64,next:[Option<u64>;2],pulse_until:[Option<u64>;2],feature_next:Option<u64>,
    motion:[Motion;2],active_features:u8,
}
impl Clock {fn new(now:u64)->Self {Self{now,next:[None;2],pulse_until:[None;2],feature_next:None,motion:Default::default(),active_features:0}}}

pub struct Bmi270 {
    regs: [u8; 128], ptr: u8, first: bool, init_at: usize,
    config: Box<[u8; 8192]>, written: Box<[bool; 8192]>,
    sample: Arc<Mutex<Bmi270Sample>>, version: u64,
    features:[[u8;16];8],clock:Option<Box<Clock>>,burst:[u8;12],
}

impl Bmi270 {
    pub const ADDRESS: u8 = 0x68;
    pub fn new(sample: Arc<Mutex<Bmi270Sample>>) -> Self {
        let mut device = Self { regs: [0; 128], ptr: 0, first: true, init_at: 0,
            config: Box::new([0; 8192]), written: Box::new([false; 8192]), sample, version: 0, features:[[0;16];8],clock:None,burst:[0;12] };
        device.reset(); device
    }
    /// One SPI CS assertion. Bosch DS000-08 section 6.4: bit 7 selects
    /// a read; a dummy byte follows the address before register data.
    pub fn spi_transfer(&mut self, tx: &[u8], rx_len: usize) -> Vec<u8> {
        let mut rx = vec![0xff; rx_len];
        let Some(&address) = tx.first() else { return rx; };
        self.start(false); self.write(address & 0x7f);
        if address & 0x80 != 0 {
            self.start(true);
            if let Some(dummy) = rx.get_mut(1) { *dummy = 0; }
            for byte in rx.iter_mut().skip(2) { *byte = self.read(); }
        } else {
            for &byte in tx.iter().skip(1) { self.write(byte); }
        }
        rx
    }
    /// Opt into guest-clock sampling. Existing polled boards never call this.
    /// Sensor inputs are held between host updates; analog filters are not modeled.
    pub fn advance_to_us(&mut self,us:u64) {
        let now=us.saturating_mul(4);
        if self.clock.is_none() {self.clock=Some(Box::new(Clock::new(now)));self.reschedule();return;}
        let mut clock=self.clock.take().expect("BMI270 clock exists");
        if now<clock.now {self.clock=Some(clock);return;}
        clock.now=now;
        let sample=*self.sample.lock().expect("BMI270 sample mutex poisoned");
        for ch in 0..2 {
            if let (Some(next),Some(period))=(clock.next[ch],self.period(ch)) {
                if next<=now {
                    let last=next+((now-next)/period)*period;
                    clock.next[ch]=last.checked_add(period);
                    clock.pulse_until[ch]=last.checked_add(625).filter(|end| *end>now);
                    let bit=if ch==0 {0x80}else{0x40};
                    self.regs[3]|=bit;self.regs[0x1d]|=bit;
                    self.store_sample(sample,ch);
                }
            }
            if clock.pulse_until[ch].is_some_and(|end| end<=now) {clock.pulse_until[ch]=None;}
        }
        if let Some(next)=clock.feature_next {
            if next<=now {
                let count=(now-next)/80_000+1;
                let last=next+(count-1)*80_000;clock.feature_next=last.checked_add(80_000);
                clock.active_features=0;
                for kind in 0..2 {
                    let (duration,axes,threshold,output)=self.motion_config(kind);
                    if output==0 {continue;}
                    let range=f64::from(2u32<<(self.regs[0x41]&3));
                    let current=sample.accel_g.map(|v|if v.is_finite(){v.clamp(-range,range)}else{f64::NAN});
                    let state=&mut clock.motion[kind];
                    let valid=(0..3).filter(|axis| axes&(1<<axis)!=0).all(|axis| current[axis].is_finite());
                    if !(0..3).filter(|axis|axes&(1<<axis)!=0).all(|axis|state.reference[axis].is_finite()) {state.reference=current;state.count=0;continue;}
                    let over=(0..3).any(|axis| axes&(1<<axis)!=0 && (current[axis]-state.reference[axis]).abs()>threshold);
                    let under=(0..3).filter(|axis| axes&(1<<axis)!=0).all(|axis| (current[axis]-state.reference[axis]).abs()<threshold);
                    let mut active=false;
                    if kind==0 {
                        if valid&&over {
                            let needed=u64::from(duration.saturating_sub(state.count));
                            if count>=needed {self.regs[0x1c]|=output;state.reference=current;state.count=0;active=count==needed;}
                            else {state.count+=count as u16;}
                        } else {state.count=0;}
                    } else {
                        let qualifying=if valid&&under {count}else{state.reference=current;state.count=0;if threshold>0.0 {count.saturating_sub(1)}else{0}};
                        if valid {state.count=(u64::from(state.count)+qualifying).min(u64::from(duration)) as u16;}
                        active=valid&&state.count>=duration;
                        if active {self.regs[0x1c]|=output;}
                    }
                    if active {clock.active_features|=output;}
                }
            }
        }
        self.clock=Some(clock);
    }
    /// Next sensor/IRQ transition in the same guest microsecond domain.
    pub fn next_deadline_us(&self)->Option<u64> {
        let c=self.clock.as_ref()?;
        c.next.into_iter().chain(c.pulse_until).chain([c.feature_next]).flatten().min().map(|t| t.div_ceil(4))
    }
    /// INT1 and INT2 physical digital drivers. None means disabled/high impedance;
    /// an open-drain released high needs the board's actual pull-up to read high.
    pub fn interrupt_pins(&self)->[Option<bool>;2] {
        std::array::from_fn(|pin| {
            let io=self.regs[0x53+pin];if io&8==0 {return None;}
            let (features,data)=if self.regs[0x55]&1!=0 {
                (self.regs[0x1c],self.regs[0x1d]&0xc0!=0)
            } else if let Some(c)=&self.clock {
                (c.active_features,c.pulse_until.iter().any(|end| end.is_some_and(|end|end>c.now)))
            } else {(0,false)};
            let active=features&self.regs[0x56+pin]!=0 || data&&self.regs[0x58]&(4<<(pin*4))!=0;
            let high=active==(io&2!=0);
            if high&&io&4!=0 {None}else{Some(high)}
        })
    }
    fn period(&self,ch:usize)->Option<u64> {
        if self.regs[0x21]&15!=1 || self.regs[0x7d]&(if ch==0 {4}else{2})==0 {return None;}
        let code=self.regs[if ch==0 {0x40}else{0x42}]&15;
        if (if ch==0 {1..=12}else{6..=13}).contains(&code) {Some(5_120_000>>(code-1))}else{None}
    }
    fn motion_config(&self,kind:usize)->(u16,u8,f64,u8) {
        let bytes=if kind==0 {&self.features[1][12..16]}else{&self.features[2][0..4]};
        let first=u16::from_le_bytes([bytes[0],bytes[1]]);let second=u16::from_le_bytes([bytes[2],bytes[3]]);
        let axes=(first>>13) as u8;let out=(second>>11)&15;
        let enabled=second&0x8000!=0 && axes!=0 && (1..=8).contains(&out) && self.regs[0x21]&15==1 && self.regs[0x7d]&4!=0;
        // Bosch DS000-08 §4.8: low-power features require ODR >= 50 Hz;
        // high-performance filter mode runs the feature engine independently.
        let rate_ok=self.regs[0x40]&0x80!=0 || (7..=12).contains(&(self.regs[0x40]&15));
        ((first&0x1fff).max(1),axes,f64::from(second&0x7ff)/2048.0,if enabled&&rate_ok {1<<(out-1)}else{0})
    }
    fn reschedule(&mut self) {
        let periods=[self.period(0),self.period(1)];
        let motion=[self.motion_config(0).3!=0,self.motion_config(1).3!=0];
        let sample=*self.sample.lock().expect("BMI270 sample mutex poisoned");
        let range=f64::from(2u32<<(self.regs[0x41]&3));
        let reference=sample.accel_g.map(|v|if v.is_finite(){v.clamp(-range,range)}else{f64::NAN});
        if let Some(c)=&mut self.clock {
            c.next=periods.map(|p|p.and_then(|p|c.now.checked_add(p)));c.pulse_until=[None;2];
            c.feature_next=motion.iter().any(|enabled|*enabled).then(||c.now.checked_add(80_000)).flatten();
            c.motion=std::array::from_fn(|_|Motion{reference,count:0});c.active_features=0;
        }
        let bits=(if periods[0].is_some(){0x80}else{0})|(if periods[1].is_some(){0x40}else{0});
        self.regs[3]=0x10|(self.regs[3]&bits);self.regs[0x1d]&=bits;
    }
    fn store_sample(&mut self,sample:Bmi270Sample,ch:usize) {
        let (start,values,scale)=if ch==0 {(0x0c,sample.accel_g,32768.0/(2u32<<(self.regs[0x41]&3)) as f64)}
            else {(0x12,sample.gyro_dps,32768.0/(2000u32>>(self.regs[0x43]&7).min(4)) as f64)};
        for (axis,value) in values.into_iter().enumerate() {
            let raw=if value.is_finite(){(value*scale).round().clamp(-32768.0,32767.0) as i16}else{0};
            self.regs[start+axis*2..start+axis*2+2].copy_from_slice(&raw.to_le_bytes());
        }
    }
    fn reset(&mut self) {
        self.regs.fill(0); self.written.fill(false); self.init_at = 0; self.version = 0;
        self.features.fill([0;16]);
        // Standard Bosch BMI270 feature profile, DS000-08 §5.2.40 pages1/2.
        self.features[1][12..16].copy_from_slice(&[5,0xe0,0xaa,0x38]);
        self.features[2][0..4].copy_from_slice(&[5,0xe0,0x90,0x30]);
        if let Some(c)=&mut self.clock {**c=Clock::new(c.now);}
        for (reg, value) in [(0x00, 0x24), (0x03, 0x10), (0x40, 0xa8), (0x41, 0x02),
            (0x42, 0xa9), (0x7c, 0x03)] { self.regs[reg] = value; }
    }
    fn latch(&mut self) {
        let sample = *self.sample.lock().expect("BMI270 sample mutex poisoned");
        let enabled = if self.regs[0x21] & 0x0f == 1 { self.regs[0x7d] } else { 0 };
        let bits = (if enabled & 4 != 0 { 0x80 } else { 0 }) | (if enabled & 2 != 0 { 0x40 } else { 0 });
        if self.version != sample.version { self.regs[0x03] |= bits; self.regs[0x1d] |= bits; self.version = sample.version; }
        self.regs[0x03] = 0x10 | (self.regs[0x03] & bits); self.regs[0x1d] &= bits;
        let acc_scale = 32768.0 / (2u32 << (self.regs[0x41] & 3)) as f64;
        let gyro_scale = 32768.0 / (2000u32 >> (self.regs[0x43] & 7).min(4)) as f64;
        for (start, values, scale, power) in [(0x0c, sample.accel_g, acc_scale, 4), (0x12, sample.gyro_dps, gyro_scale, 2)] {
            for (axis, value) in values.into_iter().enumerate() {
                let raw = if enabled & power != 0 && value.is_finite() { (value * scale).round().clamp(-32768.0, 32767.0) as i16 } else { 0 };
                self.regs[start + axis * 2..start + axis * 2 + 2].copy_from_slice(&raw.to_le_bytes());
            }
        }
    }
}

impl I2cDevice for Bmi270 {
    fn start(&mut self, read: bool) -> bool {
        if read { if self.clock.is_none() {self.latch();} self.burst.copy_from_slice(&self.regs[0x0c..0x18]); } else { self.first = true; } true
    }
    fn write(&mut self, value: u8) -> bool {
        if self.first {
            self.ptr = value & 0x7f; self.first = false;
            self.init_at = (((self.regs[0x5c] as usize) << 4) | (self.regs[0x5b] as usize & 15)) * 2;
            return true;
        }
        let reg = self.ptr as usize;
        if reg == 0x5e {
            if self.regs[0x59] == 0 && self.init_at < self.config.len() {
                self.config[self.init_at] = value; self.written[self.init_at] = true;
            }
            self.init_at = self.init_at.saturating_add(1); return true;
        }
        if (0x30..=0x3f).contains(&reg) {
            self.features[(self.regs[0x2f]&7) as usize][reg-0x30]=value;
            if matches!((self.regs[0x2f]&7,reg),(1,0x3c..=0x3f)|(2,0x30..=0x33)) {self.reschedule();}
        } else if reg == 0x7e && value == 0xb6 { self.reset(); }
        else if reg == 0x59 {
            self.regs[reg] = value & 1;
            // Accept the complete Bosch config transfer; its embedded feature code
            // is not executed. An incomplete transfer must not report init success.
            self.regs[0x21] = if value & 1 != 0 { if self.written.iter().all(|v| *v) { 1 } else { 2 } } else { 0 };
            self.version = 0;self.reschedule();
        } else if (0x2f..=0x5c).contains(&reg) || (0x68..=0x7d).contains(&reg) {
            self.regs[reg] = value;
            if reg == 0x7d { self.version = 0; }
            if matches!(reg,0x40|0x42|0x7d) {self.reschedule();}
        }
        self.ptr = self.ptr.wrapping_add(1) & 0x7f; true
    }
    fn read(&mut self) -> u8 {
        let reg = self.ptr as usize;
        if reg == 0x5e { let value = self.config.get(self.init_at).copied().unwrap_or(0); self.init_at = self.init_at.saturating_add(1); return value; }
        let value=if self.clock.is_some() && (0x0c..=0x17).contains(&reg) {self.burst[reg-0x0c]}else if (0x30..=0x3f).contains(&reg) {self.features[(self.regs[0x2f]&7) as usize][reg-0x30]}else{self.regs[reg]};
        // A burst is latched at its read address phase. The guest-clock mode clears
        // STATUS on any vector byte (DS000-08 §4.6); legacy polling retains its
        // full-vector clear contract. INT_STATUS is clear-on-read in both modes.
        if reg == 0x11 || self.clock.is_some() && (0x0c..=0x11).contains(&reg) { self.regs[0x03] &= !0x80; }
        if reg == 0x17 || self.clock.is_some() && (0x12..=0x17).contains(&reg) { self.regs[0x03] &= !0x40; }
        if reg==0x1c {self.regs[0x1c]=if self.regs[0x55]&1!=0 {self.clock.as_ref().map_or(0,|c|c.active_features)}else{0};}
        if reg == 0x1d { self.regs[0x1d] = 0; }
        self.ptr = self.ptr.wrapping_add(1) & 0x7f; value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(device: &mut Bmi270, reg: u8, bytes: &[u8]) { device.start(false); device.write(reg); for value in bytes { device.write(*value); } }
    fn read(device: &mut Bmi270, reg: u8, len: usize) -> Vec<u8> { write(device, reg, &[]); device.start(true); (0..len).map(|_| device.read()).collect() }
    fn ready(device: &mut Bmi270) { write(device, 0x59, &[0]); write(device, 0x5e, &[0; 8192]); write(device, 0x59, &[1]); write(device, 0x7d, &[6]); }
    fn vector(bytes: &[u8]) -> Vec<i16> { bytes.as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes(*b)).collect() }
    #[test]
    fn spi_dummy_config_and_samples() {
        let sample = Arc::new(Mutex::new(Bmi270Sample { accel_g: [0.0, 0.0, 1.0], gyro_dps: [125.0, 0.0, 0.0], version: 1 }));
        let mut d = Bmi270::new(sample);
        assert_eq!(d.spi_transfer(&[], 3), [0xff; 3]);
        assert_eq!(d.spi_transfer(&[0x80, 0, 0], 3), [0xff, 0, 0x24]);
        d.spi_transfer(&[0x59, 1], 0);
        assert_eq!(d.spi_transfer(&[0xa1, 0, 0], 3)[2], 2);
        d.spi_transfer(&[0x59, 0], 0);
        let mut config = vec![0; 8193]; config[0] = 0x5e;
        d.spi_transfer(&config, 0); d.spi_transfer(&[0x59, 1], 0);
        d.spi_transfer(&[0x7d, 6], 0);
        assert_eq!(d.spi_transfer(&[0xa1, 0, 0], 3)[2], 1);
        assert_eq!(vector(&d.spi_transfer(&[0x8c], 14)[2..]), [0, 0, 4096, 2048, 0, 0]);
        d.spi_transfer(&[0x7e, 0xb6], 0);
        assert_eq!(d.spi_transfer(&[0xa1], 3)[2], 0);
    }
    #[test]
    fn datasheet_reset_range_is_eight_g() {
        // Bosch BST-BMI270-DS000-08 section5.2.42, page102: ACC_RANGE reset=0x02.
        let sample=Arc::new(Mutex::new(Bmi270Sample {accel_g:[1.0,0.0,0.0],..Default::default()}));
        let mut d=Bmi270::new(sample);assert_eq!(read(&mut d,0x41,1),[2]);
        ready(&mut d);assert_eq!(vector(&read(&mut d,0x0c,6)),[4096,0,0]);
        write(&mut d,0x41,&[1]);assert_eq!(vector(&read(&mut d,0x0c,6)),[8192,0,0]);
        write(&mut d,0x7e,&[0xb6]);assert_eq!(read(&mut d,0x41,1),[2]);
    }
    #[test]
    fn config_power_ranges_and_saturation() {
        let sample = Arc::new(Mutex::new(Bmi270Sample { accel_g: [1.0, -1.0, 100.0], gyro_dps: [1000.0, -500.0, f64::NAN], version: 1 }));
        let mut d = Bmi270::new(sample);
        assert_eq!(read(&mut d, 0, 1), [0x24]); assert_eq!(read(&mut d, 0x0c, 12), [0; 12]);
        write(&mut d, 0x59, &[1]); assert_eq!(read(&mut d, 0x21, 1), [2]);
        ready(&mut d); assert_eq!(read(&mut d, 0x21, 1), [1]);
        assert_eq!(vector(&read(&mut d, 0x0c, 12)), [4096, -4096, 32767, 16384, -8192, 0]);
        write(&mut d, 0x41, &[3]); write(&mut d, 0x43, &[2]);
        assert_eq!(vector(&read(&mut d, 0x0c, 12)), [2048, -2048, 32767, 32767, -32768, 0]);
        write(&mut d, 0x7d, &[0]); assert_eq!(read(&mut d, 0x0c, 12), [0; 12]);
        write(&mut d, 0x7e, &[0xb6]); assert_eq!(read(&mut d, 0x21, 1), [0]);
    }
    #[test]
    fn read_snapshot_and_data_ready_follow_host_sample() {
        let sample = Arc::new(Mutex::new(Bmi270Sample::default())); let mut d = Bmi270::new(sample.clone()); ready(&mut d);
        assert_eq!(read(&mut d, 0x1c, 2), [0, 0xc0]); assert_eq!(read(&mut d, 0x1c, 2), [0, 0]);
        write(&mut d, 0x0c, &[]); d.start(true); let first = d.read();
        *sample.lock().unwrap() = Bmi270Sample { accel_g: [-1.0, 0.0, 0.0], gyro_dps: [0.0; 3], version: 2 };
        let mut bytes = vec![first]; bytes.extend((0..5).map(|_| d.read())); assert_eq!(vector(&bytes), [0, 4096, 0]);
        assert_eq!(read(&mut d, 0x1d, 1), [0xc0]); assert_eq!(vector(&read(&mut d, 0x0c, 6)), [-4096, 0, 0]);
    }
    fn feature(device:&mut Bmi270,kind:usize,duration:u16,axes:u8,threshold:u16,out:u8,on:bool) {
        write(device,0x2f,&[if kind==0 {1}else{2}]);
        let a=(duration&0x1fff)|(u16::from(axes&7)<<13);
        let b=(threshold&0x7ff)|(u16::from(out&15)<<11)|(u16::from(on)<<15);
        let mut bytes=[0;4];bytes[..2].copy_from_slice(&a.to_le_bytes());bytes[2..].copy_from_slice(&b.to_le_bytes());
        write(device,if kind==0 {0x3c}else{0x30},&bytes);
    }
    #[test]
    fn feature_window_is_banked_and_matches_bosch_standard_profile() {
        let mut d=Bmi270::new(Default::default());ready(&mut d);
        write(&mut d,0x2f,&[1]);assert_eq!(read(&mut d,0x3c,4),[5,0xe0,0xaa,0x38]);
        write(&mut d,0x2f,&[2]);assert_eq!(read(&mut d,0x30,4),[5,0xe0,0x90,0x30]);
        write(&mut d,0x30,&[0x42]);write(&mut d,0x2f,&[1]);assert_eq!(read(&mut d,0x30,1),[0]);
        write(&mut d,0x2f,&[0xfa]);assert_eq!(read(&mut d,0x30,1),[0x42]); // valid page bits only
        write(&mut d,0x7e,&[0xb6]);write(&mut d,0x2f,&[2]);assert_eq!(read(&mut d,0x30,1),[5]);
    }
    #[test]
    fn timed_data_ready_respects_odr_mapping_polarity_and_pulse_width() {
        let sample=Arc::new(Mutex::new(Bmi270Sample{accel_g:[0.25,0.0,0.0],..Default::default()}));
        let mut d=Bmi270::new(sample);d.advance_to_us(0);ready(&mut d);
        assert_eq!(d.next_deadline_us(),Some(5000)); // default gyro200Hz, accel100Hz
        write(&mut d,0x58,&[0x04]); // DRDY on INT1 only
        assert_eq!(d.interrupt_pins(),[None,None]);
        write(&mut d,0x53,&[0x08,0x0a]); // INT1active-low, INT2active-high
        assert_eq!(d.interrupt_pins(),[Some(true),Some(false)]);
        d.advance_to_us(4999);assert_eq!(read(&mut d,0x1d,1),[0]);
        d.advance_to_us(5000);assert_eq!(d.interrupt_pins(),[Some(false),Some(false)]);
        assert_eq!(read(&mut d,0x1d,1),[0x40]); // read doesn't shorten nonlatched pulse
        assert_eq!(d.next_deadline_us(),Some(5157));d.advance_to_us(5156);assert_eq!(d.interrupt_pins()[0],Some(false));
        d.advance_to_us(5157);assert_eq!(d.interrupt_pins()[0],Some(true));
        write(&mut d,0x58,&[0x40]);d.advance_to_us(10_000);
        assert_eq!(d.interrupt_pins(),[Some(true),Some(true)]);assert_eq!(read(&mut d,0x1d,1),[0xc0]);
        assert_eq!(vector(&read(&mut d,0x0c,6)),[1024,0,0]);
        write(&mut d,0x54,&[0x0e]);assert_eq!(d.interrupt_pins()[1],None); // open drain cannot drive high
        write(&mut d,0x7d,&[0]);assert_eq!(d.next_deadline_us(),None);assert_eq!(read(&mut d,0x1d,1),[0]);
    }
    #[test]
    fn latched_data_ready_releases_on_status_read_and_snapshot_is_atomic() {
        let sample=Arc::new(Mutex::new(Bmi270Sample{accel_g:[1.0,0.0,0.0],..Default::default()}));
        let mut d=Bmi270::new(sample.clone());d.advance_to_us(0);ready(&mut d);
        write(&mut d,0x53,&[8]);write(&mut d,0x55,&[1]);write(&mut d,0x58,&[4]);
        d.advance_to_us(10_000);d.advance_to_us(10_500);assert_eq!(d.interrupt_pins()[0],Some(false));
        assert_eq!(read(&mut d,0x1d,1),[0xc0]);assert_eq!(d.interrupt_pins()[0],Some(true));
        write(&mut d,0x0c,&[]);d.start(true);let first=d.read();
        sample.lock().unwrap().accel_g=[-1.0,0.0,0.0];d.advance_to_us(20_000);
        let mut bytes=vec![first];bytes.extend((0..5).map(|_|d.read()));assert_eq!(vector(&bytes),[4096,0,0]);
        assert_eq!(vector(&read(&mut d,0x0c,6)),[-4096,0,0]);
        write(&mut d,0x42,&[0xaf]);write(&mut d,0x40,&[0xaf]);assert_eq!(d.next_deadline_us(),None); // invalid ODR cannot shift/panic
        d.advance_to_us(u64::MAX);assert_eq!(d.interrupt_pins()[0],Some(true));
    }
    #[test]
    fn anymotion_counts_fifty_hz_samples_and_maps_the_selected_feature_bit() {
        let sample=Arc::new(Mutex::new(Bmi270Sample{accel_g:[0.0;3],..Default::default()}));
        let mut d=Bmi270::new(sample.clone());d.advance_to_us(0);ready(&mut d);write(&mut d,0x7d,&[4]);
        feature(&mut d,0,5,1,0xaa,7,true); // X-only,100ms,83mg,INT_STATUS0bit6
        write(&mut d,0x53,&[8]);write(&mut d,0x56,&[0x40]);
        sample.lock().unwrap().accel_g=[0.08,1.0,0.0];d.advance_to_us(100_000);
        assert_eq!(read(&mut d,0x1c,1),[0]); // belowthreshold + unselectedY
        sample.lock().unwrap().accel_g=[0.2,0.0,0.0];d.advance_to_us(180_000);
        assert_eq!(read(&mut d,0x1c,1),[0]);d.advance_to_us(200_000);
        assert_eq!(d.interrupt_pins()[0],Some(false));
        d.advance_to_us(220_000);assert_eq!(d.interrupt_pins()[0],Some(true)); // reference updates on assertion
        assert_eq!(read(&mut d,0x1c,1),[0x40]);assert_eq!(read(&mut d,0x1c,1),[0]);
        feature(&mut d,0,1,1,0xaa,3,true);sample.lock().unwrap().accel_g[0]=0.4;d.advance_to_us(240_000);
        assert_eq!(read(&mut d,0x1c,1),[4]);assert_eq!(d.interrupt_pins()[0],Some(true)); // out_conf reroutes status, wrongpinmap staysidle
        write(&mut d,0x56,&[4]);assert_eq!(d.interrupt_pins()[0],Some(false));
    }
    #[test]
    fn motion_latch_no_motion_and_low_power_bandwidth_boundaries() {
        let sample=Arc::new(Mutex::new(Bmi270Sample{accel_g:[0.0;3],..Default::default()}));
        let mut d=Bmi270::new(sample.clone());d.advance_to_us(0);ready(&mut d);write(&mut d,0x7d,&[4]);
        feature(&mut d,0,1,7,100,7,true);write(&mut d,0x53,&[8]);write(&mut d,0x55,&[1]);write(&mut d,0x56,&[0x60]);
        sample.lock().unwrap().accel_g=[1.0,0.0,0.0];d.advance_to_us(20_000);d.advance_to_us(40_000);
        assert_eq!(d.interrupt_pins()[0],Some(false));assert_eq!(read(&mut d,0x1c,1),[0x40]);assert_eq!(d.interrupt_pins()[0],Some(true));
        feature(&mut d,0,1,7,100,7,false);feature(&mut d,1,5,7,100,6,true);
        d.advance_to_us(140_000);assert_eq!(read(&mut d,0x1c,1),[0x20]); // stationary condition reasserts latched status
        assert_eq!(read(&mut d,0x1c,1),[0x20]);
        sample.lock().unwrap().accel_g=[0.0,0.0,0.0];d.advance_to_us(160_000);read(&mut d,0x1c,1);assert_eq!(d.interrupt_pins()[0],Some(true));
        write(&mut d,0x40,&[6]); // low-power25Hz isn't sufficient for motionfeatures
        d.advance_to_us(300_000);assert_eq!(read(&mut d,0x1c,1),[0]);
        write(&mut d,0x40,&[7]);d.advance_to_us(400_000);assert_eq!(read(&mut d,0x1c,1),[0x20]);
        write(&mut d,0x7e,&[0xb6]);assert_eq!(d.interrupt_pins(),[None,None]);assert_eq!(d.next_deadline_us(),None);
    }
    #[test]
    fn bmi_int1_motion_edge_wakes_the_corematrix_pmic_gpio4_path() {
        use crate::i2c::m5::{M5pm1,M5pm1State};
        let pm=Arc::new(Mutex::new(M5pm1State{input_mask:0x10,input_high:0x10,..Default::default()}));
        let mut pmic=M5pm1::new(pm.clone());
        fn pm_write(d:&mut M5pm1,r:u8,v:u8){d.start(false);d.write(r);d.write(v);}
        pm_write(&mut pmic,0x18,0x10);pm_write(&mut pmic,0x19,0);pm_write(&mut pmic,0x07,0x20); // L1/LDOhold
        let sample=Arc::new(Mutex::new(Bmi270Sample{accel_g:[0.0;3],..Default::default()}));
        let mut d=Bmi270::new(sample.clone());d.advance_to_us(0);ready(&mut d);
        feature(&mut d,0,5,7,0xaa,7,true);write(&mut d,0x53,&[8]);write(&mut d,0x55,&[1]);write(&mut d,0x56,&[0x40]);
        pm_write(&mut pmic,0x0c,0xa1);assert!(pm.lock().unwrap().ldo3v3_enabled);assert!(!pm.lock().unwrap().dcdc_enabled);
        sample.lock().unwrap().accel_g=[0.5,0.0,0.0];d.advance_to_us(80_000);
        pm.lock().unwrap().set_input(4,d.interrupt_pins()[0].unwrap());assert!(!pm.lock().unwrap().wake_requested);
        d.advance_to_us(100_000);pm.lock().unwrap().set_input(4,d.interrupt_pins()[0].unwrap());
        assert!(pm.lock().unwrap().wake_requested);assert_eq!(pm.lock().unwrap().wake_source(),0x20);
    }

}

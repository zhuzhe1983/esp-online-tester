//! I2C master controller (I2C0/I2C1) and the `I2cDevice` trait the board's bus devices implement.
//! The controller executes the command list (RSTART/WRITE/READ/STOP/END) written by the driver
//! after `trans_start`, clocking bytes between the FIFOs and addressed devices, and raises the
//! NACK / END_DETECT / TRANS_COMPLETE interrupts the IDF `i2c_master` driver waits for.
use crate::device::{Device, WriteEffect};
use crate::regram::RegRam;
use std::collections::VecDeque;
use emu_core::ClockDomain;

pub mod m5;
pub mod tm1680;
pub mod air_quality;
pub mod ft6336;
pub mod tca8418;
pub mod tca9555;
pub mod pi4ioe5v6408;
pub mod cst816s;
pub mod bmi270;
pub mod ina3221;
pub mod vl53l3cx;
pub mod mpu6886;
pub mod mlx90640;
pub mod lp5562;
pub mod st25r3916;
pub mod sht4x;
pub mod shtc3;
pub mod env4;
pub mod bh1750;
pub mod axp192;

pub trait I2cDevice {
    /// Current 7-bit address, sampled after each data write. Defaults to the attached address.
    fn address(&self, configured: u8) -> u8 { configured }
    /// Match the address and start the transfer; override for aliases or general calls.
    fn start_address(&mut self, configured: u8, address: u8, read: bool) -> bool {
        self.address(configured) == address && self.start(read)
    }
    /// Optional physical (SDA, SCL) attachment. None keeps controller-only addressing.
    fn pins(&self) -> Option<(u8, u8)> { None }
    /// Address phase: the master addressed this device for a read (`read`) or a write. Return ACK.
    fn start(&mut self, _read: bool) -> bool { true }
    /// One data byte from the master. Return ACK.
    fn write(&mut self, b: u8) -> bool;
    /// One data byte to the master.
    fn read(&mut self) -> u8;
    fn stop(&mut self) {}
    /// Cold address-phase observation. Nominal SCL comes from the master's registers,
    /// not a measured waveform; None means unavailable (including software I2C).
    /// Preserve address aliases/general calls and ACK behavior unless explicitly overridden.
    /// Appended to preserve existing vtable slots.
    fn start_address_with_clock(&mut self, configured: u8, address: u8, read: bool, _hz: Option<u32>) -> bool {
        self.start_address(configured, address, read)
    }
}

pub const INT_END_DETECT: u32 = 1 << 3;
pub const INT_TRANS_COMPLETE: u32 = 1 << 7;
pub const INT_TIMEOUT: u32 = 1 << 8;
pub const INT_NACK: u32 = 1 << 10;

pub struct I2c<const N: usize = 8> {
    pub regs: RegRam,
    tx: VecDeque<u8>,
    rx: VecDeque<u8>,
    pub int_raw: u32,
    pub int_ena: u32,
    /// Boxed so the controller keeps main's size and field layout.
    program: Box<Program<N>>,
    devices: Vec<(u8, Box<dyn I2cDevice>)>,
    cur: Vec<usize>,
    /// Controller clock ticks until the current command step completes; `None` while idle.
    remaining: Option<u64>,
    pins: Option<(u8, u8)>,
    expect_addr: bool,
    nack: bool,
    pub log: bool,
    pub transactions: u64,
    command_index: u8,
    byte_index: u8,
}

struct Program<const N: usize> {
    cmd: [u32; N],
    external_clock: Option<ClockParameters>,
}

#[derive(Clone, Copy)]
struct ClockParameters { num: u8, a: u8, b: u8, rc_fast: bool, explicit_hz:u32 }
impl ClockParameters {
    /// S3/C3 CLK_CONF and the existing C6 PCR adapter use six-bit A/B fields.
    fn from_config(config: u32) -> Self {
        Self { num: config as u8, a: ((config >> 8) & 63) as u8, b: ((config >> 14) & 63) as u8, rc_fast: config & (1 << 20) != 0,explicit_hz:0 }
    }
}

impl I2c { pub fn new() -> Self { Self::with_slots() } }
/// IDF v5.5.4 esp32 i2c_reg.h: sixteen slots; i2c_ll.h: opcodes 0/1/2/3/4.
impl I2c<16> { pub fn new_classic() -> Self { Self::with_slots() } }
// Non-generic method bodies keep one modern implementation across S3/C3/C6.
macro_rules! i2c_impl { ($n:literal) => {
impl I2c<$n> {
    fn with_slots() -> Self {
        I2c { regs: RegRam::new(), tx: VecDeque::new(), rx: VecDeque::new(), int_raw: 0, int_ena: 0, program: Box::new(Program { cmd: [0; $n], external_clock: None }),
              devices: Vec::new(), cur: Vec::new(), remaining: None, pins: None, expect_addr: false, nack: false, log: false, transactions: 0,
              command_index: 0, byte_index: 0 }
    }
    /// C6 supplies its PCR divider in the S3/C3 CLK_CONF layout.
    /// None restores this controller's local CLK_CONF register, including after a
    /// full-width override. Reserved bits retain the original ignored behavior.
    pub fn set_external_clock_config(&mut self, config: Option<u32>) { self.program.external_clock = config.map(ClockParameters::from_config); }
    /// Full-width divider = num + 1 + b/a; a=0 disables the fractional part.
    /// P4 IDF v5.5.3 register/hw_ver1/soc/hp_sys_clkrst_reg.h gives eight-bit
    /// NUMERATOR/DENOMINATOR fields (CTRL10 bits 17:10/25:18 for I2C0;
    /// CTRL11 bits 15:8/23:16 for I2C1). Its i2c_ll.h writes div_a to NUMERATOR.
    /// rc_fast chooses nominal 17.5 MHz; false chooses the 40 MHz crystal.
    pub fn set_external_clock_parameters(&mut self, num: u8, a: u8, b: u8, rc_fast: bool) {
        self.program.external_clock = Some(ClockParameters { num, a, b, rc_fast,explicit_hz:0 });
    }
    /// Opt-in source rate before the divider, for a separately validated SoC
    /// clock (C5 uses 48 MHz XTAL). Zero is rejected without changing state.
    /// Existing clock setters replace this override and retain their old rates.
    /// The SoC must gate stopped/unsupported sources before starting a transfer.
    pub fn set_external_clock_hz(&mut self,hz:u32,num:u8,a:u8,b:u8)->Result<(),&'static str>{
        if hz==0{return Err("I2C explicit source frequency must be nonzero");}
        self.program.external_clock=Some(ClockParameters{num,a,b,rc_fast:false,explicit_hz:hz});Ok(())
    }
    /// A device attached at an occupied address and pin pair replaces the one there: a board swapped before
    /// boot (`esp32sim_set_measured_te`) must not leave the old board's devices answering.
    pub fn attach(&mut self, addr: u8, dev: Box<dyn I2cDevice>) {
        match self.devices.iter_mut().find(|(attached, old)| *attached == addr && old.pins() == dev.pins()) {
            Some(slot) => slot.1 = dev,
            None => self.devices.push((addr, dev)),
        }
    }
    pub fn has_pinned_devices(&self) -> bool { self.devices.iter().any(|(_, d)| d.pins().is_some()) }
    /// Current controller route, supplied by the SoC before starting a command list.
    pub fn set_pins(&mut self, pins: Option<(u8, u8)>) {
        if self.pins != pins { self.cur.clear(); }
        self.pins = pins;
    }
    /// Remove a device, returning it so the host can move it to another bus or address.
    /// Other selected recipients remain active. With none left, writes NACK and reads return 0xff.
    pub fn detach(&mut self, addr: u8) -> Option<Box<dyn I2cDevice>> {
        let index = self.devices.iter().position(|(attached, _)| *attached == addr)?;
        self.cur.retain_mut(|cur| { if *cur == index { return false; } *cur -= usize::from(*cur > index); true });
        Some(self.devices.remove(index).1)
    }
    /// Remove every device without resetting registers, FIFOs or interrupt status.
    pub fn clear_devices(&mut self) {
        self.cur.clear();
        self.devices.clear();
    }
    pub fn has_device(&self, addr: u8) -> bool { self.devices.iter().any(|(attached, _)| *attached == addr) }
    #[inline(always)]
    pub fn is_active(&self) -> bool { self.remaining.is_some() }
    // Classic Arduino-ESP32 1.0.1/1.0.6 esp32-hal-i2c.c:i2cProcQueue enables
    // TXFIFO_EMPTY before TRANS_START and fills TX only in its ISR. These are
    // FIFO level conditions, independent of an active command/interrupt enable.
    // IDF 85c43024c (v3.3.5-1), esp32 i2c_reg.h: thresholds bits9:5/4:0;
    // RAW TXEMPTY/RXFULL bits1/0. Modern controller behavior is unchanged.
    fn raw_interrupts(&self) -> u32 {
        if $n != 16 { return self.int_raw; }
        let conf=self.regs.read(0x18);
        if conf & (1<<10)!=0 { return self.int_raw; } // non-FIFO access unsupported
        self.int_raw | (u32::from(self.tx.len() < ((conf>>5)&31) as usize)<<1)
            | u32::from(self.rx.len() > (conf&31) as usize)
    }
    pub fn irq(&self) -> bool {
        if self.int_raw & self.int_ena != 0 { return true; }
        // Keep disabled/non-FIFO interrupt observation as cheap as before.
        $n == 16 && self.int_ena & 3 != 0 && self.raw_interrupts() & self.int_ena != 0
    }

    pub fn read(&mut self, off: u32) -> u32 {
        match off {
            // IDF v5.5.4 S3 i2c_reg.h:173-179: BUS_BUSY bit 4. RESP_REC (bit 0; same position in
            // components/soc/{esp32s3,esp32c3,esp32c6}/register/soc/i2c_reg.h, IDF v5.5.4) is set while
            // the slave has acknowledged. The S3 and C6 headers describe it the other way round
            // ("0: ACK, 1: NACK"); an ESP32-C3 (rev v0.4) reads 1 after an acknowledged read and 0 after
            // a NACK, and esp-hal's blocking driver (1.0.0-rc.0, `check_errors`, every chip but the
            // classic ESP32) reports a data NACK when TRANS_COMPLETE is set and RESP_REC is clear. The
            // model had it inverted, so every esp-hal transaction whose completion the driver looked at
            // failed. The classic controller keeps its old value: nothing reads it there.
            0x08 => (if $n == 16 { self.nack } else { !self.nack } as u32) | ((self.is_active() as u32) << 4) | (u32::from($n == 16 && self.int_raw & INT_TIMEOUT != 0) << 2) | ((self.rx.len() as u32 & 0x3f) << 8) | ((self.tx.len() as u32 & 0x3f) << 18),   // SR: resp_rec, bus_busy, rxfifo_cnt, txfifo_cnt
            0x14 => ((self.rx.len() as u32 & 0x1f) << 5) | ((self.tx.len() as u32 & 0x1f) << 15),                        // FIFO_ST: waddr = count, raddr = 0
            0x1c => self.rx.pop_front().unwrap_or(0) as u32,
            0x20 => self.raw_interrupts(),
            0x28 => self.int_ena,
            0x2c => self.raw_interrupts() & self.int_ena,
            0x58..=0x74 => self.program.cmd[((off - 0x58) / 4) as usize],
            0x78..=0x94 if $n == 16 => self.program.cmd[((off - 0x58) / 4) as usize],
            _ => self.regs.read(off),
        }
    }

    /// FIFO reset bits are independent and may both be set by one write.
    #[allow(clippy::possible_missing_else)]
    pub fn write(&mut self, off: u32, v: u32) {
        match off {
            // IDF v5.5.4 components/soc/esp32s3/register/soc/i2c_reg.h:
            // TRANS_START bit 5 (72-78), FSM_RST bit 10 (111-117).
            // Reset cancellation of a pending callback is inferred.
            0x04 => {
                // Classic IDF v5.5.4 components/soc/esp32/register/soc/i2c_reg.h:
                // CTR has no FSM_RST field. Preserve unmodeled bit 10 in register RAM;
                // this readback policy is inferred, not a silicon claim for a reserved bit.
                let reset_mask = if $n == 16 { 0 } else { 1 << 10 };
                let reset = v & reset_mask != 0;
                self.regs.write(off, v & !((1 << 5) | reset_mask));
                if reset { self.remaining = None; self.cur.clear(); self.expect_addr = false; self.nack = false; }
                if v & (1 << 5) != 0 { self.run(); }
            }               // CTR.TRANS_START
            0x18 => { if v & (1 << 13) != 0 { self.tx.clear(); } if v & (1 << 12) != 0 { self.rx.clear(); } self.regs.write(off, v & !(3 << 12)); }
            0x1c => { if self.tx.len() < 32 { self.tx.push_back(v as u8); } }
            0x24 => self.int_raw &= !v,
            0x28 => self.int_ena = v,
            0x58..=0x74 => self.program.cmd[((off - 0x58) / 4) as usize] = v & !(1 << 31),
            0x78..=0x94 if $n == 16 => self.program.cmd[((off - 0x58) / 4) as usize] = v & !(1 << 31),
            _ => self.regs.write(off, v),
        }
    }

    fn op(&self) -> u32 {
        { let op = (self.program.cmd[usize::from(self.command_index)] >> 11) & 7;
          if $n == 16 { match op { 0 => 6, 2 => 3, 3 => 2, op => op } } else { op } }
    }

    // IDF v5.5.4 components/hal/esp32s3/include/hal/i2c_ll.h:205-235:
    // divider = NUM + 1 + B/A (the register-header A/B prose is reversed).
    // components/soc/esp32s3/register/soc/i2c_reg.h:1043-1074; C3 has the same fields.
    // C6 uses PCR instead. clk_tree_defs.h:39 (C6:47) gives nominal 17.5 MHz
    // RC_FAST; XTAL is modeled at 40 MHz and APB at 80 MHz. IDF 4.4 uses a
    // nominal 20 MHz RC clock; this model does not reproduce oscillator calibration.
    fn clock_ticks(&self, cycles: u64) -> u64 {
        let c = self.program.external_clock.unwrap_or_else(|| ClockParameters::from_config(self.regs.read(0x54)));
        let (a, b) = (u64::from(c.a), u64::from(c.b));
        let denominator = a.max(1);
        let divisor = (u64::from(c.num) + 1) * denominator + if a > 0 { b } else { 0 };
        let hz = if c.explicit_hz!=0 {u64::from(c.explicit_hz)} else if c.rc_fast { 17_500_000 } else { 40_000_000 };
        (cycles * divisor * 80_000_000).div_ceil(hz * denominator).max(1)
    }

    /// Same nominal source/divider and SCL periods as schedule(), sampled only at
    /// an address byte. Classic timing has a different layout and remains unknown.
    /// Also checked against IDF v5.5.1 esp32s3/include/hal/i2c_ll.h:165-186:
    /// low is programmed minus one; high and wait-high are literal.
    fn scl_hz(&self) -> Option<u32> {
        if $n == 16 { return None; }
        let high = self.regs.read(0x38);
        let period = (self.regs.read(0) & 0x1ff) + 1 + (high & 0x1ff) + ((high >> 9) & 127);
        Some((80_000_000 / self.clock_ticks(u64::from(period))) as u32)
    }

    // Inferred byte/ACK timing, not measured on hardware. IDF v5.5.4
    // components/hal/{esp32s3,esp32c3,esp32c6}/include/hal/i2c_ll.h
    // master_set_bus_timing (S3:175-201, C6:173-200): low/setup/hold are
    // programmed minus one; high and wait-high are literal. IDF v4.4.7
    // S3:160-171 and C3:164-175 use the same convention.
    // S3 i2c_reg.h:16-26, 932-1006 gives 9-bit periods and 7-bit wait-high.
    fn schedule(&mut self) {
        if usize::from(self.command_index) >= $n { self.remaining = None; return; }
        let mask = 0x1ff;
        let cycles = match self.op() {
            1 | 3 if self.program.cmd[usize::from(self.command_index)] & 255 != 0 => {
                let high = self.regs.read(0x38);
                let extra = (high >> 9) & 127;
                ((self.regs.read(0) & mask) + 1 + (high & mask) + extra) * 9
            }
            op @ (6 | 2) => {
                let off = if op == 6 { 0x40 } else { 0x48 };
                (self.regs.read(off) & mask) + (self.regs.read(off + 4) & mask) + 2
            }
            _ => 0,
        };
        self.remaining = Some(self.clock_ticks(cycles.into()));
    }

    fn run(&mut self) {
        if self.is_active() { return; }
        self.nack = false;
        self.transactions += 1;
        self.command_index = 0;
        self.byte_index = 0;
        self.schedule();
        // Classic adapter retains its synchronous transaction model; modern controllers
        // retain #199's scheduled byte callbacks and clock selection.
        if $n == 16 { self.advance(u64::MAX); }
    }

    fn advance(&mut self, mut ticks: u64) {
        while let Some(remaining) = self.remaining {
            if ticks < remaining { self.remaining = Some(remaining - ticks); return; }
            ticks -= remaining;
            self.step();
            if self.is_active() { self.schedule(); }
        }
    }

    fn step(&mut self) {
        let i = usize::from(self.command_index);
        let c = self.program.cmd[i];
        let n = c as u8;
        match self.op() {
            6 => self.expect_addr = true,
            1 if self.byte_index < n => {
                let b = self.tx.pop_front().unwrap_or(0);
                let ack = if self.expect_addr {
                    self.expect_addr = false;
                    let addr = b >> 1; let rd = b & 1 != 0;
                    self.cur.clear();
                    let hz = self.scl_hz();
                    for (k, (configured, device)) in self.devices.iter_mut().enumerate() {
                        if (device.pins().is_none() || device.pins() == self.pins) && device.start_address_with_clock(*configured, addr, rd, hz) {
                            self.cur.push(k);
                        }
                    }
                    if self.log { eprintln!("[i2c] start addr {addr:#04x} read={rd}"); }
                    !self.cur.is_empty()
                } else {
                    if self.log { eprintln!("[i2c]   write {b:#04x}"); }
                    let mut ack = false;
                    for &k in &self.cur {
                        let (addr, device) = &mut self.devices[k];
                        ack |= device.write(b);
                        *addr = device.address(*addr);
                    }
                    ack
                };
                if !ack && c & (1 << 8) != 0 {
                    self.nack = true; self.int_raw |= INT_NACK; self.program.cmd[i] |= 1 << 31;
                    self.cur.clear(); self.remaining = None;
                    return;
                }
                self.byte_index += 1;
                if self.byte_index < n { return; }
            }
            3 if self.byte_index < n => {
                let mut b = 0xff;
                for &k in &self.cur { b &= self.devices[k].1.read(); }
                if self.log { eprintln!("[i2c]   read  {b:#04x}"); }
                if self.rx.len() < 32 { self.rx.push_back(b); }
                self.byte_index += 1;
                if self.byte_index < n { return; }
            }
            1 | 3 => {}
            2 => {
                for &k in &self.cur { self.devices[k].1.stop(); }
                self.cur.clear(); self.int_raw |= INT_TRANS_COMPLETE; self.remaining = None;
            }
            4 => { self.int_raw |= INT_END_DETECT; self.remaining = None; }
            _ => self.remaining = None,
        }
        self.program.cmd[i] |= 1 << 31;
        self.command_index += 1;
        self.byte_index = 0;
    }

}

impl Device for I2c<$n> {
    fn read(&mut self, off: u32) -> u32 { I2c::<$n>::read(self, off) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { I2c::<$n>::write(self, off, v); WriteEffect::NONE }
    fn irq_sources(&self) -> u64 { self.irq() as u64 }
    fn clock(&self) -> Option<ClockDomain> { self.is_active().then_some(ClockDomain::Apb) }
    fn tick(&mut self, ticks: u64) { self.advance(ticks); }
    fn has_deadline(&self) -> bool { self.is_active() }
    fn next_deadline(&self) -> Option<u64> { self.remaining }
    fn debug(&mut self, on: bool) { self.log = on; }
}

} }
i2c_impl!(8);
i2c_impl!(16);

impl Default for I2c { fn default() -> Self { Self::new() } }

// ------------------------------------------------------------------ devices

/// Generic 8-bit-register device (audio codecs etc.): first written byte selects the register,
/// following bytes / reads auto-increment.
pub struct Reg8Device { pub name: &'static str, pub regs: [u8; 256], ptr: u8, first: bool }
impl Reg8Device {
    pub fn new(name: &'static str, defaults: &[(u8, u8)]) -> Self { let mut d = Reg8Device { name, regs: [0; 256], ptr: 0, first: true }; for &(r, v) in defaults { d.regs[r as usize] = v; } d }
}
impl I2cDevice for Reg8Device {
    fn start(&mut self, read: bool) -> bool { if !read { self.first = true; } true }
    fn write(&mut self, b: u8) -> bool { if self.first { self.ptr = b; self.first = false; } else { self.regs[self.ptr as usize] = b; self.ptr = self.ptr.wrapping_add(1); } true }
    fn read(&mut self) -> u8 { let v = self.regs[self.ptr as usize]; self.ptr = self.ptr.wrapping_add(1); v }
}


#[cfg(test)]
mod classic_tests {
    use super::*;
    #[test]
    fn classic_fifo_levels_prefill_before_start_and_reassert_until_serviced() {
        let mut c=I2c::new_classic();c.write(0x18,(3<<5)|2); // TX<3, RX>2
        assert_eq!(c.read(0x20)&3,2);assert!(!c.irq());c.write(0x28,3);assert!(c.irq());
        c.write(0x24,3);assert_eq!(c.read(0x2c)&3,2,"level survives W1C while FIFO remains empty");
        for byte in [0x68,0x28,0xcc] {c.write(0x1c,byte);}
        assert_eq!(c.read(0x20)&3,0);assert!(!c.irq());
        c.write(0x18,(3<<5)|2|(1<<13));assert!(c.irq(),"TX reset empties the FIFO");
        c.rx.extend([1,2]);assert_eq!(c.read(0x20)&1,0);c.rx.push_back(3);assert_eq!(c.read(0x20)&1,1);
        c.write(0x24,1);assert_eq!(c.read(0x20)&1,1);assert_eq!(c.read(0x1c),1);assert_eq!(c.read(0x20)&1,0);
        c.write(0x28,0);assert!(!c.irq());assert_eq!(c.read(0x20)&2,2,"RAW is independent of enable");
        c.write(0x18,(3<<5)|2|(1<<10));assert_eq!(c.read(0x20)&3,0,"non-FIFO mode does not inherit FIFO levels");
        let mut modern=I2c::new();modern.write(0x18,(3<<5)|2);modern.write(0x28,3);assert_eq!(modern.read(0x20)&3,0);assert!(!modern.irq(),"modern scheduled controller semantics unchanged");
    }
    #[test]
    fn classic_executes_sixteen_slots_and_old_stop_opcode() {
        let mut classic = I2c::new_classic();
        for i in 0..15 { classic.write(0x58 + 4 * i, 0); }
        classic.write(0x94, 3 << 11);
        classic.write(4, 1 << 5);
        assert_ne!(classic.read(0x94) & (1 << 31), 0);
        assert_ne!(classic.int_raw & INT_TRANS_COMPLETE, 0);
        let mut modern = I2c::new();
        modern.write(0x94, 3 << 11);
        modern.write(0x58, 2 << 11);
        modern.write(4, 1 << 5);
        modern.advance(u64::MAX);
        assert_eq!(modern.read(0x94), 3 << 11);
        assert_ne!(modern.int_raw & INT_TRANS_COMPLETE, 0);
    }
}

pub mod gt911;

#[cfg(test)]
mod clock_observation_tests {
    use super::*;
    use std::sync::{Arc,Mutex};
    struct Probe(Arc<Mutex<Vec<Option<u32>>>>);
    impl I2cDevice for Probe {
        fn pins(&self)->Option<(u8,u8)> {Some((47,48))}
        fn start_address_with_clock(&mut self,configured:u8,address:u8,_read:bool,hz:Option<u32>)->bool {
            if configured!=address {return false;}self.0.lock().unwrap().push(hz);true
        }
        fn write(&mut self,_b:u8)->bool {true}
        fn read(&mut self)->u8 {0}
    }
    fn send(i2c:&mut I2c,address:u8) {
        i2c.write(0x24,u32::MAX);i2c.write(0x18,3<<12);i2c.write(0x1c,u32::from(address<<1));
        i2c.write(0x58,6<<11);i2c.write(0x5c,(1<<11)|(1<<8)|1);i2c.write(0x60,2<<11);
        i2c.write(4,1<<5);i2c.advance(u64::MAX);
    }
    #[test]
    fn explicit_48mhz_clock_drives_byte_deadlines_and_device_observation(){
        let seen=Arc::new(Mutex::new(Vec::new()));let mut c=I2c::new();c.attach(0x6e,Box::new(Probe(seen.clone())));c.set_pins(Some((47,48)));
        c.write(0,59);c.write(0x38,60);c.set_external_clock_hz(48_000_000,0,0,0).unwrap();
        c.write(0x1c,0xdc);c.write(0x58,6<<11);c.write(0x5c,(1<<11)|(1<<8)|1);c.write(0x60,2<<11);c.write(4,1<<5);
        assert_eq!(c.next_deadline(),Some(4));c.advance(4);assert_eq!(c.next_deadline(),Some(1800));
        c.advance(1799);assert!(seen.lock().unwrap().is_empty());c.advance(1);assert_eq!(*seen.lock().unwrap(),[Some(400_000)]);
        c.advance(4);assert_ne!(c.int_raw&INT_TRANS_COMPLETE,0);assert!(!c.is_active());
        c.set_external_clock_hz(48_000_000,1,2,1).unwrap();send(&mut c,0x6e);assert_eq!(seen.lock().unwrap().last(),Some(&Some(160_000)));
    }
    #[test]
    fn explicit_zero_is_atomic_and_existing_setters_restore_legacy_sources(){
        let mut c=I2c::new();c.write(0,59);c.write(0x38,60);c.set_external_clock_hz(48_000_000,0,0,0).unwrap();
        assert!(c.set_external_clock_hz(0,255,255,255).is_err());assert_eq!(c.scl_hz(),Some(400_000));
        c.set_external_clock_parameters(0,0,0,false);assert_eq!(c.scl_hz(),Some(333_333));
        c.set_external_clock_hz(48_000_000,0,0,0).unwrap();c.set_external_clock_config(Some(0));assert_eq!(c.scl_hz(),Some(333_333));
        c.set_external_clock_hz(48_000_000,0,0,0).unwrap();c.set_external_clock_config(None);assert_eq!(c.scl_hz(),Some(333_333));
    }
    #[test]
    fn full_width_external_fraction_reaches_device_and_byte_deadlines() {
        let seen=Arc::new(Mutex::new(Vec::new()));let mut i2c=I2c::new();
        i2c.attach(0x6e,Box::new(Probe(seen.clone())));i2c.set_pins(Some((47,48)));
        i2c.write(0,49);i2c.write(0x38,50); // 100 source periods per SCL cycle.
        i2c.set_external_clock_parameters(0,128,64,false); // 40 MHz / (1 + 64/128).
        i2c.write(0x1c,0xdc);i2c.write(0x58,6<<11);
        i2c.write(0x5c,(1<<11)|(1<<8)|1);i2c.write(0x60,2<<11);i2c.write(4,1<<5);
        assert_eq!(i2c.next_deadline(),Some(6),"two source periods for START at 80 MHz APB");
        i2c.advance(5);assert_eq!(i2c.read(0x58)&(1<<31),0);
        i2c.advance(1);assert_ne!(i2c.read(0x58)&(1<<31),0);
        assert_eq!(i2c.next_deadline(),Some(2700),"9 SCL cycles for the address plus ACK");
        i2c.advance(2699);assert!(seen.lock().unwrap().is_empty());
        i2c.advance(1);assert_eq!(*seen.lock().unwrap(),[Some(266_666)]);
        assert_eq!(i2c.int_raw&(INT_NACK|INT_TRANS_COMPLETE),0);
        assert_eq!(i2c.next_deadline(),Some(6));i2c.advance(5);
        assert_eq!(i2c.int_raw&INT_TRANS_COMPLETE,0);i2c.advance(1);
        assert_ne!(i2c.int_raw&INT_TRANS_COMPLETE,0);assert_eq!(i2c.next_deadline(),None);

        // The upper bits remain meaningful at the maximum register values too.
        i2c.set_external_clock_parameters(255,255,254,false);send(&mut i2c,0x6e);
        assert_eq!(seen.lock().unwrap().last(),Some(&Some(1556)));
        // Both APIs replace, rather than stack on top of, a previous override.
        i2c.set_external_clock_config(Some(1));send(&mut i2c,0x6e);
        assert_eq!(seen.lock().unwrap().last(),Some(&Some(200_000)));
        i2c.set_external_clock_parameters(0,128,64,true);send(&mut i2c,0x6e);
        assert_eq!(seen.lock().unwrap().last(),Some(&Some(116_618)));
        i2c.write(0x54,3);i2c.set_external_clock_config(None);send(&mut i2c,0x6e);
        assert_eq!(seen.lock().unwrap().last(),Some(&Some(100_000)));
    }
    #[test]
    fn controller_samples_nominal_scl_only_on_the_matching_route_and_address() {
        let seen=Arc::new(Mutex::new(Vec::new()));let mut i2c=I2c::new();i2c.attach(0x6e,Box::new(Probe(seen.clone())));
        i2c.write(0,199);i2c.write(0x38,200); // 40 MHz / (200+200) = 100 kHz
        send(&mut i2c,0x6e);assert!(seen.lock().unwrap().is_empty());assert_ne!(i2c.int_raw&INT_NACK,0);
        i2c.set_pins(Some((48,47)));send(&mut i2c,0x6e);assert!(seen.lock().unwrap().is_empty());
        i2c.set_pins(Some((47,48)));send(&mut i2c,0x4f);assert!(seen.lock().unwrap().is_empty());
        send(&mut i2c,0x6e);assert_eq!(i2c.int_raw&INT_NACK,0);
        i2c.write(0,49);i2c.write(0x38,50);send(&mut i2c,0x6e);
        i2c.write(0x54,1);send(&mut i2c,0x6e); // integer source divider doubles
        i2c.write(0x54,(2<<8)|(1<<14));send(&mut i2c,0x6e); // 1 + 1/2
        assert_eq!(*seen.lock().unwrap(),[Some(100_000),Some(400_000),Some(200_000),Some(266_666)]);
        i2c.set_external_clock_config(Some(3));send(&mut i2c,0x6e);
        assert_eq!(seen.lock().unwrap().last(),Some(&Some(100_000)));
        // IDF v5.5.1's actual 40 MHz / 100 kHz and / 400 kHz calculations:
        // low=half_cycle-1, high=half_cycle-wait, wait=half_cycle/2-2.
        i2c.set_external_clock_config(None);i2c.write(0x54,0);
        for (low,high,wait,want) in [(199,102,98,100_000),(49,27,23,400_000)] {
            i2c.write(0,low);i2c.write(0x38,high|(wait<<9));send(&mut i2c,0x6e);
            assert_eq!(seen.lock().unwrap().last(),Some(&Some(want)));
        }
    }
    #[test]
    fn default_clock_hook_preserves_alias_and_general_call_dispatch() {
        struct Alias;
        impl I2cDevice for Alias {
            fn start_address(&mut self,configured:u8,address:u8,read:bool)->bool {configured==address||address==0&&!read}
            fn write(&mut self,_b:u8)->bool {true}
            fn read(&mut self)->u8 {0}
        }
        let mut i2c=I2c::new();i2c.attach(0x44,Box::new(Alias));
        send(&mut i2c,0);assert_eq!(i2c.int_raw&INT_NACK,0);
        send(&mut i2c,0x44);assert_eq!(i2c.int_raw&INT_NACK,0);
        send(&mut i2c,0x45);assert_ne!(i2c.int_raw&INT_NACK,0);
        assert_eq!(I2c::new_classic().scl_hz(),None);
    }
}

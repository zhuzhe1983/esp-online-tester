//! TI BQ27220 battery fuel gauge (I2C 0x55) as espressif/esp-iot-solution's bq27220 component
//! (components/sensors/battery_fuel_gauge/bq27220: bq27220.c, priv_include/bq27220_reg.h) drives
//! it. Registers are the gauge's standard commands: two little-endian bytes each, read from the
//! command address. CONTROL (0x00) takes a subcommand; its result is read from MACData (0x40) with
//! the subcommand echoed at 0x3E. Data memory is reached through 0x3E/0x3F: a two-byte write
//! selects a block (read back as the address echo, then the block at 0x40), a longer write stages
//! data, and MACDataSum/Len (0x60/0x61) commit it when the checksum (0xFF minus the byte sum of
//! address and data) and the length (data + 4) match. Security follows OperationStatus SEC
//! (bits 2:1; 3 sealed, 2 unsealed, 1 full access): UNSEAL keys 0x0414 then 0x3672, FULL ACCESS
//! key 0xFFFF twice; ENTER/EXIT_CFG_UPDATE (0x0090/0x0091/0x0092) set and clear CFGUPDATE (bit 10).
//! The battery itself is a fixed, healthy cell: 3.9 V, 25 C, no current, 80 % of 1000 mAh.
use esp_periph::i2c::I2cDevice;
use std::collections::HashMap;

pub const ADDRESS: u8 = 0x55;
const DEVICE_NUMBER: u16 = 0x0220;
const FW_VERSION: u16 = 0x0102;
const HW_VERSION: u16 = 0x00a0;
const SEC_SEALED: u16 = 3;
const SEC_UNSEALED: u16 = 2;
const SEC_FULL: u16 = 1;
const INITCOMP: u16 = 1 << 5;
const CFGUPDATE: u16 = 1 << 10;

pub struct Bq27220 {
    written: Vec<u8>,
    ptr: u8,
    /// 0x3E onwards as the gauge presents it: [address lo, address hi, 32 data bytes]
    mac: [u8; 34],
    /// staged block write (address, data) awaiting MACDataSum
    staged: Option<(u16, Vec<u8>)>,
    data_memory: HashMap<u16, u8>,
    sec: u16,
    cfg_update: bool,
    last_key: Option<u16>,
    pub commands: u64,
}

impl Default for Bq27220 { fn default() -> Self { Self::new() } }

impl Bq27220 {
    pub fn new() -> Self {
        Bq27220 { written: Vec::new(), ptr: 0, mac: [0; 34], staged: None, data_memory: HashMap::new(), sec: SEC_SEALED,
                  cfg_update: false, last_key: None, commands: 0 }
    }
    fn operation_status(&self) -> u16 { self.sec << 1 | INITCOMP | if self.cfg_update { CFGUPDATE } else { 0 } }
    fn standard(&self, cmd: u8) -> Option<u16> {
        Some(match cmd {
            0x02 | 0x0c | 0x14 | 0x1a | 0x22 | 0x24 | 0x2a | 0x32 => 0,  // rates, currents, counts: idle
            0x04 | 0x16 | 0x18 | 0x1c | 0x20 => 0xffff,                  // times: not applicable
            0x06 | 0x28 => 2982,                                          // 0.1 K: 25 C
            0x08 | 0x30 => 3900,                                          // mV
            0x0a => 0x0000,                                               // BatteryStatus: discharging, no alarms
            0x10 => 800, 0x12 | 0x3c => 1000,                             // mAh
            0x1e => 0xffff,
            0x2c => 80, 0x2e => 100,                                      // %
            0x3a => self.operation_status(),
            _ => return None,
        })
    }
    fn control(&mut self, sub: u16) {
        let result = match sub {
            0x0000 => Some(self.operation_status()),
            0x0001 => Some(DEVICE_NUMBER), 0x0002 => Some(FW_VERSION), 0x0003 => Some(HW_VERSION),
            0x0030 => { self.sec = SEC_SEALED; None }
            0x0090 if self.sec != SEC_SEALED => { self.cfg_update = true; None }
            0x0091 | 0x0092 => { self.cfg_update = false; None }
            0x3672 if self.last_key == Some(0x0414) && self.sec == SEC_SEALED => { self.sec = SEC_UNSEALED; None }
            0xffff if self.last_key == Some(0xffff) && self.sec == SEC_UNSEALED => { self.sec = SEC_FULL; None }
            _ => None,
        };
        self.last_key = Some(sub);
        if let Some(v) = result { self.mac = [0; 34]; self.mac[..2].copy_from_slice(&sub.to_le_bytes()); self.mac[2..4].copy_from_slice(&v.to_le_bytes()); }
    }
    fn select_block(&mut self, address: u16) {
        self.mac[..2].copy_from_slice(&address.to_le_bytes());
        for i in 0..32 { self.mac[2 + i] = self.data_memory.get(&address.wrapping_add(i as u16)).copied().unwrap_or(0); }
    }
    fn finish_write(&mut self) {
        let w = std::mem::take(&mut self.written);
        let Some((&cmd, data)) = w.split_first() else { return };
        self.commands += 1;
        match (cmd, data.len()) {
            (0x00, 2..) => self.control(u16::from_le_bytes([data[0], data[1]])),
            (0x3e, 2) => { self.staged = None; self.select_block(u16::from_le_bytes([data[0], data[1]])); }
            (0x3e, 3..) => self.staged = Some((u16::from_le_bytes([data[0], data[1]]), data[2..].to_vec())),
            (0x60, 2..) => if let Some((address, block)) = self.staged.take() {
                let sum = address.to_le_bytes().iter().chain(block.iter()).fold(0u8, |s, &b| s.wrapping_add(b));
                // Writes to data memory only take in full access or config update (TRM: sealed blocks writes).
                if 0xff - sum == data[0] && usize::from(data[1]) == block.len() + 4 && (self.sec == SEC_FULL || self.cfg_update) {
                    for (i, &b) in block.iter().enumerate() { self.data_memory.insert(address.wrapping_add(i as u16), b); }
                    self.select_block(address);
                }
            },
            _ => {}
        }
    }
}

impl I2cDevice for Bq27220 {
    fn start(&mut self, read: bool) -> bool {
        if !read { self.finish_write(); }
        true
    }
    fn write(&mut self, b: u8) -> bool {
        if self.written.is_empty() { self.ptr = b; }
        self.written.push(b);
        true
    }
    fn read(&mut self) -> u8 {
        // A read follows the command byte just written: drop that one-byte write.
        if self.written.len() == 1 { self.written.clear(); }
        let p = self.ptr;
        self.ptr = self.ptr.wrapping_add(1);
        if (0x3e..0x60).contains(&p) { return self.mac[usize::from(p - 0x3e)]; }
        if p == 0x60 { return 0xff - self.mac.iter().fold(0u8, |s, &b| s.wrapping_add(b)); }
        if p == 0x61 { return 36; }
        let v = self.standard(p & !1).unwrap_or(0);
        v.to_le_bytes()[usize::from(p & 1)]
    }
    fn stop(&mut self) { if self.written.len() > 1 { self.finish_write(); } }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(d: &mut Bq27220, bytes: &[u8]) { d.start(false); for &b in bytes { d.write(b); } d.stop(); }
    fn read(d: &mut Bq27220, cmd: u8, n: usize) -> Vec<u8> { d.start(false); d.write(cmd); d.start(true); let v = (0..n).map(|_| d.read()).collect(); d.stop(); v }
    fn u16_at(d: &mut Bq27220, cmd: u8) -> u16 { let v = read(d, cmd, 2); u16::from_le_bytes([v[0], v[1]]) }
    #[test]
    fn device_number_security_and_data_memory_as_the_driver_uses_them() {
        let mut d = Bq27220::new();
        write(&mut d, &[0x00, 0x01, 0x00]);
        assert_eq!(u16_at(&mut d, 0x40), 0x0220);
        assert_eq!(u16_at(&mut d, 0x08), 3900);
        assert_eq!(u16_at(&mut d, 0x3a) >> 1 & 3, SEC_SEALED);
        write(&mut d, &[0x00, 0x14, 0x04]); write(&mut d, &[0x00, 0x72, 0x36]);
        assert_eq!(u16_at(&mut d, 0x3a) >> 1 & 3, SEC_UNSEALED);
        write(&mut d, &[0x00, 0xff, 0xff]); write(&mut d, &[0x00, 0xff, 0xff]);
        assert_eq!(u16_at(&mut d, 0x3a) >> 1 & 3, SEC_FULL);
        write(&mut d, &[0x00, 0x90, 0x00]);
        assert_ne!(u16_at(&mut d, 0x3a) & CFGUPDATE, 0);
        // stage two bytes at 0x929d and commit them with their checksum
        write(&mut d, &[0x3e, 0x9d, 0x92, 0x0b, 0xb8]);
        let sum = [0x9du8, 0x92, 0x0b, 0xb8].iter().fold(0u8, |s, &b| s.wrapping_add(b));
        write(&mut d, &[0x60, 0xff - sum, 6]);
        write(&mut d, &[0x3e, 0x9d, 0x92]);
        assert_eq!(read(&mut d, 0x3e, 4), [0x9d, 0x92, 0x0b, 0xb8]);
        write(&mut d, &[0x00, 0x92, 0x00]);
        assert_eq!(u16_at(&mut d, 0x3a) & CFGUPDATE, 0);
    }
}

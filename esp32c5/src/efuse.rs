//! C5 eFuse digital read/reload controller with an explicit virtual MP rev100 profile.
//! Register/field sources: ESP-IDF v5.5.4@735507283d5b2f9fb363a1901172dbd9e847945d,
//! soc/esp32c5/register/soc/efuse_reg.h and efuse/esp32c5/esp_efuse_table.csv;
//! reset/reload, protection and completion: C5 TRM v1.1 sections 7.3.1.2/7.3.3.
//! OTP here is immutable, already-decoded logical data with valid coding, not a
//! captured die or an analog/RS decoder. No calibration, security or memory
//! capacity is invented. Programming, RS bypass and key-output modes reject.
const WORDS: [usize; 11] = [6, 6, 8, 8, 8, 8, 8, 8, 8, 8, 8];
const SHADOW_WORDS: usize = 84;
const READ_OPCODE: u32 = 0x5aa5;
const READ_DONE: u32 = 1;

fn virtual_mp_rev100_48mhz_profile() -> [[u32; 8]; 11] {
    let mut blocks = [[0; 8]; 11];
    // CSV BLOCK0 bit169: one set bit selects the board's 48 MHz XTAL in SPI boot.
    // Joint download still uses straps (bit172=0); it is not supported here.
    blocks[0][5] = 1 << 9;
    // Virtual locally administered MAC 02:00:00:c5:01:00, not a silicon identity.
    // CSV BLOCK1 bits0..47; the MAC byte order is highest byte first in the CSV.
    blocks[1][0] = 0x00c5_0100; blocks[1][1] = 0x0200;
    blocks[1][2] = 1 << 4; // wafer major1/minor0; no revision-check disable bits
    // Remaining fuses are explicitly unprovisioned: no key purpose, encryption,
    // secure boot, calibration or declared in-package flash/PSRAM capacity.
    blocks
}

pub(crate) struct C5Efuse {
    otp: [[u32; 8]; 11], shadow: [u32; SHADOW_WORDS],
    clock: u32, conf: u32, cmd_block: u32, int_raw: u32, int_ena: u32,
    date: u32, dac: u32, rd_timing: u32, wr_timing1: u32, wr_timing2: u32, wr_timing0: u32,
}
impl C5Efuse {
    pub(crate) fn new() -> Self {
        let mut e = Self { otp: virtual_mp_rev100_48mhz_profile(), shadow: [0; SHADOW_WORDS],
            clock: 2, conf: 0, cmd_block: 0, int_raw: 0, int_ena: 0,
            date: 37_818_960, dac: 19 | 255 << 9, rd_timing: 0x1201_0201,
            wr_timing1: 1 | 9831 << 8 | 1 << 24, wr_timing2: 320 | 160 << 16, wr_timing0: 1 << 13 };
        // TRM: system reset reloads readable banks before CPU boot. The digital
        // interrupt registers start at their documented reset values.
        e.reload(); e
    }
    fn reload(&mut self) {
        let rd_dis = self.otp[0][1] & 0x7f; // block4..10, CSV BLOCK0 bits32..38
        let mut at = 0;
        for (block, &words) in WORDS.iter().enumerate() {
            let protected = block >= 4 && rd_dis & (1 << (block - 4)) != 0;
            for word in 0..words { self.shadow[at + word] = if protected { 0 } else { self.otp[block][word] }; }
            at += words;
        }
    }
    // Same declared virtual identity as the digital eFuse profile, not a new
    // physical MAC. OTP is immutable and has no user override in this subset.
    pub(crate) fn logical_mac(&self)->[u8;6]{let lo=self.otp[1][0].to_be_bytes();let hi=self.otp[1][1].to_be_bytes();[hi[2],hi[3],lo[0],lo[1],lo[2],lo[3]]}
    pub(crate) fn irq(&self) -> bool { self.int_raw & self.int_ena != 0 }
    // C5 CSV BLOCK0 bits59..60 and ROM4002121c use DATA0+30[28:27].
    // The fixed lpwdt_ll.h DATA1 name conflicts with that header/ROM layout.
    pub(crate) fn wdt_delay_sel(&self) -> u8 { ((self.shadow[1] >> 27) & 3) as u8 }
    /// Fixed IDF5.5.3/5.5.4 C5 CSV/register layout: BLOCK0 bits116..117,
    /// bits80..82 parity and FORCE_USE_KEY_MANAGER_KEY bit75 (flash).
    /// Copy valid logical shadows only; no READ_CMD, OTP/key access or IRQ.
    pub(crate) fn boot_security_policy(&self) -> (u8,bool,bool) {
        let word=self.shadow[2];(((self.shadow[3]>>20)&3)as u8,((word>>16)&7).count_ones()&1!=0,word&(1<<11)!=0)
    }
    pub(crate) fn read(&mut self, off: u32) -> Option<u32> {
        if off & 3 != 0 { return None; }
        Some(match off {
            0x2c..=0x178 => self.shadow[((off - 0x2c) / 4) as usize],
            // Only valid logical OTP profiles exist. There is no physical-code
            // corruption injection or claim to repair an RS/repetition error.
            0x17c..=0x194 => 0,
            0x198 => self.date, 0x1c8 => self.clock, 0x1cc => self.conf,
            0x1d0 => 0, // ECDSA selectors stay disabled; nonzero writes reject
            0x1d4 => 1, // idle after completed power-on/explicit logical read
            0x1d8 => self.cmd_block, 0x1dc => self.int_raw,
            0x1e0 => self.int_raw & self.int_ena, 0x1e4 => self.int_ena, 0x1e8 => 0,
            0x1ec => self.dac, 0x1f0 => self.rd_timing, 0x1f4 => self.wr_timing1,
            0x1f8 => self.wr_timing2, 0x1fc => self.wr_timing0,
            _ => return None,
        })
    }
    pub(crate) fn write(&mut self, off: u32, value: u32) -> bool {
        if off & 3 != 0 { return false; }
        match off {
            // Writes cannot change read-only logical data, errors or status.
            0x2c..=0x194 | 0x1d4 | 0x1dc | 0x1e0 => {},
            0x198 => self.date = value & 0x0fff_ffff,
            0x1c8 => {
                // SRAM power-down needs a separate power model; do not let a
                // read appear to succeed after accepting an unmodeled power-off.
                if value & 1 != 0 { return false; } self.clock = value & 0x1_0006;
            }
            0x1cc => {
                let op = value & 0xffff;
                if !matches!(op, 0 | READ_OPCODE) { return false; } self.conf = op;
            }
            0x1d0 => { if value & 0xffff != 0 { return false; } },
            0x1d8 => {
                // Validate the whole command before touching shadows/interrupts.
                // PGM, reserved command bits, invalid blocks and wrong read
                // opcodes are unsupported, never successful no-op operations.
                let block = value & 0x3c;
                if value & !0x3d != 0 || block >> 2 > 10 || value & 1 != 0 && self.conf != READ_OPCODE { return false; }
                self.cmd_block = block;
                if value & 1 != 0 {
                    // Functional zero-latency read: the entire copy and read
                    // protection happen before SC completion/READ_DONE. No
                    // APB or analog latency claim is made by timing readback.
                    self.reload(); self.int_raw |= READ_DONE;
                }
            }
            0x1e4 => self.int_ena = value & 3,
            0x1e8 => self.int_raw &= !(value & 3),
            // Read/program timing can be configured without permitting a burn.
            // The functional model has no clock-domain crossing latency; UPDATE
            // is synchronous and self-clears, leaving its R/W fields readable.
            0x1ec => self.dac = value & 0x3_ffff,
            0x1f0 => self.rd_timing = value, 0x1f4 => self.wr_timing1 = value, 0x1f8 => self.wr_timing2 = value,
            0x1fc => { if value & 0xfff != 0 { return false; } self.wr_timing0 = value & 0x1f_e000; },
            // PGM staging, APB2OTP and key-manager/direct-code paths are absent.
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn read(e: &mut C5Efuse, off: u32) -> u32 { e.read(off).unwrap() }
    #[test]
    fn virtual_profile_has_exact_c5_identity_and_unprovisioned_security() {
        let mut e = C5Efuse::new();
        assert_eq!(read(&mut e, 0x40), 1 << 9); assert_eq!(read(&mut e, 0x44), 0x00c5_0100);
        assert_eq!(read(&mut e, 0x48), 0x0200); assert_eq!(read(&mut e, 0x4c), 0x10);
        for off in [0x2c, 0x30, 0x34, 0x38, 0x3c, 0x50, 0x54, 0x58, 0x6c, 0x9c, 0x178] { assert_eq!(read(&mut e, off), 0); }
        assert_eq!(read(&mut e, 0x1d4), 1); assert_eq!(read(&mut e, 0x1dc), 0); assert!(!e.irq());
        assert_eq!(e.wdt_delay_sel(), 0);
    }
    #[test]
    fn watchdog_delay_uses_actual_data0_field_and_reload() {
        let mut e = C5Efuse::new(); assert!(e.write(0x1cc, READ_OPCODE));
        for delay in 0..4 {
            e.otp[0][1] = delay << 27; e.otp[0][2] = (3-delay) << 27;
            assert!(e.write(0x1d8, 1)); assert_eq!(e.wdt_delay_sel(), delay as u8);
            assert_eq!(read(&mut e, 0x30) >> 27 & 3, delay);
        }
    }
    #[test]
    fn security_policy_observes_exact_shadow_fields_and_never_issues_a_reload() {
        let mut e=C5Efuse::new();assert_eq!(e.boot_security_policy(),(0,false,false));
        for crypt in 0u32..8{for dpa in 0..4{for forced in[false,true]{
            // Only private test OTP changes; production has no programming API.
            let old=e.boot_security_policy();e.otp[0][2]=crypt<<16|u32::from(forced)<<11;e.otp[0][3]=dpa<<20;
            assert_eq!(e.boot_security_policy(),old,"a getter cannot reload changed test OTP");
            assert!(e.write(0x1cc,READ_OPCODE));assert!(e.write(0x1d8,1));
            let shadow=e.shadow;let irq=(e.int_raw,e.int_ena,e.conf,e.cmd_block);
            let encrypted=[false,true,true,false,true,false,false,true][crypt as usize];
            for _ in 0..3{assert_eq!(e.boot_security_policy(),(dpa as u8,encrypted,forced));}
            assert_eq!(e.shadow,shadow);assert_eq!((e.int_raw,e.int_ena,e.conf,e.cmd_block),irq);
            assert!(e.write(0x34,u32::MAX));assert!(e.write(0x38,u32::MAX));assert_eq!(e.shadow,shadow,"guest RO writes cannot change policy");
        }}}
    }
    #[test]
    fn read_command_reloads_every_bank_before_done_and_preserves_read_only_data() {
        let mut e = C5Efuse::new();
        // Mutate only private logical OTP in this test. Production exposes no
        // programming API. Existing shadows must not change until READ_CMD.
        let mut expected = Vec::new();
        for (block, &words) in WORDS.iter().enumerate() {
            for (word, dest) in e.otp[block].iter_mut().enumerate().take(words) {
                let v = ((block + 1) as u32) << 24 | word as u32;
                *dest = v; expected.push(v);
            }
        }
        e.otp[0][1] = 0; expected[1] = 0; // no read protection in this scenario
        assert_eq!(read(&mut e, 0x4c), 0x10);
        assert!(!e.write(0x1d8, 1)); assert_eq!(read(&mut e, 0x1dc), 0);
        assert!(e.write(0x1cc, READ_OPCODE)); assert!(e.write(0x1d8, 1));
        for (word, &v) in expected.iter().enumerate() { assert_eq!(read(&mut e, 0x2c + 4 * word as u32), v); }
        assert_eq!(read(&mut e, 0x1d8), 0); assert_eq!(read(&mut e, 0x1dc), 1);
        assert!(e.write(0x44, u32::MAX)); assert_eq!(read(&mut e, 0x44), expected[6]);
        assert!(e.write(0x1d4, u32::MAX)); assert_eq!(read(&mut e, 0x1d4), 1);
    }
    #[test]
    fn each_read_protect_bit_hides_only_its_key_or_system_block() {
        for bit in 0..7 {
            let mut e = C5Efuse::new(); e.otp[0][1] = 1 << bit;
            for (block, data) in e.otp.iter_mut().enumerate().skip(3) { data.fill(0xc500_0000 | block as u32); }
            assert!(e.write(0x1cc, READ_OPCODE)); assert!(e.write(0x1d8, 1));
            assert_eq!(read(&mut e, 0x30) & 0x7f, 1 << bit);
            for block in 3..11 {
                let off = if block == 3 { 0x7c } else { 0x9c + (block - 4) * 0x20 };
                for word in 0..8 { assert_eq!(read(&mut e, off as u32 + word * 4), if block == bit + 4 { 0 } else { 0xc500_0000 | block as u32 }); }
            }
        }
    }
    #[test]
    fn read_done_is_sticky_masked_and_write_one_to_clear() {
        let mut e = C5Efuse::new(); assert!(e.write(0x1cc, READ_OPCODE)); assert!(e.write(0x1d8, 1));
        assert_eq!(read(&mut e, 0x1e0), 0); assert!(!e.irq());
        assert!(e.write(0x1e4, 3)); assert!(e.irq()); assert_eq!(read(&mut e, 0x1e0), 1);
        assert!(e.write(0x1e8, 2)); assert!(e.irq()); // PGM clear does not clear READ
        assert!(e.write(0x1dc, 0)); assert!(e.irq()); // RAW is not R/W
        assert!(e.write(0x1e8, 1)); assert!(!e.irq()); assert_eq!(read(&mut e, 0x1dc), 0);
        assert_eq!(read(&mut e, 0x1e8), 0); assert!(e.write(0x1d8, 1)); assert!(e.irq());
    }
    #[test]
    fn timing_defaults_masks_and_update_do_not_allow_bypass_or_programming() {
        let mut e = C5Efuse::new();
        for (off, v) in [(0x198, 37_818_960), (0x1c8, 2), (0x1ec, 0x1_fe13), (0x1f0, 0x1201_0201),
            (0x1f4, 0x0126_6701), (0x1f8, 0x00a0_0140), (0x1fc, 0x2000)] { assert_eq!(read(&mut e, off), v); }
        assert!(e.write(0x198, u32::MAX)); assert_eq!(read(&mut e, 0x198), 0x0fff_ffff);
        assert!(e.write(0x1c8, !1)); assert_eq!(read(&mut e, 0x1c8), 0x1_0006);
        assert!(!e.write(0x1c8, 1)); assert_eq!(read(&mut e, 0x1c8), 0x1_0006);
        assert!(e.write(0x1ec, u32::MAX)); assert_eq!(read(&mut e, 0x1ec), 0x3_ffff);
        for off in [0x1f0, 0x1f4, 0x1f8] { assert!(e.write(off, u32::MAX)); assert_eq!(read(&mut e, off), u32::MAX); }
        assert!(e.write(0x1fc, 0xffff_f000)); assert_eq!(read(&mut e, 0x1fc), 0x1f_e000);
        for v in [1, 2, 0x800, 0x1001] { assert!(!e.write(0x1fc, v)); assert_eq!(read(&mut e, 0x1fc), 0x1f_e000); }
    }
    #[test]
    fn unsupported_commands_reject_atomically_without_forging_completion() {
        let mut e = C5Efuse::new(); assert!(e.write(0x1cc, READ_OPCODE));
        for v in [2, 3, 0x2a, 11 << 2, 1 << 31] {
            assert!(!e.write(0x1d8, v)); assert_eq!(read(&mut e, 0x1d8), 0); assert_eq!(read(&mut e, 0x1dc), 0);
        }
        assert!(!e.write(0x1cc, 0x5a5a)); assert_eq!(read(&mut e, 0x1cc), READ_OPCODE);
        assert!(!e.write(0x1d0, 4)); assert_eq!(read(&mut e, 0x1d0), 0);
        for off in [0, 0x20, 0x19c, 0x1c4, 0x500, 0x708, 0x1000, u32::MAX] {
            assert_eq!(e.read(off), None); assert!(!e.write(off, 1));
        }
        for off in [0x2d, 0x1cd, 0x1d9, 0x1ff] { assert_eq!(e.read(off), None); assert!(!e.write(off, 0)); }
        for off in (0x17c..=0x194).step_by(4) { assert_eq!(read(&mut e, off), 0); assert!(e.write(off, u32::MAX)); assert_eq!(read(&mut e, off), 0); }
    }
}

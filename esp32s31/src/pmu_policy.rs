//! S31 PMU mode-policy register subset, separate from mode transitions.
//! ESP-IDF6.1 fff9895c82d744c7237be8847347bdd1b07c6643 pmu_reg.h,
//! esp_hal_pmu/esp32s31/pmu_ll.h and esp_hw_support/port/esp32s31/pmu_init.c.
//! Registers with generated need_des prose are not treated as analog simulation.
use emu_core::Fault;
const BASE: u32 = 0x2070_4000;
// (offset,RW mask,WT mask,header reset). ACTIVE_CK and LP_ACTIVE_CK remain
// owned by Power for the already implemented CPLL/MPLL/FOSC controls.
const CONFIG: &[(u32, u32, u32, u32)] = &[
    (0x00, 0xfff00000, 0x00000000, 0x00000000), // HP_ACTIVE_DIG_POWER
    (0x04, 0xffffffff, 0x00000000, 0xffffffff), // HP_ACTIVE_ICG_HP_FUNC
    (0x08, 0xffffffff, 0x00000000, 0xffffffff), // HP_ACTIVE_ICG_HP_APB
    (0x0c, 0xffffffff, 0x00000000, 0xffffffff), // HP_ACTIVE_ICG_HP_FUNC1
    (0x10, 0xffffffff, 0x00000000, 0xffffffff), // HP_ACTIVE_ICG_HP_APB1
    (0x14, 0xc0000000, 0x00000000, 0x00000000), // HP_ACTIVE_ICG_MODEM
    (0x18, 0x3f800000, 0x00000000, 0x00000000), // HP_ACTIVE_HP_SYS_CNTL
    (0x20, 0xfe000000, 0x00000000, 0x00000000), // HP_ACTIVE_BIAS
    (0x24, 0x6fffdcf0, 0x00000000, 0x00000000), // HP_ACTIVE_BACKUP
    (0x28, 0xffffffff, 0x00000000, 0x00000000), // HP_ACTIVE_BACKUP_CLK
    (0x2c, 0xffffffff, 0x00000000, 0x00000000), // HP_ACTIVE_BACKUP1_CLK
    (0x30, 0xfc000000, 0x00000000, 0x00000000), // HP_ACTIVE_SYSCLK
    (0x34, 0xffffc000, 0x00000008, 0xc667f180), // HP_ACTIVE_HP_REGULATOR0
    (0x38, 0xffffff00, 0x00000000, 0x00000000), // HP_ACTIVE_HP_REGULATOR1
    (0x3c, 0x80000000, 0x00000000, 0x80000000), // HP_ACTIVE_XTAL
    (0x40, 0xfff00000, 0x00000000, 0x00000000), // HP_MODEM_DIG_POWER
    (0x44, 0xffffffff, 0x00000000, 0xffffffff), // HP_MODEM_ICG_HP_FUNC
    (0x48, 0xffffffff, 0x00000000, 0xffffffff), // HP_MODEM_ICG_HP_APB
    (0x4c, 0xffffffff, 0x00000000, 0xffffffff), // HP_MODEM_ICG_HP_FUNC1
    (0x50, 0xffffffff, 0x00000000, 0xffffffff), // HP_MODEM_ICG_HP_APB1
    (0x54, 0xc0000000, 0x00000000, 0x00000000), // HP_MODEM_ICG_MODEM
    (0x58, 0x3f800000, 0x00000000, 0x00000000), // HP_MODEM_HP_SYS_CNTL
    (0x5c, 0x7ff80000, 0x00000000, 0x00000000), // HP_MODEM_HP_CK_POWER
    (0x60, 0xfe000000, 0x00000000, 0x00000000), // HP_MODEM_BIAS
    (0x64, 0x21f0cc30, 0x00000000, 0x00000000), // HP_MODEM_BACKUP
    (0x68, 0xffffffff, 0x00000000, 0x00000000), // HP_MODEM_BACKUP_CLK
    (0x6c, 0xffffffff, 0x00000000, 0x00000000), // HP_MODEM_BACKUP1_CLK
    (0x70, 0xfc000000, 0x00000000, 0x00000000), // HP_MODEM_SYSCLK
    (0x74, 0xffff8000, 0x00000000, 0xc6678000), // HP_MODEM_HP_REGULATOR0
    (0x78, 0xffffff00, 0x00000000, 0x00000000), // HP_MODEM_HP_REGULATOR1
    (0x7c, 0x80000000, 0x00000000, 0x80000000), // HP_MODEM_XTAL
    (0x80, 0xfff00000, 0x00000000, 0x00000000), // HP_SLEEP_DIG_POWER
    (0x84, 0xffffffff, 0x00000000, 0xffffffff), // HP_SLEEP_ICG_HP_FUNC
    (0x88, 0xffffffff, 0x00000000, 0xffffffff), // HP_SLEEP_ICG_HP_APB
    (0x8c, 0xffffffff, 0x00000000, 0xffffffff), // HP_SLEEP_ICG_HP_FUNC1
    (0x90, 0xffffffff, 0x00000000, 0xffffffff), // HP_SLEEP_ICG_HP_APB1
    (0x94, 0xc0000000, 0x00000000, 0x00000000), // HP_SLEEP_ICG_MODEM
    (0x98, 0x3f800000, 0x00000000, 0x00000000), // HP_SLEEP_HP_SYS_CNTL
    (0x9c, 0x7ff80000, 0x00000000, 0x00000000), // HP_SLEEP_HP_CK_POWER
    (0xa0, 0xfe000000, 0x00000000, 0x00000000), // HP_SLEEP_BIAS
    (0xa4, 0xffff37c0, 0x00000000, 0x00000000), // HP_SLEEP_BACKUP
    (0xa8, 0xffffffff, 0x00000000, 0x00000000), // HP_SLEEP_BACKUP_CLK
    (0xac, 0xffffffff, 0x00000000, 0x00000000), // HP_SLEEP_BACKUP1_CLK
    (0xb0, 0xfc000000, 0x00000000, 0x00000000), // HP_SLEEP_SYSCLK
    (0xb4, 0xffff8000, 0x00000000, 0xc6678000), // HP_SLEEP_HP_REGULATOR0
    (0xb8, 0xffffff00, 0x00000000, 0x00000000), // HP_SLEEP_HP_REGULATOR1
    (0xbc, 0x80000000, 0x00000000, 0x80000000), // HP_SLEEP_XTAL
    (0xc0, 0xffe00000, 0x00000000, 0xc6600000), // HP_SLEEP_LP_REGULATOR0
    (0xc4, 0xf0000000, 0x00000000, 0x00000000), // HP_SLEEP_LP_REGULATOR1
    (0xcc, 0xc0000000, 0x00000000, 0x00000000), // HP_SLEEP_LP_DIG_POWER
    (0xd8, 0xffe00000, 0x00000000, 0xc6600000), // LP_SLEEP_LP_REGULATOR0
    (0xdc, 0xf0000000, 0x00000000, 0x00000000), // LP_SLEEP_LP_REGULATOR1
    (0xe0, 0x80000000, 0x00000000, 0x80000000), // LP_SLEEP_XTAL
    (0xe4, 0xc0000000, 0x00000000, 0x00000000), // LP_SLEEP_LP_DIG_POWER
    (0xe8, 0xf0000000, 0x00000000, 0x40000000), // LP_SLEEP_LP_CK_POWER
    (0xec, 0xfe000000, 0x00000000, 0x00000000), // LP_SLEEP_BIAS
    (0x120, 0xf80007ff, 0, 0x1c), // force domain0 controls
    (0x124, 0xf80007ff, 0, 0x1c), // force domain1 controls
    (0x128, 0xf80007ff, 0, 0x1c), // force domain2 controls
    (0x12c, 0xf80007ff, 0, 0x1c), // force domain3 controls
    (0x130, 0xf80007ff, 0, 0x1c), // force domain4 controls
    (0x134, 0xf80007ff, 0, 0x1c), // force domain5 controls
    (0x138, 0x0000003f, 0, 0x1c), // LP peripheral force controls
    (0x13c, 0xff0000ff, 0, 0xff000000), // HP memory force controls
    (0x1e8, 0x80000000, 0, 0), // PSRAM_XPD MPLL PHY power,ldo_ll_enable unit0
    (0x218, 0x000003ff, 0, 0), // EXT_LDO_CTRL requested dref/mul/rail/ripple/current controls
    (0x15c, 0x0003ffff, 0, 0), // SLP_WAKEUP_CNTL3 parameters,not a sleep trigger
];
pub struct PmuPolicy {
    regs: [u32; 135], applied_func: [u32; 2], applied_apb: [u32; 2], modem_code: u32,
    pub updates: u64, pub dbias_requests: u64, pub readonly_write_attempts: u64,
}
impl Default for PmuPolicy {
    fn default() -> Self {
        let mut regs = [0; 135]; for &(off, _, _, reset) in CONFIG { regs[off as usize / 4] = reset; }
        Self { regs, applied_func: [u32::MAX; 2], applied_apb: [u32::MAX; 2], modem_code: 0, updates: 0, dbias_requests: 0, readonly_write_attempts: 0 }
    }
}
impl PmuPolicy {
    fn spec(offset: u32) -> Option<(u32, u32, u32)> { CONFIG.iter().find(|&&(o, _, _, _)| o == offset).map(|&(_, rw, wt, reset)| (rw, wt, reset)) }
    pub fn is_mmio(address: u32) -> bool {
        let off = (address & !3).wrapping_sub(BASE); Self::spec(off).is_some() || matches!(off, 0xf8 | 0xfc | 0x100 | 0x104)
    }
    pub fn psram_enabled(&self) -> bool { self.regs[0x1e8 / 4] & (1 << 31) != 0 }
    pub fn applied_clocks(&self) -> ([u32; 2], [u32; 2], u32) { (self.applied_func, self.applied_apb, self.modem_code) }
    pub fn read(&self, address: u32) -> Result<u32, Fault> {
        if address & 3 != 0 { return Err(Fault::Misaligned); }
        let off = address.wrapping_sub(BASE);
        if Self::spec(off).is_some() { return Ok(self.regs[off as usize / 4]); }
        if matches!(off, 0xf8 | 0xfc | 0x100 | 0x104) { return Ok(0); } // WT commands
        Err(Fault::Unmapped)
    }
    pub fn write(&mut self, address: u32, value: u32) -> Result<(), Fault> {
        if address & 3 != 0 { return Err(Fault::Misaligned); }
        let off = address.wrapping_sub(BASE);
        if let Some((rw, wt, reset)) = Self::spec(off) {
            let v = value & rw;
            // Active power loss,CPU stall,pad-hold or XTAL shutdown cannot be
            // represented by continuing execution. Sleep policies are only stored.
            if off == 0 && v != 0 || off == 0x18 && v & !0x0080_0000 != 0 || off == 0x3c && v == 0 || off == 0xcc && v != 0 { return Err(Fault::Prohibited); }
            // Deasserting force-up/no-reset/no-isolate hands control to the
            // retained active policy. Actual reset/isolation/powerdown is absent.
            if (0x120..=0x138).contains(&off) && v & !0x1c != 0 || off == 0x13c && v & 255 != 0 { return Err(Fault::Prohibited); }
            if off == 0x34 {
                // Header calls DBIAS_VOL fields RO,while LL attempts writes.
                // Follow RO semantics and preserve this discrepancy. WT init is
                // recorded as a request,not analog completion or measured voltage.
                if (value ^ reset) & 0x3ff0 != 0 { self.readonly_write_attempts = self.readonly_write_attempts.wrapping_add(1); }
                if value & wt != 0 { self.dbias_requests = self.dbias_requests.wrapping_add(1); }
            }
            self.regs[off as usize / 4] = v | (reset & !rw & !wt); return Ok(());
        }
        match off {
            0xfc | 0x100 | 0x104 => {
                if value & 0x8000_0000 == 0 { return Ok(()); }
                if off == 0x104 { self.modem_code = self.regs[0x14 / 4]; }
                else {
                    let offsets = if off == 0xfc { [4, 12] } else { [8, 16] };
                    let next = offsets.map(|i| self.regs[i / 4]);
                    // Partial clock gating has not been routed to every device.
                    // Reject it atomically,not silently leave disabled blocks alive.
                    if next != [u32::MAX; 2] { return Err(Fault::Prohibited); }
                    if off == 0xfc { self.applied_func = next; } else { self.applied_apb = next; }
                }
            }
            0xf8 => {
                if value & 0xe000_0000 != 0 { return Err(Fault::Prohibited); } // no clock-source/sleep selection transition
                if value & 0x1000_0000 == 0 { return Ok(()); }
                if self.regs[0x30 / 4] & 0x3800_0000 != 0x0800_0000 { return Err(Fault::Prohibited); } // active ICG path only
            }
            _ => return Err(Fault::Unmapped),
        }
        self.updates = self.updates.wrapping_add(1); Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sleep_policies_and_force_release_do_not_apply_powerdown_to_active_mode() {
        let mut p = PmuPolicy::default(); p.write(BASE + 0x80, u32::MAX).unwrap();
        assert_eq!(p.read(BASE + 0x80), Ok(0xfff00000)); assert_eq!(p.read(BASE), Ok(0));
        assert_eq!(p.write(BASE, 1 << 29), Err(Fault::Prohibited)); assert_eq!(p.read(BASE), Ok(0));
        for off in (0x120..=0x138).step_by(4) {
            assert_eq!(p.read(BASE + off), Ok(0x1c)); p.write(BASE + off, 0).unwrap();
            for bad in [1, 2, 32, 64] {
                if off == 0x138 && bad == 64 { continue; } // reserved,masked out
                assert_eq!(p.write(BASE + off, bad), Err(Fault::Prohibited)); assert_eq!(p.read(BASE + off), Ok(0));
            }
        }
        p.write(BASE + 0x13c, 0).unwrap(); assert_eq!(p.write(BASE + 0x13c, 1), Err(Fault::Prohibited));
        assert_eq!(p.write(BASE + 0x13c, 16), Err(Fault::Prohibited)); assert_eq!(p.read(BASE + 0x13c), Ok(0));
        assert_eq!(p.read(BASE + 0x150), Err(Fault::Unmapped)); // sleep request is not implemented
    }
    #[test]
    fn bias_readonly_header_fields_and_request_are_not_fake_voltage_completion() {
        let mut p = PmuPolicy::default(); let initial = p.read(BASE + 0x34).unwrap();
        let changed = (initial & !0x3ff0) | (13 << 4) | (28 << 9);
        p.write(BASE + 0x34, changed | 8).unwrap();
        assert_eq!(p.read(BASE + 0x34), Ok(initial)); assert_eq!(p.dbias_requests, 1); assert_eq!(p.readonly_write_attempts, 1);
        p.write(BASE + 0x34, initial & !(1 << 14)).unwrap();
        assert_eq!(p.read(BASE + 0x34), Ok(initial & !(1 << 14))); assert_eq!(p.dbias_requests, 1);
    }
    #[test]
    fn clock_policy_update_validates_active_bank_and_is_atomic_on_unsupported_gating() {
        let mut p = PmuPolicy::default(); p.write(BASE + 0x88, 0).unwrap(); // sleeping APB only
        p.write(BASE + 0x100, 1 << 31).unwrap(); assert_eq!(p.applied_apb, [u32::MAX; 2]);
        p.write(BASE + 8, !1024).unwrap(); assert_eq!(p.applied_apb, [u32::MAX; 2]);
        assert_eq!(p.write(BASE + 0x100, 1 << 31), Err(Fault::Prohibited)); assert_eq!(p.updates, 1);
        assert_eq!(p.applied_apb, [u32::MAX; 2]); p.write(BASE + 8, u32::MAX).unwrap();
        p.write(BASE + 0x100, 1 << 31).unwrap(); assert_eq!(p.read(BASE + 0x100), Ok(0));
        p.write(BASE + 0x30, 0x08000000).unwrap(); p.write(BASE + 0xf8, 1 << 28).unwrap();
        p.write(BASE + 0x30, 0x28000000).unwrap(); assert_eq!(p.write(BASE + 0xf8, 1 << 28), Err(Fault::Prohibited));
        assert_eq!(p.write(BASE + 0xf8, 1 << 31), Err(Fault::Prohibited));
    }
}

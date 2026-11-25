//! C5 clocked PC/SP and CPU access monitors, driven only by actual execution samples.
//! ESP-IDF v5.5.4@735507283d5b2f9fb363a1901172dbd9e847945d:
//! soc/esp32c5/register/soc/{bus_monitor,pcr}_reg.h, soc/interrupts.h,
//! hal/esp32c5/include/hal/assist_debug_ll.h; C5 TRM1.1 sections9.1/20.5.
const BASE: u32 = 0x600c_2000;
const PCR_ASSIST: u32 = 0x6009_6100;
const SP_MIN: u32 = 1 << 8;
const SP_MAX: u32 = 1 << 9;
const MONITORS: u32 = 0x3ff;

#[derive(Clone, Copy)]
pub(crate) enum CpuBus { Data, Peripheral }
/// Only real HP CPU data/peripheral transfers belong here, never fetch/DMA/host
/// accesses. The caller identifies the actual bus; address guesses are not used.
pub(crate) struct CpuAccess { pub bus: CpuBus, pub address: u32, pub width: u8, pub write: bool, pub pc: u32, pub sp: Option<u32> }
pub(crate) struct C5BusMonitor {
    pcr: u32, auto_gate: u32, date: u32, monitor: u32, int_ena: u32, int_state: u32,
    ranges: [(u32, u32); 4], area_condition: u32, sp_condition: u32, current_sp: Option<u32>,
    area_pc: u32, area_sp: Option<u32>, sp_min: u32, sp_max: u32, sp_pc: u32,
    record: u32, record_pc: u32, record_sp: Option<u32>, last_exception_pc: u32,
}
impl C5BusMonitor {
    pub(crate) fn new() -> Self {
        Self { pcr: 0, auto_gate: 1, date: 34_640_176, monitor: 0, int_ena: 0, int_state: 0,
            ranges: [(u32::MAX, 0); 4], area_condition: 0, sp_condition: 0, current_sp: None,
            area_pc: 0, area_sp: Some(0), sp_min: 0, sp_max: u32::MAX, sp_pc: 0,
            record: 0, record_pc: 0, record_sp: Some(0), last_exception_pc: 0 }
    }
    pub(crate) fn contains(address: u32) -> bool { (BASE..=BASE + 0x3fc).contains(&address) || address == PCR_ASSIST }
    // assist_debug_ll_enable_bus_clock controls the PCR bit. Functional
    // observations use CPU_CLK (TRM9.2), so an APB gate is not a PC/SP freeze.
    fn running(&self) -> bool { self.pcr & 2 == 0 }
    fn raw(&self) -> u32 { (self.area_condition | self.sp_condition) & self.monitor }
    fn complete(&self) -> bool { self.current_sp.is_some() || self.monitor & (SP_MIN | SP_MAX) == 0 }
    fn sample_irq(&mut self) { self.int_state = self.raw() & self.int_ena; }
    fn update_sp_condition(&mut self) {
        self.sp_condition = self.current_sp.map_or(0, |sp| if sp < self.sp_min { SP_MIN } else { 0 } | if sp > self.sp_max { SP_MAX } else { 0 });
    }
    /// IRQ is the hidden sampled state, not RAW & ENA computed on demand.
    /// Gating APB does not stop the functional CPU_CLK observation state.
    pub(crate) fn irq(&self) -> bool { self.int_state != 0 }
    pub(crate) fn source(&self) -> (usize, bool) { (27, self.irq()) }
    /// One actual CPU-context sample is one functional observation clock. The
    /// return value is false if an enabled SP operation has no real SP sample.
    pub(crate) fn observe_cpu(&mut self, pc: u32, sp: Option<u32>) -> bool {
        if !self.running() { return true; }
        self.area_condition = 0; self.current_sp = sp; self.update_sp_condition();
        if self.record & 3 == 3 { self.record_pc = pc; self.record_sp = sp; }
        if self.record & 2 != 0 && self.sp_condition & self.monitor != 0 { self.sp_pc = pc; }
        // With missing SP, preserve its previous hidden IRQ bits instead of
        // inventing a deassertion. RAW reads reject until context is complete.
        if self.complete() { self.sample_irq(); } else { self.int_state &= SP_MIN | SP_MAX; }
        self.complete() && (self.record & 3 != 3 || sp.is_some())
    }
    pub(crate) fn observe_access(&mut self, access: CpuAccess) -> bool {
        if !matches!(access.width, 1 | 2 | 4) || access.address & (u32::from(access.width) - 1) != 0 { return false; }
        let Some(last) = access.address.checked_add(u32::from(access.width) - 1) else { return false; };
        if !self.running() { return true; }
        let complete = self.observe_cpu(access.pc, access.sp);
        let first = match access.bus { CpuBus::Data => 0, CpuBus::Peripheral => 2 };
        for i in first..first + 2 {
            let (min, max) = self.ranges[i];
            if min <= max && access.address <= max && last >= min { self.area_condition |= 1 << (2 * i + usize::from(access.write)); }
        }
        if self.area_condition & self.monitor != 0 && self.record & 2 != 0 { self.area_pc = access.pc; self.area_sp = access.sp; }
        let old_sp = self.int_state & (SP_MIN | SP_MAX); self.sample_irq();
        if !self.complete() { self.int_state = self.int_state & !(SP_MIN | SP_MAX) | old_sp; }
        complete && (self.area_condition & self.monitor == 0 || self.record & 2 == 0 || access.sp.is_some())
    }
    /// Caller supplies the actual faulting/exception instruction, never mtvec or
    /// a function-hook guess. No recursive-lockup or timeout engine is modeled.
    pub(crate) fn observe_exception(&mut self, pc: u32) { if self.running() { self.last_exception_pc = pc; } }
    /// CPU Reset affects only the CPU and RCD_EN. Invalidate input context; the
    /// peripheral configuration and special PC/SP capture registers survive.
    pub(crate) fn cpu_reset(&mut self) { self.record = 0; self.current_sp = None; self.area_condition = 0; self.sp_condition = 0; }
    /// HP Core Reset resets peripherals, except the explicit reset-PC/SP
    /// capture retention in TRM20.5.2. System/LP reset retention is not inferred.
    pub(crate) fn core_reset(&mut self) {
        let (pc, sp) = (self.record_pc, self.record_sp); *self = Self::new(); self.record_pc = pc; self.record_sp = sp;
    }
    pub(crate) fn read(&self, address: u32) -> Option<u32> {
        if address & 3 != 0 { return None; }
        if address == PCR_ASSIST { return Some(self.pcr); }
        if self.pcr & 1 == 0 { return None; }
        let off = address.checked_sub(BASE)?;
        Some(match off {
            0 => self.monitor, 4 => { if !self.complete() { return None; } self.raw() }, 8 => self.int_ena, 0xc => 0,
            0x10..=0x2c => { let i = ((off - 0x10) / 8) as usize; if off & 4 == 0 { self.ranges[i].0 } else { self.ranges[i].1 } },
            0x30 => self.area_pc, 0x34 => return self.area_sp, 0x38 => self.sp_min, 0x3c => self.sp_max, 0x40 => self.sp_pc,
            0x44 => self.record, 0x48 => self.record_pc, 0x4c => return self.record_sp,
            0x70 => self.last_exception_pc, 0x74 => 0, // no attached RISC-V debug module
            0x100 | 0x104 => 0, // timeout engines remain at reset; nonzero writes reject
            0x108 => self.auto_gate, 0x3fc => self.date, _ => return None,
        })
    }
    pub(crate) fn write(&mut self, address: u32, value: u32) -> bool {
        if address & 3 != 0 { return false; }
        if address == PCR_ASSIST {
            if value & 2 != 0 { *self = Self::new(); } self.pcr = value & 3; return true;
        }
        if self.pcr & 3 != 1 { return false; }
        let Some(off) = address.checked_sub(BASE) else { return false; };
        match off {
            0 => { if value & !MONITORS != 0 { return false; } self.monitor = value; },
            8 => { if value & !MONITORS != 0 { return false; } self.int_ena = value; },
            // C5 LL pseudo-RTL: CLR affects hidden IRQ state, not live RAW.
            0xc => { self.int_state &= !(value & MONITORS); return true; },
            0x10..=0x2c => { let i = ((off - 0x10) / 8) as usize; if off & 4 == 0 { self.ranges[i].0 = value; } else { self.ranges[i].1 = value; } },
            0x38 => { self.sp_min = value; self.update_sp_condition(); }, 0x3c => { self.sp_max = value; self.update_sp_condition(); },
            0x44 => self.record = value & 3,
            4 | 0x30 | 0x34 | 0x40 | 0x48 | 0x4c | 0x70 | 0x74 => return true, // read-only captures/status
            0x100 | 0x104 => return value & 0xfffff == 0,
            0x108 => self.auto_gate = value & 1, 0x3fc => self.date = value & 0x0fff_ffff,
            _ => return false,
        }
        // Register transactions are functional observation clocks. PC/SP
        // capture itself never happens here without an actual context sample.
        if self.complete() { self.sample_irq(); } true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn w(m: &mut C5BusMonitor, off: u32, v: u32) { assert!(m.write(BASE + off, v), "offset {off:#x}"); }
    fn r(m: &C5BusMonitor, off: u32) -> u32 { m.read(BASE + off).unwrap() }
    fn enabled() -> C5BusMonitor { let mut m = C5BusMonitor::new(); assert!(m.write(PCR_ASSIST, 1)); m }
    fn access(bus: CpuBus, address: u32, write: bool, pc: u32, sp: Option<u32>) -> CpuAccess { CpuAccess { bus, address, width: 4, write, pc, sp } }
    #[test]
    fn reset_data_clock_gates_and_both_record_enables_control_actual_captures() {
        let mut m = C5BusMonitor::new(); assert_eq!(m.read(PCR_ASSIST), Some(0)); assert_eq!(m.read(BASE + 0x48), None);
        assert!(m.observe_cpu(1, Some(2))); assert!(m.write(PCR_ASSIST, 1));
        assert_eq!(r(&m, 0x48), 0); assert_eq!(r(&m, 0x4c), 0); assert_eq!(r(&m, 0x108), 1); assert_eq!(r(&m, 0x3fc), 34_640_176);
        for mode in [0, 1, 2] { w(&mut m, 0x44, mode); assert!(m.observe_cpu(10, Some(20))); assert_eq!(r(&m, 0x48), 0); }
        w(&mut m, 0x44, 3); assert!(m.observe_cpu(0x4000_1234, Some(0x4085_e000)));
        assert_eq!(r(&m, 0x48), 0x4000_1234); assert_eq!(r(&m, 0x4c), 0x4085_e000);
        w(&mut m, 0x108, 0); assert!(m.observe_cpu(30, Some(40))); assert_eq!(r(&m, 0x48), 30); // autogating is not off
        assert!(m.write(PCR_ASSIST, 0)); assert!(m.observe_cpu(50, Some(60))); assert!(m.write(PCR_ASSIST, 1)); assert_eq!(r(&m, 0x48), 50);
        w(&mut m, 0x48, 999); w(&mut m, 0x4c, 999); assert_eq!(r(&m, 0x4c), 60);
    }
    #[test]
    fn missing_sp_is_unavailable_instead_of_a_fabricated_valid_capture() {
        let mut m = enabled(); w(&mut m, 0x44, 3);
        assert!(!m.observe_cpu(0x1234, None)); assert_eq!(r(&m, 0x48), 0x1234); assert_eq!(m.read(BASE + 0x4c), None);
        w(&mut m, 0, SP_MIN); assert_eq!(m.read(BASE + 4), None);
        assert!(m.observe_cpu(0x1238, Some(0x4080_1000))); assert_eq!(r(&m, 0x4c), 0x4080_1000); assert_eq!(r(&m, 4), 0);
    }
    #[test]
    fn sp_raw_is_live_clear_targets_hidden_irq_and_violations_reassert() {
        let mut m = enabled(); w(&mut m, 0x38, 100); w(&mut m, 0x3c, 200); w(&mut m, 0x44, 2); w(&mut m, 0, SP_MIN | SP_MAX);
        for sp in [100, 150, 200] { assert!(m.observe_cpu(sp, Some(sp))); assert_eq!(r(&m, 4), 0); assert!(!m.irq()); }
        assert!(m.observe_cpu(10, Some(99))); assert_eq!(r(&m, 4), SP_MIN); assert!(!m.irq()); assert_eq!(r(&m, 0x40), 10);
        w(&mut m, 8, SP_MIN | SP_MAX); assert_eq!(m.source(), (27, true)); w(&mut m, 0xc, SP_MIN);
        assert!(!m.irq()); assert_eq!(r(&m, 4), SP_MIN); // unlike ordinary RAW/W1C blocks
        assert!(m.observe_cpu(12, Some(99))); assert!(m.irq()); assert_eq!(r(&m, 0x40), 12);
        w(&mut m, 0xc, 0); assert!(m.irq()); w(&mut m, 4, 0); assert_eq!(r(&m, 4), SP_MIN);
        assert!(m.observe_cpu(14, Some(150))); assert_eq!(r(&m, 4), 0); assert!(!m.irq()); // LL: state samples RAW, no sticky OR
        assert!(m.observe_cpu(16, Some(201))); assert_eq!(r(&m, 4), SP_MAX); assert!(m.irq());
        w(&mut m, 0, 0); assert_eq!(r(&m, 4), 0); assert!(!m.irq());
    }
    #[test]
    fn every_region_bus_direction_has_its_exact_c5_monitor_bit_and_capture() {
        for i in 0..4 {
            for write in [false, true] {
                let mut m = enabled(); let address = if i < 2 { 0x4080_1000 + i * 0x100 } else { 0x6000_1000 + i * 0x100 };
                w(&mut m, 0x10 + i * 8, address); w(&mut m, 0x14 + i * 8, address + 3);
                let bit = 1 << (2 * i + u32::from(write)); w(&mut m, 0, bit); w(&mut m, 8, bit); w(&mut m, 0x44, 2);
                let bus = if i < 2 { CpuBus::Data } else { CpuBus::Peripheral };
                assert!(m.observe_access(access(bus, address, write, 0x1234, Some(0x4085_1000))));
                assert_eq!(r(&m, 4), bit); assert!(m.irq()); assert_eq!(r(&m, 0x30), 0x1234); assert_eq!(r(&m, 0x34), 0x4085_1000);
                w(&mut m, 0xc, bit); assert!(!m.irq()); assert_eq!(r(&m, 4), bit);
                assert!(m.observe_cpu(0x1238, Some(0x4085_1000))); assert_eq!(r(&m, 4), 0); assert!(!m.irq());
            }
        }
    }
    #[test]
    fn explicit_bus_tag_direction_and_overlapping_bytes_prevent_false_matches() {
        let mut m = enabled(); w(&mut m, 0x10, 0x4080_0011); w(&mut m, 0x14, 0x4080_0012); w(&mut m, 0, 2); w(&mut m, 8, 2);
        assert!(m.observe_access(access(CpuBus::Peripheral, 0x4080_0010, true, 1, Some(100)))); assert_eq!(r(&m, 4), 0);
        assert!(m.observe_access(access(CpuBus::Data, 0x4080_0010, false, 2, Some(100)))); assert_eq!(r(&m, 4), 0);
        assert!(m.observe_access(access(CpuBus::Data, 0x4080_0010, true, 3, Some(100)))); assert_eq!(r(&m, 4), 2);
        assert!(m.irq()); assert_eq!(r(&m, 0x30), 0); // no fabricated PC while PDEBUG output is disabled
        assert!(m.observe_access(access(CpuBus::Data, 0x4080_0014, true, 4, Some(100)))); assert_eq!(r(&m, 4), 0); assert!(!m.irq());
        w(&mut m, 0x10, u32::MAX); w(&mut m, 0x14, 0); // empty reset-style interval
        assert!(m.observe_access(access(CpuBus::Data, 0x4080_0010, true, 5, Some(100)))); assert_eq!(r(&m, 4), 0);
    }
    #[test]
    fn region_missing_sp_keeps_actual_pc_but_does_not_invent_sp() {
        let mut m = enabled(); w(&mut m, 0x10, 0x4080_0000); w(&mut m, 0x14, 0x4080_ffff); w(&mut m, 0, 1); w(&mut m, 0x44, 2);
        assert!(!m.observe_access(access(CpuBus::Data, 0x4080_0010, false, 0x4000_4567, None)));
        assert_eq!(r(&m, 0x30), 0x4000_4567); assert_eq!(m.read(BASE + 0x34), None); assert_eq!(r(&m, 4), 1);
    }
    #[test]
    fn apb_gate_preserves_functional_cpu_observation_and_pcr_reset_clears_captures() {
        let mut m = enabled(); w(&mut m, 0x44, 3); w(&mut m, 0x38, 100); w(&mut m, 0, SP_MIN); w(&mut m, 8, SP_MIN);
        assert!(m.observe_cpu(1, Some(99))); assert!(m.irq()); assert!(m.write(PCR_ASSIST, 0));
        assert!(m.observe_cpu(2, Some(101))); assert!(!m.irq()); assert_eq!(m.read(BASE + 0x48), None);
        assert!(!m.write(BASE + 0x44, 0)); // APB transactions are gated, CPU_CLK observation is not
        assert!(m.write(PCR_ASSIST, 1)); assert_eq!(r(&m, 0x48), 2); assert!(m.observe_cpu(3, Some(101))); assert!(!m.irq());
        assert!(m.write(PCR_ASSIST, 3)); assert_eq!(r(&m, 0x48), 0); assert_eq!(r(&m, 0x4c), 0); assert_eq!(r(&m, 0x44), 0);
        assert!(!m.write(BASE + 0x44, 3)); assert!(m.observe_cpu(999, Some(999))); assert_eq!(r(&m, 0x48), 0);
        assert!(m.write(PCR_ASSIST, 1)); assert_eq!(r(&m, 0), 0); assert_eq!(r(&m, 8), 0); assert!(!m.irq());
    }
    #[test]
    fn cpu_reset_and_hp_core_reset_have_distinct_capture_retention() {
        let mut m = enabled(); w(&mut m, 0x44, 3); w(&mut m, 0x38, 100); w(&mut m, 8, SP_MIN);
        assert!(m.observe_cpu(0x4000_1234, Some(0x4085_0000))); m.observe_exception(0x4000_5678);
        m.cpu_reset(); assert_eq!(r(&m, 0x44), 0); assert_eq!(r(&m, 0x48), 0x4000_1234); assert_eq!(r(&m, 0x4c), 0x4085_0000);
        assert_eq!(r(&m, 0x38), 100); assert_eq!(r(&m, 8), SP_MIN);
        assert!(m.observe_cpu(0x4000_0000, Some(0))); assert_eq!(r(&m, 0x48), 0x4000_1234);
        m.core_reset(); assert_eq!(m.read(PCR_ASSIST), Some(0)); assert!(m.write(PCR_ASSIST, 1));
        assert_eq!(r(&m, 0x48), 0x4000_1234); assert_eq!(r(&m, 0x4c), 0x4085_0000); assert_eq!(r(&m, 0x44), 0);
        assert_eq!(r(&m, 0x38), 0); assert_eq!(r(&m, 8), 0); assert_eq!(r(&m, 0x70), 0);
        assert_eq!(r(&enabled(), 0x48), 0); // fresh POR instance does not import captures
    }
    #[test]
    fn exception_capture_is_actual_read_only_and_unknown_engines_reject() {
        let mut m = enabled(); assert_eq!(r(&m, 0x70), 0); m.observe_exception(0x4000_abcd); assert_eq!(r(&m, 0x70), 0x4000_abcd);
        w(&mut m, 0x70, 0); assert_eq!(r(&m, 0x70), 0x4000_abcd); assert_eq!(r(&m, 0x74), 0);
        for off in [0x50, 0x6c, 0x78, 0x10c, 0x400] { assert_eq!(m.read(BASE + off), None); assert!(!m.write(BASE + off, 1)); }
        for off in [0x100, 0x104] { assert_eq!(r(&m, off), 0); assert!(!m.write(BASE + off, 1)); assert!(m.write(BASE + off, 0)); }
        assert!(!m.write(BASE, 1 << 12)); assert!(!m.write(BASE + 8, 1 << 12)); assert_eq!(r(&m, 0), 0);
        assert_eq!(m.read(BASE + 1), None); assert!(!m.write(BASE + 1, 1)); assert_eq!(m.read(0), None);
        assert!(C5BusMonitor::contains(PCR_ASSIST)); assert!(C5BusMonitor::contains(BASE)); assert!(!C5BusMonitor::contains(BASE + 0x400));
    }
    #[test]
    fn invalid_access_samples_do_not_create_captures_or_irq() {
        let mut m = enabled(); w(&mut m, 0x44, 3); w(&mut m, 0, 0x3ff); w(&mut m, 8, 0x3ff);
        for (address, width) in [(0, 0), (0, 3), (1, 2), (2, 4), (u32::MAX, 4)] {
            assert!(!m.observe_access(CpuAccess { bus: CpuBus::Data, address, width, write: true, pc: 123, sp: Some(456) }));
            assert_eq!(r(&m, 0x48), 0); assert!(!m.irq());
        }
        w(&mut m, 0x3fc, u32::MAX); assert_eq!(r(&m, 0x3fc), 0x0fff_ffff);
    }
}

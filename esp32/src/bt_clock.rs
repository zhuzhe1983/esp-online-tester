//! Functional classic ESP32 native-clock sampler, NOT a radio/controller implementation.
//! ROM rev300 r_ld_read_clock / r_ea_time_get_slot_rounded establish +0x1c SAMP/busy,
//! low28 CLKN and +0x20 fine-time reads. Bluetooth native clock: 3.2kHz (312.5us).
//! Public reverse-engineering reference: docs/platform/stick-k016-c-bt-native-functional.md.
//! Explicit simulation policy (not measured silicon): 1us configurable completion,
//! 2MHz descending fine phase 624..0, held atomic snapshots, coalesced busy requests,
//! dedicated LC/BB clock gating and a conservative reset union. No wall clock/PC hooks.
const PERIOD: u64 = 75_000; // 240MHz reference ticks / 3200Hz
const FINE_TICK: u64 = 120; // 0.5us, 625 fine counts per half-slot
const MASK: u32 = 0x0fff_ffff;
pub const BUSY: u32 = 1 << 31;

pub struct BtNativeClock {
    enabled: bool,
    bb_enabled: bool,
    sleeping: bool,
    lp_resumed: bool,
    slot_event: bool,
    in_reset: bool,
    coarse: u32,
    phase: u64,
    snapshot: (u32, u32),
    pending: Option<u64>,
    latency: u64,
    pub requests: u64,
    pub completed: u64,
}
impl Default for BtNativeClock {
    fn default() -> Self { Self::with_latency_ticks(240) }
}
impl BtNativeClock {
    /// Positive enabled-domain guest reference ticks; zero is clamped to one.
    /// This is a functional-model parameter, NOT a calibrated hardware latency.
    pub fn with_latency_ticks(latency: u64) -> Self {
        Self { enabled:false, bb_enabled:false, sleeping:false, lp_resumed:false, slot_event:false, in_reset:false, coarse:0, phase:0, snapshot:(0,624), pending:None,
            latency:latency.max(1), requests:0, completed:0 }
    }
    pub fn clock_enabled(&self) -> bool { self.enabled }
    pub fn set_control(&mut self, enabled: bool, reset: bool) {
        if reset { let latency=self.latency; *self=Self::with_latency_ticks(latency); }
        self.in_reset=reset;
        self.enabled=enabled && !reset;
    }
    /// Separate always-retained timebase supply from the gated LC sampler.
    pub fn set_bb_clock(&mut self, enabled: bool) { self.bb_enabled=enabled; }
    pub fn enter_lp_sleep(&mut self) { self.sleeping=true; self.lp_resumed=false; }
    pub fn take_slot_event(&mut self) -> bool { std::mem::take(&mut self.slot_event) }
    fn counter_enabled(&self) -> bool {
        !self.in_reset && !self.sleeping && (self.enabled || self.lp_resumed && self.bb_enabled)
    }
    pub fn read(&self, offset: u32) -> u32 {
        match offset { 0x1c=>self.snapshot.0 | if self.pending.is_some() {BUSY} else {0},
            0x20=>self.snapshot.1, _=>0 }
    }
    pub fn write(&mut self, offset: u32, value: u32) {
        // RO timestamp bits cannot be seeded by the guest. A zero write cannot
        // cancel a pending sample; repeated requests cannot postpone completion.
        if !self.in_reset && offset==0x1c && value & BUSY!=0 && self.pending.is_none() {
            self.pending=Some(self.latency); self.requests=self.requests.wrapping_add(1);
        }
    }
    /// Apply a completed, explicitly requested LP correction to the free-running
    /// counter while retaining held samples and pending sampling requests. The
    /// LP peripheral authorizes this even when the LC clock is gated; reset wins.
    pub fn apply_lp_correction(&mut self, reference_ticks: u64) {
        if !self.in_reset {
            self.count(reference_ticks); // correction alone never creates a clock interrupt
            self.sleeping=false; self.lp_resumed=true;
        }
    }
    fn count(&mut self, ticks: u64) {
        let phase=self.phase+ticks%PERIOD;
        let steps=ticks/PERIOD+phase/PERIOD;
        self.phase=phase%PERIOD;
        self.coarse=self.coarse.wrapping_add(steps as u32)&MASK;
    }
    fn count_live(&mut self, ticks:u64) {
        // Native IRQ0 calls r_ld_fm_clk_isr, which consumes CLKN>>1 (625us).
        // Generate an event only when that same live counter crosses a full-slot
        // boundary, never on mask writes, sampled reads, correction or a host timer.
        let halfslots=ticks/PERIOD+(self.phase+ticks%PERIOD)/PERIOD;
        self.slot_event|=(halfslots+u64::from(self.coarse&1))/2!=0;
        self.count(ticks);
    }
    pub fn advance(&mut self, mut ticks: u64) {
        if !self.counter_enabled() { return; }
        if let Some(left)=self.pending.filter(|_|self.enabled) {
            if ticks<left { self.count_live(ticks); self.pending=Some(left-ticks); return; }
            // Capture at the deadline, not at the arbitrary interpreter batch end.
            self.count_live(left); ticks-=left;
            self.snapshot=(self.coarse,624-(self.phase/FINE_TICK) as u32);
            self.pending=None; self.completed=self.completed.wrapping_add(1);
        }
        self.count_live(ticks);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn enabled(latency:u64)->BtNativeClock {let mut c=BtNativeClock::with_latency_ticks(latency);c.set_control(true,false);c}
    #[test]
    fn request_busy_is_not_read_driven_or_guest_clearable() {
        let mut c=enabled(240);c.write(0x1c,BUSY|MASK);
        for _ in 0..100 {assert_eq!(c.read(0x1c),BUSY);}
        c.advance(239);assert_ne!(c.read(0x1c)&BUSY,0);c.write(0x1c,0);c.write(0x1c,BUSY);
        c.advance(1);assert_eq!(c.read(0x1c),0);assert_eq!(c.read(0x20),622);
        assert_eq!((c.requests,c.completed),(1,1));
    }
    #[test]
    fn atomic_snapshot_held_and_batch_size_independent() {
        let mut a=enabled(200);let mut b=enabled(200);
        for c in [&mut a,&mut b] {c.advance(PERIOD-100);c.write(0x1c,BUSY);}
        a.advance(PERIOD*4);for _ in 0..PERIOD*4 {b.advance(1);}
        assert_eq!((a.read(0x1c),a.read(0x20)),(1,624));
        assert_eq!((a.read(0x1c),a.read(0x20)),(b.read(0x1c),b.read(0x20)));
        a.write(0x20,0xffff);assert_eq!(a.read(0x20),624);
    }
    #[test]
    fn gate_freezes_time_and_pending_deadline_reset_cancels() {
        let mut c=enabled(240);c.write(0x1c,BUSY);c.advance(120);c.set_control(false,false);
        c.advance(u64::MAX);assert_eq!(c.read(0x1c),BUSY);c.set_control(true,false);
        c.advance(119);assert_eq!(c.completed,0);c.advance(1);assert_eq!(c.completed,1);
        c.write(0x1c,BUSY);c.set_control(true,true);c.advance(1000);
        assert_eq!((c.read(0x1c),c.read(0x20)),(0,624));assert!(!c.clock_enabled());
        c.write(0x1c,BUSY);assert_eq!(c.read(0x1c),0,"reset-held writes are ignored");
        c.set_control(true,false);c.write(0x1c,BUSY);c.advance(240);assert_eq!(c.completed,1);
    }
    #[test]
    fn monotonic_halfslots_wrap_and_extreme_deltas_do_not_overflow() {
        let mut c=enabled(1);c.advance(u64::from(MASK)*PERIOD);c.write(0x1c,BUSY);c.advance(1);
        assert_eq!(c.read(0x1c),MASK);c.advance(PERIOD-1);c.write(0x1c,BUSY);c.advance(1);
        assert_eq!(c.read(0x1c),0);c.advance(PERIOD*3);c.write(0x1c,BUSY);c.advance(1);
        assert_eq!(c.read(0x1c),3);c.advance(u64::MAX);assert!(c.read(0x20)<625);
    }
    #[test]
    fn zero_latency_still_requires_guest_time_and_fine_phase_spans_wakeup_boundary() {
        let mut c=enabled(0);c.write(0x1c,BUSY);assert_eq!(c.read(0x1c),BUSY);c.advance(1);
        assert!(c.read(0x20)>467);c.advance(120*200);c.write(0x1c,BUSY);c.advance(1);
        assert!(c.read(0x20)<467);
    }
    #[test]
    fn bus_light_sleep_freezes_sampler_and_guest_reboot_discards_pending_state() {
        use emu_core::Bus;
        let mut b=crate::bus::SocBus::new(4<<20,[0;6]);
        b.write32(0x3ff0_00cc,(1<<11)|(1<<16)|(1<<17)).unwrap();
        b.write32(0x3ff7_101c,BUSY).unwrap();b.light_sleep=true;b.tick(10000);
        assert_eq!(b.read32(0x3ff7_101c).unwrap(),BUSY,"digital sampler freezes in modeled light sleep");
        b.light_sleep=false;b.tick(240);assert_eq!(b.periph.bt_clock.completed,1);
        b.write32(0x3ff7_101c,BUSY).unwrap();esp_soc::SocBus::reboot(&mut b,[0;6]);
        assert_eq!((b.read32(0x3ff7_101c).unwrap(),b.read32(0x3ff7_1020).unwrap()),(0,624));
        assert!(!b.periph.bt_clock.clock_enabled());assert_eq!(b.periph.bt_clock.requests,0);
    }
    #[test]
    fn soc_mmio_dport_gates_resets_and_siblings_are_not_claimed() {
        let mut p=crate::periph::Peripherals::new([0;6]);
        p.write32(0x3ff7_101c,BUSY);p.tick(1000);assert_eq!(p.read32(0x3ff7_101c),BUSY);
        p.write32(0x3ff0_00cc,(1<<11)|(1<<16)|(1<<17));p.tick(240);
        assert_eq!(p.read32(0x3ff7_101c),0);assert_eq!(p.bt_clock.completed,1);
        p.write32(0x3ff7_101c,BUSY);p.write32(0x3ff0_00cc,0);p.tick(10000);
        assert_eq!(p.read32(0x3ff7_101c),BUSY);
        for bit in [3,4,9,10] {p.write32(0x3ff0_00d0,1<<bit);assert_eq!(p.read32(0x3ff7_101c),0);p.write32(0x3ff0_00d0,0);}
        // BLE+0x21c is now a separate RO sampler, not an alias of native+0x1c.
        p.write32(0x3ff7_121c,0x1234);assert_eq!(p.read32(0x3ff7_121c),0);
    }
}

#[cfg(test)] mod lp_event_tests {
 use super::*;
 #[test] fn native_fullslot_event_is_live_counter_carry_not_sampling_or_correction(){
  let mut c=BtNativeClock::with_latency_ticks(1);c.set_control(true,false);
  c.advance(PERIOD*2-1);assert!(!c.take_slot_event());c.advance(1);assert!(c.take_slot_event());assert!(!c.take_slot_event());
  c.enter_lp_sleep();c.advance(u64::MAX);assert!(!c.take_slot_event());
  c.set_control(false,false);c.set_bb_clock(true);c.apply_lp_correction(PERIOD*4);assert!(!c.take_slot_event());
  c.write(0x1c,BUSY);c.advance(PERIOD*2-1);assert_eq!(c.completed,0);assert!(!c.take_slot_event());
  c.advance(1);assert!(c.take_slot_event());c.set_control(true,false);c.advance(1);assert_eq!(c.completed,1);
 }
 #[test] fn resumed_base_clock_obeys_bb_gate_reset_wrap_and_large_ticks(){
  let mut c=BtNativeClock::default();c.set_bb_clock(true);c.apply_lp_correction(u64::from(MASK)*PERIOD);
  c.advance(PERIOD-1);assert!(!c.take_slot_event());c.advance(1);assert!(c.take_slot_event());assert_eq!(c.coarse,0);
  c.set_bb_clock(false);c.advance(u64::MAX);assert!(!c.take_slot_event());
  c.set_bb_clock(true);c.advance(u64::MAX);assert!(c.take_slot_event());
  c.set_control(true,true);c.set_bb_clock(true);c.apply_lp_correction(PERIOD*100);c.advance(u64::MAX);assert!(!c.take_slot_event());assert_eq!(c.coarse,0);
 }
 #[test] fn soc_mask_raw_w1c_and_unmasked_fullslot_presentation(){
  let mut p=crate::periph::Peripherals::new([0;6]);p.write32(0x3ff000d0,0);p.write32(0x3ff000cc,0x30800);
  p.tick(PERIOD*2-1);assert_eq!(p.read32(0x3ff71014),0);p.tick(1);assert_eq!(p.read32(0x3ff71014),1);assert_eq!(p.read32(0x3ff71010),0);
  p.write32(0x3ff7100c,1);assert_eq!(p.read32(0x3ff71010),1);assert_ne!(p.source_status(0)[0]&(1<<6),0);
  p.write32(0x3ff71018,4);assert_eq!(p.read32(0x3ff71014),1);p.write32(0x3ff71018,1);assert_eq!(p.read32(0x3ff71014),0);
  p.tick(PERIOD*2-1);assert_eq!(p.read32(0x3ff71014),0);p.tick(1);assert_eq!(p.read32(0x3ff71014),1);
 }
 #[test] fn native_lp_cross_peripheral_handoffs_are_chunk_invariant(){
  fn setup()->crate::periph::Peripherals {let mut p=crate::periph::Peripherals::new([0;6]);p.write32(0x3ff000d0,0);p.write32(0x3ff000cc,0x30800);p.write32(0x3ff000d4,79);p.write32(0x3ff000d8,0x04000001);p.write32(0x3ff71034,200);p.write32(0x3ff71030,7);p}
  let(mut a,mut b)=(setup(),setup());a.tick(200000);for _ in 0..200000 {b.tick(1);}
  for off in [0x30,0x34,0x38,0x14] {assert_eq!(a.bt_sleep.read(off),b.bt_sleep.read(off));}assert_eq!((a.bt_clock.coarse,a.bt_clock.phase),(b.bt_clock.coarse,b.bt_clock.phase));
  for p in [&mut a,&mut b] {p.write32(0x3ff71040,624);p.write32(0x3ff71044,0x80000004);p.write32(0x3ff71030,0xb);}
  a.tick(200000);for _ in 0..200000 {b.tick(1);}
  assert_eq!((a.bt_clock.coarse,a.bt_clock.phase),(b.bt_clock.coarse,b.bt_clock.phase));assert_eq!(a.bt_sleep.corrections,b.bt_sleep.corrections);assert_eq!(a.bt_sleep.read(0x14),b.bt_sleep.read(0x14));
 }
}

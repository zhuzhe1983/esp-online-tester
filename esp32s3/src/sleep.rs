//! S3 light sleep: retained CPUs/digital state, RTC timer and level wake sources.
//! ESP-IDF v5.5.2: soc/esp32s3/register/soc/{rtc_cntl,rtc_io,gpio,io_mux}_reg.h,
//! esp_hw_support/port/esp32s3/{rtc_sleep.c,include/soc/rtc.h}. No analog timing model.
use crate::bus::SocBus;
use emu_core::ClockDomain;
use esp_periph::{Device, RtcCntl, WriteEffect};

/// Keep S3 interrupt/status semantics out of the C3 and classic RTC adapters.
pub struct S3Rtc(pub RtcCntl);
impl std::ops::Deref for S3Rtc { type Target = RtcCntl; fn deref(&self) -> &RtcCntl { &self.0 } }
impl std::ops::DerefMut for S3Rtc { fn deref_mut(&mut self) -> &mut RtcCntl { &mut self.0 } }
impl Device for S3Rtc {
    fn read(&mut self, off: u32) -> u32 {
        match off { 0x48 => self.ram.read(0x44) & self.ram.read(0x40), 0x4c => 0, _ => self.0.read(off) }
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        match off {
            0x48 | 0xe4 | 0x128 | 0x130 => {}, // ST and wake/reject causes are read-only
            // RAW.SWD bit20 is R/W in the S3 header; the hardware-owned causes stay RO.
            0x44 => { let raw = self.ram.read(off); self.ram.write(off, (raw & !(1 << 20)) | (v & (1 << 20))); }
            0x4c => { let raw = self.ram.read(0x44); self.ram.write(0x44, raw & !v); }
            0x18 => { if v & 2 != 0 { self.ram.write(0x128, 0); } self.ram.write(off, v & !2); }
            0xe0 => { if v & (1 << 22) != 0 { self.ram.write(0xe4, 0); } self.ram.write(off, v & 0x3f_ffff); }
            _ => self.0.write(off, v),
        }
        WriteEffect::NONE
    }
    fn irq_sources(&self) -> u64 { u64::from(self.ram.read(0x44) & self.ram.read(0x40) != 0) }
    fn clock(&self) -> Option<ClockDomain> { Some(ClockDomain::RtcSlow) }
    fn tick(&mut self, ticks: u64) { Device::tick(&mut self.0, ticks); }
}

impl SocBus {
    #[inline(never)]
    pub(crate) fn enter_light_sleep(&mut self) {
        // DIG_PWC.DG_WRAP_PD_EN (bit31) selects deep sleep, outside this subset.
        if self.periph.rtc.ram.read(0x90) & (1 << 31) != 0 { return; }
        self.light_sleep = true;
        let cause = self.sleep_wake_cause();
        let reject = self.periph.rtc.ram.read(0x68);
        if reject & (1 << 30) != 0 && cause & (reject >> 12) & 0xf != 0 {
            self.finish_light_sleep(cause & (reject >> 12) & 0xf, true);
        } else if cause != 0 { self.finish_light_sleep(cause, false); }
        self.irq_dirty = true; // end the entering core's block before another instruction
    }

    pub(crate) fn sleep_timer_remaining(&self) -> u64 {
        let rtc = &self.periph.rtc;
        let target = u64::from(rtc.ram.read(4)) | (u64::from(rtc.ram.read(8) & 0xffff) << 32);
        let delta = target.wrapping_sub(rtc.slow_ticks) & 0xffff_ffff_ffff;
        if delta & (1 << 47) != 0 { 0 } else { delta }
    }

    fn rtc_pad_input(&self, pin: u32) -> bool {
        if pin > 21 { return false; }
        // RTCIO pad0..21 at +0x84, same GPIO numbers. MUX_SEL/FUN_SEL/IE,
        // with SLP_IE replacing FUN_IE when the sleep selection is enabled.
        let pad = self.periph.rtc.ram.read(0x484 + 4 * pin);
        pad & (1 << 19) != 0 && pad & (3 << 17) == 0
            && pad & (1 << if pad & (1 << 16) != 0 { 15 } else { 13 }) != 0
    }

    fn sleep_wake_cause(&self) -> u32 {
        let rtc = &self.periph.rtc;
        let enabled = rtc.ram.read(0x3c) >> 15;
        let input = self.periph.gpio.input;
        let mut cause = 0;
        if enabled & 8 != 0 && self.sleep_timer_remaining() == 0 { cause |= 8; }
        let pwc = rtc.ram.read(0x88);
        let rtc_periph_on = pwc & (1 << 18) == 0 && (pwc & (1 << 20) == 0 || pwc & (1 << 19) != 0);
        if enabled & 4 != 0 && rtc_periph_on {
            // gpio_reg.h: PINn.WAKEUP_ENABLE bit10, INT_TYPE 9:7 (4 low, 5 high).
            for (pin, cfg) in self.periph.gpio.pin[..49].iter().enumerate() {
                // IO_MUX SLP_SEL bit1 selects sleep input-enable bit4; otherwise FUN_IE bit9.
                let pad = self.periph.io_mux.read(4 + 4 * pin as u32);
                let input_enabled = pad & (1 << if pad & 2 != 0 { 4 } else { 9 }) != 0;
                if input_enabled && cfg & (1 << 10) != 0 && match (cfg >> 7) & 7 {
                    4 => input & (1 << pin) == 0, 5 => input & (1 << pin) != 0, _ => false,
                } { cause |= 4; break; }
            }
        }
        if enabled & 1 != 0 && rtc_periph_on {
            let pin = rtc.ram.read(0x4dc) >> 27;
            if self.rtc_pad_input(pin) && (input & (1 << pin) != 0) == (rtc.ram.read(0x64) & (1 << 30) != 0) { cause |= 1; }
        }
        if enabled & 2 != 0 && self.ext1_active() != 0 { cause |= 2; }
        cause
    }

    fn ext1_active(&self) -> u32 {
        let rtc = &self.periph.rtc;
        let mask = rtc.ram.read(0xe0) & 0x3f_ffff;
        let levels = self.periph.gpio.input as u32;
        // S3 is ANY_LOW/ANY_HIGH; classic's ALL_LOW is not applicable.
        let active = mask & if rtc.ram.read(0x64) & (1 << 31) != 0 { levels } else { !levels };
        (0..22).filter(|&pin| active & (1 << pin) != 0 && self.rtc_pad_input(pin)).fold(0, |out, pin| out | (1 << pin))
    }

    pub(crate) fn check_sleep_wake(&mut self) {
        if self.light_sleep {
            let cause = self.sleep_wake_cause();
            if cause != 0 { self.finish_light_sleep(cause, false); }
        }
    }

    fn finish_light_sleep(&mut self, cause: u32, rejected: bool) {
        let ext1 = if cause & 2 != 0 { self.ext1_active() } else { 0 };
        let rtc = &mut self.periph.rtc;
        let state = rtc.ram.read(0x18); let raw = rtc.ram.read(0x44);
        rtc.ram.write(0x18, state & !(1 << 31));
        rtc.ram.write(0x44, raw | if rejected { 2 } else { 1 });
        rtc.ram.write(if rejected { 0x128 } else { 0x130 }, cause);
        if !rejected && ext1 != 0 { rtc.ram.write(0xe4, ext1); }
        self.light_sleep = false;
        self.irq_dirty = true;
    }
}

//! Classic I2S RX register adapter and native descriptor DMA.
//! ESP-IDF v5.5.4 components/soc/esp32/register/soc/i2s_reg.h: CONF bits 21-128,
//! interrupt _S defines 142-184, RXEOF_NUM 676-682, CONF_CHAN 692-695, INLINK_ADDR 763-768,
//! LC_CONF 856-868, clocks 1340-1390. include/soc/gpio_sig_map.h:69-72,299,309-312,343
//! supplies the GPIO matrix signals; dport_reg.h:947,965 defines the clock gates.
//! Samples are packed at DMA boundaries; no bit-level clock model is claimed.
use esp_periph::{Device, RegRam, WriteEffect};
use esp_periph::i2s::{I2s, PcmSources};
use xtensa_lx7::bus::{Bus, Fault};
use crate::bus::{SocBus, dma_range};
#[cfg(test)]
use crate::bus::{DRAM_HIGH, DRAM_LOW};
use crate::periph::CPU_HZ;
use crate::periph::ClassicGpio;

/// I2S_CONF_REG reset: RX/TX_MSB_RIGHT (bits 17/16) and RX/TX_RIGHT_FIRST (bits 9/8) set.
const CONF_RESET: u32 = (1 << 17) | (1 << 16) | (1 << 9) | (1 << 8);
/// I2S_LC_CONF_REG reset: OUT_EOF_MODE (bit 8) set, CHECK_OWNER (bit 12) clear.
const LC_CONF_RESET: u32 = 1 << 8;
/// LC_CONF CHECK_OWNER, at the bit position the shared channel's CONF1 uses too.
const CHECK_OWNER: u32 = 1 << 12;

/// Parallel receiver configuration, not an audio sample rate. FIFO packing and delivery
/// are deliberately separate from this register adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CameraRx {
    pub fifo_mode: u8,
    pub descriptor_enabled: bool,
    pub fifo_mode_forced: bool,
    pub slave: bool,
    pub started: bool,
    pub reset: bool,
}

pub struct ClassicI2s {
    pub inner: I2s,
    ram: RegRam,
    dma: esp_periph::gdma::GdmaInCh,
    pub(crate) clock_enabled: bool,
    #[cfg(test)]
    pub(crate) step_calls: u64,
    camera_capable: bool,
    pub(crate) camera: Option<Box<crate::camera_dma::Capture>>,
    camera_mode: bool,
}
impl Default for ClassicI2s { fn default() -> Self { Self::new() } }
impl ClassicI2s {
    pub fn new() -> Self {
        Self::for_port(0)
    }
    pub(crate) fn for_port(port: usize) -> Self {
        let mut ram = RegRam::new();
        ram.write(8, CONF_RESET);
        ram.write(0x24, 64);
        ram.write(0x60, LC_CONF_RESET);
        Self { inner: I2s::new(CPU_HZ), ram, dma: esp_periph::gdma::GdmaInCh { conf1: LC_CONF_RESET & CHECK_OWNER, ..Default::default() }, clock_enabled: false,
            #[cfg(test)] step_calls: 0, camera_capable: port == 0, camera: None, camera_mode: false }
    }
    /// RXEOF_NUM (0x24) in words: IDF v5.5.4 hal/esp32/include/hal/i2s_ll.h:639-643.
    pub(crate) fn rx_eof_words(&self) -> u32 { self.ram.read(0x24) }
    /// ESP-IDF v4.4.7 components/soc/esp32/include/soc/i2s_reg.h:
    /// CONF2 0xa8, LCD_EN bit 5 and CAMERA_EN bit 0 (1298-1346);
    /// FIFO_CONF 0x20, force/mode/descriptor bits (640-670);
    /// CONF RX_START/SLAVE/RESET/FIFO_RESET (91-130); LC_CONF IN_RST,
    /// AHBM_FIFO_RST, AHBM_RST (927-948). i2s_struct.h matches the layout.
    /// esp32-camera v2.1.4 target/esp32/ll_cam.c:294-337 configures I2S0 only.
    /// This is header/driver-derived, not silicon-verified. No PCM width/rate
    /// validation applies: that driver explicitly programs RX_BITS_MOD=0.
    pub fn camera_rx(&self) -> Option<CameraRx> {
        if !self.camera_capable || self.ram.read(0xa8) & 0x21 != 0x21 { return None; }
        let conf = self.ram.read(8);
        let fifo = self.ram.read(0x20);
        let reset = conf & 0xa != 0 || self.ram.read(0x60) & 0xd != 0;
        Some(CameraRx {
            fifo_mode: ((fifo >> 16) & 7) as u8,
            descriptor_enabled: fifo & (1 << 12) != 0,
            fifo_mode_forced: fifo & (1 << 20) != 0,
            slave: conf & (1 << 7) != 0,
            started: conf & (1 << 5) != 0 && !reset,
            reset,
        })
    }
    /// Writes go through `Device::write`, as the other chips' dispatch does, so the shared
    /// inherent writer keeps its single caller and S3/C3/C6 code generation is unchanged.
    fn sync(&mut self) {
        let conf = self.ram.read(8);
        if let Some(camera) = self.camera_rx() {
            // Do not interpret parallel camera bits as PCM clocks or slots. Stop and
            // reset the audio producer on mode entry, so no stale PCM phase can leak
            // through camera mode, even with otherwise valid audio rate registers.
            // Camera start/reset state is decoded above, not mirrored as audio RX_START.
            Device::write(&mut self.inner, 0x30, 0);
            Device::write(&mut self.inner, 0x20, if camera.reset { 3 } else { 1 });
            return;
        }
        let clock = self.ram.read(0xac);
        let sample = self.ram.read(0xb0);
        let bits = (sample >> 18) & 63;
        let bck = (sample >> 6) & 63;
        let (a, b) = ((clock >> 14) & 63, (clock >> 8) & 63);
        let div = if b == 0 || a < b { 0 } else { b | ((a % b) << 9) | ((a / b - 1) << 18) };
        let supported = clock & (1 << 21) == 0 && bck != 0 && [16, 24, 32].contains(&bits);
        Device::write(&mut self.inner, 0x30, (clock & 255) | (2 << 27) | if supported { 1 << 26 } else { 0 });
        Device::write(&mut self.inner, 0x38, div);
        Device::write(&mut self.inner, 0x28, (bck.saturating_sub(1) << 7) | (bits.saturating_sub(1) << 13) | ((if bits <= 16 { 15 } else { 31 }) << 18));
        // RX_CHAN_MOD 1/2 selects one slot and RX_MSB_RIGHT (CONF bit 17) decides which:
        // IDF v5.5.4 hal/esp32/include/hal/i2s_ll.h:859-875 writes RIGHT as msb_right ? 1 : 2
        // and LEFT as msb_right ? 2 : 1. The shared TDM mask has left in bit 0, right in bit 1.
        let channel = (self.ram.read(0x2c) >> 3) & 3;
        let msb_right = conf & (1 << 17) != 0;
        let mask = match channel { 1 | 2 => if (channel == 1) == msb_right { 2 } else { 1 }, _ => 3 };
        Device::write(&mut self.inner, 0x50, (1 << 16) | mask);
        // Shared RX_CONF bit 0 RX_RESET <- CONF bit 1; bit 1 RX_FIFO_RESET <- CONF bit 3 (CONF
        // bit 2 is TX_FIFO_RESET); bit 2 RX_START <- bit 5; bit 3 RX_SLAVE_MOD <- bit 7.
        // Classic keeps a 24-bit sample in a 32-bit word (i2s_ll.h:676-680 sets RX_FIFO_MOD 2;
        // esp_driver_i2s/i2s_common.c:447-448 allocates 4 bytes), so 24-bit RX sets RX_24_FILL_EN.
        let fill24 = if bits == 24 { 1 << 16 } else { 0 };
        Device::write(&mut self.inner, 0x20, ((conf >> 1) & 1) | ((conf >> 2) & 2) | ((conf >> 3) & 4) | ((conf & (1 << 7)) >> 4) | fill24);
    }
    #[inline]
    pub(crate) fn data(&mut self, cycles: u64, now: u64, port: usize, gpio: &ClassicGpio, sources: Option<&mut PcmSources>) -> Vec<u8> {
        let (data, clock) = [(155, [27, 28]), (181, [164, 165])][port];
        let signals = esp_periph::i2s::RxSignals { data, clock, input_select_bit: 7, output_mask: 0x3ff };
        self.inner.receive(cycles, now, false, &gpio.gpio, signals, sources)
    }
    pub(crate) fn abort_camera_dma(&mut self) {
        self.camera = None;
        let mut irq = false;
        self.dma.fail_receive(&mut irq);
        self.raise(self.dma.int_raw, self.dma.eof_desc);
        self.dma.int_raw = 0;
    }
    /// Map the shared channel's GDMA-layout interrupt bits onto classic INT_RAW: IN_DONE 0,
    /// IN_SUC_EOF 1, IN_DSCR_ERR 3 and IN_DSCR_EMPTY 4 become bits 8, 9, 13 and 15.
    #[inline]
    pub(crate) fn raise(&mut self, gdma: u32, eof_desc: u32) {
        if gdma & (1 << 1) != 0 { self.ram.write(0x3c, eof_desc); }
        self.inner.int_raw |= ((gdma & 3) << 8) | ((gdma & (1 << 3)) << 10) | ((gdma & (1 << 4)) << 11);
    }
}
impl Device for ClassicI2s {
    fn read(&mut self, off: u32) -> u32 {
        match off {
            0xc | 0x10 | 0x14 => self.inner.read(off),
            // IDF v5.5.4 i2s_reg.h:105-128: all four reset bits are plain R/W.
            8 => self.ram.read(off),
            0xbc => 1,
            _ => self.ram.read(off),
        }
    }
    fn write(&mut self, off: u32, value: u32) -> WriteEffect {
        let previous = self.ram.read(off);
        self.ram.write(off, value);
        // Cache dispatch on configuration writes: a PCM-only channel must not
        // call the camera producer or poll camera registers on every DMA step.
        if off == 0xa8 { self.camera_mode = self.camera_capable && value & 0x21 == 0x21; }
        if self.camera_rx().is_some() || self.camera.is_some() {
            // Stop/restart, mode/FIFO reconfiguration and all native RX reset
            // pulses discard the old frame. INLINK restarts begin at fresh VSYNC.
            if (off == 8 && (previous ^ value) & 0xaa != 0)
                || (off == 0x60 && value & 0xd != 0)
                || (off == 0x34 && value & (7 << 28) != 0)
                || (off == 0xa8 && previous != value)
                || (off == 0x20 && previous != value) {
                self.camera = None;
            }
            if (off == 8 && (value & 0xa != 0 || value & (1 << 5) == 0)) || (off == 0x60 && value & 0xd != 0) {
                self.dma.running = false;
                self.dma.buf_pos = 0;
                self.dma.rx_eof_pos = 0;
            }
        }
        match off {
            0x14 | 0x18 => { Device::write(&mut self.inner, off, value); }
            0x34 => {
                // ESP32 TRM v5.8, section 2.6 (I2S DMA): INLINK_START is
                // self-cleared by hardware. Consume it synchronously, so a later
                // volatile RMW (notably IDF v4.4.7 i2s_ll.h:535-538 STOP)
                // cannot replay a stale START. Keep the descriptor address.
                // No new priority rule for explicitly combined commands is inferred.
                self.ram.write(off, value & !(1 << 29));
                if value & (1 << 28) != 0 { self.dma.running = false; }
                if value & (3 << 29) != 0 { self.dma.desc = 0x3ff0_0000 | (value & 0xfffff); self.dma.buf_pos = 0; self.dma.rx_eof_pos = 0; self.dma.running = true; }
            }
            0x60 => self.dma.conf1 = value & CHECK_OWNER,
            8 | 0x2c | 0xa8 | 0xac | 0xb0 => self.sync(),
            _ => {}
        }
        WriteEffect::NONE
    }
    fn irq_sources(&self) -> u64 { self.inner.irq() as u64 }
}

/// Classic I2S DMA as the shared receive channel sees it: internal DRAM only. ESP-IDF v5.5.4
/// components/soc/esp32/include/soc/soc.h:203-205 (SOC_DMA_LOW/HIGH) bounds DMA-capable
/// memory to 0x3FFA_E000..0x4000_0000. Descriptor reads, buffer writes and descriptor
/// writeback anywhere else (registers, ROM, IRAM, RTC memory, flash) fault without access.
struct DramDma<'a>(&'a mut SocBus);
impl DramDma<'_> {
    fn reaches(addr: u32, len: u32) -> bool {
        dma_range(addr, len as usize).is_some()
    }
}
impl Bus for DramDma<'_> {
    fn read8(&mut self, _: u32) -> Result<u8, Fault> { Err(Fault::Prohibited) }
    fn read16(&mut self, _: u32) -> Result<u16, Fault> { Err(Fault::Prohibited) }
    fn read32(&mut self, addr: u32) -> Result<u32, Fault> {
        if !Self::reaches(addr, 4) { return Err(Fault::Prohibited); }
        self.0.dma_read_word(addr).ok_or(Fault::Unmapped)
    }
    fn write8(&mut self, addr: u32, v: u8) -> Result<(), Fault> {
        if Self::reaches(addr, 1) && self.0.dma_write_byte(addr, v) { Ok(()) } else { Err(Fault::Prohibited) }
    }
    fn write16(&mut self, _: u32, _: u16) -> Result<(), Fault> { Err(Fault::Prohibited) }
    fn write32(&mut self, addr: u32, v: u32) -> Result<(), Fault> {
        if Self::reaches(addr, 4) && self.0.dma_write_word(addr, v) { Ok(()) } else { Err(Fault::Prohibited) }
    }
    fn fetch(&mut self, _: u32) -> Result<[u8; 4], Fault> { Err(Fault::Prohibited) }
    fn note_code_page(&mut self, _: u32) {}
}

impl SocBus {
    /// Observe the effective start on configuration writes, not at a later tick.
    pub(crate) fn camera_dma_written(&mut self) {
        let i2s = &mut self.periph.i2s[0];
        if i2s.dma.running && i2s.camera_rx().is_some_and(|rx| rx.started) && i2s.camera.is_none() {
            i2s.camera = Some(Box::new(crate::camera_dma::Capture::armed(self.cycles)));
        }
    }
    #[inline]
    pub(crate) fn i2s_step(&mut self, cycles: u64) {
        for port in 0..2 {
            if !self.periph.i2s[port].clock_enabled || !self.periph.i2s[port].dma.running { continue; }
            #[cfg(test)] { self.periph.i2s[port].step_calls += 1; }
            // Pin-routed sources live on controller 0, as on the other chips.
            let mut bank = self.periph.i2s[0].inner.take_pcm_bank();
            let data = if self.periph.i2s[port].camera_mode {
                self.camera_dma_data(cycles).unwrap_or_else(|| self.periph.i2s[port].data(cycles, self.cycles, port, &self.periph.gpio, bank.as_deref_mut()))
            } else {
                self.periph.i2s[port].data(cycles, self.cycles, port, &self.periph.gpio, bank.as_deref_mut())
            };
            self.periph.i2s[0].inner.restore_pcm_bank(bank);
            let mut dma = self.periph.i2s[port].dma;
            let mut irq = false;
            // IDF v5.5.4 hal/esp32/include/hal/i2s_ll.h:639-643 counts RXEOF_NUM in words.
            if let Some(eof_bytes) = self.periph.i2s[port].ram.read(0x24).checked_mul(4) {
                dma.receive(&mut DramDma(self), &data, Some(eof_bytes), false, |a| !DramDma::reaches(a, 1), &mut irq);
            } else {
                dma.fail_receive(&mut irq);
            }
            self.periph.i2s[port].raise(dma.int_raw, dma.eof_desc);
            dma.int_raw = 0;
            self.periph.i2s[port].dma = dma;
            self.periph.i2s[port].inner.recycle_rx_buffer(data);
            self.irq_dirty |= irq;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Replay volatile RMW operations in esp32-camera v2.1.4 ll_cam.c:250-337.
    // Offsets/bits are from the pinned IDF v4.4.7 i2s_reg.h and i2s_struct.h.
    fn rmw(b: &mut SocBus, base: u32, off: u32, clear: u32, set: u32) {
        let old = b.read32(base + off).unwrap();
        b.write32(base + off, (old & !clear) | set).unwrap();
    }
    fn cam_resets(b: &mut SocBus, base: u32) {
        for (off, bit) in [(8, 1), (8, 3), (0x60, 0), (0x60, 2), (0x60, 3)] {
            rmw(b, base, off, 0, 1 << bit);
            rmw(b, base, off, 1 << bit, 0);
        }
    }
    fn cam_config(b: &mut SocBus, base: u32, mode: u32) {
        cam_resets(b, base);
        rmw(b, base, 8, 0, 1 << 7);
        for bit in [9, 17, 11, 15, 13] { rmw(b, base, 8, 1 << bit, 0); }
        rmw(b, base, 0xa8, 0, 1 << 5);
        rmw(b, base, 0xa8, 0, 1);
        rmw(b, base, 0xac, 63 << 14, 0);
        rmw(b, base, 0xac, 63 << 8, 0);
        rmw(b, base, 0xac, 255, 2);
        rmw(b, base, 0x20, 0, 1 << 12);
        rmw(b, base, 0x20, 7 << 16, mode << 16);
        rmw(b, base, 0x20, 0, 1 << 20);
        rmw(b, base, 0x2c, 3 << 3, 1 << 3);
        rmw(b, base, 0xb0, 63 << 18, 0);
        b.write32(base + 0x1c, 0).unwrap();
        rmw(b, base, 0x1c, 0, 1 << 21);
    }
    #[test]
    fn inlink_start_self_clears_without_replaying_on_rmw() {
        for port in 0..2 {
            let mut i=ClassicI2s::for_port(port); // includes PCM-only I2S1
            i.write(0x34,0xb0000 | (1<<29));
            assert!(i.dma.running);
            assert_eq!(i.dma.desc,0x3ffb_0000);
            assert_eq!(i.read(0x34)&((1<<29)|0xfffff),0xb0000);
            i.dma.buf_pos=16;
            i.dma.rx_eof_pos=16;
            // An address-only RMW must not silently replay START/reset progress.
            let old=i.read(0x34);
            i.write(0x34,(old & !0xfffff)|0xb0020);
            assert!(i.dma.running);
            assert_eq!((i.dma.desc,i.dma.buf_pos,i.dma.rx_eof_pos),(0x3ffb_0000,16,16));
            let old=i.read(0x34);
            i.write(0x34,old | (1<<28));
            assert!(!i.dma.running);
            assert_eq!(i.read(0x34)&(1<<29),0);
            // Explicit new START is still accepted, using the updated address.
            let old=i.read(0x34);
            i.write(0x34,(old & !(7<<28)) | (1<<29));
            assert!(i.dma.running);
            assert_eq!((i.dma.desc,i.dma.buf_pos,i.dma.rx_eof_pos),(0x3ffb_0020,0,0));
            assert_eq!(i.read(0x34)&(1<<29),0);
        }
    }

    #[test]
    fn camera_driver_config_start_stop_trace() {
        for mode in [1, 3] {
            let mut b = SocBus::new(4 << 20, [0; 6]);
            let base = 0x3ff4_f000;
            b.write32(0x3ff0_00c0, 1 << 4).unwrap();
            b.write32(0x3ff0_00c4, 0).unwrap();
            b.write32(base + 0xb0, 16 << 18).unwrap(); // nonzero width before config
            cam_config(&mut b, base, mode);
            let configured = CameraRx { fifo_mode: mode as u8, descriptor_enabled: true,
                fifo_mode_forced: true, slave: true, started: false, reset: false };
            assert_eq!(b.periph.i2s[0].camera_rx(), Some(configured));
            assert_eq!(b.read32(base + 0xb0).unwrap() & (63 << 18), 0);
            assert_eq!(b.read32(base + 0x1c).unwrap(), 1 << 21);
            // ll_cam_start: disable RX, enable EOF interrupt, pulse resets, set
            // word-count/address, start in-link, then RX. No capture is claimed here.
            rmw(&mut b, base, 8, 1 << 5, 0);
            b.periph.i2s[0].inner.int_raw |= 1 << 9; // pre-existing pending EOF
            rmw(&mut b, base, 0x18, 0, 1 << 9); // I2S_ISR_ENABLE clears before enabling
            rmw(&mut b, base, 0x14, 0, 1 << 9);
            assert_eq!(b.read32(base + 0x10).unwrap(), 0);
            cam_resets(&mut b, base);
            b.write32(base + 0x24, 960).unwrap();
            rmw(&mut b, base, 0x34, 0xfffff, 0xae000);
            rmw(&mut b, base, 0x34, 0, 1 << 29);
            assert!(!b.periph.i2s[0].camera_rx().unwrap().started);
            rmw(&mut b, base, 8, 0, 1 << 5);
            assert_eq!(b.periph.i2s[0].camera_rx(), Some(CameraRx { started: true, ..configured }));
            assert!(!b.periph.i2s[0].inner.active());
            assert_eq!(b.periph.i2s[0].inner.rx_rate(), None);
            assert_eq!(b.read32(base + 0x14).unwrap(), 1 << 9);
            assert_eq!(b.read32(base + 0x24).unwrap(), 960);
            // ll_cam_stop
            rmw(&mut b, base, 8, 1 << 5, 0);
            rmw(&mut b, base, 0x14, 1 << 9, 0);
            rmw(&mut b, base, 0x18, 0, 1 << 9); // I2S_ISR_DISABLE clears after disabling
            rmw(&mut b, base, 0x34, 0, 1 << 28);
            assert_eq!(b.periph.i2s[0].camera_rx(), Some(configured));
            assert_eq!(b.read32(base + 0x14).unwrap(), 0);
        }
    }
    #[test]
    fn camera_requires_both_mode_bits_on_port_zero_only() {
        let mut b = SocBus::new(0, [0; 6]);
        for (port, base) in [(0, 0x3ff4_f000), (1, 0x3ff6_d000)] {
            for conf2 in [0, 1, 0x20, 0x21] {
                b.write32(base + 0xa8, conf2).unwrap();
                assert_eq!(b.periph.i2s[port].camera_rx().is_some(), port == 0 && conf2 == 0x21);
            }
        }
        cam_config(&mut b, 0x3ff6_d000, 1);
        assert_eq!(b.periph.i2s[1].camera_rx(), None);
    }
    #[test]
    fn camera_reset_bits_and_fifo_fields_are_live() {
        let mut i = ClassicI2s::new();
        i.write(0xa8, 0x21);
        i.write(8, (1 << 5) | (1 << 7));
        for (off, bit, reset) in [(8, 1, true), (8, 3, true), (0x60, 0, true),
            (0x60, 2, true), (0x60, 3, true), (8, 0, false), (8, 2, false), (0x60, 1, false)] {
            let old = i.read(off);
            i.write(off, old | (1 << bit));
            let c = i.camera_rx().unwrap();
            assert_eq!(c.reset, reset, "reset {off:#x}:{bit}");
            assert_eq!(c.started, !reset);
            assert_eq!(i.read(off), old | (1 << bit)); // plain R/W, not self-clearing
            i.write(off, old);
            assert!(i.camera_rx().unwrap().started);
        }
        for mode in 0..8 {
            i.write(0x20, mode << 16);
            let c = i.camera_rx().unwrap();
            assert_eq!(c.fifo_mode, mode as u8); // validity/packing belongs to the packer
            assert!(!c.descriptor_enabled);
            assert!(!c.fifo_mode_forced);
        }
        i.write(8, 1 << 5);
        assert!(!i.camera_rx().unwrap().slave);
    }
    #[test]
    fn camera_bypasses_audio_and_mode_exit_restores_pcm() {
        for port in 0..2 {
            let mut i = ClassicI2s::for_port(port);
            i.write(0xac, 25 | (1 << 8) | (2 << 14));
            i.write(0xb0, (25 << 6) | (16 << 18));
            i.write(8, 1 << 5);
            assert!(i.inner.rx_data(30_000, false, |_| [1, 2]).is_empty());
            i.write(0xa8, 0x21);
            if port == 0 {
                assert!(!i.inner.active());
                assert_eq!(i.inner.rx_rate(), None);
                assert!(i.inner.rx_data(1_000_000, false, |_| [1, 2]).is_empty());
                // Even nonsensical audio divider/width values do not reject camera state.
                i.write(0xac, 1 << 21);
                i.write(0xb0, 0);
                assert!(i.camera_rx().unwrap().started);
                i.write(0xac, 25 | (1 << 8) | (2 << 14));
                i.write(0xb0, (25 << 6) | (16 << 18));
                i.write(0xa8, 0);
                assert!(i.inner.rx_data(1_000, false, |_| [1, 2]).is_empty());
                assert_eq!(i.inner.rx_data(30_000, false, |_| [1, 2]), [1, 0, 2, 0]);
            } else {
                assert!(i.inner.active());
                assert_eq!(i.inner.rx_rate(), Some(7843));
                assert_eq!(i.inner.rx_data(1_000, false, |_| [1, 2]), [1, 0, 2, 0]);
            }
        }
    }
    #[test]
    fn classic_i2s_clock_reset_and_pending_gates() {
        for (port, bit) in [4, 21].into_iter().enumerate() {
            let mut b = SocBus::new(4 << 20, [0; 6]);
            b.periph.i2s[port].dma.running = true;
            b.i2s_step(64);
            assert_eq!(b.periph.i2s[port].step_calls, 0);
            b.write32(0x3ff0_00c0, 1 << bit).unwrap();
            b.write32(0x3ff0_00c4, 1 << bit).unwrap();
            b.i2s_step(64);
            assert_eq!(b.periph.i2s[port].step_calls, 0);
            b.write32(0x3ff0_00c4, 0).unwrap();
            b.i2s_step(64);
            assert_eq!(b.periph.i2s[port].step_calls, 1);
            b.write32(0x3ff0_00c0, 0).unwrap();
            b.i2s_step(64);
            assert_eq!(b.periph.i2s[port].step_calls, 1);
            b.periph.i2s[port].dma.running = false;
            b.write32(0x3ff0_00c0, 1 << bit).unwrap();
            b.i2s_step(64);
            assert_eq!(b.periph.i2s[port].step_calls, 1);
        }
    }
    #[test]
    fn classic_i2s_dma_reach_reads_and_writes() {
        for address in [DRAM_LOW, DRAM_HIGH - 4, crate::bus::ROM_DRAM_LOW,
            DRAM_LOW - 4, DRAM_HIGH - 3, 0x4008_0000, 0x3ff8_0000, 0x5000_0000, u32::MAX] {
            let allowed = address == DRAM_LOW || address == DRAM_HIGH - 4;
            let mut b = SocBus::new(0, [0; 6]);
            let mut dma = DramDma(&mut b);
            assert_eq!(dma.read32(address).is_ok(), allowed, "read {address:#x}");
            assert_eq!(dma.write32(address, 42).is_ok(), allowed, "write {address:#x}");
        }
    }

    /// 16-bit stereo at 7843 Hz (PLL160 / 25.5 / BCK 25 / 32): one frame takes 30,601 CPU cycles.
    fn receiver(bits: u32, conf: u32) -> ClassicI2s {
        let mut i = ClassicI2s::new();
        i.write(0xac, 25 | (1 << 8) | (2 << 14));
        i.write(0xb0, (25 << 6) | (bits << 18));
        i.write(8, conf);
        i
    }
    #[test]
    fn classic_fractional_clock_width_and_slave_selection() {
        let mut i = receiver(16, 0);
        assert_eq!(i.inner.rx_rate(), Some(7843));
        i.write(0xb0, (25 << 6) | (24 << 18));
        assert_eq!(i.inner.rx_rate(), Some(3922));
        i.write(0xac, 25 | (1 << 21));
        assert_eq!(i.inner.rx_rate(), None);
        i.write(0xac, 25);
        i.write(0xb0, 16 << 18);
        assert_eq!(i.inner.rx_rate(), None);
        i.write(0xb0, (25 << 6) | (8 << 18));
        assert_eq!(i.inner.rx_rate(), None);
        i.write(0xac, 25 | (1 << 8) | (2 << 14));
        i.write(0xb0, (25 << 6) | (16 << 18));
        i.write(8, (1 << 5) | (1 << 7));
        assert!(i.inner.rx_data(40_000, false, |_| [1, 2]).is_empty());
        i.write(8, 1 << 5);
        assert_eq!(i.inner.rx_data(40_000, false, |_| [1, 2]), [1, 0, 2, 0]);
    }

    /// IDF's slot encoding (i2s_ll.h:859-875) for both RX_MSB_RIGHT values; [left, right] = [1, 2].
    #[test]
    fn rx_chan_mod_selects_slot_by_msb_right() {
        assert_eq!(ClassicI2s::new().read(8), 0x0003_0300); // i2s_reg.h:21-125 defaults
        for (msb_right, left_mod, right_mod) in [(1 << 17, 2, 1), (0, 1, 2)] {
            let mut i = receiver(16, (1 << 5) | msb_right);
            i.write(0x2c, left_mod << 3);
            assert_eq!(i.inner.rx_data(40_000, false, |_| [1, 2]), [1, 0], "left, msb_right {msb_right:#x}");
            i.write(0x2c, right_mod << 3);
            assert_eq!(i.inner.rx_data(40_000, false, |_| [1, 2]), [2, 0], "right, msb_right {msb_right:#x}");
            i.write(0x2c, 0);
            assert_eq!(i.inner.rx_data(40_000, false, |_| [1, 2]), [1, 0, 2, 0]);
        }
    }

    /// CONF bit 3 is RX_FIFO_RESET and bit 2 TX_FIFO_RESET (i2s_reg.h:105-125): only the RX
    /// resets (bits 1 and 3) discard the receiver's fractional frame time.
    #[test]
    fn rx_fifo_reset_is_conf_bit_3_not_tx_bit_2() {
        for (reset, restarts) in [(1 << 3, true), (1 << 1, true), (1 << 2, false), (1 << 0, false)] {
            let mut i = receiver(16, 1 << 5);
            assert!(i.inner.rx_data(30_000, false, |_| [1, 2]).is_empty());
            i.write(8, (1 << 5) | reset);
            assert_eq!(i.read(8), (1 << 5) | reset, "reset bit {reset:#x} remains readable");
            i.write(8, 1 << 5);
            assert_eq!(i.inner.rx_data(1_000, false, |_| [1, 2]).is_empty(), restarts, "reset bit {reset:#x}");
        }
    }

    /// Classic keeps 24-bit samples in 32-bit words, as IDF sizes them; 16/32-bit are unchanged.
    #[test]
    fn classic_24_bit_samples_fill_words() {
        for (bits, cycles, frame) in [(24, 80_000, vec![0, 0, 1, 0, 0, 0, 0xfe, 0xff]), (32, 80_000, vec![0, 0, 1, 0, 0, 0, 0xfe, 0xff]), (16, 40_000, vec![1, 0, 0xfe, 0xff])] {
            let mut i = receiver(bits, 1 << 5);
            assert_eq!(i.inner.rx_data(cycles, false, |_| [1, -2]), frame, "{bits}-bit");
        }
    }
}

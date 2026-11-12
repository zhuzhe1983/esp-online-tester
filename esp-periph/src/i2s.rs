//! I2S RX and TX: the clock tree that sets the frame rate, and the sample sink the SoC's DMA pump fills.
use crate::device::{Device, WriteEffect};
use crate::regram::RegRam;
mod rx;
mod sources;
pub use sources::{PcmPins, PcmSource, PcmSources, RxSignals};
pub type PcmInput = PcmSource;


pub struct I2s {
    /// RX_CONF/TX_CONF change only through `write`, which keeps `streaming` current.
    pub(crate) rx_conf: u32, pub(crate) tx_conf: u32, pub int_raw: u32, pub int_ena: u32,
    ram: RegRam,
    /// TX_CONF1 (0x2c: data/slot/frame widths, BCK divider), TX_CLKM_CONF (0x34: source, integer MCLK divider),
    /// TX_CLKM_DIV_CONF (0x3c: fractional MCLK divider x/y/z/yn1), TX_TDM_CTRL (0x54: slot count)
    pub tx_conf1: u32, pub tx_clkm_conf: u32, pub tx_clkm_div_conf: u32, pub tx_tdm_ctrl: u32,
    /// frame rate on the wire, derived from the clock registers exactly as the silicon divides
    /// its source clock; 44.1 kHz until firmware programs the clock
    pub sample_rate: u32,
    pub bytes_per_frame: u32,
    acc: u64,
    /// decoded left-channel samples (host sink)
    pub pcm: Vec<i16>,
    pub frames_out: u64,
    pub tx_started_log: bool,
    /// TX or RX enabled, cached on RX_CONF/TX_CONF writes for the SoC's per-flush gate.
    streaming: bool,
    cpu_hz: u64,
    /// Receiver state, allocated on first host input or first supported RX interval.
    rx: Option<Box<rx::RxState>>,
    /// The P4/S31 transmitter layout (`new_v2`): TX_CONF MONO bit 6 and BCK_DIV_NUM 26:21,
    /// TX_CONF1 BITS_MOD 18:14 and HALF_SAMPLE_BITS 26:19, the MCLK divider outside the block
    /// (`set_v2_clock`). TX side only; RX keeps the classic field positions.
    v2: bool,
    /// v2: the TX clock control and divider words from the clock/reset block
    v2_clock: (u32, u32),
}
impl I2s {
    pub fn new(cpu_hz: u64) -> Self {
        let mut ram = RegRam::new();
        // RX_EOF_NUM resets to 0x40: esp32s3 i2s_reg.h:1059, esp32c3 i2s_reg.h:1005, esp32c6 i2s_reg.h:1226.
        ram.write(0x64, 0x40);
        I2s { cpu_hz, streaming: false, rx: None, v2: false, v2_clock: (0, 0), rx_conf: 0, tx_conf: 0, int_raw: 0, int_ena: 0, ram, tx_conf1: 0, tx_clkm_conf: 0, tx_clkm_div_conf: 0, tx_tdm_ctrl: 0xffff, sample_rate: 44100, bytes_per_frame: 1, acc: 0, pcm: Vec::new(), frames_out: 0, tx_started_log: false }
    }
    /// The P4/S31 register layout (IDF master soc/esp32s31/register/soc/i2s_reg.h, IDF v5.5.4
    /// soc/esp32p4/register/hw_ver3/soc/i2s_reg.h): 0x30-0x40 are RX recombination registers there,
    /// not clock registers, and the TX field positions differ (see `v2`).
    pub fn new_v2(cpu_hz: u64) -> Self { I2s { v2: true, ..Self::new(cpu_hz) } }
    /// v2: the clock/reset block's TX clock words as `i2s_ll_tx_set_raw_clk_div` and
    /// `i2s_ll_tx_clk_set_src` leave them (S31 HP_SYS_CLKRST I2Sn_TX_CTRL0: CLK_EN 0, SRC_SEL 2:1
    /// (0 XTAL, 1 APLL, 2 RC_FAST, 3 external), DIV_N 10:3; I2Sn_TX_DIV_CTRL0: X 8:0, Y 17:9,
    /// Z 26:18, YN1 27).
    pub fn set_v2_clock(&mut self, ctrl0: u32, div: u32) {
        if self.v2_clock != (ctrl0, div) { self.v2_clock = (ctrl0, div); self.update_rate(); }
    }
    fn tx_mono(&self) -> bool { self.tx_conf & (1 << if self.v2 { 6 } else { 5 }) != 0 }
    pub fn tx_running(&self) -> bool { self.tx_conf & (1 << 2) != 0 }
    /// TX or RX running.
    #[inline(always)]
    pub fn active(&self) -> bool { self.streaming }
    /// The SoC's per-flush gate: false while neither direction runs, discarding the TX frame
    /// phase exactly as `frames_due` does for a stopped transmitter.
    #[inline(always)]
    pub fn poll_active(&mut self) -> bool {
        if !self.streaming { self.acc = 0; }
        self.streaming
    }
    /// Packed DMA sample width, independent of padding in the wire's time slots.
    pub fn sample_bytes(&self) -> usize { (((self.tx_conf1 >> if self.v2 { 14 } else { 13 }) & 0x1f) + 1).div_ceil(8) as usize }
    pub fn read(&self, off: u32) -> u32 {
        match off {
            0xc => self.int_raw, 0x10 => self.int_raw & self.int_ena, 0x14 => self.int_ena,
            0x20 => self.rx_conf & !(1 << 8) & !3, 0x24 => self.tx_conf & !(1 << 8) & !3,   // update/reset bits self-clear
            0x6c => if self.tx_running() { 0 } else { 1 },                                   // STATE: tx_idle
            0x54 => self.tx_tdm_ctrl,
            0x80 => 0x2003070,
            _ => self.ram.read(off),
        }
    }
    pub fn write(&mut self, off: u32, v: u32) {
        match off {
            0x14 => self.int_ena = v, 0x18 => self.int_raw &= !v,
            0x20 => { self.rx_conf = v; if v & 3 != 0 { self.reset_rx_phase(); } self.streaming = (self.rx_conf | self.tx_conf) & 4 != 0; }
            0x24 => { self.tx_conf = v; self.update_rate(); self.streaming = (self.rx_conf | self.tx_conf) & 4 != 0; }
            0x2c => { self.tx_conf1 = v; self.ram.write(off, v); self.update_rate(); }
            0x34 | 0x3c if self.v2 => self.ram.write(off, v),
            0x34 => { self.tx_clkm_conf = v; self.ram.write(off, v); self.update_rate(); }
            0x3c => { self.tx_clkm_div_conf = v; self.ram.write(off, v); self.update_rate(); }
            0x54 => { self.tx_tdm_ctrl = v; self.ram.write(off, v); self.update_rate(); }
            _ => self.ram.write(off, v),
        }
    }
    pub fn irq(&self) -> bool { self.int_raw & self.int_ena != 0 }
    /// The TX frame rate the clock tree produces:
    ///   MCLK = src / (div_num + b/a)   with b/a recovered from the x/y/z/yn1 fields the way
    ///                                  `i2s_ll_tx_set_mclk` encodes them (b = z, a = (x+1)·z + y;
    ///                                  with yn1 the fraction is 1 − b/a)
    ///   BCK  = MCLK / (bck_div_num + 1)
    ///   fs   = BCK / (2 · (half_sample_bits + 1))
    /// Returns None while the clock is off or unprogrammed, so the default stays in force.
    pub fn derive_rate(&self) -> Option<u32> {
        if self.v2 {
            let (c, d) = self.v2_clock;
            if c & 1 == 0 || (c >> 1) & 3 != 0 { return None; }                   // off, or a source other than XTAL
            // into the classic words: CLK_ACTIVE, XTAL, DIV_N; z/y/x/yn1; BCK divider at conf1 12:7
            let conf = (1 << 26) | ((c >> 3) & 0xff);
            let div = ((d >> 18) & 0x1ff) | (((d >> 9) & 0x1ff) << 9) | ((d & 0x1ff) << 18) | (d & (1 << 27));
            let conf1 = ((self.tx_conf >> 21) & 0x3f) << 7;
            return Self::clock_rate(conf, div, conf1, 2 * (((self.tx_conf1 >> 19) & 0xff) as u64 + 1));
        }
        Self::clock_rate(self.tx_clkm_conf, self.tx_clkm_div_conf, self.tx_conf1, 2 * (((self.tx_conf1 >> 18) & 0x3f) as u64 + 1))
    }
    // ESP-IDF v5.5.5 components/hal/esp32s3/include/hal/i2s_ll.h,
    // i2s_ll_rx_clk_set_src and i2s_ll_rx_set_mclk: XTAL/PLL240/PLL160,
    // integer plus fractional divider. C6 i2s_ll.h:270-285 uses the same selectors.
    fn clock_rate(c: u32, d: u32, conf1: u32, frame_bits: u64) -> Option<u32> {
        if c & (1 << 26) == 0 { return None; }                                   // TX_CLK_ACTIVE
        let src: u64 = match (c >> 27) & 3 { 0 => 40_000_000, 1 => 240_000_000, 2 => 160_000_000, _ => return None };   // XTAL / PLL240M / PLL160M / external
        let n = (c & 0xff) as u64;
        if n == 0 { return None; }
        let (z, y, x, yn1) = ((d & 0x1ff) as u64, ((d >> 9) & 0x1ff) as u64, ((d >> 18) & 0x1ff) as u64, d & (1 << 27) != 0);
        let (a, b) = if z == 0 { (1, 0) } else { let a = (x + 1) * z + y; (a, if yn1 { a - z } else { z }) };
        let bck = ((conf1 >> 7) & 0x3f) as u64 + 1;
        // WS_WIDTH controls the WS pulse, not the frame duration. IDF programs
        // HALF_SAMPLE_BITS = slot_bits * total_slots / 2 - 1 for standard/TDM TX.
        let denom = (n * a + b) * bck * frame_bits;
        if denom == 0 { return None; }
        let fs = (src * a + denom / 2) / denom;
        if !(1_000..=400_000).contains(&fs) { return None; }
        Some(fs as u32)
    }
    fn update_rate(&mut self) {
        if let Some(fs) = self.derive_rate() { self.sample_rate = fs; }
        let slots = ((self.tx_tdm_ctrl >> 16) & 0xf) + 1;
        let active = (self.tx_tdm_ctrl & ((1 << slots) - 1)).count_ones();
        // SKIP_MSK consumes DMA data for inactive slots too. Otherwise only
        // enabled slots have data; mono reuses one sample across the frame.
        let dma_slots = if self.tx_mono() { u32::from(active != 0) }
            else if self.tx_tdm_ctrl & (1 << 20) != 0 { slots } else { active };
        self.bytes_per_frame = self.sample_bytes() as u32 * dma_slots;
    }
    /// Number of frames due after `cycles` CPU cycles at the configured sample rate.
    pub fn frames_due(&mut self, cycles: u64) -> u32 {
        if !self.tx_running() { self.acc = 0; return 0; }
        self.acc += cycles * self.sample_rate as u64;
        let n = (self.acc / self.cpu_hz) as u32;
        self.acc %= self.cpu_hz;
        n
    }
}

impl Device for I2s {
    fn read(&mut self, off: u32) -> u32 { I2s::read(self, off) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        I2s::write(self, off, v);
        if off == 0x20 { WriteEffect::RX_CONF } else { WriteEffect::NONE }
    }
    fn irq_sources(&self) -> u64 { self.irq() as u64 }
}

#[cfg(test)]
mod v2_tests {
    use super::*;

    /// 16 kHz, 16-bit stereo from XTAL as IDF programs it on the S31: MCLK = 256 fs = 40 MHz /
    /// (9 + 49/64) (yn1: z = 15, x = 3, y = 4), BCK = MCLK / 8, 32-bit frames.
    #[test]
    fn v2_takes_its_clock_from_outside_and_its_fields_from_their_own_places() {
        let mut i = I2s::new_v2(320_000_000);
        i.write(0x2c, (15 << 14) | (15 << 19));                          // TX_CONF1: 16-bit, half frame 16
        i.write(0x54, (1 << 16) | 3);                                     // TX_TDM_CTRL: two slots, both active
        i.write(0x34, 0xffff_ffff);                                       // RX_RECOMB_DMA_CH0: not a clock register
        assert_eq!((i.sample_rate, i.bytes_per_frame), (44100, 4));
        i.write(0x24, (7 << 21) | (1 << 2));                              // TX_CONF: BCK_DIV_NUM 8, TX_START
        i.set_v2_clock(1 | (9 << 3), 3 | (4 << 9) | (15 << 18) | (1 << 27));
        assert_eq!(i.sample_rate, 16000);
        i.write(0x24, (7 << 21) | (1 << 6) | (1 << 2));                   // TX_MONO
        assert_eq!(i.bytes_per_frame, 2);
    }
}

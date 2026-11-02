//! Original ESP32 RMT transmitter: eight channels sharing 512 words of RAM.
//! ESP-IDF v5.5.4 components/soc/esp32/register/soc/rmt_reg.h:79-138 (idle, clock and continuous bits); 0xa0-0xac interrupt banks and 0x800 RAM window.
//! Transaction completion and timing behavior are inferred, not hardware-validated.
use emu_core::ClockDomain;
use esp_periph::{Device, RegRam, WriteEffect};

const CHANNELS: usize = 8;
const WORDS: usize = 64;
pub const SIGNAL0: usize = 87;
const CONTINUOUS: u32 = 1 << 6;
const APB_SOURCE: u32 = 1 << 17;
const IDLE_LEVEL: u32 = 1 << 18;
const IDLE_ENABLE: u32 = 1 << 19;
/// Wrap mode refills the 64-word block, so a 256-LED matrix sends 6144 symbols in one
/// transmission. The limit and the policy past it are the shared model's.
use esp_periph::rmt::MAX_FRAME_BITS;

#[derive(Default)]
struct Channel {
    running: bool,
    rd: usize,
    phase: u8,
    symbol: u32,
    remaining: u64,
    since_threshold: u32,
    errors: u32,
    level: bool,
    bits: Vec<bool>,
}

pub struct ClassicRmt {
    regs: RegRam,
    mem: [u32; CHANNELS * WORDS],
    ch: [Channel; CHANNELS],
    raw: u32,
    ena: u32,
    pub clock_enabled: bool,
    pub outputs: Vec<(usize, bool)>,
    pub done: Vec<(usize, Vec<bool>)>,
    pub tx_count: u64,
    #[cfg(test)]
    pub(crate) tick_calls: u64,
}

impl Default for ClassicRmt {
    fn default() -> Self {
        Self::new()
    }
}

impl ClassicRmt {
    pub fn new() -> Self {
        let mut regs = RegRam::new();
        for n in 0..CHANNELS as u32 {
            regs.write(0x20 + n * 8, 0x3110_0002);
            regs.write(0x24 + n * 8, 0xf20);
            regs.write(0xb0 + n * 4, 0x0040_0040);
            regs.write(0xd0 + n * 4, 128);
        }
        regs.write(0xfc, 0x1602_2600);
        Self {
            regs,
            mem: [0; CHANNELS * WORDS],
            ch: Default::default(),
            raw: 0,
            ena: 0,
            clock_enabled: false,
            outputs: (0..CHANNELS).map(|n| (n, false)).collect(),
            done: Vec::new(),
            tx_count: 0,
            #[cfg(test)] tick_calls: 0,
        }
    }

    fn conf1(&self, n: usize) -> u32 {
        self.regs.read(0x24 + n as u32 * 8)
    }
    fn words(&self, n: usize) -> usize {
        ((self.regs.read(0x20 + n as u32 * 8) >> 24) & 15) as usize * WORDS
    }
    fn divider(&self, n: usize) -> u64 {
        let div = (self.regs.read(0x20 + n as u32 * 8) & 255) as u64;
        let div = if div == 0 { 256 } else { div };
        div * if self.conf1(n) & APB_SOURCE != 0 {
            1
        } else {
            80
        }
    }
    fn powered(&self) -> bool {
        self.clock_enabled && self.regs.read(0x20) & (1 << 30) == 0
    }
    fn level(&mut self, n: usize, level: bool) {
        self.ch[n].level = level;
        self.outputs.push((n, level));
    }
    fn idle(&mut self, n: usize) {
        let conf = self.conf1(n);
        if conf & IDLE_ENABLE != 0 {
            self.level(n, conf & IDLE_LEVEL != 0);
        }
    }
    fn error(&mut self, n: usize, status: u32) {
        self.ch[n].errors |= status;
        self.raw |= 1 << (3 * n + 2);
    }
    fn finish(&mut self, n: usize) {
        self.tx_count += 1;
        self.done.retain(|(channel, _)| *channel != n);
        let bits = std::mem::take(&mut self.ch[n].bits);
        if bits.len() <= MAX_FRAME_BITS { self.done.push((n, bits)); }
        self.idle(n);
        if self.conf1(n) & CONTINUOUS != 0 {
            self.ch[n].rd = 0;
            self.ch[n].phase = 0;
            self.ch[n].remaining = self.divider(n); // one idle tick between loops
        } else {
            self.ch[n].running = false;
            self.raw |= 1 << (3 * n);
        }
    }

    fn advance(&mut self, n: usize, mut ticks: u64) {
        while self.ch[n].running {
            if ticks < self.ch[n].remaining {
                self.ch[n].remaining -= ticks;
                break;
            }
            ticks -= self.ch[n].remaining;
            self.ch[n].remaining = 0;
            if self.ch[n].phase == 2 {
                self.ch[n].phase = 0;
                self.ch[n].rd += 1;
                self.ch[n].since_threshold += 1;
                let limit = self.regs.read(0xd0 + n as u32 * 4) & 0x1ff;
                if limit != 0 && self.ch[n].since_threshold >= limit {
                    self.ch[n].since_threshold = 0;
                    self.raw |= 1 << (24 + n);
                }
            }
            if self.ch[n].phase == 0 {
                let words = self.words(n);
                if words == 0 || words > self.mem.len() {
                    self.error(n, 1 << 29);
                    self.ch[n].running = false;
                    self.idle(n);
                    break;
                }
                if self.ch[n].rd >= words {
                    if self.regs.read(0xf0) & 2 != 0 {
                        self.ch[n].rd = 0;
                    } else {
                        self.error(n, 1 << 29);
                        self.ch[n].running = false;
                        self.idle(n);
                        break;
                    }
                }
                self.ch[n].symbol = self.mem[(n * WORDS + self.ch[n].rd) % self.mem.len()];
                let sym = self.ch[n].symbol;
                if sym & 0x7fff != 0 && self.ch[n].bits.len() <= MAX_FRAME_BITS { self.ch[n].bits.push(esp_periph::rmt::symbol_bit(sym)); }
            }
            let half = (self.ch[n].symbol >> (16 * self.ch[n].phase)) as u16;
            self.ch[n].phase += 1;
            let duration = u64::from(half & 0x7fff) * self.divider(n);
            self.level(n, half & 0x8000 != 0);
            if duration == 0 {
                self.finish(n);
                continue;
            }
            self.ch[n].remaining = duration;
        }
    }


}

impl Device for ClassicRmt {
    fn read(&mut self, off: u32) -> u32 {
        match off {
            0x60..=0x7c => {
                let n = ((off - 0x60) / 4) as usize;
                let c = &self.ch[n];
                c.errors
                    | (u32::from(c.running) << 24)
                    | (((n * WORDS + c.rd) as u32 & 0x3ff) << 12)
                    | (n * WORDS) as u32
            }
            0x00..=0x1c | 0x80..=0x9c => 0,
            0xa0 => self.raw,
            0xa4 => self.raw & self.ena,
            0xa8 => self.ena,
            0xac => 0,
            0x800..=0xffc => self.mem[((off - 0x800) / 4) as usize],
            _ => self.regs.read(off),
        }
    }
    fn write(&mut self, off: u32, value: u32) -> WriteEffect {
        match off {
            0x00..=0x1c => {}
            0x24..=0x5c if off % 8 == 4 => {
                let n = ((off - 0x24) / 8) as usize;
                self.regs.write(off, value & 0xfffff & !0x1d & !(1 << 16));
                let c = &mut self.ch[n];
                if value & (1 << 3) != 0 {
                    c.rd = 0;
                    c.phase = 0;
                    c.remaining = 0;
                    c.errors &= !(1 << 29);
                }
                if value & 1 != 0 {
                    c.running = true;
                    c.phase = 0;
                    c.remaining = 0;
                    c.since_threshold = 0;
                    c.bits.clear();
                }
                if !self.ch[n].running {
                    self.idle(n);
                }
                if self.powered() {
                    self.advance(n, 0);
                }
            }
            0x60..=0xa4 => {}
            0xa8 => self.ena = value,
            0xac => self.raw &= !value,
            0xd0..=0xec => self.regs.write(off, value & 0x1ff),
            0xf0 => self.regs.write(off, value & 3),
            0x800..=0xffc => self.mem[((off - 0x800) / 4) as usize] = value,
            _ => self.regs.write(off, value),
        }
        WriteEffect::NONE
    }
    fn irq_sources(&self) -> u64 {
        u64::from(self.raw & self.ena != 0)
    }
    fn clock(&self) -> Option<ClockDomain> {
        (self.powered() && self.ch.iter().any(|c| c.running)).then_some(ClockDomain::Apb)
    }
    fn tick(&mut self, ticks: u64) {
        #[cfg(test)] { self.tick_calls += 1; }
        if self.powered() {
            for n in 0..CHANNELS {
                self.advance(n, ticks);
            }
        }
    }
    fn has_deadline(&self) -> bool {
        true
    }
    fn next_deadline(&self) -> Option<u64> {
        if !self.powered() {
            return None;
        }
        self.ch
            .iter()
            .filter(|c| c.running)
            .map(|c| c.remaining.max(1))
            .min()
    }
}

#[cfg(test)]
mod tests {
    use super::ClassicRmt;
    use esp_periph::Device;

    const APB: u32 = 1 << 17;
    const OWNER: u32 = 1 << 5;
    const IDLE: u32 = 1 << 19;

    fn model() -> ClassicRmt {
        let mut r = ClassicRmt::new();
        r.clock_enabled = true;
        r.write(0xf0, 1);
        r
    }

    fn conf1(n: u32) -> u32 {
        0x24 + 8 * n
    }
    fn ram(word: u32) -> u32 {
        0x800 + 4 * word
    }
    fn item(high: u32, low: u32) -> u32 {
        high | (1 << 15) | (low << 16)
    }
    fn configure(r: &mut ClassicRmt, n: u32, blocks: u32, div: u32, apb: bool) {
        r.write(0x20 + 8 * n, (blocks << 24) | div);
        r.write(conf1(n), OWNER | IDLE | if apb { APB } else { 0 });
    }
    fn start(r: &mut ClassicRmt, n: u32) {
        let conf = r.read(conf1(n));
        r.write(conf1(n), conf | 1 | (1 << 3) | (1 << 4));
    }

    #[test]
    fn unending_stream_bounds_bits_and_does_not_publish_truncation() {
        let mut r = model();
        configure(&mut r, 0, 1, 1, true);
        r.write(0xf0, 3);
        for i in 0..64 { r.write(ram(i), item(1, 1)); }
        start(&mut r, 0);
        r.tick(2 * super::MAX_FRAME_BITS as u64 + 20_000);
        assert_eq!(r.ch[0].bits.len(), super::MAX_FRAME_BITS + 1);
        for i in 0..64 { r.write(ram(i), 0); }
        r.tick(128);
        assert!(r.done.is_empty());
        assert_eq!(r.read(0xa0) & 1, 1);
    }

    /// A 32×8 WS2812 matrix is 6144 symbols, sent through the 64-word block in wrap mode.
    #[test]
    fn a_wrapped_256_pixel_frame_is_published_whole() {
        let mut r = model();
        configure(&mut r, 0, 1, 1, true);
        r.write(0xf0, 3);
        for i in 0..64 { r.write(ram(i), item(1, 1)); }
        start(&mut r, 0);
        r.tick(2 * 6144 - 1);
        for i in 0..64 { r.write(ram(i), 0); }
        r.tick(128);
        assert_eq!(r.done.len(), 1);
        assert_eq!(r.done[0].1.len(), 6144);
        assert_eq!(r.tx_count, 1);
    }

    #[test]
    fn eight_channels_have_independent_ram_and_tx_interrupts() {
        let mut r = model();
        let ends = (0..8).fold(0, |bits, n| bits | (1 << (3 * n)));
        r.write(0xa8, ends);
        for n in 0..8 {
            configure(&mut r, n, 1, 1, true);
            r.write(ram(64 * n), item(n + 1, 2));
            r.write(ram(64 * n + 1), 0);
            start(&mut r, n);
        }
        assert_eq!(r.read(0xa0), 0);
        r.tick(3);
        assert_eq!(r.read(0xa4), 1);
        r.tick(7);
        assert_eq!(r.read(0xa4), ends);
        assert_eq!(r.irq_sources(), 1);
        for n in 0..8 {
            assert_eq!(r.done.iter().find(|(channel, _)| *channel == n as usize).unwrap().1, [n + 1 > 2]);
            assert_eq!(r.read(0x60 + 4 * n) & (7 << 24), 0);
            assert_eq!(r.read(conf1(n)) & (1 | (1 << 3) | (1 << 4)), 0);
        }
        r.write(0xac, ends);
        assert_eq!(r.read(0xa0), 0);
        assert_eq!(r.irq_sources(), 0);
    }

    #[test]
    fn borrowed_blocks_share_ram_and_wrap_after_word_511() {
        for n in [2, 7] {
            let mut r = model();
            configure(&mut r, n, 2, 1, true);
            for i in 0..65 {
                r.write(ram((64 * n + i) % 512), item(1, 1));
            }
            r.write(ram((64 * n + 65) % 512), 0);
            start(&mut r, n);
            r.tick(129);
            assert_eq!(r.read(0xa0) & (1 << (3 * n)), 0);
            r.tick(1);
            assert_eq!(r.read(0xa0), 1 << (3 * n));
        }
    }

    #[test]
    fn zero_divider_is_256_for_both_clock_sources() {
        for apb in [true, false] {
            let mut r = model();
            configure(&mut r, 0, 1, 0, apb);
            r.write(ram(0), item(1, 1));
            r.write(ram(1), 0);
            start(&mut r, 0);
            let half = if apb { 256 } else { 256 * 80 };
            assert_eq!(r.next_deadline(), Some(half));
            r.tick(half - 1);
            assert_eq!(r.read(0xa0), 0);
            assert_eq!(r.next_deadline(), Some(1));
            r.tick(1);
            assert_eq!(r.next_deadline(), Some(half));
            r.tick(half - 1);
            assert_eq!(r.read(0xa0), 0);
            r.tick(1);
            assert_eq!(r.read(0xa0), 1);
            assert_eq!(r.next_deadline(), None);
        }
    }

    #[test]
    fn zero_duration_ends_at_the_first_or_second_half_boundary() {
        for first in [0, 7] {
            let mut r = model();
            configure(&mut r, 0, 1, 3, true);
            let second = if first == 0 { 32767 } else { 0 };
            r.write(ram(0), item(first, second));
            start(&mut r, 0);
            if first != 0 {
                r.tick(20);
                assert_eq!(r.read(0xa0), 0);
                assert_eq!(r.next_deadline(), Some(1));
                r.tick(1);
            }
            assert_eq!(r.read(0xa0), 1);
            assert_eq!(r.read(0xa4), 0, "raw completion latches while masked");
            r.write(0xa8, 1);
            assert_eq!(r.read(0xa4), 1);
            assert_eq!(r.next_deadline(), None);
            r.write(0xac, 1);
            r.write(ram(0), item(2, 0));
            start(&mut r, 0);
            r.tick(5);
            assert_eq!(r.read(0xa0), 0);
            r.tick(1);
            assert_eq!(r.read(0xa0), 1, "a reset pointer starts the next frame");
        }
    }

    #[test]
    fn threshold_interrupts_allow_refilling_a_wrapped_96_word_stream() {
        let mut r = model();
        configure(&mut r, 0, 1, 1, true);
        r.write(0xf0, 3);
        r.write(0xd0, 32);
        r.write(0xa8, 1 | (1 << 24));
        for i in 0..64 {
            r.write(ram(i), item(4, 4));
        }
        start(&mut r, 0);
        r.tick(255);
        assert_eq!(r.read(0xa4), 0);
        r.tick(1);
        assert_eq!(r.read(0xa4), 1 << 24);
        r.write(0xac, 1 << 24);
        for i in 0..32 {
            r.write(ram(i), item(4, 4));
        }
        r.tick(256);
        assert_eq!(r.read(0xa4), 1 << 24);
        r.write(0xac, 1 << 24);
        r.write(ram(32), 0);
        r.tick(255);
        assert_eq!(r.read(0xa4), 0);
        r.tick(1);
        assert_eq!(r.read(0xa4), 1 | (1 << 24));
        assert_eq!(r.next_deadline(), None);
    }

    #[test]
    fn continuous_output_has_an_idle_tick_and_stops_after_eof() {
        let mut r = model();
        configure(&mut r, 0, 1, 1, true);
        r.write(conf1(0), APB | OWNER | IDLE | (1 << 6));
        r.write(0xd0, 0xffff_fe00);
        assert_eq!(r.read(0xd0), 0, "classic has no finite loop-count fields");
        r.write(ram(0), item(2, 3));
        r.write(ram(1), 0);
        start(&mut r, 0);
        r.tick(5);
        assert_eq!(r.read(0xa0), 0);
        assert_eq!(r.next_deadline(), Some(1), "one idle tick between laps");
        r.tick(1);
        assert_eq!(r.next_deadline(), Some(2));
        r.tick(4);
        assert_eq!(r.next_deadline(), Some(1));
        r.write(conf1(0), APB | OWNER | IDLE);
        for i in 0..64 {
            r.write(ram(i), 0);
        }
        assert_eq!(r.read(0xa0), 0);
        r.tick(1);
        assert_eq!(r.read(0xa0), 1);
        assert_eq!(r.next_deadline(), None);
    }

    #[test]
    fn no_wrap_exhaustion_and_a_shrunken_allocation_raise_memory_empty() {
        for shrink in [false, true] {
            let mut r = model();
            configure(&mut r, 0, if shrink { 2 } else { 1 }, 1, true);
            for i in 0..65 {
                r.write(ram(i), item(1, 1));
            }
            start(&mut r, 0);
            if shrink {
                r.tick(128);
                r.write(0x20, (1 << 24) | 1);
                r.tick(1);
            } else {
                r.tick(127);
            }
            assert_eq!(r.read(0xa0), 0);
            r.tick(1);
            assert_eq!(r.read(0xa0), 4, "exhaustion is an error, not TX_END");
            assert_ne!(r.read(0x60) & (1 << 29), 0);
            assert_eq!(r.next_deadline(), None);
        }
    }

    #[test]
    fn pointer_reset_discards_the_old_pulse_deadline() {
        let mut r = model();
        configure(&mut r, 0, 1, 1, true);
        r.write(ram(0), item(10, 0));
        start(&mut r, 0);
        r.tick(5);
        r.write(conf1(0), APB | OWNER | IDLE | (1 << 3));
        assert_eq!(r.next_deadline(), Some(10));
        r.tick(9);
        assert_eq!(r.read(0xa0), 0);
        r.tick(1);
        assert_eq!(r.read(0xa0), 1);
    }
}

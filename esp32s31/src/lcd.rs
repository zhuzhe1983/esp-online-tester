//! S31 LCD RGB registers and bounded scan timing; pixels come only from AXI-GDMA.
//! ESP-IDF v6.1 fff9895c: esp_hal_lcd/esp32s31/include/hal/lcd_ll.h and
//! soc/esp32s31/register/soc/{lcd_cam,hp_sys_clkrst}_reg.h. No i80/camera/converter model.
pub const BASE: u32 = 0x2039_6000;
pub const BUS_CLOCK: u32 = 0x2058_714c;
pub const PIXEL_CLOCK: u32 = BUS_CLOCK + 4;
pub const SHARED_CLOCK: u32 = BUS_CLOCK + 8;
pub const USER: u32 = BASE + 0x14;
pub const IRQ_SOURCE: usize = 13;
pub const DMA_TRIGGER: u32 = 0; // esp_hal_dma/esp32s31/include/hal/gdma_channel.h LCD0, AXI
const START: u32 = 1 << 27;
const MAX_FRAME: usize = 16 * 1024 * 1024;
pub struct Lcd {
    regs: [u32; 64], clocks: [u32; 3], elapsed: u64, frame_cycles: u64,
    pub(crate) data: Vec<u8>, pub(crate) bytes: usize, pub(crate) geometry: (u32, u32, u32),
    pub(crate) start_pending: bool, failed: bool, pub frames: u64, pub faults: u64,
}
impl Default for Lcd {
    fn default() -> Self { let mut regs = [0; 64]; regs[0] = 1 << 6; Self { regs, clocks: [0; 3], elapsed: 0, frame_cycles: 0, data: Vec::new(), bytes: 0, geometry: (0, 0, 0), start_pending: false, failed: false, frames: 0, faults: 0 } }
}
impl Lcd {
    pub fn is_mmio(a: u32) -> bool { (BASE..BASE + 0x100).contains(&a) || matches!(a & !3, BUS_CLOCK | PIXEL_CLOCK | SHARED_CLOCK) }
    pub fn read(&self, a: u32) -> u32 { match a { BUS_CLOCK..=SHARED_CLOCK => self.clocks[((a - BUS_CLOCK) / 4) as usize],
        a if a == BASE + 0x6c => self.regs[0x68 / 4] & self.regs[0x64 / 4], a if a == BASE + 0x70 => 0,
        a if (BASE..BASE + 0x100).contains(&a) => self.regs[((a - BASE) / 4) as usize], _ => 0 } }
    pub fn write(&mut self, a: u32, v: u32) {
        if (BUS_CLOCK..=SHARED_CLOCK).contains(&a) {
            self.clocks[((a - BUS_CLOCK) / 4) as usize] = v;
            if a == BUS_CLOCK && v & 10 != 0 { let clocks = self.clocks; *self = Self { clocks, ..Default::default() }; }
            return;
        }
        let off = a.wrapping_sub(BASE);
        if off >= 0x100 { return; }
        match off {
            0x14 => {
                let start = v & START != 0 && self.regs[0x14 / 4] & START == 0;
                self.regs[0x14 / 4] = v & !((1 << 21) | (1 << 28)); // UPDATE/RESET self clear
                if v & (1 << 28) != 0 { self.regs[0x14 / 4] &= !START; self.data.clear(); self.bytes = 0; self.start_pending = false; }
                else if start { self.start_pending = true; }
            }
            0x18 => { self.regs[off as usize / 4] = v & !(1 << 27); if v & (1 << 27) != 0 { self.data.clear(); self.elapsed = 0; } }
            0x64 => self.regs[off as usize / 4] = v & 0x13,
            0x68 | 0x6c => {}, 0x70 => self.regs[0x68 / 4] &= !v,
            _ => self.regs[off as usize / 4] = v,
        }
    }
    pub fn irq(&self) -> bool { self.regs[0x64 / 4] & self.regs[0x68 / 4] != 0 }
    fn gated(&self) -> bool { self.clocks[0] & 15 == 5 && self.clocks[1] & 4 != 0 && self.clocks[2] & 4 != 0 }
    pub fn running(&self) -> bool { self.gated() && self.regs[0x14 / 4] & START != 0 && self.regs[0x18 / 4] & 8 != 0 && !self.failed && self.bytes != 0 }
    pub fn begin(&mut self, pll160_hz: Option<u64>) -> Option<(u32, u32, u32)> {
        self.start_pending = false; self.elapsed = 0; self.data.clear(); self.failed = false;
        let bpp = ((self.regs[0x14 / 4] >> 19) & 3) + 1;
        let (w, h) = ((self.regs[0x20 / 4] & 0xffff) + 1, (self.regs[0x1c / 4] & 0xffff) + 1);
        // 24-bit and color conversion are deliberately not handed to the RGB565/EPD callback.
        if bpp > 2 || self.regs[0x10 / 4] & (1 << 31) != 0 || self.regs[0x14 / 4] & ((1 << 17) | (1 << 18)) != 0 { self.fail(); return None; }
        self.geometry = (w, h, bpp); self.bytes = (u64::from(w) * u64::from(h) * u64::from(bpp)).min(usize::MAX as u64) as usize;
        if self.bytes > MAX_FRAME || !self.gated() { self.fail(); return None; }
        let c = self.clocks[1];
        let source = match c & 3 { 0 => 40_000_000, 1 => pll160_hz.unwrap_or(0), _ => 0 };
        if source == 0 { self.fail(); return None; }
        let a = u64::from((c >> 19) & 255).max(1); let b = u64::from((c >> 11) & 255);
        let divisor = (u64::from((c >> 3) & 255) + 1) * a + b;
        let pixel_div = if self.regs[0] & (1 << 6) != 0 { 1 } else { u64::from(self.regs[0] & 63) + 1 };
        let total = (u64::from(self.regs[0x20 / 4] >> 16) + 1) * (u64::from(self.regs[0x1c / 4] >> 16) + 1);
        self.frame_cycles = ((u128::from(total) * u128::from(divisor) * u128::from(pixel_div) * u128::from(crate::periph::CPU_HZ)).div_ceil(u128::from(source) * u128::from(a))).min(u128::from(u64::MAX)) as u64;
        self.frame_cycles = self.frame_cycles.max(1);
        self.running().then_some(self.geometry)
    }
    pub fn advance(&mut self, cycles: u64) { self.elapsed = self.elapsed.saturating_add(cycles); }
    pub fn due(&self) -> usize {
        if !self.running() { return 0; }
        let n = (u128::from(self.elapsed) * self.bytes as u128 / u128::from(self.frame_cycles)).min(self.bytes as u128) as usize;
        n.saturating_sub(self.data.len())
    }
    pub fn fail(&mut self) { self.failed = true; self.faults += 1; self.regs[0x68 / 4] |= 1 << 4; }
    pub fn finish(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        if !self.running() || self.data.len() != self.bytes || self.elapsed < self.frame_cycles { return None; }
        let (w, h, bpp) = self.geometry; let mut data = std::mem::take(&mut self.data);
        let user = self.regs[0x14 / 4];
        if bpp == 2 && user & (1 << 23) != 0 { for pair in data.as_chunks_mut::<2>().0 { pair.swap(0, 1); } }
        if user & (1 << 22) != 0 { if bpp == 1 { for b in &mut data { *b = b.reverse_bits(); } } else { for p in data.as_chunks_mut::<2>().0 { *p = u16::from_le_bytes(*p).reverse_bits().to_le_bytes(); } } }
        self.frames += 1; self.regs[0x68 / 4] |= 3; // VSYNC_END and TRANS_DONE, after scan, not DMA EOF
        self.elapsed = self.elapsed.saturating_sub(self.frame_cycles);
        if self.regs[0x18 / 4] & (1 << 25) == 0 { self.regs[0x14 / 4] &= !START; }
        Some((w, h, data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(crate) fn start(l: &mut Lcd, w: u32, h: u32) {
        l.write(BUS_CLOCK, 5); l.write(PIXEL_CLOCK, 4); l.write(SHARED_CLOCK, 4);
        l.write(BASE + 0x18, 8); l.write(BASE + 0x20, (w - 1) | (w - 1) << 16);
        l.write(BASE + 0x1c, (h - 1) | (h - 1) << 16); l.write(USER, START | (1 << 24));
        assert_eq!(l.begin(None), Some((w, h, 1)));
    }
    #[test]
    fn guest_geometry_timing_gates_and_w1c_are_independent_of_dma() {
        for (w, h) in [(304, 684), (305, 688)] {
            let mut l = Lcd::default(); start(&mut l, w, h);
            assert_eq!(l.bytes, (w * h) as usize); assert_eq!(l.frame_cycles, u64::from(w * h) * 8);
            l.write(BASE + 0x64, 3); l.advance(l.frame_cycles - 1);
            l.data.resize(l.bytes, 0x55); assert!(l.finish().is_none()); assert!(!l.irq());
            l.advance(1); let (width, height, data) = l.finish().unwrap();
            assert_eq!((width, height, data.len()), (w, h, (w * h) as usize));
            assert_eq!(l.read(BASE + 0x6c), 3); assert!(!l.running());
            l.write(BASE + 0x70, 1); assert_eq!(l.read(BASE + 0x6c), 2);
            l.write(BASE + 0x64, 0); assert!(!l.irq()); assert_eq!(l.read(BASE + 0x68), 2);
        }
        let mut l = Lcd::default(); start(&mut l, 4, 2); l.write(PIXEL_CLOCK, 0); l.advance(64);
        assert!(!l.running()); assert_eq!(l.due(), 0); assert!(l.finish().is_none());
        l.write(PIXEL_CLOCK, 4); assert!(l.running());
        l.write(USER, 1 << 28); assert!(!l.running()); assert!(!l.start_pending);
    }
    #[test]
    fn malformed_geometry_and_unsupported_conversion_cannot_allocate_a_frame() {
        let mut l = Lcd::default();
        l.write(BUS_CLOCK, 5); l.write(PIXEL_CLOCK, 4); l.write(SHARED_CLOCK, 4);
        l.write(BASE + 0x18, 8); l.write(BASE + 0x20, 65535); l.write(BASE + 0x1c, 65535);
        l.write(USER, START); assert_eq!(l.begin(None), None); assert!(l.data.is_empty());
        l.write(USER, 0); l.write(BASE + 0x20, 0); l.write(BASE + 0x1c, 0);
        l.write(BASE + 0x10, 1 << 31); l.write(USER, START); assert_eq!(l.begin(None), None);
        l.write(BASE + 0x64, 1 << 4); assert!(l.irq()); assert_eq!(l.faults, 2);
    }
}

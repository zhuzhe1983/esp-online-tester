//! DesignWare AXI DMAC, P4 ESP-IDF v5.4.2 dw_gdma_reg.h / dw_gdma_ll.h.
//! Four channels, 64-byte LLIs and direct blocks. This implements the memory to
//! DSI flow used by esp_lcd_panel_dpi; CSI/ISP and shadow-register modes refuse.
use esp_periph::{Device, RegRam, WriteEffect};
use esp_periph::sdmmc::SdmmcMemory;
pub const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;
const CHANNELS: usize = 4;
const MAX_ITEMS: usize = 1024;
const SRC_ERR: u32 = 1 << 5;
const DST_ERR: u32 = 1 << 6;
const LLI_ERR: u32 = 1 << 9;
const INVALID: u32 = 1 << 13;
pub struct DwGdma { ram: RegRam, enabled: u32, suspended: u32, pub transfers: u64, pub errors: u64 }
impl Default for DwGdma { fn default() -> Self { Self::new() } }
impl DwGdma {
    pub fn new() -> Self {
        let mut ram = RegRam::new(); ram.write(8, 842018858);
        for ch in 0..CHANNELS { let b = 0x100 * (ch as u32 + 1); ram.write(b + 0x18, 2 << 8 | 2 << 11); ram.write(b + 0x24, 0x6001b); }
        Self { ram, enabled: 0, suspended: 0, transfers: 0, errors: 0 }
    }
    pub fn pending(&self) -> bool { self.enabled & !self.suspended != 0 && self.ram.read(0x10) & 1 != 0 }
    fn fail(&mut self, ch: usize, error: u32) { self.errors += 1; self.complete(ch, error); }
    fn complete(&mut self, ch: usize, events: u32) {
        self.enabled &= !(1 << ch); let b = (ch as u32 + 1) * 0x100;
        let events = events & self.ram.read(b + 0x80); self.ram.write(b + 0x88, self.ram.read(b + 0x88) | events);
    }
    pub fn service_dsi(&mut self, mem: &mut impl SdmmcMemory, expected: usize) -> Option<Vec<u8>> {
        if !self.pending() { return None; }
        let ch = (self.enabled & !self.suspended).trailing_zeros() as usize;
        let b = (ch as u32 + 1) * 0x100;
        let cfg = self.ram.read(b + 0x20); let flow = self.ram.read(b + 0x24);
        if flow & 7 != 1 || flow & ((1 << 4) | (3 << 12)) != 0 { self.fail(ch, DST_ERR); return None; }
        let linked = cfg & 0xf == 0xf;
        if cfg & 0xf != 0 && !linked { self.fail(ch, INVALID); return None; }
        let mut addr = self.ram.read(b + 0x28) & !0x3f;
        if self.ram.read(b + 0x2c) != 0 { self.fail(ch, LLI_ERR); return None; }
        let mut out = Vec::new();
        for _ in 0..MAX_ITEMS {
            let (src, dst, count, next, ctl, ctl_hi) = if linked {
                if addr == 0 { self.fail(ch, LLI_ERR); return None; }
                let mut data = [0; 64];
                if mem.read(addr, &mut data).is_err() { self.fail(ch, LLI_ERR); return None; }
                let w = |i: usize| u32::from_le_bytes(data[i..i + 4].try_into().unwrap());
                if w(4) != 0 || w(12) != 0 || w(28) != 0 { self.fail(ch, LLI_ERR); return None; }
                (w(0), w(8), w(16) & 0x3f_ffff, w(24) & !0x3f, w(32), w(36))
            } else { (self.ram.read(b), self.ram.read(b + 8), self.ram.read(b + 0x10) & 0x3f_ffff, 0, self.ram.read(b + 0x18), self.ram.read(b + 0x1c)) };
            if linked && ctl_hi & (1 << 31) == 0 { self.fail(ch, INVALID); return None; }
            // Memory port 1, incrementing source; DSI port 0, fixed destination.
            if ctl & 1 == 0 || ctl & 1 << 4 != 0 { self.fail(ch, SRC_ERR); return None; }
            if dst != crate::dsi::PIXEL_SINK || ctl & (1 << 2) != 0 || ctl & (1 << 6) == 0 { self.fail(ch, DST_ERR); return None; }
            let (sw, dw) = (ctl >> 8 & 7, ctl >> 11 & 7);
            if sw > 3 || dw > 3 { self.fail(ch, INVALID); return None; }
            let n = (count as usize + 1) << sw;
            if !n.is_multiple_of(1 << dw) || out.len().checked_add(n).is_none_or(|len| len > MAX_FRAME_BYTES || len > expected) { self.fail(ch, INVALID); return None; }
            let at = out.len(); out.resize(at + n, 0);
            if mem.read(src, &mut out[at..]).is_err() { self.fail(ch, SRC_ERR); return None; }
            if linked {
                // DMAC consumes VALID and writes the completed block count to the LLI.
                if mem.write(addr + 36, &(ctl_hi & !(1 << 31)).to_le_bytes()).is_err()
                    || mem.write(addr + 48, &(count + 1).to_le_bytes()).is_err() { self.fail(ch, 1 << 10); return None; }
            }
            self.ram.write(b + 0x30, count + 1);
            if ctl_hi & (1 << 26) != 0 { let raw = self.ram.read(b + 0x88); self.ram.write(b + 0x88, raw | (1 & self.ram.read(b + 0x80))); }
            if !linked || ctl_hi & (1 << 30) != 0 {
                if out.len() != expected { self.fail(ch, INVALID); return None; }
                self.transfers += 1; self.complete(ch, 2 | 1 << 3 | 1 << 4); return Some(out);
            }
            if next == 0 { self.fail(ch, LLI_ERR); return None; }
            addr = next; self.ram.write(b + 0x28, next | 1);
        }
        self.fail(ch, LLI_ERR); None
    }
    fn interrupt_status(&self) -> u32 {
        if self.ram.read(0x10) & 2 == 0 { return 0; }
        (0..CHANNELS).fold(0, |mask, ch| { let b = (ch as u32 + 1) * 0x100; mask | if self.ram.read(b + 0x88) & self.ram.read(b + 0x90) != 0 { 1 << ch } else { 0 } })
    }
}
impl Device for DwGdma {
    fn read(&mut self, off: u32) -> u32 { match off { 0x18 => self.enabled | self.suspended << 16, 0x1c | 0x58 => 0, 0x30 => self.interrupt_status(), _ => self.ram.read(off) } }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        match off {
            0 | 8 | 0x30 => {},
            0x18 => {
                let en = v >> 8 & 0xf; self.enabled = (self.enabled & !en) | (v & en);
                let sus = v >> 24 & 0xf; self.suspended = (self.suspended & !sus) | ((v >> 16) & sus);
            }
            0x1c => { let abort = v & (v >> 8) & 0xf; for ch in 0..CHANNELS { if abort & (1 << ch) != 0 { self.complete(ch, 1 << 31); } } }
            0x58 if v & 1 != 0 => *self = Self::new(),
            o if (0x100..0x500).contains(&o) && o & 0xff == 0x98 => { let raw = o - 0x10; self.ram.write(raw, self.ram.read(raw) & !v); }
            o if (0x100..0x500).contains(&o) && o & 0xff == 0x88 => {},
            _ => self.ram.write(off, v),
        }
        WriteEffect::NONE
    }
    fn irq_sources(&self) -> u64 { u64::from(self.interrupt_status() != 0) }
}

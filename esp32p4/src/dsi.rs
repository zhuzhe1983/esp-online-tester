//! P4 DSI host/bridge digital subset, ESP-IDF v5.4.2 mipi_dsi_{host,bridge}_reg.h
//! and hal/{mipi_dsi,mipi_dsi_phy,mipi_dsi_brg}_ll.h. Fixed D-PHY lanes, packet
//! FIFOs and DPI scanout; no analog signalling, ECC injection, YUV conversion or BIST.
use emu_core::ClockDomain;
use esp_periph::{Device, RegRam, WriteEffect};
use std::collections::VecDeque;

pub const PIXEL_SINK: u32 = 0x5010_5000;
const FIFO_BYTES: usize = 4096;
pub struct Packet { pub vc: u8, pub kind: u8, pub payload: Vec<u8> }
pub struct Dsi {
    ram: RegRam,
    payload: Vec<u8>, rx: VecDeque<u8>, packets: VecDeque<Packet>,
    pub packets_sent: u64, pub frames: u64, pub refused: u64,
    pub bus_clock: bool, pub phy_clock: bool, pub supply: bool, pub dpi_hz: u64,
    cycle: u64, lock_at: Option<u64>, next_frame: Option<u64>, due: bool,
    test_address: Option<u8>, test_data: u8, pll_programmed: bool,
    phy_registers: [u8; 256],
}
impl Default for Dsi { fn default() -> Self { Self::new() } }
impl Dsi {
    pub fn new() -> Self {
        let mut ram = RegRam::new(); ram.write(0, 0x3134_302a); ram.write(0x34, 1); ram.write(0xa4, 1);
        ram.write(0xc4, u32::MAX); ram.write(0xc8, u32::MAX);
        Self { ram, payload: Vec::new(), rx: VecDeque::new(), packets: VecDeque::new(), packets_sent: 0, frames: 0, refused: 0,
            bus_clock: false, phy_clock: false, supply: false, dpi_hz: 0, cycle: 0, lock_at: None, next_frame: None, due: false,
            test_address: None, test_data: 0, pll_programmed: false, phy_registers: [0; 256] }
    }
    pub fn reset(&mut self) { let cycle = self.cycle; *self = Self::new(); self.cycle = cycle; }
    pub fn set_environment(&mut self, bus: bool, phy: bool, supply: bool, dpi_hz: u64) {
        self.bus_clock = bus; self.phy_clock = phy; self.supply = supply; self.dpi_hz = dpi_hz; self.update();
    }
    fn phy_enabled(&self) -> bool { self.bus_clock && self.phy_clock && self.supply && self.ram.read(0xa0) & 7 == 7 && self.pll_programmed }
    pub fn locked(&self) -> bool { self.phy_enabled() && self.lock_at.is_some_and(|at| at <= self.cycle) }
    pub fn video_enabled(&self) -> bool {
        self.locked() && self.ram.read(4) & 1 != 0 && self.ram.read(0x34) & 1 == 0 && self.ram.read(0x804) & 1 != 0
            && self.ram.read(0x840) & 1 != 0 && self.dpi_hz != 0 && self.ram.read(0xa8) & 5 == 0
    }
    pub fn geometry(&self) -> Option<(u32, u32, usize)> {
        let (w, h) = (self.ram.read(0x838) >> 16 & 0xfff, self.ram.read(0x830) >> 16 & 0xfff);
        let format = self.ram.read(0x818);
        let bytes = match format & 3 { 2 => 2, 0 | 1 => 3, _ => return None };
        if w == 0 || h == 0 || w > 2048 || h > 2048 || format & 0x40 != 0 { None } else { Some((w, h, bytes)) }
    }
    fn frame_cycles(&self) -> u64 {
        let ht = self.ram.read(0x838) & 0xffff; let vt = self.ram.read(0x830) & 0xffff;
        if self.dpi_hz == 0 { return crate::periph::CPU_HZ / 60; }
        (ht as u64 * vt as u64 * crate::periph::CPU_HZ / self.dpi_hz).max(crate::periph::CPU_HZ / 240)
    }
    fn update(&mut self) {
        if self.phy_enabled() { if self.lock_at.is_none() { self.lock_at = Some(self.cycle.saturating_add(32)); } }
        else { self.lock_at = None; }
        if self.video_enabled() {
            let first = self.cycle.saturating_add(self.frame_cycles());
            let at = *self.next_frame.get_or_insert(first);
            if self.cycle >= at { self.due = true; }
        } else { self.next_frame = None; self.due = false; }
    }
    pub fn frame_due(&self) -> bool { self.due && self.video_enabled() }
    pub fn finish_frame(&mut self, data: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
        self.due = false; self.next_frame = Some(self.cycle.saturating_add(self.frame_cycles()));
        let Some((w, h, bytes)) = self.geometry() else { self.underrun(); return None };
        if !self.video_enabled() || data.len() != w as usize * h as usize * bytes { self.underrun(); return None; }
        let out = if bytes == 2 { data.to_vec() } else {
            let mut out = Vec::with_capacity(w as usize * h as usize * 2);
            for c in data.as_chunks::<3>().0 { let p = (c[0] as u16 >> 3) << 11 | (c[1] as u16 >> 2) << 5 | c[2] as u16 >> 3; out.extend_from_slice(&p.to_le_bytes()); }
            out
        };
        self.frames = self.frames.wrapping_add(1); Some((w, h, out))
    }
    pub fn underrun(&mut self) { self.ram.write(0x858, self.ram.read(0x858) | 1); self.refused = self.refused.wrapping_add(1); }
    pub fn take_packet(&mut self) -> Option<Packet> { self.packets.pop_front() }
    pub fn response(&mut self, data: &[u8]) { self.rx.extend(data.iter().copied().take(FIFO_BYTES)); }
    fn header(&mut self, v: u32) {
        let (kind, vc, a, b) = ((v & 0x3f) as u8, (v >> 6 & 3) as u8, (v >> 8) as u8, (v >> 16) as u8);
        if !self.locked() || self.ram.read(4) & 1 == 0 { self.refused += 1; self.payload.clear(); return; }
        let payload = match kind {
            0x05 | 0x06 | 0x13 | 0x14 => vec![a], 0x15 | 0x23 | 0x24 => vec![a, b], 0x03 | 0x04 => Vec::new(),
            0x29 | 0x39 => {
                let n = a as usize | (b as usize) << 8;
                if n == 0 || n > self.payload.len() || n > FIFO_BYTES { self.refused += 1; self.payload.clear(); return; }
                self.payload.drain(..n).collect()
            }
            // Set maximum return packet size, consumed by the host rather than by the panel.
            0x37 => { self.ram.write(0x6c, v); return; }
            _ => { self.refused += 1; self.payload.clear(); return; }
        };
        self.payload.clear();
        if self.packets.len() < 64 { self.packets.push_back(Packet { vc, kind, payload }); self.packets_sent += 1; } else { self.refused += 1; }
    }
}
impl Device for Dsi {
    fn read(&mut self, off: u32) -> u32 {
        self.update();
        match off {
            0x70 => { let mut v = 0; for i in 0..4 { v |= (self.rx.pop_front().unwrap_or(0) as u32) << (i * 8); } v }
            0x74 => 1 | 1 << 16 | 1 << 18 | if self.payload.is_empty() { 4 } else { 0 } | if self.payload.len() >= FIFO_BYTES { 8 } else { 0 } | if self.rx.is_empty() { 16 } else { 0 },
            0xb0 => {
                if !self.locked() { return 0; }
                let ulps = self.ram.read(0xa8); let lanes = self.ram.read(0xa4) & 3;
                1 | if ulps & 1 == 0 { 1 << 2 | 1 << 3 } else { 0 } | if ulps & 4 == 0 { 1 << 4 | 1 << 5 | if lanes != 0 { 1 << 7 | 1 << 8 } else { 0 } } else { 0 }
            }
            0x814 => 1024, 0x854 | 0x844 => 0, 0x85c => self.ram.read(0x858) & self.ram.read(0x850),
            _ => self.ram.read(off),
        }
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        match off {
            0 | 0x74 | 0xb0 | 0x814 | 0x858 | 0x85c => {},
            0x70 => { if self.payload.len() + 4 <= FIFO_BYTES { self.payload.extend_from_slice(&v.to_le_bytes()); } else { self.refused += 1; } }
            0x6c => self.header(v),
            0x854 => self.ram.write(0x858, self.ram.read(0x858) & !v),
            0xb4 => {
                // Address on falling TESTCLK; data on rising TESTCLK (HAL sequence).
                let old = self.ram.read(off); self.ram.write(off, v);
                if v & 1 != 0 { self.test_address = None; self.pll_programmed = false; }
                if old & 2 != 0 && v & 2 == 0 {
                    let ctl = self.ram.read(0xb8);
                    if ctl & (1 << 16) != 0 { self.test_address = Some(ctl as u8); }
                }
                if old & 2 == 0 && v & 2 != 0 && self.ram.read(0xb8) & (1 << 16) == 0 {
                    if let Some(a) = self.test_address { self.phy_registers[a as usize] = self.test_data; if a == 0x18 && self.test_data & 0x80 != 0 { self.pll_programmed = true; } }
                }
            }
            0xb8 => { self.test_data = v as u8; self.ram.write(off, v); }
            0x80c | 0x810 | 0x868 => self.ram.write(off, v & !(1 << 31)),
            4 => { self.ram.write(off, v & 1); if v & 1 == 0 { self.payload.clear(); self.rx.clear(); self.packets.clear(); } }
            _ => self.ram.write(off, v),
        }
        self.update(); WriteEffect::NONE
    }
    fn irq_sources(&self) -> u64 { u64::from(self.ram.read(0x858) & self.ram.read(0x850) != 0) }
    fn clock(&self) -> Option<ClockDomain> { self.phy_enabled().then_some(ClockDomain::Cpu) }
    fn tick(&mut self, n: u64) { self.cycle = self.cycle.saturating_add(n); self.update(); }
    fn has_deadline(&self) -> bool { true }
    fn next_deadline(&self) -> Option<u64> { self.lock_at.filter(|&at| at > self.cycle).into_iter().chain(self.next_frame).map(|at| at.saturating_sub(self.cycle).max(1)).min() }
}

//! Shared 48-ID, 3-level-bit Espressif CLIC register arbitration, independent of bus address.
// The register field layout matches P4 IDF v5.5.3 and S31 IDF v6.1
// components/soc/<chip>/include/soc/clic_reg.h. Standard-CSR threshold placement is
// a CPU/SoC concern: callers set `thresh` in the same high-byte form before arbitration.
// No timing or silicon validation is implied by sharing this implementation.
pub const CLIC_IDS: usize = 48;
pub const CLIC_EXT_OFFSET: u32 = 16;

/// One core's CLIC register block. `CLIC_INT_CTRL(i)` packs four byte registers: IP (byte 0,
/// bit 0), IE (byte 1, bit 8), the attributes (byte 2: SHV bit 16, TRIG bits 18:17, MODE bits
/// 23:22, read-only machine mode) and CTL (byte 3; reset 0x1F); with NLBITS = 3 the level is
/// CTL[7:5], the unimplemented low bits reading as ones. IDF v5.5.3 include/soc/clic_reg.h.
/// A level-triggered ID's IP follows its matrix input. An edge-triggered ID's IP latches the
/// rising edge, clears when the core takes it hardware-vectored (CLIC specification) and when
/// software writes 1 to it without changing the rest of the word, which is how IDF acknowledges
/// one (riscv/include/esp_private/interrupt_clic.h:321-324, `rv_utils_intr_edge_ack`, "clearing
/// its pending flag" in esp_hw_support/include/hal/interrupt_controller_hal.h:226).
#[derive(Clone)]
pub struct ClicCore {
    /// CLIC_INT_CONFIG.MNLBITS (bits 4:1); NVBITS reads 1
    pub mnlbits: u32,
    /// CLIC_INT_THRESH: on rev < 3 the machine-mode threshold is this byte, bits 31:24
    /// (IDF v5.5.3 riscv/include/esp_private/interrupt_clic.h:50-58)
    pub thresh: u32,
    /// IE, the attributes and CTL of each ID; IP is kept apart
    pub ctrl: [u32; CLIC_IDS],
    /// edge-triggered IDs: IP latched by an edge
    pub ip_latch: u64,
    /// IDs whose interrupt-matrix input is asserted now
    pub input: u64,
}
impl Default for ClicCore { fn default() -> Self { ClicCore { mnlbits: 0, thresh: 0, ctrl: [0x1f << 24 | 3 << 22; CLIC_IDS], ip_latch: 0, input: 0 } } }
impl ClicCore {
    fn edge(&self, id: usize) -> bool { self.ctrl[id] & (1 << 17) != 0 }
    pub fn pending(&self, id: usize) -> bool { if self.edge(id) { self.ip_latch & (1 << id) != 0 } else { self.input & (1 << id) != 0 } }
    pub fn enabled(&self, id: usize) -> bool { self.ctrl[id] & (1 << 8) != 0 }
    /// The 8-bit level of an ID: CTL with the bits below NLBITS read as ones.
    pub fn level(&self, id: usize) -> u32 { (self.ctrl[id] >> 24) | 0x1f }
    pub fn vectored(&self, id: usize) -> bool { self.ctrl[id] & (1 << 16) != 0 }
    /// The highest-level enabled, pending ID above the threshold, the higher ID on a tie (the
    /// RISC-V CLIC specification's rule): what the core takes if MIE is set and its own level
    /// is lower.
    pub fn candidate(&self) -> Option<(usize, u32)> {
        let thresh = self.thresh >> 24;
        let mut best: Option<(usize, u32)> = None;
        for id in 0..CLIC_IDS {
            if !self.enabled(id) || !self.pending(id) { continue; }
            let l = self.level(id);
            if l > thresh && best.is_none_or(|(_, b)| l >= b) { best = Some((id, l)); }
        }
        best
    }
    /// A new matrix input: edge-triggered IDs latch the rising edges (TRIG bit 18 selects the
    /// falling edge, not used by IDF).
    pub fn set_input(&mut self, input: u64) {
        let rising = input & !self.input;
        for id in 0..CLIC_IDS { if rising & (1 << id) != 0 && self.edge(id) { self.ip_latch |= 1 << id; } }
        self.input = input;
    }
    /// The core took `id`: a hardware-vectored edge interrupt is no longer pending.
    pub fn taken(&mut self, id: usize) { if id < CLIC_IDS && self.edge(id) && self.vectored(id) { self.ip_latch &= !(1 << id); } }
    pub fn read(&self, off: u32) -> u32 {
        match off {
            0x0 => 1 | (self.mnlbits & 0x1e),                                                     // INT_CONFIG: NVBITS = 1
            0x4 => CLIC_IDS as u32 | 3 << 21,                                                     // INT_INFO: NUM_INT 48, CTLBITS 3
            0x8 => self.thresh,
            0x1000..=0x10bc => { let id = ((off - 0x1000) / 4) as usize; self.ctrl[id] | self.pending(id) as u32 }
            _ => 0,
        }
    }
    /// A write of the byte lanes in `lanes` (bit n = byte n): the ROM and IDF write the four byte
    /// registers of a control word one at a time.
    pub fn write(&mut self, off: u32, v: u32, lanes: u32) {
        let mask = (0..4).filter(|b| lanes & (1 << b) != 0).fold(0u32, |m, b| m | 0xff << (8 * b));
        match off {
            0x0 => self.mnlbits = (self.mnlbits & !mask) | (v & mask & 0x1e),
            0x8 => self.thresh = (self.thresh & !mask) | (v & mask & 0xff00_0000),
            0x1000..=0x10bc => {
                let id = ((off - 0x1000) / 4) as usize;
                let before = self.ctrl[id];
                self.ctrl[id] = (self.ctrl[id] & !mask) | (v & mask & 0xff07_0100) | 3 << 22;
                // IDF acknowledges an edge with a 32-bit REG_SET_BIT of IP (interrupt_clic.h:321-324)
                // and reconfigures SHV and TRIG with the same read-modify-write (interrupt_clic_ll.h:
                // 99-102, esp_rom/patches/esp_rom_clic.c), which writes a pending IP back as 1: only a
                // write that changes nothing else is the acknowledge.
                if lanes & 1 != 0 && v & 1 != 0 && self.edge(id) && self.ctrl[id] == before { self.ip_latch &= !(1 << id); }
            }
            _ => {}
        }
    }
}

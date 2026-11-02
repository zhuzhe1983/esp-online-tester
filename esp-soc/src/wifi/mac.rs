//! The 802.11 MAC of the ESP32-S3 and ESP32-C3, one model for both. The closed `libpp`/`libnet80211`
//! drive it; Espressif does not document it. The register layout comes from the blobs' own accesses
//! (docs/wifi-plan.md) and is the same on both chips: the 0x60033000 block and its two neighbours, the
//! same offsets, the same `hal_mac_*` helpers (IDF v5.5.4 `components/esp_wifi/lib/esp32s3/libpp.a`
//! and `.../esp32c3/libpp.a`). What differs per chip is outside this register model: the CPU clock the
//! TSF counts from (`WifiMac::new`), and the bus glue that moves frames between the air and the
//! chip's memory (`wifi_tx_step`, `wifi_rx_deliver`).
//!
//! The classic ESP32 (`esp32/src/wifi.rs`, 0x3ff73000, slots at 0xd20 - 8n, three TSF timers) and the
//! C6 (`esp32c6/src/wifi.rs`, shifted offsets, 14-bit descriptor fields, 92-byte `rx_ctrl`, no TSF)
//! keep their own copies. Merging them needs a per-chip register map and descriptor layout here, and
//! each brings the receive hand-over below with it (EX238: they still restart only from `rx_next == 0`).
use esp_periph::{Device, RegRam, WriteEffect, DMA_ADDR_BASE};

/// Transmit queues. Queue n has its PLCP0 register at 0xd08 - 8n (`hal_mac_txq_enable`:
/// `(0x0c0067a1 - n) << 3`, both blobs), and the per-queue state is 11 bits wide
/// (`hal_mac_get_txq_state(0)` masks 0xca8 to bits 10:0 on both chips, `hal_mac_clr_txq_state(0)`
/// writes 0xca4 bits 10:0). A twelfth slot would be 0xcb0, which is the completion state register
/// (`hal_mac_get_txq_state(2)`), and a thirteenth 0xca8, so eleven is where the PLCP slots end.
/// Derived from the blobs, not from a silicon datasheet.
pub const TXQ_COUNT: u32 = 11;

/// The MAC's registers: blocks 0x33 and 0x34 (`ram`) and the WDEV block 0x35 (`ram2`).
///   TX: queue n starts when software sets bits 31:30 of PLCP0 (0xd08 - 8n, low 20 bits = the descriptor
///       address); completion sets TXQ state type 2 (0xcb0) bit n and event bit 7.
///   RX: descriptor list head at 0x088 (`dma_list_item`: size:12 length:12 _:6 has_data:1 owner:1, buffer,
///       next), the descriptor the DMA fills next at 0x08c, the last one it filled at 0x090.
pub struct WifiMac {
    pub ram: RegRam,
    pub ram2: RegRam,
    pub log: bool,
    /// TSF: 1 MHz counter (offset applied to the CPU cycle clock), latched into WDEV 0x18/0x1c
    pub tsf_offset: i64,
    pub tsf_latched: u64,
    /// the CPU cycle clock, set by the chip when software touches the MAC
    pub now_cycles: u64,
    cycles_per_us: u64,
    /// interrupt events (0xc3c; cleared by writing 0xc40): bit 7 = TX complete, bits 14/24 = RX data (libpp wDev_ProcessFiq)
    pub events: u32,
    pub pwr_events: u32,
    /// TXQ state type 2, queues that completed (0xcb0 bits 3:0, cleared by writing 0xcac)
    pub txq_complete: u32,
    /// TXQ state types 0/1, per-queue errors (0xca8 bits 10:0, cleared via 0xca4)
    pub txq_error: u32,
    /// (queue, descriptor address) of each transmission software started, for the chip's bus to send
    pub tx_pending: Vec<(u8, u32)>,
    /// RX descriptor list: head written by the driver (0x088), the descriptor the hardware fills next, the last one filled
    pub rx_base: u32,
    pub rx_next: u32,
    pub rx_last: u32,
    /// `BASE_RX_DSCR` was written since the last frame went in: the driver may have handed over a
    /// fresh list. A stopped receive DMA restarts there (`rx_restart_at_base`); a running one keeps
    /// its pointer, because software rewrites the base on every recycle.
    pub rx_handover: bool,
    /// the access point, the network and the counts, shared with the other chips
    pub link: super::StationLink,
}

impl WifiMac {
    /// A MAC whose TSF counts from a CPU clock of `cpu_hz`.
    pub fn new(cpu_hz: u64) -> Self {
        WifiMac {
            ram: RegRam::new(), ram2: RegRam::new(), log: false,
            tsf_offset: 0, tsf_latched: 0, now_cycles: 0, cycles_per_us: cpu_hz / 1_000_000,
            events: 0, pwr_events: 0, txq_complete: 0, txq_error: 0, tx_pending: Vec::new(),
            rx_base: 0, rx_next: 0, rx_last: 0, rx_handover: false, link: Default::default(),
        }
    }
    pub fn irq(&self) -> bool { self.events != 0 || self.pwr_events != 0 }
    /// The transmit queue whose PLCP0 register is at `off`, if any.
    fn txq_of(off: u32) -> Option<u8> {
        if off <= 0xd08 && (0xd08 - off).is_multiple_of(8) && (0xd08 - off) / 8 < TXQ_COUNT { Some(((0xd08 - off) / 8) as u8) } else { None }
    }
    pub fn read(&mut self, block: u32, off: u32) -> u32 {
        let v = match (block, off) {
            (0x33, 0xd14) => self.ram.read(off) | 1,                 // hal_init: writes bit 1, waits for bit 0
            (0x33, 0xc3c) => self.events,
            (0x33, 0x088) => self.rx_base & 0xf_ffff, (0x33, 0x08c) => self.rx_next & 0xf_ffff, (0x33, 0x090) => self.rx_last,
            (0x33, 0xca8) => self.txq_error & 0x7ff,                 // txq state types 0/1 (errors/collisions)
            (0x33, 0xcb0) => self.txq_complete & 0xf,                // txq state type 2: completed queues
            (0x35, 0x118) => self.pwr_events,
            (0x35, 0x18) => self.tsf_latched as u32,
            (0x35, 0x1c) => (self.tsf_latched >> 32) as u32,
            (0x35, 0x128) => self.ram2.read(off),
            (0x33, _) => self.ram.read(off),
            (_, _) => self.ram2.read(off),
        };
        if self.log { eprintln!("[wifi] rd {:#x}+{:#05x} -> {:#010x}", block, off, v); }
        v
    }
    pub fn write(&mut self, block: u32, off: u32, v: u32) {
        if self.log { eprintln!("[wifi] wr {:#x}+{:#05x} <- {:#010x}", block, off, v); }
        match (block, off) {
            (0x33, 0xc40) => { self.events &= !v; }
            (0x33, 0x088) => {
                // BASE_RX_DSCR: the head of the receive list. Software rewrites it whenever it recycles
                // descriptors, so this must not rewind a running DMA — doing so re-delivers into descriptors
                // the stack has already moved past. It is also how software hands the DMA a fresh list, and
                // the bus takes it there only once the DMA has stopped, because the descriptor it wants is
                // not usable (Arduino-ESP32 arms four at a time and writes the base of the next list while
                // the last frame of the current one is still being processed).
                // Inferred from the S3 5.1 blob's register writes under emulation, not from a public header:
                // the latch is cleared by the next frame that goes in, so the rule assumes the driver
                // publishes the base after the current list is exhausted (see docs/decisions.md, EX238).
                self.rx_base = DMA_ADDR_BASE | (v & 0xf_ffff);
                self.rx_handover = true;
                if self.rx_next == 0 { self.rx_next = self.rx_base; }
                self.ram.write(off, v);
            }
            (0x33, 0x084) => {
                // DSCR_RELOAD: software has appended recycled descriptors and asks the hardware to
                // re-read the chain.
                // Measured against the blob: rewinding here makes every second frame land in a
                // descriptor the stack has moved past, and it is recycled instead of indicated. The
                // hardware keeps its own pointer; base only matters once the ring has run dry.
                if v & 1 != 0 && self.rx_next == 0 { self.rx_next = self.rx_base; }
                self.ram.write(off, v & !1);
            }
            (0x35, 0x11c) => { self.pwr_events &= !v; }
            (0x35, 0x0c) => {
                let now = (self.now_cycles / self.cycles_per_us) as i64;
                if v & 3 != 0 { self.tsf_latched = (now + self.tsf_offset) as u64; }                              // latch
                if v & (1 << 4) != 0 { let set = (self.ram2.read(0x10) as u64) | ((self.ram2.read(0x14) as u64) << 32); self.tsf_offset = set as i64 - now; }   // load
                self.ram2.write(off, v);
            }
            (0x33, 0xca4) => { self.txq_error &= !(v & 0x7ff); }
            (0x33, 0xcac) => { self.txq_complete &= !(v & 0xf); }
            (0x33, o) if Self::txq_of(o).is_some() => {                                   // MAC_TX_PLCP0[queue]
                self.ram.write(off, v);
                if v & (1 << 31) != 0 { let q = Self::txq_of(o).unwrap(); self.tx_pending.push((q, DMA_ADDR_BASE | (v & 0xf_ffff))); }
            }
            (0x33, _) => self.ram.write(off, v),
            (_, _) => self.ram2.write(off, v),
        }
    }
    /// Hardware finished sending the frame in `queue`.
    pub fn tx_done(&mut self, queue: u8) {
        self.txq_complete |= 1 << queue; self.events |= 1 << 7;
        let o = 0xd08 - 8 * queue as u32; let v = self.ram.read(o); self.ram.write(o, v & !(3 << 30));
        // result word (hal_mac_get_txq_pmd): bits 15:12 = status code, 0 = success (3 would trap the blob)
        let r = 0x320 - 76 * queue as u32; let w = self.ram2.read(r); self.ram2.write(r, w & !(0xf << 12));
    }
    /// The receive DMA found the descriptor at `rx_next` unusable, so it has stopped. If software wrote
    /// `BASE_RX_DSCR` since the last frame went in, that was the hand-over of a fresh list: the DMA takes
    /// it as its new head and the caller tries the frame once more (true). A driver that hands the
    /// hardware one short list at a time needs this (EX238); one long ring never gets here.
    pub fn rx_restart_at_base(&mut self) -> bool {
        if !self.rx_handover { return false; }
        self.rx_next = self.rx_base;
        self.rx_handover = false;
        true
    }
    /// A frame went into the descriptor at `desc`, whose `next` word is `next`: the DMA moves on and
    /// raises the RX event. The registers hold masked descriptor addresses; `rx_last` carries a 0x01
    /// prefix (S3 silicon).
    pub fn rx_filled(&mut self, desc: u32, next: u32) {
        self.rx_last = (desc & 0xf_ffff) | (1 << 24);
        self.rx_next = next & 0xf_ffff;
        self.rx_handover = false;
        self.link.rx_delivered(desc);
        self.events |= (1 << 14) | (1 << 24);   // RX data (wDev_ProcessFiq tests 0x1004000)
    }
}

/// The MAC spans blocks 0x33..0x35; the chip's table mounts it three times with `delta` = block index << 12.
impl Device for WifiMac {
    fn read(&mut self, off: u32) -> u32 { WifiMac::read(self, 0x33 + (off >> 12), off & 0xfff) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { WifiMac::write(self, 0x33 + (off >> 12), off & 0xfff, v); WriteEffect::NONE }
    fn irq_sources(&self) -> u64 { self.irq() as u64 }
    fn debug(&mut self, on: bool) { self.log = on; }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Eleven PLCP slots; the state registers that would be slots 11 and 12 stay state registers.
    #[test]
    fn plcp_slots_end_before_the_txq_state_registers() {
        let mut mac = WifiMac::new(160_000_000);
        for q in 0..TXQ_COUNT { mac.write(0x33, 0xd08 - 8 * q, (1 << 31) | 0x100); }
        assert_eq!(mac.tx_pending.iter().map(|&(q, _)| u32::from(q)).collect::<Vec<_>>(), (0..TXQ_COUNT).collect::<Vec<_>>());
        mac.tx_pending.clear();
        mac.write(0x33, 0xcb0, (1 << 31) | 0x100);
        mac.write(0x33, 0xca8, (1 << 31) | 0x100);
        assert!(mac.tx_pending.is_empty(), "0xcb0 and 0xca8 are TXQ state, not PLCP slots 11 and 12");
    }

    /// The latch: a base write arms one restart, a delivered frame or a restart consumes it.
    #[test]
    fn base_write_arms_one_restart() {
        let mut mac = WifiMac::new(240_000_000);
        assert!(!mac.rx_restart_at_base(), "no base written yet");
        mac.write(0x33, 0x088, 0x1_0000);
        assert_eq!(mac.rx_next, DMA_ADDR_BASE | 0x1_0000, "an empty pointer takes the first base");
        mac.rx_filled(DMA_ADDR_BASE | 0x1_0000, DMA_ADDR_BASE | 0x1_000c);
        assert_eq!((mac.rx_next, mac.rx_last), (0x1_000c, 0x101_0000));
        assert!(!mac.rx_restart_at_base(), "the frame that went in cleared the latch");
        mac.write(0x33, 0x088, 0x2_0000);
        assert_eq!(mac.rx_next, 0x1_000c, "a running DMA keeps its pointer");
        assert!(mac.rx_restart_at_base());
        assert_eq!(mac.rx_next, DMA_ADDR_BASE | 0x2_0000);
        assert!(!mac.rx_restart_at_base(), "one restart per base write");
    }
}

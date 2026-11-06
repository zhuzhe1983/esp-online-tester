//! C61 Wi-Fi MAC v3 and PHY handshakes, audited against ESP-IDF v5.5.4 libraries.
//! See docs/wifi-c61.md for fixed source revisions, disassembly functions and limits.
//! No RF/PHY accuracy is claimed: calibration has ideal zero measurements and
//! synchronous completion; frames pass through the shared protocol-level virtual AP.
use esp_periph::{Device, RegRam, WriteEffect};

/// C61 libphy phy_freq_chan_en_sw / phy_iq_est_enable_new / phy_txdc_cal.
/// These offsets differ from C6 even though both blocks start at 0x600a0000.
#[derive(Default)]
pub struct ModemBb { ram: RegRam, pub chan_index: u32, pub chan_switches: u32, chan_done: bool }
impl ModemBb { pub fn new() -> Self { Self::default() } }
impl Device for ModemBb {
    fn read(&mut self, off: u32) -> u32 {
        let value = self.ram.read(off);
        match off {
            0x028 => value & !(1 << 8) | u32::from(self.chan_done) << 8,
            0x418 => value & !(1 << 22) | u32::from(value & 1 != 0) << 22,
            0x47c => value & !(1 << 16) | u32::from(self.ram.read(0x450) & 3 == 3) << 16,
            // phy_pwdet_wait_idle polls this three-bit state for 7; no analogue
            // power-detector acquisition runs in the ideal digital model.
            0x80c => value & !(7 << 14) | 7 << 14,
            _ => value,
        }
    }
    fn write(&mut self, off: u32, value: u32) -> WriteEffect {
        if off == 0x01c && value & (1 << 17) != 0 && self.ram.read(off) & (1 << 17) == 0 {
            self.chan_index = value & 0x7f; self.chan_switches = self.chan_switches.wrapping_add(1); self.chan_done = true;
        }
        self.ram.write(off, value); WriteEffect::NONE
    }
}

const SRAM_BASE: u32 = 0x4080_0000;
const EVENT_TX: u32 = 1 << 7;
const EVENT_RX: u32 = 1 << 14;
const TXQ0: u32 = 0xd6c;
const TXQ_STRIDE: u32 = 16;
const TXQ_COUNT: usize = 11;
const RX_CTRL: usize = 64; // MAC_VERSION_NUM=3, get_sublen_offset: 56 + 8, no CSI/SIG-B dump

/// C61 libpp hal_* register interface, not an alias of another SoC's device.
/// Unknown registers retain written values; unimplemented radio features are not
/// reported as working. DMA always bounds-checks actual HP SRAM, never host pointers.
pub struct WifiMac {
    ram: RegRam,
    pub log: bool,
    events: u32,
    rx_base: u32, rx_next: u32, rx_last: u32,
    rx_handover: Option<u32>,
    tx_complete: u32,
    tx_pending: [Option<u32>; TXQ_COUNT],
    pub link: esp_soc::wifi::StationLink,
}
impl Default for WifiMac { fn default() -> Self { Self::new() } }
impl WifiMac {
    pub fn new() -> Self { Self { ram: RegRam::new(), log: false, events: 0, rx_base: 0, rx_next: 0, rx_last: 0,
        rx_handover: None, tx_complete: 0, tx_pending: [None; TXQ_COUNT], link: Default::default() } }
    fn addr(low: u32) -> u32 { SRAM_BASE | low & 0xf_ffff }
    fn tx_queue(off: u32) -> Option<usize> {
        (off <= TXQ0 && (TXQ0 - off).is_multiple_of(TXQ_STRIDE) && (TXQ0 - off) / TXQ_STRIDE < TXQ_COUNT as u32)
            .then(|| ((TXQ0 - off) / TXQ_STRIDE) as usize)
    }
    /// The only per-tick predicate; inactive Wi-Fi does no DMA/network work.
    pub fn has_pending_work(&self) -> bool { self.tx_pending.iter().any(Option::is_some) || self.link.ap().is_some() }
    /// Service bounded guest descriptors and virtual-air traffic at nominal guest microseconds.
    /// Returns whether the MAC's interrupt state may have changed.
    pub fn service(&mut self, sram: &mut [u8], now_us: u64) -> bool {
        let old_events = self.events;
        for queue in 0..TXQ_COUNT {
            let Some(low) = self.tx_pending[queue].take() else { continue };
            let desc = Self::addr(low);
            let word = read32(sram, desc);
            let packet_addr = read32(sram, desc.wrapping_add(4));
            let packet = region(sram, packet_addr, ((word >> 14) & 0x3fff) as usize).unwrap_or_default();
            let length = packet.get(..4).map_or(0, |v| (u32::from_le_bytes(v.try_into().unwrap()) & 0x3fff) as usize);
            // C61 dbg_read_tx_ppdu skips this prefix when ebuf flag 0x2000 is
            // set. This path handles prefixed, unaggregated packets only; the
            // live firmware must qualify which TX modes it actually selects.
            let frame = packet.get(8..8 + length).unwrap_or_default();
            if !frame.is_empty() {
                if self.log { eprintln!("[c61-wifi] TX queue {queue} desc {desc:#010x} {}", esp_soc::wifi::describe(frame)); }
                self.link.station_tx(frame, now_us);
            }
            self.tx_complete |= 1 << queue; self.events |= EVENT_TX;
            let off = TXQ0 - queue as u32 * TXQ_STRIDE;
            self.ram.write(off, self.ram.read(off) & !(3 << 30));
        }
        if self.link.ap().is_some() {
            if !self.link.nothing_due(now_us) {
                let last = self.link.last_rx_desc();
                let busy = last != 0 && read32(sram, last) & esp_soc::wifi::RX_DESC_HAS_DATA != 0;
                if let Some(frame) = self.link.next_rx(now_us, busy) { self.receive(sram, &frame, now_us); }
            }
            self.link.net_step(now_us);
        }
        self.events != old_events
    }
    fn receive(&mut self, sram: &mut [u8], frame: &[u8], now_us: u64) {
        let total = RX_CTRL + frame.len() + 4;
        if total > 0x3fff { self.link.drop_rx(); return; }
        let usable = |sram: &[u8], low| {
            let desc = Self::addr(low); let word = read32(sram, desc);
            low != 0 && word & (1 << 31) != 0 && word & (1 << 30) == 0 && (word & 0x3fff) as usize >= total
        };
        // A new ring head may be published while the last old descriptor still
        // holds a frame. Never rewind a usable ring; take the new head only once
        // the current descriptor cannot receive. This is a digital DMA policy,
        // not yet a C61 silicon-calibrated timing model.
        if !usable(sram, self.rx_next) {
            if let Some(head) = self.rx_handover.take() { self.rx_next = head; }
        }
        if self.rx_next == 0 { self.link.drop_rx_unarmed(); return; }
        if !usable(sram, self.rx_next) { self.link.drop_rx(); return; }
        let desc = Self::addr(self.rx_next); let word = read32(sram, desc);
        let buffer = read32(sram, desc.wrapping_add(4)); let next = read32(sram, desc.wrapping_add(8));
        let group = frame.get(4).is_some_and(|v| v & 1 != 0);
        let rssi = self.link.ap().map_or(-40, |ap| ap.rx_meta(frame).1);
        let mut bytes = vec![0u8; total];
        let flags = rssi as u8 as u32 | 1 << 28 | if group { 0 } else { 1 << 29 };
        bytes[0..4].copy_from_slice(&flags.to_le_bytes());
        bytes[11] = if group { 0x80 } else { 0 };
        bytes[12..16].copy_from_slice(&(now_us as u32).to_le_bytes());
        bytes[20] = (-90i8) as u8;
        // Firmware writes channel byte 28 itself. No CSI/SIG-B information is invented.
        bytes[56..60].copy_from_slice(&((frame.len() as u32 + 4) | (frame.len() as u32) << 16).to_le_bytes());
        bytes[RX_CTRL..RX_CTRL + frame.len()].copy_from_slice(frame);
        bytes[RX_CTRL + frame.len()..].copy_from_slice(&esp_soc::wifi::fcs(frame).to_le_bytes());
        if !store(sram, buffer, &bytes) { self.link.drop_rx(); return; }
        let filled = word & !(0x3fff << 14) | (total as u32) << 14 | 1 << 30;
        if !store(sram, desc, &filled.to_le_bytes()) { self.link.drop_rx(); return; }
        self.rx_last = desc & 0xf_ffff; self.rx_next = next & 0xf_ffff; self.rx_handover = None;
        self.link.rx_delivered(desc); self.events |= EVENT_RX;
        if self.log { eprintln!("[c61-wifi] RX desc {desc:#010x} {}", esp_soc::wifi::describe(frame)); }
    }
}
impl Device for WifiMac {
    fn read(&mut self, off: u32) -> u32 {
        let value = match off {
            0xddc => { let value = self.ram.read(off); value & !1 | u32::from(value & 2 != 0) }
            0xc48 => self.events & self.ram.read(0xc40),
            0xc4c | 0xcb4 => 0,
            0x084 => self.rx_base, 0x088 => self.rx_next, 0x08c => self.rx_last,
            0xc70 => if self.link.last_rx_desc() != 0 { self.link.last_rx_desc() } else { SRAM_BASE },
            0xcb8 => self.tx_complete,
            _ => self.ram.read(off),
        };
        if self.log { eprintln!("[c61-wifi] rd +{off:#05x} -> {value:#010x}"); } value
    }
    fn write(&mut self, off: u32, value: u32) -> WriteEffect {
        if self.log { eprintln!("[c61-wifi] wr +{off:#05x} <- {value:#010x}"); }
        match off {
            0xc4c => self.events &= !value,
            0x084 => { self.rx_base = value & 0xf_ffff; self.rx_handover = Some(self.rx_base); if self.rx_next == 0 { self.rx_next = self.rx_base; } self.ram.write(off, value); }
            0x080 => { if value & 1 != 0 && self.rx_next == 0 { self.rx_next = self.rx_base; } self.ram.write(off, value & !1); }
            0xcb4 => self.tx_complete &= !value,
            _ => { self.ram.write(off, value); if let Some(queue) = Self::tx_queue(off) { if value & (1 << 31) != 0 { self.tx_pending[queue] = Some(value & 0xf_ffff); } } }
        }
        WriteEffect::NONE
    }
    fn irq_sources(&self) -> u64 { u64::from(self.events & self.ram.read(0xc40) != 0) }
    fn debug(&mut self, enabled: bool) { self.log = enabled; }
}
fn region(bytes: &[u8], addr: u32, len: usize) -> Option<&[u8]> { let start = addr.checked_sub(SRAM_BASE)? as usize; bytes.get(start..start.checked_add(len)?) }
fn read32(bytes: &[u8], addr: u32) -> u32 { region(bytes, addr, 4).map_or(0, |v| u32::from_le_bytes(v.try_into().unwrap())) }
fn store(bytes: &mut [u8], addr: u32, input: &[u8]) -> bool {
    let Some(start) = addr.checked_sub(SRAM_BASE).map(|v| v as usize) else { return false };
    let Some(target) = start.checked_add(input.len()).and_then(|end| bytes.get_mut(start..end)) else { return false };
    target.copy_from_slice(input); true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn phy_c61_starts_only_its_own_handshakes() {
        let mut bb = ModemBb::new(); assert_eq!(bb.read(0x028) & 256, 0);
        bb.write(0xc0, 1 << 14); assert_eq!(bb.read(0x028) & 256, 0); // C6 command must not complete C61
        bb.write(0x1c, 31 | 1 << 17); assert_eq!(bb.read(0x028) & 256, 256); assert_eq!(bb.chan_index, 31);
        bb.write(0x450, 2); assert_eq!(bb.read(0x47c) & (1 << 16), 0);
        bb.write(0x450, 3); assert_ne!(bb.read(0x47c) & (1 << 16), 0);
        bb.write(0x450, 0); assert_eq!(bb.read(0x47c) & (1 << 16), 0);
        bb.write(0x418, 1); assert_ne!(bb.read(0x418) & (1 << 22), 0);
        bb.write(0x418, 0); assert_eq!(bb.read(0x418) & (1 << 22), 0);
    }
    #[test] fn mac_dma_c61_rx_header_and_consumption() {
        let mut mac = WifiMac::new(); let mut ram = vec![0; 4096];
        store(&mut ram, SRAM_BASE + 0x100, &(1u32 << 31 | 1700).to_le_bytes());
        store(&mut ram, SRAM_BASE + 0x104, &(SRAM_BASE + 0x400).to_le_bytes());
        mac.write(0xc40, EVENT_RX | EVENT_TX); mac.write(0x84, 0x100); let frame = [0x80,0,0,0,0xff,0xff,0xff,0xff,0xff,0xff,0,0,0,0,0,0,0,0,0,0,0,0,0,0];
        mac.receive(&mut ram, &frame, 1234);
        assert_eq!(read32(&ram, SRAM_BASE + 0x400) & (3 << 28), 1 << 28);
        assert_eq!(read32(&ram, SRAM_BASE + 0x40c), 1234);
        assert_eq!(read32(&ram, SRAM_BASE + 0x438), 28 | 24 << 16);
        assert_eq!(&ram[0x440..0x458], frame);
        assert_eq!(read32(&ram, SRAM_BASE + 0x100), 1 << 31 | 1 << 30 | 92 << 14 | 1700);
        assert_eq!(mac.read(0xc48), EVENT_RX); assert_eq!(mac.read(0x8c), 0x100); assert_eq!(mac.read(0x88), 0);
        mac.write(0xc4c, EVENT_TX); assert_ne!(mac.irq_sources(), 0);
        mac.write(0xc4c, EVENT_RX); assert_eq!(mac.irq_sources(), 0);
    }
    #[test] fn malformed_guest_dma_is_bounded() {
        let mut mac = WifiMac::new(); let mut ram = vec![0; 256]; let unchanged = ram.clone();
        for low in [0,0xfffff,0x100,0x80] { mac.write(0x84, low); mac.receive(&mut ram, &[0; 24], 1); }
        assert_eq!(ram, unchanged);
        for queue in 0..TXQ_COUNT { mac.write(TXQ0 - queue as u32 * TXQ_STRIDE, 0xc00f_ffff); }
        assert!(mac.has_pending_work()); assert!(mac.service(&mut ram, 0)); assert_eq!(mac.read(0xcb8), 0x7ff);
        assert!(!mac.has_pending_work()); mac.write(0xcb4, 0x7ff); assert_eq!(mac.read(0xcb8), 0);
    }
    #[test] fn dma_roundtrip_probe_auth_association_and_ethernet_relay() {
        use esp_soc::wifi::{ApConfig, StaState};
        let mut mac = WifiMac::new(); let mut ram = vec![0; 8192];
        let cfg = ApConfig::parse("ssid=c61-test").unwrap(); let bssid = cfg.bssid;
        let sta = [2, 0xca, 0x61, 0, 0, 1];
        mac.link.attach_new(cfg, None, &Default::default()); mac.link.set_relay(true);
        mac.write(0xc40, EVENT_RX | EVENT_TX);
        // Guest-visible descriptors and frame bytes drive the MAC. Nothing
        // changes the AP state or calls its management handlers from the test.
        fn tx(mac: &mut WifiMac, ram: &mut [u8], frame: &[u8], now: u64) {
            store(ram, SRAM_BASE + 0x200, &(((frame.len() as u32 + 8) << 14) | 1 << 31).to_le_bytes());
            store(ram, SRAM_BASE + 0x204, &(SRAM_BASE + 0x1000).to_le_bytes());
            store(ram, SRAM_BASE + 0x1000, &(frame.len() as u32).to_le_bytes());
            store(ram, SRAM_BASE + 0x1008, frame);
            mac.write(TXQ0, 0xc000_0200); mac.service(ram, now);
            assert_eq!(mac.read(0xcb8), 1); mac.write(0xcb4, 1); mac.write(0xc4c, EVENT_TX);
        }
        fn rx(mac: &mut WifiMac, ram: &mut [u8], now: u64) -> Vec<u8> {
            store(ram, SRAM_BASE + 0x100, &(1u32 << 31 | 1700).to_le_bytes());
            store(ram, SRAM_BASE + 0x104, &(SRAM_BASE + 0x400).to_le_bytes());
            mac.write(0x84, 0x100); mac.service(ram, now);
            assert_ne!(mac.read(0xc48) & EVENT_RX, 0); mac.write(0xc4c, EVENT_RX);
            let length = (read32(ram, SRAM_BASE + 0x438) & 0x3fff) as usize - 4;
            let frame = ram[0x440..0x440 + length].to_vec();
            assert_eq!(read32(ram, SRAM_BASE + 0x440 + length as u32), esp_soc::wifi::fcs(&frame));
            store(ram, SRAM_BASE + 0x100, &0u32.to_le_bytes()); frame
        }
        let header = |fc: u16, dst: [u8; 6], a3: [u8; 6]| {
            let mut frame = fc.to_le_bytes().to_vec(); frame.extend_from_slice(&[0, 0]);
            frame.extend_from_slice(&dst); frame.extend_from_slice(&sta); frame.extend_from_slice(&a3);
            frame.extend_from_slice(&[0, 0]); frame
        };
        let mut probe = header(0x40, [0xff; 6], [0xff; 6]); probe.extend_from_slice(b"\x00\x08c61-test\x01\x01\x82");
        tx(&mut mac, &mut ram, &probe, 1000); let response = rx(&mut mac, &mut ram, 2600);
        assert_eq!(response[0], 0x50); assert_eq!(&response[4..10], sta);
        assert!(response.windows(8).any(|s| s == b"c61-test"));
        let mut auth = header(0xb0, bssid, bssid); auth.extend_from_slice(&[0, 0, 1, 0, 0, 0]);
        tx(&mut mac, &mut ram, &auth, 3000); let response = rx(&mut mac, &mut ram, 3400);
        assert_eq!(response[0], 0xb0); assert_eq!(&response[24..30], &[0, 0, 2, 0, 0, 0]);
        assert_eq!(mac.link.ap().unwrap().state, StaState::Authenticated);
        let mut assoc = header(0, bssid, bssid); assoc.extend_from_slice(b"\x01\x04\x01\x00\x00\x08c61-test\x01\x01\x82");
        tx(&mut mac, &mut ram, &assoc, 4000); let response = rx(&mut mac, &mut ram, 4400);
        assert_eq!(response[0], 0x10); assert_eq!(&response[26..28], &[0, 0]);
        assert_eq!(mac.link.ap().unwrap().state, StaState::Associated);
        let peer = [2, 3, 4, 5, 6, 7]; let mut data = header(0x0108, bssid, peer);
        data.extend_from_slice(b"\xaa\xaa\x03\x00\x00\x00\x08\x00c61-data");
        tx(&mut mac, &mut ram, &data, 5000);
        let eth = mac.link.take_relay_frames(); assert_eq!(eth.len(), 1);
        assert_eq!(&eth[0][..6], peer); assert_eq!(&eth[0][6..12], sta); assert_eq!(&eth[0][12..], b"\x08\x00c61-data");
        let mut incoming = Vec::from(sta); incoming.extend_from_slice(&peer); incoming.extend_from_slice(b"\x08\x00reply-data");
        mac.link.receive_relay_frame(&incoming).unwrap(); let response = rx(&mut mac, &mut ram, 5600);
        assert_eq!(&response[4..10], sta); assert_eq!(&response[16..22], peer);
        assert!(response.ends_with(b"reply-data")); assert_eq!(mac.link.tx_frames(), 4); assert_eq!(mac.link.rx_frames(), 4);
    }
}

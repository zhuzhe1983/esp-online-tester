//! Classic ESP32 Wi-Fi MAC adapter for the shared virtual access point and network.
//! MAC and PHY meanings are inferred from Arduino-ESP32 3.3.8 libpp/libphy;
//! no public MAC register header or hardware timing validation is claimed.

use crate::bus::{SocBus, dma_range};
#[cfg(test)]
use crate::bus::{DRAM_HIGH, ROM_DRAM_LOW};
use crate::periph::CPU_HZ;
use esp_periph::{Device, RegRam, WriteEffect};
use esp_soc::wifi;
#[cfg(test)]
use esp_soc::wifi::VirtualAp;

const DMA_BASE: u32 = 0x3ff0_0000;
const TXQ0: u32 = 0xd20;
const EVENT_TX: u32 = 1 << 7;

// A received frame raises cause bits 24, 5 and 2 together (inferred, not checked on hardware):
// the esp32-open-mac QEMU model sets 0x1000024 on RX (esp32-open-mac/qemu esp-develop 4a5c772a,
// hw/xtensa/esp32_wifi.c:481); the Espressif blob takes RX on bit 24; esp-wifi-hal 0.3.1
// src/ll.rs:182 is `rx => [0x100020, @chip(esp32) 0x4]`, so on the ESP32 it tests 0x100024
// (bits 20, 5, 2).
const EVENT_RX: u32 = (1 << 24) | (1 << 5) | (1 << 2);

#[derive(Default)]
pub struct WifiMac {
    ram: [RegRam; 3],
    pub log: bool,
    pub now_cycles: u64,
    tsf_offset: [u64; 3],
    tsf_latched: [u64; 3],
    pub events: u32,
    pub txq_complete: u32,
    pub tx_pending: Vec<(u8, u32)>,
    pub rx_base: u32,
    pub rx_next: u32,
    pub rx_last: u32,
    pub link: wifi::StationLink,
    pub(crate) clock_enabled: bool,
    #[cfg(test)]
    pub(crate) step_calls: u64,
}

impl WifiMac {

    /// DPORT_WIFIMAC_RST clears the MAC registers only. The access point, network, relay queues
    /// and frame counts belong to the host side. S3/C3/C6 leave both MAC registers
    /// and StationLink alone on MAC reset; register clearing here is classic-only.
    /// A guest reset applies `StationLink::surviving_reboot` instead.
    pub fn reset(&mut self) {
        let link = std::mem::take(&mut self.link);
        let log = self.log;
        *self = Self::default();
        self.link = link;
        self.log = log;
    }

    fn read_reg(&self, off: u32) -> u32 {
        self.ram[(off >> 12) as usize].read(off)
    }
    fn write_reg(&mut self, off: u32, value: u32) {
        self.ram[(off >> 12) as usize].write(off, value);
    }
    fn txq_of(off: u32) -> Option<u8> {
        (off <= TXQ0 && (TXQ0 - off).is_multiple_of(8) && (TXQ0 - off) / 8 < 5)
            .then(|| ((TXQ0 - off) / 8) as u8)
    }
    pub fn tx_done(&mut self, queue: u8) {
        self.txq_complete |= 1 << queue;
        self.events |= EVENT_TX;
        let off = TXQ0 - 8 * queue as u32;
        self.write_reg(off, self.read_reg(off) & !(3 << 30));
        self.write_reg(0x1270 - 60 * queue as u32, 0);
    }

    fn rx_filled(&mut self, desc: u32, next: u32) {
        self.rx_last = desc;
        self.rx_next = next;
        self.link.rx_delivered(desc);
        self.events |= EVENT_RX;
    }
}

impl Device for WifiMac {
    fn read(&mut self, off: u32) -> u32 {
        let v = match off {
            0xd24 => self.read_reg(off) | 1, // hal_init ready
            0xc00 => (self.now_cycles / (CPU_HZ / 1_000_000)) as u32,
            0xc48 => self.events,
            0x088 => DMA_BASE | (self.rx_base & 0xf_ffff),
            0x08c => DMA_BASE | (self.rx_next & 0xf_ffff),
            0x090 => DMA_BASE | (self.rx_last & 0xf_ffff),
            0xcc0 | 0xcbc | 0xcc4 => 0,
            0xcc8 => self.txq_complete,
            0x2014 => self.tsf_latched[0] as u32,
            0x2018 => (self.tsf_latched[0] >> 32) as u32,
            0x2054 => self.tsf_latched[1] as u32,
            0x2058 => (self.tsf_latched[1] >> 32) as u32,
            0x2090 => self.tsf_latched[2] as u32,
            0x2094 => (self.tsf_latched[2] >> 32) as u32,
            _ => self.read_reg(off),
        };
        if self.log {
            eprintln!("[wifi] rd {:#010x} -> {:#010x}", 0x3ff7_3000 + off, v);
        }
        v
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        if self.log {
            eprintln!("[wifi] wr {:#010x} <- {:#010x}", 0x3ff7_3000 + off, v);
        }
        match off {
            0xc4c => self.events &= !v,
            0xcc4 => self.txq_complete &= !v,
            0x088 => {
                self.rx_base = if v & 0xf_ffff == 0 {
                    0
                } else {
                    DMA_BASE | (v & 0xf_ffff)
                };
                if self.rx_next == 0 {
                    self.rx_next = self.rx_base;
                }
            }
            0x084 => {
                if v & 1 != 0 && self.rx_next == 0 {
                    self.rx_next = self.rx_base;
                }
                self.write_reg(off, v & !1);
            }
            0x2010 => {
                let now = self.now_cycles / (CPU_HZ / 1_000_000);
                for n in 0..3 {
                    if v & (1 << n) != 0 {
                        self.tsf_latched[n] = now.wrapping_add(self.tsf_offset[n]);
                    }
                }
                for (n, lo) in [(0, 0x2024), (1, 0x2060), (2, 0x209c)] {
                    if v & (1 << (6 + n)) != 0 {
                        let set = self.read_reg(lo) as u64 | ((self.read_reg(lo + 4) as u64) << 32);
                        self.tsf_offset[n] = set.wrapping_sub(now);
                    }
                }
                self.write_reg(off, v & !0x1c0);
            }
            o if Self::txq_of(o).is_some() => {
                self.write_reg(off, v);
                if v & (1 << 31) != 0 {
                    self.tx_pending
                        .push((Self::txq_of(o).unwrap(), DMA_BASE | (v & 0xf_ffff)));
                }
            }
            _ => self.write_reg(off, v),
        }
        WriteEffect::NONE
    }
    fn irq_sources(&self) -> u64 {
        (self.events & self.read_reg(0xc40) != 0) as u64
    }
    fn debug(&mut self, on: bool) {
        self.log = on;
    }
}

impl SocBus {
    #[inline]
    pub(crate) fn wifi_step(&mut self) {
        #[cfg(test)] { self.periph.wifi.step_calls += 1; }
        // IDF v5.5.4 register/soc/dport_reg.h:1040-1043 (DPORT_WIFI_CLK_WIFI_EN) and
        // 1064,1069 (DPORT_CORE_RST_EN_REG, DPORT_WIFIMAC_RST).
        if !self.periph.wifi.clock_enabled {
            return;
        }
        self.periph.wifi.now_cycles = self.cycles;
        let now_us = self.cycles / (CPU_HZ / 1_000_000);
        let trace = self.periph.wifi.log || self.debug.has("wifi-frames");
        for (queue, desc) in std::mem::take(&mut self.periph.wifi.tx_pending) {
            if dma_range(desc, 12).is_none() {
                continue;
            }
            let Ok((_, d)) = self.read_dma_descriptor(desc) else { continue };
            // libpp's general TX convention: length is the TX byte count, not size.
            // ESP-IDF v5.5.2 soc/include/soc/lldesc.h:32: "Size (or length for TX)".
            // Keep ownership and bound the entire length against mapped DRAM below.
            if !d.owner_dma { continue; }
            let Some(range) = dma_range(d.buf, d.length as usize) else {
                continue;
            };
            let frame = &self.dram[range];
            if trace {
                eprintln!(
                    "[wifi] TX queue {} desc {:#010x} {}",
                    queue,
                    desc,
                    wifi::describe(frame)
                );
            }
            self.periph.wifi.tx_done(queue);
            self.irq_dirty = true;
            self.periph.wifi.link.station_tx(frame, now_us);
        }
        if self.periph.wifi.read_reg(0x084) & (1 << 31) != 0
            && !self.periph.wifi.link.nothing_due(now_us)
        {
            let d = self.periph.wifi.link.last_rx_desc();
            let busy = d != 0 && self.dma_read_word(d).unwrap_or(0) & wifi::RX_DESC_HAS_DATA != 0;
            if let Some(frame) = self.periph.wifi.link.next_rx(now_us, busy) {
                self.wifi_rx_deliver(&frame, now_us, trace);
            }
        }
        self.periph.wifi.link.net_step(now_us);
    }
    fn wifi_rx_deliver(&mut self, frame: &[u8], now_us: u64, trace: bool) {
        if self.wifi_rx_fill(frame, now_us, trace).is_none() { self.periph.wifi.link.drop_rx(); }
    }
    fn wifi_rx_fill(&mut self, frame: &[u8], now_us: u64, trace: bool) -> Option<()> {
        let desc = self.periph.wifi.rx_next;
        dma_range(desc, 12)?;
        let (dw0, d) = self.read_dma_descriptor(desc).ok()?;
        let total = 28 + frame.len() + 4;
        if !d.owner_dma || d.eof || d.buf == 0 || d.size < total as u32 { return None; }
        let range = dma_range(d.buf, total)?;
        let (chan, rssi) = { let (c, r) = self.periph.wifi.link.ap().unwrap().rx_meta(frame); (c as u32, i32::from(r)) };
        let broadcast = frame.get(4).is_some_and(|v| v & 1 != 0);
        // wDev_ProcessRxSucData subtracts the classic 96 dB RSSI offset.
        let w0 = ((rssi + 96) as u32 & 0xff) | (1 << 28) | if broadcast { 0 } else { 1 << 29 };
        // Classic rx_ctrl is 28 bytes: noise at +8, channel at +10, sig_len/state at +24.
        let mut bytes = Vec::with_capacity(total);
        for w in [
            w0,
            0,
            0xa6 | (chan << 16),
            now_us as u32,
            0,
            0,
            (frame.len() + 4) as u32,
        ] {
            bytes.extend_from_slice(&w.to_le_bytes());
        }
        bytes.extend_from_slice(frame);
        bytes.extend_from_slice(&wifi::fcs(frame).to_le_bytes());
        self.dram[range].copy_from_slice(&bytes);
        self.dma_write_word(
            desc,
            (dw0 & !(0xfff << 12)) | ((total as u32) << 12) | (3 << 30),
        );
        self.periph.wifi.rx_filled(desc, d.next);
        self.irq_dirty = true;
        if trace {
            eprintln!("[wifi] RX desc {:#010x} {}", desc, wifi::describe(frame));
        }
        Some(())
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classic_wifi_dma_reach_descriptors_and_payloads() {
        for address in [crate::bus::DRAM_LOW, DRAM_HIGH - 12, ROM_DRAM_LOW,
            crate::bus::DRAM_LOW - 12, DRAM_HIGH - 11, 0x4008_0000, 0x3ff8_0000, 0x5000_0000, u32::MAX] {
            let allowed = address == crate::bus::DRAM_LOW || address == DRAM_HIGH - 12;
            assert_eq!(dma_range(address, 12).is_some(), allowed, "descriptor {address:#x}");
            let mut b = SocBus::new(0, [0; 6]);
            assert_eq!(b.read_dma_descriptor(address).is_ok(), allowed, "read {address:#x}");
        }
        assert!(dma_range(DRAM_HIGH - 1, 1).is_some());
        assert!(dma_range(DRAM_HIGH - 1, 2).is_none());
        assert!(dma_range(ROM_DRAM_LOW, 1).is_none());
        assert!(dma_range(crate::bus::DRAM_LOW, usize::MAX).is_none());
    }

    #[test]
    fn classic_wifi_dma_reach_tx_payload_engine() {
        use xtensa_lx7::bus::Bus;
        let frame = [0u8; 24];
        for (buffer, valid) in [
            (ROM_DRAM_LOW, false),
            (crate::bus::DRAM_LOW, true),
            (DRAM_HIGH - frame.len() as u32, true),
            (DRAM_HIGH - frame.len() as u32 + 1, false),
        ] {
            let mut b = SocBus::new(0, [0; 6]);
            b.periph.wifi.link.attach(
                Some(VirtualAp::new(wifi::ApConfig::parse("").unwrap(), false)),
                Some(esp_soc::net::VirtualNet::new(false)),
            );
            b.periph.write32(0x3ff0_00cc, 0x406);
            let desc = 0x3ffb_0000;
            let control = (1 << 31) | (24 << 12) | 24;
            b.write32(desc, control).unwrap();
            b.write32(desc + 4, buffer).unwrap();
            b.write32(desc + 8, 0).unwrap();
            // Seed via the CPU backing, not the DMA helper under test. The
            // crossing case seeds only its in-range bytes.
            let off = (buffer - ROM_DRAM_LOW) as usize;
            let count = frame.len().min(b.dram.len() - off);
            b.dram[off..off + count].copy_from_slice(&frame[..count]);
            let before = b.dram.clone();
            let queue_control = 0xc000_0000 | (desc & 0xf_ffff);
            b.periph.wifi.write(TXQ0, queue_control);
            b.periph.wifi.write(0xc40, EVENT_TX);
            b.irq_dirty = false;
            b.wifi_step();
            assert_eq!(b.periph.wifi.link.tx_frames(), u64::from(valid), "{buffer:#x}");
            assert_eq!(b.periph.wifi.events, if valid { EVENT_TX } else { 0 }, "{buffer:#x}");
            assert_eq!(b.periph.wifi.txq_complete, u32::from(valid), "{buffer:#x}");
            assert_eq!(b.periph.wifi.read(TXQ0), if valid { queue_control & !(3 << 30) } else { queue_control });
            assert_eq!(b.periph.wifi.irq_sources(), u64::from(valid));
            assert_eq!(b.irq_dirty, valid);
            assert_eq!(b.dram, before, "TX must not write payload or descriptor at {buffer:#x}");
        }
    }

    #[test]
    fn classic_wifi_dma_reach_rx_payload_engine() {
        use xtensa_lx7::bus::Bus;
        let mut frame = [0u8; 24];
        frame[0] = 0x80;
        frame[4..10].fill(0xff);
        let total = 28 + frame.len() + 4;
        for (buffer, valid) in [
            (ROM_DRAM_LOW, false),
            (crate::bus::DRAM_LOW, true),
            (DRAM_HIGH - total as u32, true),
            (DRAM_HIGH - total as u32 + 1, false),
        ] {
            let mut b = SocBus::new(0, [0; 6]);
            b.periph.wifi.link.attach(
                Some(VirtualAp::new(wifi::ApConfig::parse("").unwrap(), false)),
                Some(esp_soc::net::VirtualNet::new(false)),
            );
            b.dram.fill(0xa5);
            let desc = 0x3ffb_0000;
            let next = desc + 12;
            let control = (1 << 31) | total as u32;
            b.write32(desc, control).unwrap();
            b.write32(desc + 4, buffer).unwrap();
            b.write32(desc + 8, next).unwrap();
            b.periph.wifi.write(0x088, desc);
            b.periph.wifi.write(0x084, 1 << 31);
            b.periph.wifi.write(0xc40, EVENT_RX);
            b.irq_dirty = false;
            let mut expected = b.dram.clone();
            // Build the expected classic RX header/frame/FCS independently of
            // dma_range; invalid transfers must leave the entire backing alone.
            if valid {
                let mut bytes = Vec::new();
                for word in [0x1000_0038u32, 0, 0xa6 | (6 << 16), 1234, 0, 0, 28] {
                    bytes.extend_from_slice(&word.to_le_bytes());
                }
                bytes.extend_from_slice(&frame);
                bytes.extend_from_slice(&wifi::fcs(&frame).to_le_bytes());
                let off = (buffer - ROM_DRAM_LOW) as usize;
                expected[off..off + total].copy_from_slice(&bytes);
                let off = (desc - ROM_DRAM_LOW) as usize;
                expected[off..off + 4].copy_from_slice(&(control | ((total as u32) << 12) | (1 << 30)).to_le_bytes());
            }
            assert_eq!(b.wifi_rx_fill(&frame, 1234, false).is_some(), valid, "{buffer:#x}");
            assert!(b.dram == expected, "RX payload/descriptor writes at {buffer:#x}");
            assert_eq!(b.periph.wifi.rx_base, desc);
            assert_eq!(b.periph.wifi.rx_next, if valid { next } else { desc });
            assert_eq!(b.periph.wifi.rx_last, if valid { desc } else { 0 });
            assert_eq!(b.periph.wifi.read(0x084), 1 << 31);
            assert_eq!(b.periph.wifi.events, if valid { EVENT_RX } else { 0 });
            assert_eq!(b.periph.wifi.link.last_rx_desc(), if valid { desc } else { 0 });
            assert_eq!(b.periph.wifi.link.rx_frames(), u64::from(valid));
            assert_eq!(b.periph.wifi.link.rx_dropped(), 0);
            assert_eq!(b.periph.wifi.irq_sources(), u64::from(valid));
            assert_eq!(b.irq_dirty, valid);
        }
    }

    /// A frame from a network around reaches the classic RX descriptor with that network's channel
    /// and RSSI (`VirtualAp::rx_meta`): the RSSI plus the classic 96 dB offset in word 0's low byte,
    /// the channel at +10 (word 2 bits 16-23, next to the -90 dBm noise floor); a frame from the
    /// access point keeps its channel and -40 dBm (56).
    #[test]
    fn classic_rx_ctrl_carries_a_nearby_networks_channel_and_rssi() {
        use xtensa_lx7::bus::Bus;
        let home = [0x02, 1, 2, 3, 4, 5];
        for (from, chan, rssi) in [(home, 1u32, -60i32), ([2, 0x53, 0x49, 0x4d, 0, 1], 6, -40)] {
            let mut b = SocBus::new(0, [0; 6]);
            b.periph.wifi.link.attach(Some(VirtualAp::new(wifi::ApConfig::parse("ssid=demo,chan=6,bssid=02:53:49:4d:00:01").unwrap(), false)), None);
            let left_out = b.periph.wifi.link.set_beacons(vec![wifi::BeaconAp { ssid: "Home".into(), bssid: home, channel: 1, rssi: -60, secure: true }]);
            assert_eq!(left_out, Some(vec![]));
            let mut frame = [0u8; 24];
            frame[0] = 0x80;                                                     // beacon
            frame[4..10].fill(0xff);
            frame[10..16].copy_from_slice(&from);
            let (desc, buf) = (0x3ffb_0000, 0x3ffb_1000);
            b.write32(desc, (1 << 31) | 0xfff).unwrap();                         // owner DMA, size 4095
            b.write32(desc + 4, buf).unwrap();
            b.write32(desc + 8, desc + 12).unwrap();
            b.periph.wifi.write(0x088, desc);
            b.periph.wifi.write(0x084, 1 << 31);
            assert!(b.wifi_rx_fill(&frame, 1234, false).is_some());
            assert_eq!(b.read32(buf).unwrap() & 0xff, (rssi + 96) as u32, "RSSI of {from:02x?}");
            assert_eq!(b.read32(buf + 8).unwrap(), 0xa6 | (chan << 16), "channel of {from:02x?}");
        }
    }

    #[test]
    fn authentication_tx_reads_full_length_not_buffer_capacity() {
        // A complete open-system authentication request is 30 bytes. Truncating
        // to the libpp descriptor's size=28 silently prevents an AP response.
        for size in [28, 30, 32] {
            let mut b = SocBus::new(0, [0; 6]);
            let cfg = wifi::ApConfig::parse("").unwrap();
            let sta = [2, 0, 0, 0, 0, 1];
            let mut frame = [0u8; 32];
            frame[0] = 0xb0;
            frame[4..10].copy_from_slice(&cfg.bssid);
            frame[10..16].copy_from_slice(&sta);
            frame[16..22].copy_from_slice(&cfg.bssid);
            frame[26] = 1;
            frame[30..].fill(0xa5); // Beyond length: must not be transmitted.
            b.periph.wifi.link.attach(Some(VirtualAp::new(cfg, false)), None);
            b.periph.write32(0x3ff0_00cc, 0x406);
            let desc = 0x3ffb_0000;
            let buf = DRAM_HIGH - 32;
            for (i, word) in frame.as_chunks::<4>().0.iter().enumerate() {
                b.dma_write_word(buf + 4 * i as u32, u32::from_le_bytes(*word));
            }
            let control = (1 << 31) | (30 << 12) | size;
            b.dma_write_word(desc, control);
            b.dma_write_word(desc + 4, buf);
            b.periph.wifi.write(TXQ0, 0xc00b_0000);
            b.wifi_step();
            assert_eq!(b.periph.wifi.events, EVENT_TX, "size={size}");
            assert_eq!(b.periph.wifi.txq_complete, 1);
            assert_eq!(b.periph.wifi.read(TXQ0) >> 30, 0);
            assert!(b.irq_dirty);
            assert_eq!(b.periph.wifi.link.tx_frames(), 1);
            assert_eq!(b.dma_read_word(desc), Some(control));
            let replies = &b.periph.wifi.link.ap().unwrap().queue;
            assert_eq!(replies.len(), 1, "size={size}");
            assert_eq!(replies[0].at_us, 300);
            assert_eq!(replies[0].frame.len(), 30);
            assert_eq!(replies[0].frame[0], 0xb0);
            assert_eq!(&replies[0].frame[4..10], &sta);
            assert_eq!(&replies[0].frame[24..], &[0, 0, 2, 0, 0, 0]);
        }
    }

    #[test]
    fn invalid_tx_descriptors_never_publish_completion() {
        // Including length>size in each negative pins the checks that remain
        // necessary after removing the RX-capacity interpretation from TX.
        for (desc, control, buffer) in [
            (0x3ffb_0000, 30 << 12 | 28, 0x3ffb_1000),
            (0x3ffb_0000, 1 << 31 | 30 << 12 | 28, DRAM_HIGH - 28),
            (0x3ffb_0000, 1 << 31 | 30 << 12 | 28, ROM_DRAM_LOW - 4),
            (0x3ffb_0000, 1 << 31 | 30 << 12 | 28, u32::MAX - 4),
            (DRAM_HIGH - 8, 1 << 31 | 30 << 12 | 28, 0x3ffb_1000),
        ] {
            let mut b = SocBus::new(0, [0; 6]);
            b.periph.write32(0x3ff0_00cc, 0x406);
            b.dma_write_word(desc, control);
            b.dma_write_word(desc + 4, buffer);
            b.periph.wifi.write(TXQ0, 0xc000_0000 | (desc & 0xf_ffff));
            b.wifi_step();
            assert_eq!(b.periph.wifi.events, 0, "desc={desc:#x} buffer={buffer:#x}");
            assert_eq!(b.periph.wifi.txq_complete, 0);
            assert_eq!(b.periph.wifi.read(TXQ0) >> 30, 3);
            assert_eq!(b.periph.wifi.link.tx_frames(), 0);
        }
    }

    #[test]

    fn shared_station_paces_beacons_and_preserves_relay_selection_on_reboot() {
        use esp_soc::SocBus as _;
        let mut b = SocBus::new(0, [0; 6]);
        b.attach_wifi(wifi::ApConfig::parse("").unwrap(), None).unwrap();
        b.set_ethernet_relay(true).unwrap();
        assert!(b.receive_ethernet_frame(&[0; 13]).is_err());
        b.receive_ethernet_frame(&[0; 14]).unwrap();
        b.periph.write32(0x3ff0_00cc, 0x406);
        b.periph.wifi.write(0x084, 1 << 31);
        let desc = 0x3ffb_0000;
        b.periph.wifi.write(0x088, desc);
        b.dma_write_word(desc, 1 << 31 | 1600);
        b.dma_write_word(desc + 4, 0x3ffb_1000);
        b.dma_write_word(desc + 8, desc);
        b.cycles = 24_000_000;
        xtensa_lx7::bus::Bus::tick(&mut b, 1);
        assert_eq!(b.periph.wifi.link.rx_frames(), 1);
        b.periph.wifi.link.ap_mut().unwrap().queue.push(wifi::AirFrame { at_us: 100_000, frame: vec![0; 24] });
        b.wifi_step();
        assert_eq!(b.periph.wifi.link.rx_frames(), 1);
        b.cycles += 240 * 400;
        b.wifi_step();
        assert_eq!(b.periph.wifi.link.rx_frames(), 1);
        b.dma_write_word(desc, 1 << 31 | 1600);
        b.wifi_step();
        assert_eq!(b.periph.wifi.link.rx_frames(), 2);
        b.reboot([0; 6]);
        assert!(b.periph.wifi.link.ap().is_some());
        assert!(b.periph.wifi.link.relay());
        assert_eq!(b.periph.wifi.link.rx_frames(), 0);
        assert_eq!(b.periph.wifi.link.last_rx_desc(), 0);
        assert!(b.take_ethernet_frames().is_empty());
    }

    /// A received frame sets cause 0x0100_0024. 0x01e839e0 is the enable mask both an IDF blob
    /// and an esp-wifi-hal 0.3.1 firmware write (bits 24 and 5, not 2); bit 24, 5 or 2 alone each
    /// raise the line; INT_CLR of all three bits clears it.
    #[test]
    fn rx_cause_raises_bits_24_5_and_2() {
        let mut w = WifiMac::default();
        w.write(0x088, 0x3ffb_0000);
        w.rx_filled(0x3ffb_0000, 0x3ffb_000c);
        assert_eq!(w.read(0xc48), 0x0100_0024);
        assert_eq!(w.irq_sources(), 0);
        for mask in [0x01e8_39e0, 1 << 24, 1 << 5, 1 << 2] {
            w.write(0xc40, mask);
            assert_eq!(w.irq_sources(), 1, "{mask:#x}");
        }
        w.write(0xc40, !0x0100_0024);
        assert_eq!(w.irq_sources(), 0);
        w.write(0xc40, 0x01e8_39e0);
        w.write(0xc4c, 0x0100_0024);
        assert_eq!((w.read(0xc48), w.irq_sources()), (0, 0));
    }

    #[test]
    fn registers_and_tsf_match_classic_hal() {
        let mut w = WifiMac::default();
        for (off, value) in [(0x024, 1), (0x1024, 2), (0x2024, 3)] {
            w.write(off, value);
        }
        assert_eq!((w.read(0x024), w.read(0x1024), w.read(0x2024)), (1, 2, 3));
        w.write(0xd24, 2);
        assert_eq!(w.read(0xd24), 3);
        w.write(0xd20, 0xc02b_0040);
        assert_eq!(w.tx_pending, [(0, 0x3ffb_0040)]);
        w.tx_done(0);
        assert_eq!(w.read(0xcc8), 1);
        assert_eq!(w.read(0xc48), EVENT_TX);
        assert_eq!(w.read(0xd20) >> 30, 0);
        assert_eq!(w.irq_sources(), 0);
        w.write(0xc40, EVENT_TX | EVENT_RX);
        assert_eq!(w.irq_sources(), 1);
        w.write(0xcc4, 1);
        w.write(0xc4c, EVENT_TX);
        assert_eq!((w.read(0xcc8), w.irq_sources()), (0, 0));
        w.write(0x088, 0x3ffb_0000);
        w.rx_filled(0x3ffb_0000, 0x3ffb_000c);
        w.write(0x088, 0x3ffb_0100);
        w.write(0x084, 1);
        assert_eq!(w.read(0x08c), 0x3ffb_000c);
        assert_eq!(w.read(0x090), 0x3ffb_0000);
        assert_eq!(w.read(0xc48), EVENT_RX);
        w.rx_next = 0;
        assert_eq!(w.read(0x08c), DMA_BASE);
        w.write(0x084, 1);
        assert_eq!(w.read(0x08c), 0x3ffb_0100);
        assert_eq!(w.read(0x084) & 1, 0);
        w.now_cycles = CPU_HZ;
        assert_eq!(w.read(0xc00), 1_000_000);
        w.write(0x2010, 1);
        assert_eq!(w.read(0x2014), 1_000_000);
        w.write(0x2060, 42);
        w.write(0x2064, 1);
        w.write(0x2010, 1 << 7);
        w.now_cycles += 240;
        w.write(0x2010, 2);
        assert_eq!((w.read(0x2054), w.read(0x2058)), (43, 1));
        w.write(0x2024, 7);
        w.write(0x2010, 1 << 6);
        w.write(0x2010, 1);
        assert_eq!(w.read(0x2014), 7);
        w.reset();
        assert_eq!((w.events, w.rx_next, w.link.tx_frames()), (0, 0, 0));
    }

    #[test]
    fn dport_gates_transmit_routes_interrupt_and_resets_mac() {
        use xtensa_lx7::bus::Bus;
        let mut b = SocBus::new(0, [2, 0, 0, 0, 0, 1]);
        b.periph.wifi.link.attach(Some(VirtualAp::new(wifi::ApConfig::parse("").unwrap(), false)), Some(esp_soc::net::VirtualNet::new(false)));
        b.dma_write_word(0x3ffb_0000, (1 << 31) | (24 << 12) | 24);
        b.dma_write_word(0x3ffb_0004, 0x3ffb_1000);
        b.write32(0x3ff0_0104, 7).unwrap();
        b.write32(0x3ff7_3d20, 0xc00b_0000).unwrap();
        b.wifi_step();
        assert_eq!(b.periph.wifi.link.tx_frames(), 0);
        b.write32(0x3ff0_00cc, 0x406).unwrap();
        b.wifi_step();
        assert_eq!(b.periph.wifi.link.tx_frames(), 1);
        assert_eq!(b.periph.cpu_lines(0) & (1 << 7), 0);
        b.write32(0x3ff7_3c40, EVENT_TX).unwrap();
        assert_eq!(b.periph.cpu_lines(0) & (1 << 7), 1 << 7);
        b.write32(0x3ff0_00d0, 1 << 2).unwrap();
        assert_eq!(b.periph.wifi.events, 0);
        assert_eq!(b.periph.cpu_lines(0) & (1 << 7), 0);
        assert!(b.periph.wifi.link.ap().is_some() && b.periph.wifi.link.net().is_some());
        // The MAC reset leaves the host-side link and its counts alone, as on S3/C3/C6.
        assert_eq!(b.periph.wifi.link.tx_frames(), 1);
    }

    #[test]
    fn receive_dma_uses_classic_header_and_rejects_invalid_buffers() {
        let mut b = SocBus::new(0, [2, 0, 0, 0, 0, 1]);
        b.periph.wifi.link.attach(Some(VirtualAp::new(wifi::ApConfig::parse("").unwrap(), false)), Some(esp_soc::net::VirtualNet::new(false)));
        let desc = 0x3ffb_0000;
        let buf = 0x3ffb_1000;
        let control = (1 << 31) | 1600;
        b.dma_write_word(desc, control);
        b.dma_write_word(desc + 4, buf);
        b.dma_write_word(desc + 8, desc + 12);
        b.periph.wifi.write(0x088, desc);
        let mut frame = [0u8; 24];
        frame[0] = 0x80;
        frame[4..10].fill(0xff);
        b.wifi_rx_deliver(&frame, 1234, false);
        assert_eq!(
            b.dma_read_word(desc),
            Some(control | (56 << 12) | (1 << 30))
        );
        assert_eq!(b.dma_read_word(buf), Some(0x1000_0038));
        assert_eq!(b.dma_read_word(buf + 12), Some(1234));
        assert_eq!(b.dma_read_word(buf + 24), Some(28));
        assert_eq!(b.dma_read_word(buf + 28), Some(0x80));
        assert_eq!(b.dma_read_word(buf + 52), Some(wifi::fcs(&frame)));
        assert_eq!(b.periph.wifi.read(0x090), desc);
        assert_eq!(b.periph.wifi.read(0x08c), desc + 12);
        assert_eq!(b.periph.wifi.irq_sources(), 0);
        b.periph.wifi.write(0xc40, EVENT_RX);
        assert_eq!(b.periph.wifi.irq_sources(), 1);
        b.periph.wifi.rx_next = desc;
        b.wifi_rx_deliver(&frame, 2000, false);
        assert_eq!(b.periph.wifi.link.rx_dropped(), 1);
        b.dma_write_word(desc, control);
        b.dma_write_word(desc + 4, DRAM_HIGH - 4);
        b.wifi_rx_deliver(&frame, 3000, false);
        assert_eq!(b.periph.wifi.link.rx_dropped(), 2);
        assert_eq!(b.dma_read_word(desc), Some(control));
        assert!(dma_range(0x3ff0_0000, 4).is_none());
        assert!(dma_range(u32::MAX, 4).is_none());
    }
}
/// Classic analog I2C master, shared by the APB and ROM AHB register aliases.
#[derive(Default)]
pub struct Analog {
    ram: RegRam,
    registers: esp_periph::Regi2c,
}
impl Device for Analog {
    fn read(&mut self, off: u32) -> u32 {
        let value = self.ram.read(off);
        match off {
            0 | 4 | 8 | 12 | 16 => {
                let result = self.registers.read(off, value, |_| &0);
                // RFPLL host 1, slave 0x62, register 7 polls calibration-done bit 7.
                if off == 4 && value & 0xffff == 0x0762 { result | (1 << 23) } else { result }
            }
            // ram_txdc_cal_v70 waits for bit 24, then reads comparator signs 31:30.
            0x4c => (value & 0x3fff_ffff) | (1 << 24),
            // ram_get_fm_sar_dout waits for all three conversion busy bits to clear.
            0x50 => value & !(7 << 24),
            // set_channel_rfpll_freq waits until the channel update is no longer busy.
            0x168 => value & !(1 << 31),
            0xc4 => value & !(1 << 8), // phy_force_wifi_chan: channel request completes
            _ => value,
        }
    }
    fn write(&mut self, off: u32, value: u32) -> WriteEffect {
        if matches!(off, 0 | 4 | 8 | 12 | 16) { self.registers.write(off, value); }
        self.ram.write(off, value);
        WriteEffect::NONE
    }
}

/// FE IQ estimation completes without modelling the RF calibration arithmetic.
#[derive(Default)]
pub struct FrontEnd {
    ram: RegRam,
}
impl Device for FrontEnd {
    fn read(&mut self, off: u32) -> u32 {
        let value = self.ram.read(off);
        // ram_iq_est_enable starts bits 0/1 and waits for the signed done bit.
        if off == 0x7c && value & 3 == 3 {
            value | (1 << 31)
        } else {
            value
        }
    }
    fn write(&mut self, off: u32, value: u32) -> WriteEffect {
        self.ram
            .write(off, value & if off == 0x7c { !(1 << 31) } else { u32::MAX });
        WriteEffect::NONE
    }
}

#[cfg(test)]
mod phy_tests {
    use super::*;

    #[test]
    fn analog_hosts_and_calibration_handshakes() {
        let mut analog = Analog::default();
        for (host, data) in [(0, 0x5au32), (4, 0xa5), (8, 0x69), (12, 0x96)] {
            analog.write(host, (1 << 24) | (data << 16) | 0x0362);
            analog.write(host, 0x0362);
            assert_eq!(analog.read(host), (data << 16) | 0x0362);
        }
        analog.write(4, 0x0762);
        assert_eq!(analog.read(4), 0x0080_0762);
        analog.write(0x4c, 0xc011_3cf3);
        assert_eq!(analog.read(0x4c), 0x0111_3cf3);
        analog.write(0x50, 0x0700_0002);
        assert_eq!(analog.read(0x50), 2);
        analog.write(0xc4, 0x5300_2d18);
        assert_eq!(analog.read(0xc4), 0x5300_2c18);
        analog.write(0x168, 0x8000_1234);
        assert_eq!(analog.read(0x168), 0x1234);
        analog.write(0x44, 0x1234);
        assert_eq!(analog.read(0x44), 0x1234);

        let mut fe = FrontEnd::default();
        fe.write(0x7c, 1);
        assert_eq!(fe.read(0x7c), 1);
        fe.write(0x7c, 3);
        assert_eq!(fe.read(0x7c), 0x8000_0003);
        fe.write(0x7c, 0x8000_0001);
        assert_eq!(fe.read(0x7c), 1);
    }
}

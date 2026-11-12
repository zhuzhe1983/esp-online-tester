//! The S31's WiFi side, as far as the ESP-Mosaico factory firmware (IDF v6.1-dev, `phy_version
//! 101`, `HAL_MAC_ESP32AX_764`) has asked for it. The method is the C6's (`docs/wifi-c6-plan.md`):
//! no RF or PHY simulation, every register below is here because the closed PHY/WiFi library
//! was seen waiting on it, with the waiting code named. The modem sits where the C6's does, with
//! the C6's `0x600A_xxxx` at `0x2010_xxxx`: baseband `0x2010_0000`, 802.15.4 `0x2010_3000`
//! (`IEEE802154_REG_BASE`), modem syscon `0x2010_9C00`, LP con `0x2010_F000`, analog I2C
//! `0x2010_F800`.
use esp_periph::{Device, RegRam, WriteEffect};

pub const BB_BASE: u32 = 0x2010_0000;
pub const BB_END: u32 = 0x2010_3000;

/// The modem baseband block (`0x2010_0000`). Register RAM, plus the handshakes the PHY polls:
///
/// - **Channel switch.** The ROM's `phy_freq_chan_en_sw` puts the channel index into `+0x1C`
///   bits 7:0 and pulses bit 19 (start); `phy_set_chan_freq_sw_start` then spins on `+0x28`
///   bit 8 (done). (The C6: `+0xC0` bits 13:7, start bit 14, done `+0xCC` bit 8.) Synthesiser
///   settling is not modelled: the switch is done at the start pulse and stays done.
/// - **IQ estimate.** The ROM's `phy_iq_est_enable` (from `phy_dc_iq_est`, which
///   `phy_rf_init`'s calibration runs) sets `+0x450` bit 0 (enable), then bit 1 (start), and
///   spins on `+0x47C` bit 16 (done). The C6 has the same pair at `+0x474`/`+0x4A0`. There is
///   no signal to estimate: done follows the start bit, the results read as written, zero.
/// - **TX DC calibration.** The ROM's `phy_txdc_cal` pulses `+0x418` bit 0 and spins on bit 22
///   (done), then steps the DAC offsets by the comparator bits 29 and 28. The register is at the
///   C6's offset and gets the C6's answer: done follows bit 0, the comparator bits read zero,
///   an ideal zero DC imbalance; the search runs to its end without hardware.
/// - **Power-detector tone.** The ROM's `phy_pwdet_tone_start` restarts `+0x808` bit 0 and
///   spins until `+0x80C` bits 16:14 read 7 (the C6's pair is `+0x810`/`+0x814`). They read 7
///   while bit 0 is set.
pub struct ModemBb {
    ram: RegRam,
    chan_done: bool,
    /// the channel index of the last switch, and how many there were (for tests and `--debug`)
    pub chan_index: u32,
    pub chan_switches: u32,
}
impl Default for ModemBb { fn default() -> Self { Self::new() } }
impl ModemBb { pub fn new() -> Self { ModemBb { ram: RegRam::new(), chan_done: false, chan_index: 0, chan_switches: 0 } } }

const FREQ_CHAN: u32 = 0x1c;
const FREQ_CHAN_START: u32 = 1 << 19;
const FREQ_STATUS: u32 = 0x28;
const FREQ_STATUS_DONE: u32 = 1 << 8;
const TXDC: u32 = 0x418;
const PWDET: u32 = 0x808;
const PWDET_STATUS: u32 = 0x80c;
const IQ_EST: u32 = 0x450;
const IQ_EST_START: u32 = 1 << 1;
const IQ_EST_STATUS: u32 = 0x47c;
const IQ_EST_DONE: u32 = 1 << 16;

impl Device for ModemBb {
    fn read(&mut self, off: u32) -> u32 {
        match off {
            FREQ_STATUS => self.ram.read(off) & !FREQ_STATUS_DONE | if self.chan_done { FREQ_STATUS_DONE } else { 0 },
            TXDC => self.ram.read(off) & 0x003f_ffff | (u32::from(self.ram.read(off) & 1 != 0) << 22),
            PWDET_STATUS => self.ram.read(off) & !(7 << 14) | if self.ram.read(PWDET) & 1 != 0 { 7 << 14 } else { 0 },
            IQ_EST_STATUS => self.ram.read(off) & !IQ_EST_DONE | if self.ram.read(IQ_EST) & IQ_EST_START != 0 { IQ_EST_DONE } else { 0 },
            _ => self.ram.read(off),
        }
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        if off == FREQ_CHAN && v & FREQ_CHAN_START != 0 && self.ram.read(off) & FREQ_CHAN_START == 0 {
            self.chan_index = v & 0xff; self.chan_switches += 1; self.chan_done = true;
        }
        self.ram.write(off, v);
        WriteEffect::NONE
    }
}

pub const MAC_BASE: u32 = 0x2010_4000;
pub const MAC_END: u32 = 0x2010_6000;
/// `ETS_MODEM_WIFI_MAC_INTR_SOURCE` (IDF master `soc/esp32s31/include/soc/interrupts.h`).
pub const MAC_SOURCE: usize = 120;

/// The 802.11 MAC (`0x2010_4000`) that the closed `libpp`/`libnet80211` drive
/// (`mac_version:HAL_MAC_ESP32AX_764`). It is the C6's MAC (`esp32c6::wifi::WifiMac`,
/// `HAL_MAC_ESP32AX_761`) with a few registers moved by one word; every offset below is read off
/// the S31 `libpp.a` function named with it (IDF master `esp32-wifi-lib`, `esp32s31/`):
///
/// - **`hal_init`** sets `+0xDE0` bit 1 and spins until bit 0 reads set (C6 `+0xDDC`).
/// - **Events** `+0xC48` (`hal_mac_interrupt_get_event`), cleared by writing `+0xC4C`
///   (`hal_mac_interrupt_clr_event`), as on the C6: bit 7 a transmission completed, bit 14 a
///   frame was received. Any event asserts `ETS_MODEM_WIFI_MAC_INTR_SOURCE`.
/// - **RX ring**, as on the C6: base `+0x084` (`hal_mac_rx_set_base`), next `+0x088`
///   (`hal_mac_rx_read_rxdscrnext`), last `+0x08C` with its high 12 bits from `+0xC70`
///   (`hal_mac_rx_get_last_dscr`), reload `+0x080` bit 0 (`hal_mac_rx_set_dscr_reload`).
/// - **TX queues**: queue n's control word at `+0xD70 - 16n` (`hal_mac_txq_enable`; C6
///   `+0xD6C`), bits 31:30 start it. Completion is bit n of `+0xCBC`, cleared through `+0xCB8`;
///   the error word is `+0xCB4`, cleared through `+0xCB0` (`hal_mac_get_txq_state` /
///   `hal_mac_clr_txq_state`; the C6's are one word lower). The result word
///   (`hal_mac_get_txq_pmd`, `0x2010_553C - 124n`) stays zero: success.
pub struct WifiMac {
    ram: RegRam,
    pub log: bool,
    pub events: u32,
    pub rx_base: u32, pub rx_next: u32, pub rx_last: u32,
    pub txq_complete: u32, pub tx_pending: Vec<(u8, u32)>,
    /// the access point, the network and the counts, shared with the other chips
    pub link: esp_soc::wifi::StationLink,
}
impl Default for WifiMac { fn default() -> Self { Self::new() } }

const MAC_INIT: u32 = 0xde0;
const MAC_INIT_READY: u32 = 1;
const MAC_EVENTS: u32 = 0xc48;
const MAC_EVENTS_CLR: u32 = 0xc4c;
pub const EVENT_TX_DONE: u32 = 1 << 7;
pub const EVENT_RX: u32 = 1 << 14;
const RX_RELOAD: u32 = 0x080;
const RX_BASE: u32 = 0x084;
const RX_NEXT: u32 = 0x088;
const RX_LAST: u32 = 0x08c;
const ADDR_HIGH: u32 = 0xc70;
const TXQ0: u32 = 0xd70;
const TXQ_STRIDE: u32 = 16;
const TXQ_COUNT: u32 = 11;
const TXQ_COMPLETE_CLR: u32 = 0xcb8;
const TXQ_COMPLETE: u32 = 0xcbc;
/// descriptors are in internal SRAM (`0x2F00_0000`, 512 KiB): the high 12 bits of a DMA address
const SRAM_HIGH: u32 = 0x2f00_0000;

impl WifiMac {
    pub fn new() -> Self {
        WifiMac { ram: RegRam::new(), log: false, events: 0, rx_base: 0, rx_next: 0, rx_last: 0, txq_complete: 0, tx_pending: Vec::new(), link: Default::default() }
    }
    /// A descriptor address from the 20 bits a register holds.
    pub fn addr(&self, low: u32) -> u32 { SRAM_HIGH | (low & 0xf_ffff) }
    fn txq_of(off: u32) -> Option<u8> {
        (off <= TXQ0 && (TXQ0 - off).is_multiple_of(TXQ_STRIDE) && (TXQ0 - off) / TXQ_STRIDE < TXQ_COUNT).then(|| ((TXQ0 - off) / TXQ_STRIDE) as u8)
    }
    /// The hardware sent the frame in `queue`.
    pub fn tx_done(&mut self, queue: u8) {
        self.txq_complete |= 1 << queue; self.events |= EVENT_TX_DONE;
        let off = TXQ0 - TXQ_STRIDE * queue as u32;
        let v = self.ram.read(off); self.ram.write(off, v & !(3 << 30));
    }
    /// The hardware filled `desc` and moved on to `next`.
    pub fn rx_filled(&mut self, desc: u32, next: u32) {
        self.rx_last = desc & 0xf_ffff; self.rx_next = next & 0xf_ffff; self.link.rx_delivered(desc); self.events |= EVENT_RX;
    }
    pub fn irq(&self) -> bool { self.events != 0 }
}

impl Device for WifiMac {
    fn read(&mut self, off: u32) -> u32 {
        let v = match off {
            MAC_INIT => self.ram.read(off) | MAC_INIT_READY,
            MAC_EVENTS => self.events,
            RX_BASE => self.rx_base & 0xf_ffff, RX_NEXT => self.rx_next & 0xf_ffff, RX_LAST => self.rx_last,
            TXQ_COMPLETE => self.txq_complete,
            ADDR_HIGH => if self.link.last_rx_desc() != 0 { self.link.last_rx_desc() } else { SRAM_HIGH },
            TXQ_COMPLETE_CLR => 0,
            _ => self.ram.read(off),
        };
        if self.log { eprintln!("[wifi] rd +{:#05x} -> {:#010x}", off, v); }
        v
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        if self.log { eprintln!("[wifi] wr +{:#05x} <- {:#010x}", off, v); }
        match off {
            MAC_EVENTS_CLR => self.events &= !v,
            RX_BASE => {
                self.rx_base = v & 0xf_ffff;
                if self.rx_next == 0 { self.rx_next = self.rx_base; }
                self.ram.write(off, v);
            }
            RX_RELOAD => {
                if v & 1 != 0 && self.rx_next == 0 { self.rx_next = self.rx_base; }
                self.ram.write(off, v & !1);
            }
            TXQ_COMPLETE_CLR => self.txq_complete &= !v,
            o if Self::txq_of(o).is_some() => {
                self.ram.write(off, v);
                if v & (1 << 31) != 0 { self.tx_pending.push((Self::txq_of(o).unwrap(), v & 0xf_ffff)); }
            }
            _ => self.ram.write(off, v),
        }
        WriteEffect::NONE
    }
    fn irq_sources(&self) -> u64 { self.irq() as u64 }
    fn debug(&mut self, on: bool) { self.log = on; }
}

/// Sub-word access to a word device: read-modify-write of the containing word.
pub fn read_sized(d: &mut impl Device, off: u32, bytes: usize) -> u32 {
    let w = d.read(off & !3);
    if bytes == 4 { w } else { (w >> ((off & 3) * 8)) & ((1u32 << (bytes * 8)) - 1) }
}
pub fn write_sized(d: &mut impl Device, off: u32, value: u32, bytes: usize) {
    if bytes == 4 { d.write(off & !3, value); return; }
    let shift = (off & 3) * 8; let mask = ((1u32 << (bytes * 8)) - 1) << shift;
    let w = d.read(off & !3);
    d.write(off & !3, (w & !mask) | ((value << shift) & mask));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn iq_estimate_is_done_once_started() {
        let mut bb = ModemBb::new();
        bb.write(IQ_EST, 1); assert_eq!(bb.read(IQ_EST_STATUS) & IQ_EST_DONE, 0);
        bb.write(IQ_EST, 3); assert_ne!(bb.read(IQ_EST_STATUS) & IQ_EST_DONE, 0);
        bb.write(IQ_EST, 0); assert_eq!(bb.read(IQ_EST_STATUS) & IQ_EST_DONE, 0);
    }
    #[test]
    fn channel_switch_tx_dc_and_power_detector_handshakes() {
        let mut bb = ModemBb::new();
        assert_eq!(bb.read(FREQ_STATUS) & FREQ_STATUS_DONE, 0);
        bb.write(FREQ_CHAN, 0x16); bb.write(FREQ_CHAN, 0x16 | FREQ_CHAN_START); bb.write(FREQ_CHAN, 0x16);
        assert_ne!(bb.read(FREQ_STATUS) & FREQ_STATUS_DONE, 0); assert_eq!((bb.chan_index, bb.chan_switches), (0x16, 1));
        bb.write(TXDC, 0x3000_0001); assert_eq!(bb.read(TXDC), 1 << 22 | 1);
        bb.write(PWDET, 1); assert_eq!(bb.read(PWDET_STATUS) >> 14 & 7, 7);
        bb.write(PWDET, 0); assert_eq!(bb.read(PWDET_STATUS) >> 14 & 7, 0);
    }
    #[test]
    fn mac_ready_events_and_tx_queues_at_the_s31_offsets() {
        let mut mac = WifiMac::new();
        assert_eq!(mac.read(MAC_INIT) & MAC_INIT_READY, 1);
        mac.write(0xd70 - 16 * 2, 0xc066_bb30);
        assert_eq!(std::mem::take(&mut mac.tx_pending), vec![(2, 0x6_bb30)]); assert_eq!(mac.addr(0x6_bb30), 0x2f06_bb30);
        mac.tx_done(2);
        assert_eq!(mac.read(0xcbc), 1 << 2); assert_eq!(mac.read(MAC_EVENTS), EVENT_TX_DONE); assert!(mac.irq());
        assert_eq!(mac.read(0xd70 - 32) >> 30, 0);
        mac.write(0xcb8, 1 << 2); mac.write(MAC_EVENTS_CLR, EVENT_TX_DONE);
        assert_eq!(mac.read(0xcbc), 0); assert!(!mac.irq());
        mac.write(0xd6c, 0xc000_0000); assert!(mac.tx_pending.is_empty());
    }
}

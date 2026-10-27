//! Functional S31 USB Serial/JTAG EP1 packet boundary, with explicit host traffic.
//! ESP-IDF v6.1 fff9895c82d744c7237be8847347bdd1b07c6643:
//! soc/esp32s31/register/soc/{usb_serial_jtag,hp_sys_clkrst}_reg.h and
//! esp_hal_usb/esp32s31/include/hal/usb_serial_jtag_ll.h. No USB enumeration,
//! PHY/JTAG/SOF/48-MHz timing or automatic host polling is synthesized.
use std::collections::VecDeque;
use emu_core::Fault;
pub const BASE: u32 = 0x2039_1000;
pub const CLOCK: u32 = 0x2058_7144;
const PACKET: usize = 64;
const RECV: u32 = 1 << 2;
const EMPTY: u32 = 1 << 3;
const TOKEN_IN: u32 = 1 << 8;
const SUPPORTED_IRQ: u32 = RECV | EMPTY | TOKEN_IN;
pub struct UsbConsole { tx: Vec<u8>, ready: bool, rx: VecDeque<u8>, raw: u32, ena: u32, apb: bool, attached: bool }
impl Default for UsbConsole {
    fn default() -> Self { Self { tx: Vec::with_capacity(PACKET), ready: false, rx: VecDeque::new(), raw: EMPTY, ena: 0, apb: true, attached: false } }
}
impl UsbConsole {
    /// Attach a functional packet peer; this is not USB enumeration or a physical cable.
    pub fn attach_host(&mut self, attached: bool) { self.attached = attached; }
    pub fn clock_read(&self) -> u32 { self.apb as u32 }
    pub fn clock_write(&mut self, value: u32) { self.apb = value & 1 != 0; }
    pub fn irq(&self) -> bool { self.apb && self.raw & self.ena != 0 }
    pub fn host_send(&mut self, packet: &[u8]) -> Result<(), Fault> {
        if !self.attached || !self.apb { return Err(Fault::Prohibited); }
        if packet.is_empty() || packet.len() > PACKET || !self.rx.is_empty() { return Err(Fault::Prohibited); }
        self.rx.extend(packet); self.raw |= RECV; Ok(())
    }
    /// Consume a real committed IN packet. Until this occurs DATA_FREE remains low.
    /// An empty Vec is the ZLP explicitly produced by a second WR_DONE after draining 64 B.
    pub fn host_receive(&mut self) -> Option<Vec<u8>> {
        if !self.attached || !self.apb || !self.ready { return None; }
        self.ready = false; self.raw |= EMPTY | TOKEN_IN; Some(std::mem::take(&mut self.tx))
    }
    pub fn read(&mut self, off: u32) -> Result<u32, Fault> {
        if off & 3 != 0 { return Err(Fault::Misaligned); }
        if !self.apb { return Err(Fault::Prohibited); }
        match off {
            0 => Ok(self.rx.pop_front().unwrap_or(0) as u32),
            4 => Ok(if self.ready { 0 } else { 2 } | if self.rx.is_empty() { 0 } else { 4 }),
            8 => Ok(self.raw), 0xc => Ok(self.raw & self.ena), 0x10 => Ok(self.ena), 0x14 => Ok(0),
            _ => Err(Fault::Unmapped),
        }
    }
    pub fn write(&mut self, off: u32, value: u32) -> Result<(), Fault> {
        if off & 3 != 0 { return Err(Fault::Misaligned); }
        if !self.apb { return Err(Fault::Prohibited); }
        match off {
            0 => {
                if self.ready { return Err(Fault::Prohibited); }
                self.tx.push(value as u8); self.raw &= !EMPTY;
                // v6.1 LL documents automatic flush at 64 B; a following WR_DONE is a no-op.
                if self.tx.len() == PACKET { self.ready = true; }
            }
            4 => if value & 1 != 0 { self.ready = true; self.raw &= !EMPTY; },
            8 | 0x14 => self.raw &= !value,
            0xc => {}, // RO masked status.
            0x10 => {
                if value & !SUPPORTED_IRQ != 0 { return Err(Fault::Prohibited); }
                self.ena = value;
            }
            _ => return Err(Fault::Unmapped),
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_packet_waits_for_real_host_consumption_then_zlp_is_distinct() {
        let mut u = UsbConsole::default(); assert_eq!(u.read(4), Ok(2));
        for byte in 0..64 { u.write(0, byte).unwrap(); }
        assert_eq!(u.read(4), Ok(0)); u.write(4, 1).unwrap();
        assert_eq!(u.host_receive(), None); assert_eq!(u.write(0, 65), Err(Fault::Prohibited));
        u.attach_host(true); assert_eq!(u.host_receive(), Some((0..64).collect())); assert_eq!(u.read(4), Ok(2));
        u.write(4, 1).unwrap(); assert_eq!(u.host_receive(), Some(vec![])); assert_eq!(u.host_receive(), None);
    }
    #[test]
    fn short_packet_and_rx_interrupt_require_actual_host_operations_and_bus_clock() {
        let mut u = UsbConsole::default(); u.attach_host(true); u.write(0x10, RECV | EMPTY).unwrap();
        u.write(0x14, EMPTY).unwrap(); assert!(!u.irq());
        u.write(0, 65).unwrap(); assert_eq!(u.host_receive(), None); u.write(4, 1).unwrap();
        assert_eq!(u.host_receive(), Some(vec![65])); assert!(u.irq()); u.write(0x14, EMPTY).unwrap();
        u.host_send(b"AB").unwrap(); assert!(u.irq()); assert_eq!(u.read(4), Ok(6));
        assert_eq!(u.host_send(b"C"), Err(Fault::Prohibited)); assert_eq!(u.read(0), Ok(65)); assert_eq!(u.read(0), Ok(66));
        u.write(0x14, RECV).unwrap(); assert!(!u.irq()); assert_eq!(u.read(4), Ok(2));
        u.clock_write(0); assert_eq!(u.read(4), Err(Fault::Prohibited)); assert_eq!(u.host_send(b"C"), Err(Fault::Prohibited));
        u.clock_write(1); assert_eq!(u.write(0x10, 2), Err(Fault::Prohibited)); assert_eq!(u.read(0x18), Err(Fault::Unmapped));
    }
}

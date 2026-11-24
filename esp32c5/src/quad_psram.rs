//! A deliberately explicit virtual 64-Mbit Quad PSRAM die for C5.
//!
//! Command/mode rules: AP Memory APS6404L-3SQR rev2.7 sections 9–14;
//! initialization and ID fields: ESP-IDF v5.5.4 `esp_psram_impl_ap_quad.c`
//! and `esp_quad_psram_defs_ap.h`. This is a compatible virtual device, not
//! an identification dump of the undocumented die inside a C5HR8 package.
//! Only the documented capacity/non-2T fields are populated in its EID.
//! Array contents begin as deterministic zeroes supplied by the host allocator;
//! no claim is made about real power-on contents, analog timing or calibration.
use esp_periph::SpiMem;

pub(crate) const CAPACITY: usize = 8 << 20;
pub(crate) const NATIVE_PADS: u32 = (1 << 15) | (1 << 17) | (1 << 18) | (1 << 20) | (1 << 21) | (1 << 22);
const SERIAL_PADS: u32 = (1 << 15) | (1 << 17) | (1 << 21) | (1 << 22);

#[derive(Clone, Default)]
pub(crate) struct QuadPsram {
    bytes: usize,
    qpi: bool,
    reset_enabled: bool,
    wrap32: bool,
}

impl QuadPsram {
    pub fn configure(&mut self, bytes: usize) -> Result<(), String> {
        if bytes != 0 && bytes != CAPACITY {
            return Err("C5 Quad PSRAM currently supports absent or an explicit 8 MiB die".into());
        }
        if self.bytes != bytes {
            *self = Self { bytes, ..Self::default() };
        }
        Ok(())
    }

    pub fn bytes(&self) -> usize { self.bytes }
    pub fn qpi(&self) -> bool { self.qpi }
    pub fn wrap32(&self) -> bool { self.wrap32 }

    fn buffer_write(port: &mut SpiMem, start: usize, bytes: &[u8]) {
        for (n, &value) in bytes.iter().enumerate() {
            let byte = start + n;
            let shift = (byte & 3) * 8;
            port.w[byte / 4] = (port.w[byte / 4] & !(255 << shift)) | u32::from(value) << shift;
        }
    }

    /// An ordinary wrong mode, unselected/misrouted die or unknown opcode sees
    /// no response, not fabricated success data. Unsupported host transfer
    /// encodings return false so the C5 wrapper exposes its modeling boundary.
    pub fn transfer(&mut self, port: &mut SpiMem, memory: &mut [u8], pads: u32, bus_hz: u32) -> bool {
        let ctrl = port.regs.read(8);
        let user = port.regs.read(0x18);
        let user1 = port.regs.read(0x1c);
        let user2 = port.regs.read(0x20);
        let read = user & (1 << 28) != 0;
        let write = user & (1 << 27) != 0;
        let rx_bits = if read { (port.regs.read(0x28) & 1023) + 1 } else { 0 };
        let tx_bits = if write { (port.regs.read(0x24) & 1023) + 1 } else { 0 };
        let rx_start = if user & (1 << 24) != 0 { 32 } else { 0 };
        let tx_start = if user & (1 << 25) != 0 { 32 } else { 0 };
        if user & (1 << 31) == 0 || user2 >> 28 != 7 || read && write
            || ctrl & ((1 << 4) | (1 << 5) | (1 << 6) | (1 << 9)) != 0
            || port.regs.read(0xd4) & 1 != 0
            || port.regs.read(0x34) & ((1 << 9) | (1 << 10)) != 0
            || user & ((1 << 9) | (1 << 26)) != 0 || !rx_bits.is_multiple_of(8) || !tx_bits.is_multiple_of(8)
            || rx_start + rx_bits as usize / 8 > 64 || tx_start + tx_bits as usize / 8 > 64 {
            return false;
        }
        let rx_len = rx_bits as usize / 8;
        let tx: Vec<u8> = (0..tx_bits as usize / 8).map(|i| {
            let byte = tx_start + i;
            (port.w[byte / 4] >> ((byte & 3) * 8)) as u8
        }).collect();
        Self::buffer_write(port, rx_start, &vec![255; rx_len]);
        let command_quad = ctrl & (1 << 8) != 0;
        let read_quad = ctrl & (1 << 13) != 0 && ctrl & (1 << 24) != 0;
        let write_quad = user & (1 << 15) != 0;
        let quad = command_quad || read && read_quad || write && write_quad;
        let needed_pads = if quad { NATIVE_PADS } else { SERIAL_PADS };
        if self.bytes != CAPACITY || memory.len() != CAPACITY || pads & needed_pads != needed_pads
            || command_quad != self.qpi || bus_hz == 0 {
            return true;
        }
        let opcode = user2 as u8;
        let armed = self.reset_enabled;
        self.reset_enabled = false;
        let address_enabled = user & (1 << 30) != 0;
        let dummy = if user & (1 << 29) != 0 { (user1 & 63) + 1 } else { 0 };
        let address_24 = address_enabled && user1 >> 26 == 23;
        let no_payload = !address_enabled && !read && !write && dummy == 0;
        if no_payload {
            match opcode {
                0x66 => self.reset_enabled = true,
                0x99 if armed => { self.qpi = false; self.wrap32 = false; },
                0x35 if !self.qpi => self.qpi = true,
                0xf5 if self.qpi => self.qpi = false,
                0xc0 => self.wrap32 = !self.wrap32,
                _ => {},
            }
            return true;
        }
        // Other single/dual/quad data selections are not interchangeable with
        // the required QIO phase. In particular a quad command alone is not QPI.
        let read_mode_ok = if read_quad {
            ctrl & ((1 << 14) | (1 << 20) | (1 << 23)) == 0
        } else {
            ctrl & ((1 << 14) | (1 << 20) | (1 << 23) | (1 << 24)) == 0
        };
        let write_mode_ok = if write_quad { user & (7 << 12) == 0 } else { user & (15 << 12) == 0 };
        if !address_24 || read && !read_mode_ok || write && !write_mode_ok { return true; }
        if opcode == 0x9f && !self.qpi && read && !read_quad && dummy == 0 && bus_hz <= 33_000_000 {
            // IDF defines manufacturer 0D, known-good-die 5D, density 010 and
            // EID[41]=1 for an 8 MiB non-2T virtual die. Manufacturing bits are
            // intentionally zero and are never described as a real serial ID.
            let id = [0x0d, 0x5d, 0x42, 0, 0, 0, 0, 0];
            Self::buffer_write(port, rx_start, &id[..rx_len.min(id.len())]);
            return true;
        }
        let valid_read = read && match (self.qpi, opcode) {
            (false, 0x03) => !read_quad && dummy == 0 && bus_hz <= 33_000_000,
            (false, 0x0b) => !read_quad && dummy == 8,
            (true, 0x0b) => read_quad && dummy == 4 && bus_hz <= 66_000_000,
            (_, 0xeb) => read_quad && dummy == 6,
            _ => false,
        };
        let valid_write = write && dummy == 0 && match (self.qpi, opcode) {
            (false, 0x02) => !write_quad,
            (true, 0x02) | (_, 0x38) => write_quad,
            _ => false,
        };
        if !valid_read && !valid_write || bus_hz > if self.wrap32 { 109_000_000 } else { 84_000_000 } { return true; }
        let address = (port.regs.read(4) & 0x7f_ffff) as usize;
        let physical = |i: usize| {
            if self.wrap32 { (address & !31) | ((address + i) & 31) }
            else { (address + i) & (CAPACITY - 1) }
        };
        if valid_write {
            for (i, &value) in tx.iter().enumerate() { memory[physical(i)] = value; }
        } else {
            let bytes: Vec<u8> = (0..rx_len).map(|i| memory[physical(i)]).collect();
            Self::buffer_write(port, rx_start, &bytes);
        }
        true
    }
}

//! Lepton 3.0 minimal factory-host CCI + Raw14 VoSPI subset, NOT a calibrated imager.
//! FLIR IDD 110-0144-04 rev303 sections 2.1, 4.7.15/16; engineering rev204 4.2.3.
//! M5-StickT a502cb40: 0x2a on SDA21/SCL22, reset26, VSYNC38, SPI CLK2/MISO25/CS0.
use esp_periph::i2c::I2cDevice;
use std::sync::{Arc, Mutex};
const PERIOD: u64 = 240_000_000 / 106;
const PULSE: u64 = 120_000;
#[derive(Clone, Copy, Debug)]
pub struct Raw14Scene {
    pub background: u16,
    pub hotspot: u16,
    pub center: [u16; 2],
    pub radius: u16,
}
impl Default for Raw14Scene {
    fn default() -> Self {
        Self {
            background: 8000,
            hotspot: 9000,
            center: [80, 60],
            radius: 12,
        }
    }
}
impl Raw14Scene {
    fn sample(&self, x: u16, y: u16) -> u16 {
        let dx = i32::from(x) - i32::from(self.center[0]);
        let dy = i32::from(y) - i32::from(self.center[1]);
        if dx * dx + dy * dy <= i32::from(self.radius).pow(2) {
            self.hotspot
        } else {
            self.background
        }
    }
}
pub struct Lepton {
    pub reset_high: bool,
    pub gpio_mode: u16,
    pub phase: i16,
    scene: Raw14Scene,
    data: [u16; 16],
    length: u16,
    command: u16,
    error: i8,
    busy: bool,
    pub cci_reads: u64,
    pub cci_writes: u64,
    pub commands: u64,
    pub unsupported: u64,
    pub packets: u64,
    pub spi_reads: u64,
    pub trace: Vec<String>,
    cycle: u64,
    next: Option<u64>,
    pub vsync: bool,
    reset_fall: Option<u64>,
    segment: u8,
    next_segment: u8,
    packet: u8,
    byte: usize,
    buffer: [u8; 164],
    ready: bool,
    cs_high: bool,
    cs_since: u64,
}
impl Default for Lepton {
    fn default() -> Self {
        Self::new()
    }
}
impl Lepton {
    pub fn new() -> Self {
        Self {
            reset_high: true,
            gpio_mode: 0,
            phase: 0,
            scene: Default::default(),
            data: [0; 16],
            length: 0,
            command: 0,
            error: 0,
            busy: false,
            cci_reads: 0,
            cci_writes: 0,
            commands: 0,
            unsupported: 0,
            packets: 0,
            spi_reads: 0,
            trace: Vec::new(),
            cycle: 0,
            next: None,
            vsync: false,
            reset_fall: None,
            segment: 1,
            next_segment: 1,
            packet: 0,
            byte: 0,
            buffer: [0; 164],
            ready: false,
            cs_high: true,
            cs_since: 0,
        }
    }
    fn note(&mut self, text: String) {
        if self.trace.len() < 48 {
            self.trace.push(text);
        }
    }
    pub fn set_scene(&mut self, s: Raw14Scene) -> bool {
        if s.background == 0
            || s.hotspot == 0
            || s.background > 0x3fff
            || s.hotspot > 0x3fff
            || s.center[0] >= 160
            || s.center[1] >= 120
            || s.radius > 160
        {
            return false;
        }
        self.scene = s;
        true
    }
    pub fn reset_line(&mut self, high: bool) {
        if self.reset_high && !high {
            if self.vsync { self.reset_fall=Some(self.cycle); }
            self.command=0; self.length=0; self.data.fill(0);
            self.gpio_mode = 0;
            self.phase = 0;
            self.busy = false;
            self.error = 0;
            self.next = None;
            self.vsync = false;
            self.ready = false;
            self.next_segment = 1;
            self.byte = 0;
        }
        self.reset_high = high;
    }
    pub fn cs(&mut self, high: bool) {
        if high && !self.cs_high {
            self.cs_since = self.cycle;
        }
        if !high && self.cs_high && self.cycle.saturating_sub(self.cs_since) > 44_400_000 {
            self.next_segment = 1;
            self.ready = false;
            self.byte = 0;
        }
        self.cs_high = high;
    }
    fn status(&self) -> u16 {
        if !self.reset_high {
            0
        } else {
            ((self.error as u8 as u16) << 8) | 6 | u16::from(self.busy)
        }
    }
    fn read_word(&mut self, reg: u16) -> u16 {
        self.cci_reads += 1;
        let v = match reg {
            0 => 0,
            2 => self.status(),
            4 => self.command,
            6 => self.length,
            8..=0x26 if reg & 1 == 0 => self.data[usize::from((reg - 8) / 2)],
            _ => 0xffff,
        };
        self.note(format!("R{reg:04x}={v:04x}"));
        v
    }
    fn write_word(&mut self, reg: u16, value: u16) {
        self.cci_writes += 1;
        self.note(format!("W{reg:04x}={value:04x}"));
        match reg {
            4 => {
                if self.busy {
                    return;
                }
                self.command = value;
                self.busy = true;
                self.error = 0;
                self.commands += 1;
            }
            6 => self.length = value,
            8..=0x26 if reg & 1 == 0 => self.data[usize::from((reg - 8) / 2)] = value,
            _ => {}
        }
    }
    fn complete(&mut self) {
        if !self.busy {
            return;
        }
        self.busy = false;
        // Factory host sends a one-word enum; SDK IDD specifies two. This narrow
        // legacy profile is explicit, not a claim about all physical Lepton revisions.
        let enum_len = self.length == 1 || self.length == 2 && self.data[1] == 0;
        self.error = match self.command {
            0x4855 if !enum_len => -6,
            0x4855 if self.data[0] > 5 => -3,
            0x4855 => {
                self.gpio_mode = self.data[0];
                self.next = if self.gpio_mode == 5 {
                    Some(self.cycle + PERIOD)
                } else {
                    None
                };
                0
            }
            0x4859
                if !(self.length == 1
                    || self.length == 2
                        && self.data[1] == if (self.data[0] as i16) < 0 { 0xffff } else { 0 }) =>
            {
                -6
            }
            0x4859 if !(-3..=3).contains(&(self.data[0] as i16)) => -3,
            0x4859 => {
                let phase = self.data[0] as i16;
                if let Some(at) = self.next {
                    if !self.vsync {
                        let shift = i64::from(phase - self.phase) * PULSE as i64;
                        self.next = Some((at as i64 + shift).max(self.cycle as i64 + 1) as u64);
                    }
                }
                self.phase = phase;
                0
            }
            0x4854 => {
                self.length = 2;
                self.data[0] = self.gpio_mode;
                self.data[1] = 0;
                0
            }
            0x4858 => {
                self.length = 2;
                self.data[0] = self.phase as u16;
                self.data[1] = if self.phase < 0 { 0xffff } else { 0 };
                0
            }
            0x0210 | 0x0214 => {
                self.length = 1;
                self.data[0] = 29815;
                0
            } // virtual FPA/AUX 25 C, not scene temperature
            _ => {
                self.unsupported += 1;
                -7
            }
        };
    }
    pub fn deadline(&self) -> Option<u64> {
        self.reset_fall.or(self.next)
    }
    pub fn advance(&mut self, cycle: u64) -> Vec<(u64, bool)> {
        let mut edges = Vec::new();
        if self.reset_fall.is_some_and(|at| at<=cycle) {edges.push((self.reset_fall.take().unwrap(),false));}
        while let Some(at) = self.next {
            if at > cycle {
                break;
            }
            self.cycle = at;
            self.vsync = !self.vsync;
            edges.push((at, self.vsync));
            if self.vsync {
                self.segment = self.next_segment;
                self.next_segment = self.next_segment % 4 + 1;
                self.packet = 0;
                self.byte = 0;
                self.ready = true;
                self.next = Some(at + PULSE);
            } else {
                self.next = Some(at + PERIOD - PULSE);
            }
        }
        self.cycle = self.cycle.max(cycle);
        edges
    }
    pub fn receive(&mut self, n: usize) -> Vec<u8> {
        self.spi_reads += 1;
        if self.cs_high || !self.reset_high {return vec![0xff;n];}
        let mut result = Vec::with_capacity(n);
        for _ in 0..n {
            if self.byte == 0 {
                self.buffer.fill(0);
                if !self.ready {
                    self.buffer[0] = 0x0f;
                } else {
                    let id = (if self.packet == 20 {
                        u16::from(self.segment) << 12
                    } else {
                        0
                    }) | u16::from(self.packet);
                    self.buffer[..2].copy_from_slice(&id.to_be_bytes());
                    for col in 0..80 {
                        let x = u16::from(self.packet & 1) * 80 + col as u16;
                        let y = u16::from(self.segment - 1) * 30 + u16::from(self.packet / 2);
                        let value = self.scene.sample(x, y);
                        self.buffer[4 + 2 * col..6 + 2 * col].copy_from_slice(&value.to_be_bytes());
                    }
                    let crc = packet_crc(&self.buffer);
                    self.buffer[2..4].copy_from_slice(&crc.to_be_bytes());
                }
            }
            result.push(self.buffer[self.byte]);
            self.byte += 1;
            if self.byte == 164 {
                self.byte = 0;
                if self.ready {
                    self.packets += 1;
                    self.packet += 1;
                    if self.packet == 60 {
                        self.ready = false;
                    }
                }
            }
        }
        result
    }
}
fn packet_crc(packet: &[u8; 164]) -> u16 {
    let mut crc = 0u16;
    for (i, &v) in packet.iter().enumerate() {
        let byte = if i == 0 {
            v & 15
        } else if i == 2 || i == 3 {
            0
        } else {
            v
        };
        crc ^= u16::from(byte) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}
pub struct Cci {
    state: Arc<Mutex<Lepton>>,
    pointer: u16,
    address_bytes: u8,
    high: u8,
    read_latch: u16,
}
impl Cci {
    pub fn new(state: Arc<Mutex<Lepton>>) -> Self {
        Self {
            state,
            pointer: 0,
            address_bytes: 0,
            high: 0,
            read_latch: 0,
        }
    }
}
impl I2cDevice for Cci {
    fn start(&mut self, read: bool) -> bool {
        if !read {
            self.address_bytes = 0;
        }
        self.state.lock().expect("Lepton poisoned").reset_high
    }
    fn write(&mut self, v: u8) -> bool {
        if self.address_bytes < 2 {
            self.pointer = (self.pointer << 8) | u16::from(v);
            self.address_bytes += 1;
            return true;
        }
        if self.pointer & 1 == 0 {
            self.high = v;
        } else {
            self.state
                .lock()
                .expect("Lepton poisoned")
                .write_word(self.pointer - 1, u16::from_be_bytes([self.high, v]));
        }
        self.pointer = self.pointer.wrapping_add(1);
        true
    }
    fn read(&mut self) -> u8 {
        let mut s = self.state.lock().expect("Lepton poisoned");
        let v = if self.pointer & 1 == 0 {
            self.read_latch = s.read_word(self.pointer);
            (self.read_latch >> 8) as u8
        } else {
            let b = self.read_latch as u8;
            if self.pointer == 3 {
                s.complete();
            }
            b
        };
        self.pointer = self.pointer.wrapping_add(1);
        v
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn wr(d: &mut Cci, r: u16, values: &[u16]) {
        d.start(false);
        for v in r.to_be_bytes() {
            assert!(d.write(v));
        }
        for v in values {
            for b in v.to_be_bytes() {
                assert!(d.write(b));
            }
        }
        d.stop();
    }
    fn rd(d: &mut Cci, r: u16, n: usize) -> Vec<u16> {
        wr(d, r, &[]);
        assert!(d.start(true));
        (0..n)
            .map(|_| u16::from_be_bytes([d.read(), d.read()]))
            .collect()
    }
    #[test]
    fn cci_factory_source_sequence_busy_error_and_big_endian_word_length() {
        let state = Arc::new(Mutex::new(Lepton::new()));
        let mut a = Cci::new(state.clone());
        let mut b = Cci::new(state.clone());
        assert_eq!(rd(&mut a, 2, 1), [6]);
        wr(&mut a, 6, &[1, 5]);
        wr(&mut a, 4, &[0x4855]);
        assert_eq!(rd(&mut b, 2, 1), [7]);
        assert_eq!(rd(&mut b, 2, 1), [6]);
        assert_eq!(state.lock().unwrap().gpio_mode, 5);
        wr(&mut a, 6, &[1, 3]);
        wr(&mut a, 4, &[0x4859]);
        rd(&mut a, 2, 1);
        assert_eq!(state.lock().unwrap().phase, 3);
        wr(&mut a, 6, &[2, 0xfffd, 0xffff]);
        wr(&mut a, 4, &[0x4859]);
        rd(&mut a, 2, 1);
        assert_eq!(rd(&mut a, 2, 1), [6]);
        assert_eq!(state.lock().unwrap().phase, -3);
        wr(&mut a, 6, &[3]);
        wr(&mut a, 4, &[0x4855]);
        rd(&mut a, 2, 1);
        assert_eq!(rd(&mut a, 2, 1), [0xfa06]);
        wr(&mut a, 6, &[1, 6]);
        wr(&mut a, 4, &[0x4855]);
        rd(&mut a, 2, 1);
        assert_eq!(rd(&mut a, 2, 1), [0xfd06]);
        wr(&mut a, 4, &[0x0214]);
        rd(&mut a, 2, 1);
        assert_eq!(
            rd(&mut a, 6, 2),
            [1, 29815],
            "length is 16-bit words, not source driver's questionable bytes/2"
        );
        wr(&mut a, 4, &[0xdead]);
        rd(&mut a, 2, 1);
        assert_eq!(rd(&mut a, 2, 1), [0xf906]);
        state.lock().unwrap().reset_line(false);
        assert!(!a.start(false));
        state.lock().unwrap().reset_line(true);
        assert_eq!(rd(&mut a, 2, 1), [6]);
    }
    #[test]
    fn vospi_packets_four_segments_scene_mapping_and_crc() {
        let mut s = Lepton::new();
        s.cs(false);
        s.gpio_mode = 5;
        s.next = Some(PERIOD);
        s.set_scene(Raw14Scene {
            background: 1234,
            hotspot: 4321,
            center: [0, 0],
            radius: 0,
        });
        for seg in 1u8..=4 {
            let edges = s.advance(PERIOD * u64::from(seg));
            assert!(edges.iter().any(|e| e.1));
            for line in 0..60 {
                let bytes = s.receive(164);
                let id = u16::from_be_bytes([bytes[0], bytes[1]]);
                // Independent Python binascii.crc_hqx oracle over the specified Raw14 packet.
                if seg == 1 && line == 0 {
                    assert_eq!(&bytes[2..4], &0x5f28u16.to_be_bytes());
                }
                assert_eq!(id & 0xfff, line);
                if line == 20 {
                    assert_eq!(id >> 12, seg as u16);
                }
                assert_eq!(
                    u16::from_be_bytes([bytes[2], bytes[3]]),
                    packet_crc(bytes.as_slice().try_into().unwrap())
                );
                let raw = u16::from_be_bytes([bytes[4], bytes[5]]);
                assert_eq!(raw, if seg == 1 && line == 0 { 4321 } else { 1234 });
            }
            assert_eq!(s.receive(2), [0x0f, 0]);
        }
        assert_eq!(s.packets, 240);
        assert!(!s.set_scene(Raw14Scene {
            background: 0,
            ..Default::default()
        }));
    }
}

//! Opt-in CO5300 QSPI transport around the existing GRAM engine. The legacy
//! Co5300 callers are unchanged. Physical pad/power/reset checks belong to the
//! board; this layer checks every serial phase and the whole CS-delimited packet.
//! Sources: docs/platform/waveshare-amoled143c-runtime.md.
use crate::{board::SpiQspiPins, devices::co5300::{Co5300, Co5300DisplayState}};

/// SH8601-specific observations. The underlying RGB565 address storage is
/// shared code only; CO5300 reset/page/orientation semantics are not inherited.
/// Official SH8601A v0.0 pp42,56,57,119,123,136,138 versus pinned Waveshare
/// Arduino_SH8601.h: 16-bit QSPI is a documented BSP/data-sheet conflict.
#[derive(Clone, Copy)]
struct Sh8601BspState { state: Co5300DisplayState, brightness: u16 }
impl Default for Sh8601BspState {
    fn default() -> Self { Self { state: Co5300DisplayState { brightness:255, control_display:Some(0x28), ..Default::default() }, brightness:255 } }
}
pub struct QspiDcsPanel {
    memory: Co5300,
    sh8601: Option<Sh8601BspState>,
    reference_madctl: u8,
    pub madctl: u8,
    pub page: u8,
    pub inverted: bool,
    selected: bool,
    discard: bool,
    header: [u8; 4],
    header_len: usize,
    parameter_count: usize,
    command: u8,
    normal_command: bool,
    pub commands: u64,
    pub opaque_commands: u64,
    pub opaque_bytes: u64,
    pub rejected: u64,
    pub changes: u64,
}
impl QspiDcsPanel {
    /// Presentation coordinates are relative to the board's fixed BSP MADCTL
    /// orientation, not a calibrated physical glass/PCB pose. Only the supplied
    /// visible window is stored; this is not an assertion of full silicon GRAM size.
    pub fn co5300(width: usize, height: usize, x_gap: u16, reference_madctl: u8) -> Self {
        Self { memory: Co5300::new(width, height, x_gap).with_display_state(), sh8601:None,
            reference_madctl: reference_madctl & 0xe8, madctl: 0, page: 0, inverted: false,
            selected: false, discard: false, header: [0; 4], header_len: 0,
            parameter_count: 0, command: 0, normal_command: false,
            commands: 0, opaque_commands: 0, opaque_bytes: 0, rejected: 0, changes: 0 }
    }
    /// Explicit manufacturer-BSP RGB565 subset, not full SH8601 silicon emulation.
    /// The preliminary data sheet says QSPI accepts 24-bit pixels only, while
    /// the pinned manufacturer's shipped Arduino driver sends RGB565. This API
    /// intentionally names that unresolved conflict. Reserved MADCTL bits,
    /// private registers, readback IDs, TE and physical analog timing are absent.
    pub fn sh8601_bsp_rgb565(width: usize, height: usize, x_gap: u16, reference_madctl: u8) -> Self {
        assert_eq!(reference_madctl & !0x48, 0, "SH8601 reference includes reserved MADCTL bits");
        let mut panel = Self::co5300(width, height, x_gap, reference_madctl);
        panel.sh8601 = Some(Sh8601BspState::default());
        panel
    }
    pub fn controller(&self) -> &'static str { if self.sh8601.is_some() { "SH8601 manufacturer-BSP RGB565 subset" } else { "CO5300" } }
    pub fn brightness_10bit(&self) -> Option<u16> { self.sh8601.map(|s|s.brightness) }
    pub fn width(&self) -> usize { self.memory.width() }
    pub fn height(&self) -> usize { self.memory.height() }
    pub fn frames(&self) -> u64 { self.memory.frames }
    pub fn pixels_written(&self) -> u64 { self.memory.pixels_written }
    pub fn state(&self) -> Co5300DisplayState { self.sh8601.map_or_else(||self.memory.display_state().expect("CO5300 state enabled"), |s|s.state) }
    pub fn window(&self) -> [u16; 4] { self.memory.window() }
    pub fn version(&self) -> u64 { self.changes.wrapping_add(self.pixels_written()).wrapping_add(self.state().revision) }
    fn orientation_supported(&self) -> bool {
        if self.sh8601.is_some() { self.madctl & !0x48 == 0 } else { self.width() == self.height() || (self.madctl | self.reference_madctl) & 0x20 == 0 }
    }
    pub fn visible(&self) -> bool { self.state().visible() && self.orientation_supported() }
    pub fn frame(&self) -> Vec<u16> {
        let (w, h) = (self.width(), self.height());
        let mut out = vec![0; w * h];
        if !self.visible() { return out; }
        let orientation = self.madctl ^ self.reference_madctl;
        for (i, &pixel) in self.memory.frame.iter().enumerate() {
            let (mut x, mut y) = (i % w, i / w);
            // Apply guest row/column exchange and mirrors, then invert the
            // fixed board reference transform. XOR alone is insufficient:
            // exchanging axes does not commute with an individual mirror.
            // This is the evidenced BSP digital presentation policy; the
            // data-sheet bit-5 wording conflict remains physically unverified.
            // Only square windows support MV here; rectangular address-range
            // exchange remains explicitly unsupported rather than cropped.
            if self.madctl & 0x20 != 0 { std::mem::swap(&mut x, &mut y); }
            if self.madctl & 0x40 != 0 { x = w - 1 - x; }
            if self.madctl & 0x80 != 0 { y = h - 1 - y; }
            if self.reference_madctl & 0x80 != 0 { y = h - 1 - y; }
            if self.reference_madctl & 0x40 != 0 { x = w - 1 - x; }
            if self.reference_madctl & 0x20 != 0 { std::mem::swap(&mut x, &mut y); }
            let mut pixel = if orientation & 8 != 0 { (pixel & 0x07e0) | (pixel >> 11) | ((pixel & 31) << 11) } else { pixel };
            if self.inverted { pixel ^= 0xffff; }
            out[y * w + x] = pixel;
        }
        out
    }
    fn reset_registers(&mut self) {
        // CO5300 CASET/RASET default tables explicitly keep frame memory on
        // both software and hardware reset. Event counters remain monotonic.
        let frame = std::mem::take(&mut self.memory.frame);
        let (frames, pixels) = (self.memory.frames, self.memory.pixels_written);
        self.memory.reset();
        if let Some(state) = &mut self.sh8601 {
            // SH8601 reset table says RAM becomes random, unlike CO5300.
            // Deterministic zero is the emulator's explicit UNKNOWN RAM policy,
            // never a synthetic factory image or asserted physical reset value.
            *state = Sh8601BspState { state:Co5300DisplayState { revision:state.state.revision.wrapping_add(1), ..Sh8601BspState::default().state }, ..Default::default() };
        } else { self.memory.frame = frame; }
        self.memory.frames = frames; self.memory.pixels_written = pixels;
        self.madctl = 0; self.page = 0; self.inverted = false; self.changes += 1;
    }
    pub fn reset(&mut self) { self.reset_registers(); self.deselect(); }
    pub fn deselect(&mut self) {
        self.selected = false; self.discard = false; self.header_len = 0;
        self.parameter_count = 0; self.normal_command = false;
    }
    /// A malformed phase cannot resynchronize on accidental pixel/header bytes
    /// before the real CS is released. The board owns the actual CS edge.
    pub fn reject_until_deselect(&mut self) {
        self.selected = true; self.discard = true; self.rejected += 1;
    }
    pub fn transfer(&mut self, p: SpiQspiPins, bytes: &[u8]) -> bool {
        if self.discard { return false; }
        if !self.selected {
            self.selected = true; self.header_len = 0; self.parameter_count = 0;
            self.memory.begin_transaction();
        }
        let bits = u64::from(p.command_bits) + u64::from(p.address_bits) + u64::from(p.data_bits);
        if p.unsupported_mode || p.sio || p.full_duplex || p.dummy_cycles != 0
            || p.clock_idle_high || p.clock_output_edge || p.read_bits != 0 || p.read_width != 0
            || !p.command_bits.is_multiple_of(8) || !p.address_bits.is_multiple_of(8)
            || !p.data_bits.is_multiple_of(8) || bits != bytes.len() as u64 * 8
            || (p.command_bits == 0) != (p.command_width == 0)
            || (p.address_bits == 0) != (p.address_width == 0)
            || (p.data_bits == 0) != (p.data_width == 0)
        { self.reject_until_deselect(); return false; }
        let mut at = 0;
        for (n, width) in [(usize::from(p.command_bits) / 8, p.command_width),
            (usize::from(p.address_bits) / 8, p.address_width), (p.data_bits as usize / 8, p.data_width)] {
            if n == 0 { continue; }
            if !self.phase(&bytes[at..at + n], width) { self.reject_until_deselect(); return false; }
            at += n;
        }
        true
    }
    fn phase(&mut self, mut bytes: &[u8], width: u8) -> bool {
        if self.header_len < 4 {
            if width != 1 { return false; }
            let n = (4 - self.header_len).min(bytes.len());
            self.header[self.header_len..self.header_len + n].copy_from_slice(&bytes[..n]);
            self.header_len += n; bytes = &bytes[n..];
            if self.header_len == 4 {
                let [opcode, zero1, cmd, zero2] = self.header;
                if !matches!(opcode, 0x02 | 0x32) || zero1 != 0 || zero2 != 0
                    || (opcode == 0x32 && !matches!(cmd, 0x2c | 0x3c)) { return false; }
                self.command = cmd; self.commands += 1;
                self.normal_command = self.page == 0 && matches!(cmd,
                    0x01 | 0x10 | 0x11 | 0x20 | 0x21 | 0x28 | 0x29 | 0x2a | 0x2b | 0x2c |
                    0x34 | 0x35 | 0x36 | 0x3a | 0x3c | 0x44 | 0x51 | 0x53);
                if self.normal_command {
                    if cmd == 1 { self.reset_registers(); }
                    else {
                        self.memory.transaction_stream(&self.header);
                        if let Some(s) = &mut self.sh8601 { match cmd { 0x10=>s.state.sleep_out=false,0x11=>s.state.sleep_out=true,0x28=>s.state.display_on=false,0x29=>s.state.display_on=true,_=>{} } s.state.revision=s.state.revision.wrapping_add(1); }
                    }
                    if cmd == 0x20 { self.inverted = false; }
                    if cmd == 0x21 { self.inverted = true; }
                    self.changes += 1;
                } else { self.opaque_commands += 1; }
            }
        }
        if bytes.is_empty() { return true; }
        if self.header_len != 4 || width != if self.header[0] == 0x32 { 4 } else { 1 } { return false; }
        let cmd = self.command;
        if !self.normal_command {
            // FE selects the manufacturer register page. An unknown page must
            // not turn an unrelated vendor register into a standard DCS command.
            if self.sh8601.is_none() && cmd == 0xfe && self.parameter_count == 0 { self.page = bytes[0]; }
            self.opaque_bytes += bytes.len() as u64; self.parameter_count += bytes.len();
            return true;
        }
        if matches!(cmd, 0x2c | 0x3c) {
            let [x0, x1, y0, y1] = self.memory.window();
            if !self.state().rgb565() || !self.orientation_supported() || x0 > x1 || y0 > y1 { return false; }
        } else {
            let needed = match cmd { 0x2a | 0x2b => 4, 0x44 => 2,
                0x51 if self.sh8601.is_some() => 2,
                0x35 | 0x36 | 0x3a | 0x51 | 0x53 => 1, _ => 0 };
            if self.parameter_count + bytes.len() > needed { return false; }
            if cmd == 0x36 { self.madctl = bytes[0]; self.changes += 1; }
        }
        if let Some(s) = &mut self.sh8601 {
            if matches!(cmd,0x2c|0x3c) {
                if !s.state.visible() { s.state.pixel_bytes_while_hidden+=bytes.len() as u64; }
            } else {
                match cmd {
                    0x3a=>s.state.pixel_format=bytes[0],
                    0x53=>s.state.control_display=Some(bytes[0]),
                    0x51=>{ for (i,&v) in bytes.iter().enumerate() { if self.parameter_count+i==0 { s.brightness=(s.brightness&0x300)|u16::from(v); } else { s.brightness=(s.brightness&255)|(u16::from(v&3)<<8); } } s.state.brightness=if s.brightness==0 {0}else{255}; },
                    _=>{}
                }
                s.state.revision=s.state.revision.wrapping_add(1);
            }
        }
        self.parameter_count += bytes.len(); self.memory.transaction_stream(bytes); true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pins(n: usize, width: u8) -> SpiQspiPins {
        SpiQspiPins { data_width: width, data_bits: n as u32 * 8, ..Default::default() }
    }
    fn command(p: &mut QspiDcsPanel, cmd: u8, data: &[u8]) {
        assert!(p.transfer(pins(4, 1), &[2, 0, cmd, 0]));
        if !data.is_empty() { assert!(p.transfer(pins(data.len(), 1), data)); }
        p.deselect();
    }
    fn ready() -> QspiDcsPanel {
        let mut p = QspiDcsPanel::co5300(2, 1, 8, 0xc0);
        command(&mut p, 0x36, &[0xc0]); command(&mut p, 0x3a, &[0x55]);
        command(&mut p, 0x11, &[]); command(&mut p, 0x29, &[]);
        command(&mut p, 0x53, &[0x20]); command(&mut p, 0x51, &[255]);
        command(&mut p, 0x2a, &[0, 8, 0, 9]); command(&mut p, 0x2b, &[0, 0, 0, 0]); p
    }
    #[test]
    fn sh8601_bsp_rgb565_keeps_its_own_reset_brightness_and_reserved_orientation() {
        let mut p=QspiDcsPanel::sh8601_bsp_rgb565(2,1,0,0);
        assert_eq!(p.state().control_display,Some(0x28)); assert_eq!(p.brightness_10bit(),Some(255));
        assert_eq!(p.state().pixel_format,0x77); assert!(!p.visible());
        command(&mut p,0x3a,&[5]); command(&mut p,0x11,&[]); command(&mut p,0x29,&[]);
        command(&mut p,0x2a,&[0,0,0,1]); command(&mut p,0x2b,&[0,0,0,0]);
        assert!(p.transfer(pins(4,1),&[0x32,0,0x2c,0])); assert!(p.transfer(pins(4,4),&[0xf8,0,0,31])); p.deselect();
        assert_eq!(p.frame(),[0xf800,31]); command(&mut p,0x36,&[0x40]); assert_eq!(p.frame(),[31,0xf800]);
        command(&mut p,0x36,&[2]); assert!(!p.visible(),"BSP low-bit flip constant conflicts with data sheet; never silently reinterpret it");
        command(&mut p,0x36,&[0]); command(&mut p,0x51,&[0,0]); assert!(!p.visible());
        command(&mut p,0x51,&[0,1]); assert_eq!(p.brightness_10bit(),Some(256)); assert!(p.visible());
        command(&mut p,0x51,&[12]); assert_eq!(p.brightness_10bit(),Some(268));
        p.reset(); assert_eq!(p.pixels_written(),2); assert_eq!(p.brightness_10bit(),Some(255));
        command(&mut p,0x3a,&[5]); command(&mut p,0x11,&[]); command(&mut p,0x29,&[]);
        assert_eq!(p.frame(),[0,0],"undefined SH reset memory uses deterministic unknown-zero policy, never CO retention");
    }
    #[test]
    fn sh8601_does_not_borrow_co5300_vendor_pages_or_accept_wrong_quad_header() {
        let mut p=QspiDcsPanel::sh8601_bsp_rgb565(1,1,0,0);
        command(&mut p,0xfe,&[1]); assert_eq!(p.page,0,"no SH FE register-page meaning is asserted");
        command(&mut p,0x11,&[]); command(&mut p,0x29,&[]); assert!(p.visible());
        assert!(!p.transfer(pins(4,4),&[2,0,0x28,0])); assert!(p.visible());
        assert!(!p.transfer(pins(4,1),&[2,0,0x28,0])); p.deselect();
        command(&mut p,0x28,&[]); assert!(!p.visible());
    }
    #[test]
    fn square_mv_transposes_real_pixels_and_composes_with_reference_mirrors() {
        for reference in [0, 0x30, 0x60, 0xa0, 0xe0] {
            let mut p = QspiDcsPanel::co5300(2, 2, 0, reference);
            command(&mut p, 0x36, &[reference]); command(&mut p, 0x3a, &[0x55]);
            command(&mut p, 0x11, &[]); command(&mut p, 0x29, &[]);
            command(&mut p, 0x53, &[0x20]); command(&mut p, 0x51, &[255]);
            command(&mut p, 0x2a, &[0, 0, 0, 1]); command(&mut p, 0x2b, &[0, 0, 0, 1]);
            assert!(p.transfer(pins(4, 1), &[0x32, 0, 0x2c, 0]));
            assert!(p.transfer(pins(8, 4), &[0xf8, 0, 7, 0xe0, 0, 31, 255, 255])); p.deselect();
            assert_eq!(p.frame(), [0xf800, 0x07e0, 0x001f, 0xffff], "same guest/reference orientation is identity");
            if reference == 0 {
                command(&mut p, 0x36, &[0x20]);
                assert_eq!(p.frame(), [0xf800, 0x001f, 0x07e0, 0xffff]);
                command(&mut p, 0x36, &[0x60]);
                assert_eq!(p.frame(), [0x001f, 0xf800, 0xffff, 0x07e0]);
            }
        }
        let mut p = ready(); command(&mut p, 0x36, &[0x20]);
        assert!(!p.visible(), "rectangular swapped address ranges remain unsupported");
        assert!(p.transfer(pins(4, 1), &[0x32, 0, 0x2c, 0]));
        assert!(!p.transfer(pins(2, 4), &[0xf8, 0]));
    }
    #[test]
    fn split_header_quad_pixels_and_header_looking_pixel_chunks_keep_cs_identity() {
        let mut p = ready();
        for b in [0x32, 0, 0x2c, 0] { assert!(p.transfer(pins(1, 1), &[b])); }
        assert!(p.transfer(pins(1, 4), &[0xf8])); assert!(p.transfer(pins(3, 4), &[0, 7, 0xe0]));
        assert_eq!(p.frame(), [0xf800, 0x07e0]);
        assert!(p.transfer(pins(4, 4), &[0x02, 0, 0x29, 0]));
        assert_eq!(p.frame(), [0x0200, 0x2900]); assert_eq!(p.pixels_written(), 4);
        p.deselect(); command(&mut p, 0x36, &[0]); assert_eq!(p.frame(), [0x2900, 0x0200]);
        command(&mut p, 0x36, &[0xc8]); assert_eq!(p.frame()[1], 0x0105);
    }
    #[test]
    fn malformed_phases_never_resynchronize_before_physical_cs_release() {
        let mut p = ready();
        assert!(!p.transfer(pins(4, 4), &[2, 0, 0x28, 0]));
        assert!(!p.transfer(pins(4, 1), &[2, 0, 0x28, 0])); assert!(p.visible());
        p.deselect(); command(&mut p, 0x28, &[]); assert!(!p.visible());
        let mut wrong = pins(4, 1); wrong.dummy_cycles = 1;
        assert!(!p.transfer(wrong, &[2, 0, 0x29, 0])); p.deselect();
        command(&mut p, 0x29, &[]); assert!(p.visible());
        assert!(!p.transfer(pins(4, 1), &[0x32, 0, 0x2a, 0])); p.deselect();
        assert_eq!(p.window(), [8, 9, 0, 0]);
    }
    #[test]
    fn formats_visibility_and_reset_keep_the_actual_gram_without_fake_pixels() {
        let mut p = ready(); command(&mut p, 0x2c, &[0xf8, 0, 7, 0xe0]);
        command(&mut p, 0x51, &[0]); assert_eq!(p.frame(), [0, 0]);
        command(&mut p, 0x51, &[1]); assert_eq!(p.frame(), [0xf800, 0x07e0]);
        command(&mut p, 0x3a, &[0x66]);
        assert!(p.transfer(pins(4, 1), &[0x32, 0, 0x2c, 0]));
        assert!(!p.transfer(pins(4, 4), &[0, 0, 0, 0])); assert_eq!(p.pixels_written(), 2); p.deselect();
        command(&mut p, 0x3a, &[0xd7]); // bit7 selects VIPF=5, not IFPF=7
        assert!(p.state().rgb565()); command(&mut p, 0x3c, &[0, 0]); assert_eq!(p.pixels_written(), 3);
        p.reset(); assert_eq!(p.pixels_written(), 3); assert!(!p.visible());
        command(&mut p, 0x36, &[0xc0]); command(&mut p, 0x11, &[]); command(&mut p, 0x29, &[]);
        assert_eq!(p.frame(), [0, 0x07e0], "reset does not clear GRAM");
    }
    #[test]
    fn wrong_address_windows_and_vendor_pages_cannot_mutate_visible_memory() {
        let mut p = ready(); command(&mut p, 0x2a, &[0, 0, 0, 1]);
        command(&mut p, 0x2c, &[0xff, 0xff, 0xff, 0xff]); assert_eq!(p.pixels_written(), 0);
        command(&mut p, 0xfe, &[1]); command(&mut p, 0x28, &[]); assert!(p.visible());
        command(&mut p, 0xfe, &[0]); command(&mut p, 0x28, &[]); assert!(!p.visible());
        let malformed = SpiQspiPins { command_width: 1, command_bits: 8, address_width: 4,
            address_bits: 24, ..Default::default() };
        assert!(!p.transfer(malformed, &[2, 0, 0x29, 0])); assert!(!p.visible());
    }
}
